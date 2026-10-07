//! daemon 客户端：本地 HTTP 调用 + 首次使用自动拉起 detached daemon。

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// daemon 的缺省端口，`BROWSE_PORT` 与 `BROWSE_NAME` 派生端口可覆盖。
///
/// 取 9880 是为避开 bh 的 9876。
pub const DEFAULT_PORT: u16 = 9880;

/// 返回 daemon 监听的 `host:port` 串；多实例由 `BROWSE_NAME` 派生端口（ADR-0006）。
///
/// # Examples
///
/// ```
/// # // BROWSE_PORT/BROWSE_NAME 均未设时才是默认口（CI 干净环境成立）
/// if std::env::var_os("BROWSE_PORT").is_none() && std::env::var_os("BROWSE_NAME").is_none() {
///     assert_eq!(browse_cli::client::daemon_bind(), "127.0.0.1:9880");
/// }
/// ```
pub fn daemon_bind() -> String {
    format!("127.0.0.1:{}", browse_core::paths::daemon_port())
}

fn http() -> String {
    format!("http://{}", daemon_bind())
}

/// 返回 daemon 日志与运行面目录（`%USERPROFILE%\.browse-rs[\<name>]`，多实例各一份）。
pub fn state_dir() -> PathBuf {
    browse_core::paths::state_dir()
}

fn client() -> reqwest::Client {
    // 本机回环绝不过系统代理
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(600))
        .build()
        .expect("reqwest client")
}

/// 探测 daemon 是否在跑：GET /health 通即为在。
pub async fn daemon_alive() -> Option<Value> {
    let c = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(400))
        .build()
        .ok()?;
    let resp = c.get(format!("{}/health", http())).send().await.ok()?;
    resp.json::<Value>().await.ok()
}

/// daemon 不在跑时 detached 拉起 `browse --serve` 并探活到通；已在跑则直接返回。
///
/// # Errors
///
/// 拉起失败（exe 找不到）或 10 秒内 /health 不通。
pub async fn ensure_daemon() -> Result<()> {
    ensure_daemon_with_env(&[]).await
}

/// 同 [`ensure_daemon`]，但给新拉起的 daemon 进程注入环境变量
/// （#25.1/#25.5：idle-timeout 与代理旗标只影响新 daemon；已在跑的
/// daemon 不动，其口径以启动时为准）。
///
/// # Errors
///
/// daemon 起不来或就绪探测超时。
pub async fn ensure_daemon_with_env(envs: &[(&str, String)]) -> Result<()> {
    if daemon_alive().await.is_some() {
        return Ok(());
    }
    let exe = std::env::current_exe().context("current_exe")?;
    let dir = state_dir();
    std::fs::create_dir_all(&dir).ok();
    let log = dir.join("daemon.log");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};
        use windows_sys::Win32::System::Console::{
            GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
        };
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        // 先摘掉本进程 stdio 句柄的可继承位：daemon spawn 走
        // bInheritHandles=TRUE（stdio 指到日志文件），而 bash 管道默认
        // 可继承；不摘的话常驻 daemon 会握着调用方 `browse '…' | jq`
        // 的管道写端，管道永不 EOF。显式配置的日志句柄不受影响
        for slot in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let h = unsafe { GetStdHandle(slot) };
            if !h.is_null() {
                unsafe { SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0) };
            }
        }
        let out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)?;
        let mut cmd = Command::new(exe);
        cmd.envs(envs.iter().map(|(k, v)| (*k, v.clone())));
        cmd.args(["--serve", "--bind", &daemon_bind()]);
        let err_out = out.try_clone().ok();
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(out);
        if let Some(e) = err_out {
            cmd.stderr(e);
        }
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
        let child = cmd
            .spawn()
            .with_context(|| format!("拉起 daemon（日志 {}）", log.display()))?;
        drop(child);
    }
    #[cfg(not(windows))]
    {
        // Unix：std 打开的 fd 全带 CLOEXEC，只 dup2 配置过的 stdio，
        // 调用方管道不会被 daemon 带走，直接 spawn 即可
        let out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)?;
        let mut cmd = Command::new(exe);
        cmd.envs(envs.iter().map(|(k, v)| (*k, v.clone())));
        cmd.args(["--serve", "--bind", &daemon_bind()]);
        let err_out = out.try_clone().ok();
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(out);
        if let Some(e) = err_out {
            cmd.stderr(e);
        }
        let child = cmd
            .spawn()
            .with_context(|| format!("拉起 daemon（日志 {}）", log.display()))?;
        drop(child);
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while tokio::time::Instant::now() < deadline {
        if daemon_alive().await.is_some() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err(anyhow!(
        "browse: daemon 10 秒未就绪；看日志 {}（tail 或删除后重跑）",
        log.display()
    ))
}

/// 技能层透传环境（#50/#51）：拉起新 daemon 时把 CLI 进程里的 workspace
/// 根与两个触发开关显式带给子进程。评审 G4 注记：daemon 由本 crate 的
/// `Command` spawn、环境默认整体继承（全仓无 env_clear），三变量只走 env
/// 不像 `BROWSE_SECRETS` 有 argv 来源，故本透传在现状下与继承等价——保留
/// 是防御（未来 env_clear 或非 CLI 拉起路径）。「只影响新拉起的 daemon，
/// 已在跑的以启动时口径为准，改这些配置要重启 daemon」语义不变。
///
/// # Examples
///
/// ```
/// // 未设三变量时透传清单为空（不注无谓 env）
/// if std::env::var_os("BROWSE_WORKSPACE").is_none() {
///     assert!(browse_cli::client::skills_passthrough_env().is_empty());
/// }
/// ```
pub fn skills_passthrough_env() -> Vec<(&'static str, String)> {
    let mut v = Vec::new();
    for k in [
        "BROWSE_WORKSPACE",
        "BROWSE_DOMAIN_SKILLS",
        "BROWSE_PAGE_SKILLS",
    ] {
        if let Some(val) = std::env::var_os(k).map(|s| s.to_string_lossy().into_owned())
            && !val.is_empty()
        {
            v.push((k, val));
        }
    }
    v
}

/// 把方言片段 POST 到 daemon 的 /eval 求值。
///
/// 返回最后一条语句的值；daemon 侧错误进 `Err`（错误串已是给人/agent 的下一步指令形态）。
///
/// # Errors
///
/// HTTP 失败或 daemon 求值失败（语法错、CDP 错、守卫拦截、超时）。
pub async fn eval(code: &str, new_tab: bool, js: bool, timeout_s: Option<u64>) -> Result<Value> {
    let c = client();
    let mut rq = c
        .post(format!("{}/eval", http()))
        .json(&json!({ "code": code, "new_tab": new_tab, "js": js, "timeout_s": timeout_s }));
    // HTTP 客户端超时随兜底界联动（全量审 L2）：界 + 60 秒余量，防 reqwest
    // 缺省 600 秒先到丢掉 daemon 的结构化 timeout 回执
    if let Some(s) = timeout_s {
        rq = rq.timeout(std::time::Duration::from_secs(s + 60));
    }
    let resp = rq.send().await.context("POST /eval")?;
    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .with_context(|| format!("解析 /eval 响应（HTTP {status}）"))?;
    if body.get("ok").and_then(Value::as_bool) == Some(true) {
        // #60 操作时刻引擎上下文：正常态安静；异常（指纹变化、跨宿主）
        // 出 stderr 告警行；BROWSE_ENGINE_CONTEXT 非 0/false 显紧凑行
        if let Some(ctx) = body.get("engineContext") {
            render_engine_context(ctx);
        }
        Ok(body.get("value").cloned().unwrap_or(Value::Null))
    } else {
        // 结构化 timeout/stopped 面一行 JSON 进 stderr（评审 G-16，与
        // rune 腿同律）：上层 agent 可直接解析换通道重试，不依赖人读错误行
        let detail = body.get("timeout").or(body.get("stopped"));
        if let Some(t) = detail
            && let Ok(line) = serde_json::to_string(t)
        {
            eprintln!("{line}");
        }
        // 前缀由 daemon 侧统一加（防双重 browse:）
        Err(anyhow!(
            "{}",
            body.get("error")
                .and_then(Value::as_str)
                .unwrap_or("未知错误")
        ))
    }
}

/// 把 Rune 编排脚本 POST 到 daemon 的 /rune 执行（账本 #67）。
///
/// 返回 rune 回执 JSON（`{"ok": …}` 或 `{"error": …}`）；宿主终态进
/// `Err`（错误串已是 CTA 形态）。
///
/// # Errors
///
/// HTTP 失败或 daemon 执行失败（编译错、VM 终态、限额击穿、超时）。
pub async fn rune(
    code: &str,
    root: Option<String>,
    stdin: Option<String>,
    vars_syntax: browse_rune::vars_syntax::VarsSyntax,
    timeout_s: Option<u64>,
) -> Result<Value> {
    let c = client();
    let mut rq = c
        .post(format!("{}/rune", http()))
        .json(&json!({ "code": code, "root": root, "stdin": stdin, "vars_syntax": vars_syntax.name(), "timeout_s": timeout_s }));
    // 同 L2：HTTP 超时随兜底界联动
    if let Some(s) = timeout_s {
        rq = rq.timeout(std::time::Duration::from_secs(s + 60));
    }
    let resp = rq.send().await.context("POST /rune")?;
    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .with_context(|| format!("解析 /rune 响应（HTTP {status}）"))?;
    if body.get("ok").and_then(Value::as_bool) == Some(true) {
        if let Some(ctx) = body.get("engineContext") {
            render_engine_context(ctx);
        }
        Ok(body.get("value").cloned().unwrap_or(Value::Null))
    } else {
        // v0.26.1 超时 CTA 最小形（批次二扩到主动停止面）：daemon 的结构化
        // timeout/stopped 面以一行 JSON 打到 stderr（上层 agent 可直接解析
        // 换通道重试），人读错误行照旧
        let detail = body.get("timeout").or(body.get("stopped"));
        if let Some(t) = detail
            && let Ok(line) = serde_json::to_string(t)
        {
            eprintln!("{line}");
        }
        Err(anyhow!(
            "{}",
            body.get("error")
                .and_then(Value::as_str)
                .unwrap_or("未知错误")
        ))
    }
}

/// 主动停止在跑的 rune 执行（批次二总台指令）：POST /rune/stop，回
/// `{ok, running, requested, note}`。
///
/// # Errors
///
/// HTTP 失败或响应解析失败。
pub async fn rune_stop() -> Result<Value> {
    let c = client();
    let resp = c
        .post(format!("{}/rune/stop", http()))
        .send()
        .await
        .context("POST /rune/stop")?;
    resp.json().await.context("解析 /rune/stop 响应")
}

/// #60 渲染操作信封的引擎上下文（crate 内缝，eval 与 fetch 引擎腿共用）：
/// 指纹变化与跨宿主各出告警行（跨宿主每进程一次防刷屏），紧凑行按
/// `BROWSE_ENGINE_CONTEXT` 开。正常态零输出（默认输出与现状兼容）。
pub(crate) fn render_engine_context(ctx: &Value) {
    let show = std::env::var_os("BROWSE_ENGINE_CONTEXT")
        .map(|v| {
            let v = v.to_string_lossy().to_ascii_lowercase();
            !(v == "0" || v == "false")
        })
        .unwrap_or(false);
    if show {
        let prov = ctx.get("provenance");
        let origin = prov
            .and_then(|p| p.get("origin"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        let host = prov
            .and_then(|p| p.get("hostContext"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        eprintln!("引擎：{origin} @ {host}");
    }
    for c in ctx
        .get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        eprintln!("⚠ 引擎变化：{c}");
    }
    // 跨宿主与同 OS 异机（#59 同口径；每进程一次防批处理刷屏）
    use std::sync::OnceLock;
    static HOST_CHECKED: OnceLock<bool> = OnceLock::new();
    let first = HOST_CHECKED.get().is_none();
    HOST_CHECKED.set(true).ok();
    if first {
        let d_os = ctx
            .pointer("/daemon/os")
            .and_then(Value::as_str)
            .unwrap_or("");
        let d_host = ctx
            .pointer("/daemon/hostname")
            .and_then(Value::as_str)
            .unwrap_or("");
        if d_os.is_empty() {
            return;
        }
        let cli_os = std::env::consts::OS;
        if d_os != cli_os {
            eprintln!(
                "⚠ 跨宿主：CLI({cli_os}) 与 daemon({d_os}/{d_host}) 不同机（localhost 转发场景）"
            );
        } else if d_host != "unknown" {
            let cli_host = std::process::Command::new("hostname")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|h| !h.is_empty())
                .unwrap_or_default();
            if !cli_host.is_empty() && cli_host != d_host {
                eprintln!(
                    "⚠ 同 OS 异机：CLI({cli_host}) 与 daemon({d_os}/{d_host}) 不是同一台（隧道或转发场景）"
                );
            }
        }
    }
}

/// 解码 `-b/--b64` 通道的片段实参（#22）：标准 base64 解码为 UTF-8
/// 字符串。PowerShell 与复杂 shell 的引号与编码面一并绕开（方言与
/// `--js` 两形态通用）。
///
/// # Errors
///
/// 实参不是合法 base64，或解码后不是 UTF-8 文本（错误带生成 base64 的
/// 可照抄命令 CTA）。
///
/// # Examples
///
/// ```
/// assert_eq!(browse_cli::client::decode_arg_b64("cmV0dXJuIDE=").unwrap(), "return 1");
/// // 空白容忍（PowerShell 折行形）
/// assert_eq!(
///     browse_cli::client::decode_arg_b64("cmV0\ndXJuIDE=").unwrap(),
///     "return 1"
/// );
/// assert!(browse_cli::client::decode_arg_b64("!!!").is_err());
/// ```
pub fn decode_arg_b64(s: &str) -> Result<String> {
    // 字母表先行校验：宿主 base64_decode 查表对非法字符得 0（静默出垃圾），
    // 通道面要给出错而不是喂垃圾给引擎
    let alphabet_ok = s.bytes().all(|b| {
        b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=' || b.is_ascii_whitespace()
    });
    if !alphabet_ok {
        anyhow::bail!(
            "-b 实参不是 base64 字母表（A-Za-z0-9+/= 与空白）；下一步：base64 -w0 <文件> 生成（PowerShell 用 [Convert]::ToBase64String）"
        );
    }
    let bytes = browse_core::js_host::base64_decode(s).map_err(|e| {
        anyhow!(
            "-b 实参不是合法 base64：{e}；下一步：base64 -w0 <文件> 生成（PowerShell 用 [Convert]::ToBase64String([IO.File]::ReadAllBytes(\"<文件>\"))）"
        )
    })?;
    String::from_utf8(bytes)
        .map_err(|_| anyhow!("-b 解码后不是 UTF-8 文本；下一步：确认原文是方言片段或 JS 源码文本"))
}

/// `@` 文件实参展开（#64）：片段实参以 `@` 开头即读文件内容为片段体，
/// 其余实参原样保留。方言与 `--js` 两形态通用（同一实参道）；绝对与
/// 相对路径皆可（相对即当前工作目录）。先于 `--b64` 解码展开，
/// `--b64 @<存 base64 的文件>` 组合形自然成立；stdin 管道形态不经此道
/// （管道内容是片段体本身，不前缀展开）。
///
/// # Errors
///
/// `@` 后路径读取失败（不存在、无权限、非 UTF-8 等）报带下一步提示的
/// 错误（调用方直出 exit 2，与 `--b64` 坏串同族）。
///
/// # Examples
///
/// ```
/// # use browse_cli::client::expand_at_args;
/// // 非 @ 前缀实参原样保留
/// let mut args = vec!["return 1".to_string()];
/// expand_at_args(&mut args).unwrap();
/// assert_eq!(args[0], "return 1");
/// // 空 @ 前缀（裸 @）读不了，报错
/// let mut bad = vec!["@".to_string()];
/// assert!(expand_at_args(&mut bad).is_err());
/// ```
pub fn expand_at_args(snippets: &mut [String]) -> Result<()> {
    for snip in snippets.iter_mut() {
        if let Some(path) = snip.strip_prefix('@') {
            // 裸 @（路径空）特例文案（评审一轮 G5、二轮 G9 覆盖双形）：
            // 不落「读文件失败 ：」的悬空冒号形
            if path.is_empty() {
                anyhow::bail!(
                    "@ 后缺文件路径；下一步：browse --js @<路径> 或 browse '@<路径>'（绝对或相对皆可）"
                );
            }
            *snip = std::fs::read_to_string(path).map_err(|e| {
                anyhow!(
                    "@实参读文件失败 {path}：{e}；下一步：核对路径与权限，或直接贴片段实参（cat 文件 | browse 管道亦可）"
                )
            })?;
        }
    }
    Ok(())
}

// ---- #66 JS 命令小程序：@ 实参 + --vars 模板道 ----

/// 解析 `--vars` 单值（`键=值`）：键是标识符（字母或下划线开头，字母
/// 数字下划线续），值是首个 `=` 后的整段——可空串、可含 `=` 与引号，
/// 原样直取（shell 引号由调用方管）。非 `k=v` 形返回 `None`（调用方
/// 按用法错处理）。
///
/// # Examples
///
/// ```
/// # use browse_cli::client::parse_vars_arg;
/// assert_eq!(
///     parse_vars_arg("q=hi there").map(|(k, v)| (k, v)),
///     Some(("q".to_string(), "hi there".to_string()))
/// );
/// assert_eq!(parse_vars_arg("empty=").map(|(_, v)| v), Some(String::new()));
/// assert_eq!(parse_vars_arg("a=b=c").map(|(_, v)| v), Some("b=c".to_string()));
/// assert!(parse_vars_arg("nokey").is_none());
/// assert!(parse_vars_arg("1k=v").is_none());
/// ```
#[must_use]
pub fn parse_vars_arg(s: &str) -> Option<(String, String)> {
    let (k, v) = s.split_once('=')?;
    let mut cs = k.chars();
    let head_ok = cs
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if head_ok && cs.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some((k.to_string(), v.to_string()))
    } else {
        None
    }
}

/// 扫裸标识符占位符（`{{ 键 }}` 形，两侧空白容忍，键是纯标识符）里
/// 未给的键：引擎 Strict 错误不保证带键名，验收面要求点名，故引擎报
/// 未定义类错误后用这层做带名富化（评审 #66 一轮 F1：富化在渲染后，
/// 死分支占位符不误报）；带过滤器或表达式的占位（`{{ k | default("x") }}`）
/// 不属裸形，交给引擎（可选参由此通行）。
fn missing_bare_placeholders(
    syntax: browse_rune::vars_syntax::VarsSyntax,
    body: &str,
    vars: &[(String, String)],
) -> Vec<String> {
    let (open, close): (&str, &str) = match syntax {
        browse_rune::vars_syntax::VarsSyntax::Classic => ("{{", "}}"),
        browse_rune::vars_syntax::VarsSyntax::Erb => ("<%=", "%>"),
    };
    let keys: Vec<&str> = vars.iter().map(|(k, _)| k.as_str()).collect();
    let mut missing: Vec<String> = Vec::new();
    let mut i = 0;
    while let Some(rel) = body[i..].find(open) {
        let start = i + rel;
        if let Some(end) = body[start + open.len()..].find(close) {
            let inner = body[start + open.len()..start + open.len() + end].trim();
            let bare = !inner.is_empty()
                && inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && inner
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
            if bare && !keys.contains(&inner) && !missing.iter().any(|m| m == inner) {
                missing.push(inner.to_string());
            }
            i = start + open.len() + end + close.len();
            continue;
        }
        i = start + open.len();
    }
    missing
}

/// JS 族模板渲染（方言片段与 `--js` 小程序，G-1 对齐批）：先抽出注释
/// 区换占位（注释对 Jinja 惰性，逐字回填），渲染余下源——字符串里的
/// `{{ 键 }}` 照旧参与渲染；r# 是 Rune 语法不在此面重写。
///
/// # Errors
///
/// 占位冲撞/被模板误删，或渲染错误（同 [`render_template`]）。
pub fn render_js_template(
    syntax: browse_rune::vars_syntax::VarsSyntax,
    body: &str,
    vars: &[(String, String)],
) -> Result<String> {
    vars_value_guard(vars)?;
    let syntax = browse_rune::vars_syntax::directive_of(body).unwrap_or(syntax);
    let (placed, literals) = browse_rune::rawstr::extract_comment_regions(body)
        .map_err(|e| anyhow::anyhow!("注释惰性化预处理失败：{}", e.message))?;
    let rendered = render_template_with(syntax, &placed, vars)?;
    browse_rune::rawstr::subst_raw_strings(&rendered, &literals)
        .map_err(|e| anyhow::anyhow!("注释惰性化预处理失败：{}", e.message))
}

/// --vars 值前缀守卫（REQ-020 注入理论形收口）：值恰含在用占位前缀
/// 会被全量替换（nonce 的等价防护，确定性更优）；拦在渲染前，exit 2。
fn vars_value_guard(vars: &[(String, String)]) -> Result<()> {
    const PREFIX: &str = "__BROWSE_RN_RAW_";
    for (k, v) in vars {
        if v.contains(PREFIX) {
            anyhow::bail!("--vars {k} 的值含内部占位前缀 {PREFIX}；下一步：值里去掉该前缀串再试");
        }
    }
    Ok(())
}

/// rune 面模板渲染（批次二 r# 裸串、批次三注释惰性化）：先抽出 r# 区
/// 与注释区换占位（两类区对 Jinja 惰性，`{{`/`{%`/字面 raw 标签原样），
/// 渲染余下源，再回填（r# 区转义普通串、注释区逐字原文）；「裸串与注
/// 释免渲染」与 `{% raw %}` 各管一层（raw 管 Jinja 面任意区，r# 管 Rune
/// 串字面量面）；无 r# 记号且无注释时与 [`render_template`] 等价。
///
/// # Errors
///
/// r# 词法错（未闭合/占位冲撞/占位被模板误删）或渲染错误（同
/// [`render_template`]）。
pub fn render_rune_template(
    syntax: browse_rune::vars_syntax::VarsSyntax,
    body: &str,
    vars: &[(String, String)],
) -> Result<String> {
    vars_value_guard(vars)?;
    let syntax = browse_rune::vars_syntax::directive_of(body).unwrap_or(syntax);
    let (placed, literals) = browse_rune::rawstr::extract_raw_strings(body)
        .map_err(|e| anyhow::anyhow!("r# 裸串预处理失败：{}", e.message))?;
    let rendered = render_template_with(syntax, &placed, vars)?;
    browse_rune::rawstr::subst_raw_strings(&rendered, &literals)
        .map_err(|e| anyhow::anyhow!("r# 裸串预处理失败：{}", e.message))
}

/// `--vars` 模板渲染（#66 命令小程序）：Jinja 语法渲染模板源，`{{ 键 }}`
/// 直插值（值按源码文本，引号由模板管），`| default()` 给可选参、
/// `{% for %}` 生成面可用（引擎 minijinja，ADR-0009）。何时用：@ 实参
/// 与 `--vars` 同给时，本函数是填充唯一道；不给 `--vars` 的 @ 实参走
/// [`expand_at_args`] 纯透传（#64 语义零漂移）。
///
/// # Errors
///
/// 裸占位符有未给键：报错点名全部未填键并给 `--vars` 补齐 CTA（引擎
/// Strict 兜底复杂表达式面的未定义值，错误形不带键名）；模板语法错：
/// 报引擎原始错误。
///
/// # Examples
///
/// ```
/// # use browse_cli::client::render_template;
/// let vars = vec![("n".to_string(), "2".to_string())];
/// assert_eq!(
///     render_template("return {{ n }} + 1", &vars).unwrap(),
///     "return 2 + 1"
/// );
/// // default 过滤器：可选参不填不报错
/// assert_eq!(render_template("{{ opt | default(\"x\") }}", &vars).unwrap(), "x");
/// // 未填键硬错且点名
/// let err = render_template("{{ a }} + {{ b }}", &vars).unwrap_err();
/// let msg = err.to_string();
/// assert!(msg.contains("a") && msg.contains("b"), "{msg}");
/// // 值原样直插（含引号与等号）
/// let v2 = vec![("q".to_string(), "'a=b'".to_string())];
/// assert_eq!(render_template("f({{ q }})", &v2).unwrap(), "f('a=b')");
/// ```
pub fn render_template(body: &str, vars: &[(String, String)]) -> Result<String> {
    // 兼容入口（0.30.3）：恒经典定界 {{ 键 }}，绕过缺省 ERB——四面缺省
    // 走 render_rune_template/render_js_template，外部直调本函数保持旧
    // 契约
    render_template_with(browse_rune::vars_syntax::VarsSyntax::Classic, body, vars)
}

/// [`render_template`] 的定界形态参数版（0.30.3：四面同源语法选择）。
///
/// # Errors
///
/// 同 [`render_template`]（未填键点名、语法错带 CTA）。
pub fn render_template_with(
    syntax: browse_rune::vars_syntax::VarsSyntax,
    body: &str,
    vars: &[(String, String)],
) -> Result<String> {
    let env = browse_rune::vars_syntax::env_for(syntax);
    // BTreeMap 收拢：同键重复 --vars 后值胜
    let map: std::collections::BTreeMap<String, String> = vars.iter().cloned().collect();
    let ctx = minijinja::Value::from_serialize(&map);
    match env.render_str(body, ctx) {
        Ok(out) => Ok(out),
        // 引擎先渲染、扫描器只做带名富化（评审 #66 一轮 F1 反转权威序）：
        // Strict 引擎只对活分支里的未定义值报错（{% if false %}/{% raw %}
        // 等死分支占位符不误报），报错是未定义类时扫描器补键名 CTA。
        // 判据走结构化 kind（二轮 G9）：措辞启发式不耐引擎版本变化
        Err(e) => {
            if e.kind() == minijinja::ErrorKind::UndefinedError {
                let missing = missing_bare_placeholders(syntax, body, vars);
                if !missing.is_empty() {
                    let cta = missing
                        .iter()
                        .map(|k| format!("{k}=<值>"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    anyhow::bail!(
                        "模板未填占位符：{}；下一步：--vars {} 补齐，或模板里改 {{{{ 键 | default(…) }}}} 给默认值",
                        missing.join("、"),
                        cta
                    );
                }
            }
            anyhow::bail!(
                "模板渲染失败：{e}；下一步：核对 --vars 键名与模板表达式（语法错修模板，未定义值属硬错）"
            )
        }
    }
}

/// 模板源解析（#66）：`--vars` 模板道的 @ 实参解析序——snippets 根
/// 第一等：`根/<名>` 原样与 `根/<名>.js` 两试（命中且 canonicalize 后
/// 仍在根内即用；越根候选跳过根道，守卫语义同 snippets show）；
/// 根未命中回落字面路径（`#64` 语义：绝对/相对皆可，cwd 基准）。
/// 返回 `(命中路径, 源码)`；全未命中报错列出试过的位。
///
/// # Errors
///
/// 所有候选位都不存在或读失败。
///
/// # Examples
///
/// ```
/// # use browse_cli::client::load_template;
/// // 全未命中（根也不在册）：报错列出试过的候选位
/// let err = load_template("no/such/tmpl", std::path::Path::new("/no/such/root")).unwrap_err();
/// assert!(err.to_string().contains("no/such/tmpl"), "{}", err);
/// ```
pub fn load_template(name: &str, root: &std::path::Path) -> Result<(String, String)> {
    load_template_ext(name, root, "js", "模板")
}

/// [`load_template`] 的扩展名与语境标签变体（#67：Rune 脚本用 `.rn` 与
/// 「脚本」文案；评审 #67 一轮 G7）。
///
/// # Errors
///
/// 同 [`load_template`]：所有候选位都不存在或读失败。
pub fn load_template_ext(
    name: &str,
    root: &std::path::Path,
    ext: &str,
    kind: &str,
) -> Result<(String, String)> {
    if name.is_empty() {
        anyhow::bail!(
            "@ 后缺{kind}名；下一步：browse --js @<库内名|路径> --vars 键=值，或 browse rune @<名>（Rune 面）"
        );
    }
    let mut tried: Vec<String> = Vec::new();
    // 根道：原样 + 补 .js 两试（<site>/<task> 省后缀直达库内分层）
    if root.canonicalize().is_ok() {
        for cand in [root.join(name), {
            let mut p = root.join(name);
            p.set_extension(ext);
            p
        }] {
            if let (Ok(canon), Ok(root_canon)) = (cand.canonicalize(), root.canonicalize())
                && canon.starts_with(&root_canon)
                && let Ok(body) = std::fs::read_to_string(&canon)
            {
                return Ok((canon.display().to_string(), body));
            }
            tried.push(cand.display().to_string());
        }
    }
    // 字面道（#64 回落）：任何路径形皆可
    let lit = std::path::Path::new(name);
    if let Ok(body) = std::fs::read_to_string(lit) {
        return Ok((
            std::fs::canonicalize(lit)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| name.to_string()),
            body,
        ));
    }
    tried.push(name.to_string());
    anyhow::bail!(
        "{kind} {name} 未命中（试过：{}）；下一步：browse snippets list 看库内在册，或核对文件路径",
        tried.join("、")
    );
}

/// GET /health 取 daemon 状态面；daemon 不在时报可照抄的拉起提示。
///
/// # Errors
///
/// daemon 不在或响应坏。
pub async fn health() -> Result<Value> {
    daemon_alive()
        .await
        .ok_or_else(|| anyhow!("browse: daemon 不在（先随便跑一条片段自动拉起，或 browse up）"))
}

/// `engine_up` 的参数面：CLI up 旗标直通（字段名即 /engine/up 请求键）。
pub struct UpParams {
    /// spawn 时无头。
    pub headless: bool,
    /// 显式 chrome 路径。
    pub chrome: Option<String>,
    /// 显式 WS URL。
    pub ws: Option<String>,
    /// 显式端口。
    pub port: Option<u16>,
    /// spawn 走 CDP 管道。
    pub pipe: bool,
    /// 自定义 user-data-dir。
    pub profile: Option<String>,
    /// 引擎代理（#25.5）。
    pub proxy: Option<String>,
    /// 代理旁路（#25.5）。
    pub proxy_bypass: Option<String>,
    /// 隔离态 profile（#25.3 拆出）。
    pub isolated: bool,
    /// 引擎附加旗标（#48，可叠加）。
    pub engine_args: Vec<String>,
}

/// POST /engine/up 显式起引擎，走 ensure 全链（附着优先缺则 spawn）。
///
/// # Errors
///
/// HTTP 失败或引擎 ensure 失败（连不上、chrome 找不到、spawn 超时）。
pub async fn engine_up(p: UpParams) -> Result<Value> {
    let c = client();
    let resp = c
        .post(format!("{}/engine/up", http()))
        .json(&json!({
            "headless": p.headless,
            "chrome": p.chrome,
            "ws": p.ws,
            "port": p.port,
            "pipe": p.pipe,
            "profile": p.profile,
            "proxy": p.proxy,
            "proxy_bypass": p.proxy_bypass,
            "isolated": p.isolated,
            "engine_args": p.engine_args,
        }))
        .send()
        .await
        .context("POST /engine/up")?;
    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .with_context(|| format!("解析 /engine/up 响应（HTTP {status}）"))?;
    if body.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(body)
    } else {
        Err(anyhow!(
            "{}",
            body.get("error")
                .and_then(Value::as_str)
                .unwrap_or("未知错误")
        ))
    }
}

/// POST /quit 退 daemon，daemon 侧顺带只终结自起引擎（附着来源不动）。
///
/// # Errors
///
/// daemon 不在（幂等场景调用方先判活）或 HTTP 失败。
pub async fn quit() -> Result<()> {
    client()
        .post(format!("{}/quit", http()))
        .send()
        .await
        .context("POST /quit")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use browse_rune::vars_syntax::VarsSyntax;

    /// 评审 F-1（0.30.0 对齐轮）：占位遇 Jinja 控制流——死分支丢弃静默
    /// 略过、循环体复制全量回填，不再误报 exit 2；rune 与 JS 双面锁。
    #[test]
    fn template_placeholder_survives_jinja_control_flow() {
        let vars = vec![
            ("q".to_string(), "v".to_string()),
            ("list".to_string(), "ab".to_string()),
        ];
        // 死分支：区间被 Jinja 丢弃，渲染成功且注释消失（注释到行尾，
        // 控制流标签须另起行——与真实 JS/Rune 词法一致）
        let dead = "<% if false %>\n// dead {{ nope }}\n<% endif %>return \"<%= q %>\";";
        let out = render_rune_template(VarsSyntax::default(), dead, &vars).unwrap();
        assert!(!out.contains("nope"), "{out}");
        assert!(out.contains("\"v\""), "{out}");
        let out = render_js_template(VarsSyntax::default(), dead, &vars).unwrap();
        assert!(!out.contains("nope"), "{out}");
        // 循环体：复制两次全量回填
        let body = "<% for c in list %>\n// loop {{ nope }}\n<% endfor %>return \"<%= q %>\";";
        let out = render_rune_template(VarsSyntax::default(), body, &vars).unwrap();
        assert_eq!(out.matches("// loop {{ nope }}").count(), 2, "{out}");
        assert!(out.contains("\"v\""), "{out}");
        let out = render_js_template(VarsSyntax::default(), body, &vars).unwrap();
        assert_eq!(out.matches("// loop {{ nope }}").count(), 2, "{out}");
    }

    /// G-1 对齐批：JS 族（方言/--js 面）注释惰性化，字符串照旧渲染。
    #[test]
    fn js_template_comments_are_inert() {
        let body = "// 用法 {{ nope }} 与 {% raw %} 字面\nreturn \"<%= q %>\";";
        let vars = vec![("q".to_string(), "填了".to_string())];
        let out = render_js_template(VarsSyntax::default(), body, &vars).unwrap();
        assert!(
            out.contains("// 用法 {{ nope }} 与 {% raw %} 字面\n"),
            "{out}"
        );
        assert!(out.contains("\"填了\""), "{out}");
    }

    /// REQ-020 注入理论形收口：值含占位前缀即拦。
    #[test]
    fn vars_value_with_placeholder_prefix_rejected() {
        let vars = vec![("q".to_string(), "__BROWSE_RN_RAW_0__".to_string())];
        let err = render_rune_template(
            VarsSyntax::Classic,
            "let s = r#\"x\"#; let v = \"{{ q }}\";",
            &vars,
        );
        assert!(err.is_err());
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("占位前缀"), "{msg}");
        let err2 = render_js_template(VarsSyntax::Classic, "// c\nreturn \"{{ q }}\";", &vars);
        assert!(err2.is_err());
    }

    /// 批次三①：rune 模板道里注释区 Jinja 惰性化（含槽占位渲染、注释
    /// 字面 `{{ }}` 与 raw 标签逐字回填）。
    #[test]
    fn rune_template_comments_are_inert() {
        let body = "// 用法：{{ q }} 会填，注释里 {{ not_given }} 与 {% raw %} 字面\nlet s = \"<%= q %>\";";
        let vars = vec![("q".to_string(), "填了".to_string())];
        let out = render_rune_template(VarsSyntax::default(), body, &vars).unwrap();
        assert!(
            out.contains("// 用法：{{ q }} 会填，注释里 {{ not_given }} 与 {% raw %} 字面\n"),
            "{out}"
        );
        assert!(out.contains("\"填了\""), "{out}");
    }

    /// #64 文件实参形态：@ 前缀实参换成文件内容，其余实参不动；读失败
    /// 报错带 CTA。纯文件系统，零网络。
    #[test]
    fn at_args_expand_from_file() {
        // cwd 是进程全局态：与 env 敏感测试同锁串行（TEST_ENV_LOCK 惯例）
        let _env_guard = crate::TEST_ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "browse-at-args-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let file = dir.join("snip.js");
        std::fs::write(&file, "return 1 + 1").expect("写临时片段");
        // 相对路径形态：以临时目录为 cwd 跑（子进程不行，这里直接拼相对名）
        let mut args = vec![
            "keep-me".to_string(),
            format!("@{}", file.display()),
            "@/definitely/not/here.js".to_string(),
        ];
        let prev = std::env::current_dir().expect("取 cwd");
        std::env::set_current_dir(&dir).expect("切 cwd");
        let r = expand_at_args(&mut args);
        std::env::set_current_dir(prev).ok();
        std::fs::remove_dir_all(&dir).ok();

        // 第三个 @ 是坏路径：整批报错带 CTA，且错误文本点名坏路径
        let err = r.expect_err("坏路径应报错");
        assert!(
            err.to_string().contains("@实参读文件失败"),
            "错误应点名 @ 实失败：{err}"
        );
        assert!(
            err.to_string().contains("/definitely/not/here.js"),
            "错误应带路径：{err}"
        );

        // 去掉坏路径重跑：非 @ 实参原样、@ 相对路径（cwd 基准）展开成文件内容
        let mut ok_args = vec!["keep-me".to_string(), "@snip.js".to_string()];
        let prev = std::env::current_dir().expect("取 cwd");
        std::fs::create_dir_all(&dir).expect("重建临时目录");
        std::fs::write(&file, "return 1 + 1").expect("重写临时片段");
        std::env::set_current_dir(&dir).expect("切 cwd");
        let r = expand_at_args(&mut ok_args);
        std::env::set_current_dir(prev).ok();
        std::fs::remove_dir_all(&dir).ok();
        r.expect("好路径应通过");
        assert_eq!(ok_args[0], "keep-me", "非 @ 实参原样保留");
        assert_eq!(ok_args[1], "return 1 + 1", "@ 实参换成文件内容");
    }

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "browse-tmpl-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&d).expect("建临时目录");
        d
    }

    /// #66 模板源解析序：snippets 根第一等（原样 + 补 .js 两试），越根
    /// 候选跳过根道；根未命中回落字面路径。纯文件系统，零网络。
    #[test]
    fn template_resolution_root_first_then_fallback() {
        let _env_guard = crate::TEST_ENV_LOCK.lock().unwrap();
        let dir = tmp_dir("resolve");
        let root = dir.join("root");
        std::fs::create_dir_all(root.join("x.test")).expect("建库");
        std::fs::write(root.join("x.test/t.js"), "// 库内\n").expect("写库内模板");
        std::fs::write(dir.join("t.js"), "// 字面\n").expect("写字面模板");

        // 省后缀：根/<名>.js 重试命中（根道第一等）
        let (_, body) = load_template("x.test/t", &root).expect("根道 .js 重试应命中");
        assert_eq!(body, "// 库内\n");
        // 带后缀原样命中根道
        let (_, body) = load_template("x.test/t.js", &root).expect("根道原样应命中");
        assert_eq!(body, "// 库内\n");
        // 根未命中回落字面路径（cwd 基准；绝对与相对同效）
        let prev = std::env::current_dir().expect("取 cwd");
        std::env::set_current_dir(&dir).expect("切 cwd");
        let r = load_template("t.js", &root);
        std::env::set_current_dir(prev).ok();
        let (_, body) = r.expect("字面回落应命中");
        assert_eq!(body, "// 字面\n");
        // 越根相对名：根道候选跳过，字面道（cwd 基准）命中库外真文件
        // （cwd 收进子目录，.. 稳定落在自管探针上，不碰共享 /tmp）
        let inner = dir.join("inner");
        std::fs::create_dir_all(&inner).expect("建子目录");
        std::fs::write(dir.join("escape.js"), "// 库外\n").expect("写库外探针");
        let prev = std::env::current_dir().expect("取 cwd");
        std::env::set_current_dir(&inner).expect("切 cwd");
        let r = load_template("../escape.js", &root);
        std::env::set_current_dir(prev).ok();
        let (_, body) = r.expect("越根走字面道应命中");
        assert_eq!(body, "// 库外\n");
        // 全未命中：报错列出试过的候选
        let err = load_template("no/such", &root).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no/such"), "{msg}");
        assert!(msg.contains("x.test") || msg.contains("root"), "{msg}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// #66 渲染面：重复键后值胜、值含等号引号空串原样直插、未填键点名、
    /// default 过滤器可选参通行。
    #[test]
    fn template_render_values_and_missing() {
        let vars = vec![
            ("k".to_string(), "first".to_string()),
            ("k".to_string(), "second".to_string()),
            ("e".to_string(), String::new()),
            ("q".to_string(), "'a=b'".to_string()),
        ];
        assert_eq!(
            render_template("f({{ k }},{{ e }},{{ q }})", &vars).unwrap(),
            "f(second,,'a=b')"
        );
        // 未填键点名（default 参不算缺）
        let err = render_template("{{ a }} {{ b | default(1) }} {{ c }}", &[]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("a") && msg.contains("c"), "{msg}");
        assert!(!msg.contains("b"), "default 参不点名：{msg}");
        // 语法错带 CTA
        let err = render_template("{% if %}", &[]).unwrap_err();
        assert!(err.to_string().contains("模板渲染失败"), "{err}");
        // 死分支与 raw 块不误报（评审 #66 一轮 F1 反转后：引擎先渲染，
        // 扫描器只在未定义类报错后富化——{% if false %}/{% raw %} 里的
        // 占位符不触发「未填」）
        let vars1 = vec![("k".to_string(), "1".to_string())];
        assert_eq!(
            render_template("{% if false %}{{ neverused }}{% endif %}ok", &vars1).unwrap(),
            "ok"
        );
        assert_eq!(
            render_template("{% raw %}{{ y }}{% endraw %}", &vars1).unwrap(),
            "{{ y }}"
        );
        // 生成面实弹：for 遍历字符串按字符（G4 口径）
        let list = vec![("list".to_string(), "ab".to_string())];
        assert_eq!(
            render_template("{% for c in list %}{{ c }}-{% endfor %}", &list).unwrap(),
            "a-b-"
        );
    }
}
