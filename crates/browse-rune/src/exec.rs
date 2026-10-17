//! Rune 执行器（账本 #67）：一套 `Context` 编译一次，VM 经
//! `try_clone` 派发，`send_execute` 跨线程执行体包
//! [`rune::runtime::budget`]（指令数）与 [`rune::alloc::limit`]（分配）
//! 双限额后 `async_complete`。
//!
//! 结果桥形（探针定案）：`send_execute` 对逃逸值包一层外部引用（serde
//! 与 `FromValue` 都穿不透），唯脚本到原生函数的参数方向畅通——执行器
//! 在源码尾自动追加 `__browse_rune_wrap__` 包装：VM 内拆 `main` 的
//! `Result`、经 `json::to_string` 序列化成字符串、调 `cdp::__result`
//! 回传 Rust 侧槽位。回执形恒 `{"ok": <json>}` 或 `{"error": <json>}`。
//!
//! 错误面（方案约定）：脚本可见失败（`cdp::CdpError`、脚本自造
//! `Timeout` 等）走 `{"error": …}` 属正常回执；`VmError`（含双限额击穿）
//! 是终态——映射为 [`crate::run`] 的 `Err`，不让脚本重试。

use std::sync::Arc;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::module::{ModuleCtx, cdp_module};

/// 双限额（方案条款：执行时套两层；原生函数自己尊重预算——本模块的
/// 长动作都有界：goto 等 15s、wait_event 按参）。
#[derive(Debug, Clone, Copy)]
pub struct Caps {
    /// VM 指令预算（`rune::runtime::budget`）。
    pub instructions: usize,
    /// 分配限额字节（`rune::alloc::limit`）。
    pub memory: usize,
}

/// 缺省限额：指令一千万（编排脚本量级富余），分配 16MB。
pub const DEFAULT_CAPS: Caps = Caps {
    instructions: 10_000_000,
    memory: 16 * 1024 * 1024,
};

/// 执行选项（三形态批）：snippets 根（`js_lib` 解析位）与管道参数串
/// （`cdp::stdin()` 的值）。
#[derive(Clone, Default)]
pub struct RunOpts {
    /// `js_lib` 的片段库根（daemon 侧三序收口后的终值）。
    pub scripts_root: std::path::PathBuf,
    /// 管道参数道捕获串（无管道即空串）。
    pub stdin: String,
    /// 三支柱同源宿主函数束（daemon 注入；缺省时对应 cdp 函数报未注入）。
    pub host_fns: crate::module::HostFns,
    /// 在飞调用面探针（v0.26.1 超时 CTA）：调用方持克隆，整体超时丢弃
    /// 执行 future 后读在飞面。
    pub op_probe: crate::module::OpProbe,
    /// 模板定界形态（0.30.3 缺省 ERB）：js_lib 与 cdp::render 跟随本执
    /// 行的选择（CLI 旗标/件首行指令收口后递入）。
    pub vars_syntax: crate::vars_syntax::VarsSyntax,
}

/// 自动追加的结果回传包装（入口恒 `pub async fn main()`）。
const WRAPPER: &str = r#"
pub async fn __browse_rune_wrap__() {
    let r = main().await;
    match r {
        Ok(v) => cdp::__result(format!("ok:{}", match json::to_string(v) { Ok(s) => s, Err(_) => format!("{:?}", v) })),
        Err(e) => cdp::__result(format!("err:{}", match cdp::__describe(e) { Some(s) => s, None => format!("{:?}", e) })),
    }
}
"#;

/// 编译并执行一段 Rune 编排脚本（入口恒 `pub async fn main()`）。
///
/// 返回回执 JSON：`{"ok": <main 成功载荷>}` 或 `{"error": <脚本可见
/// 失败载荷>}`；`Err` 是宿主侧终态（编译错、`VmError`、限额击穿）。
///
/// # Errors
///
/// 编译失败、VM 终态错误、双限额击穿、结果回传道异常。
pub async fn run(
    session: Arc<cdp::Session>,
    source: &str,
    caps: Caps,
    opts: RunOpts,
) -> Result<Value> {
    // r# 裸串预处理（免渲染道兜底；CLI 模板道已在渲染侧先抽后填，此处
    // 对无 r# 记号的源是幂等空转）
    let source = crate::rawstr::rewrite_raw_strings(source)
        .map_err(|e| anyhow!("r# 裸串预处理失败：{}", e.message))?;
    run_with_cleanup(session, &source, caps, opts).await
}

/// [`run`] 的实现腿：收尾清账（对象组释放加引导脚本注销）。
async fn run_with_cleanup(
    session: Arc<cdp::Session>,
    source: &str,
    caps: Caps,
    opts: RunOpts,
) -> Result<Value> {
    let cleanup = run_capture_cleanup(session, source, caps, opts).await;
    let (out, cleanup) = cleanup;
    if let Some(c) = &cleanup {
        c.run().await;
    }
    out
}

/// 裸执行腿：返回结果与收尾器（None = 编译前失败，无清账面）。
async fn run_capture_cleanup(
    session: Arc<cdp::Session>,
    source: &str,
    caps: Caps,
    opts: RunOpts,
) -> (Result<Value>, Option<crate::module::Cleanup>) {
    match run_raw(session, source, caps, opts).await {
        (out, Some(c)) => (out, Some(c)),
        (out, None) => (out, None),
    }
}

async fn run_raw(
    session: Arc<cdp::Session>,
    source: &str,
    caps: Caps,
    opts: RunOpts,
) -> (Result<Value>, Option<crate::module::Cleanup>) {
    let result_slot = Arc::new(std::sync::Mutex::new(None::<String>));
    let mut context = match rune::Context::with_default_modules() {
        Ok(c) => c,
        Err(e) => return (Err(anyhow!("Rune 上下文构建失败：{e}")), None),
    };
    let (module, cleanup) = match cdp_module(ModuleCtx {
        session,
        result_slot: result_slot.clone(),
        scripts_root: opts.scripts_root,
        stdin: opts.stdin,
        host_fns: opts.host_fns,
        op_probe: opts.op_probe,
        vars_syntax: opts.vars_syntax,
    }) {
        Ok(v) => v,
        Err(e) => return (Err(anyhow!("cdp 模块注册失败：{e}")), None),
    };
    let build_ctx = (|| -> Result<(), anyhow::Error> {
        context.install(rune_modules::json::module(true).map_err(|e| anyhow!("{e}"))?)?;
        context.install(rune_modules::time::module(true).map_err(|e| anyhow!("{e}"))?)?;
        context.install(module).map_err(|e| anyhow!("{e}"))?;
        Ok(())
    })();
    if let Err(e) = build_ctx {
        return (Err(e), Some(cleanup));
    }

    let mut sources = rune::Sources::new();
    let inserted = rune::Source::new("script", format!("{source}\n{WRAPPER}"))
        .map_err(|e| anyhow!("{e}"))
        .and_then(|src| sources.insert(src).map_err(|e| anyhow!("{e}")));
    if let Err(e) = inserted {
        return (Err(e), Some(cleanup));
    }
    let mut diagnostics = rune::Diagnostics::new();
    let built = rune::prepare(&mut sources)
        .with_context(&context)
        .with_diagnostics(&mut diagnostics)
        .build();
    let unit = match built {
        Ok(u) => u,
        Err(e) => {
            // 诊断渲染进无色缓冲（BuildError 自身只有「见诊断」一句）
            let mut buf = rune::termcolor::Buffer::no_color();
            let _ = diagnostics.emit(&mut buf, &sources);
            let text = String::from_utf8_lossy(buf.as_slice()).trim().to_string();
            return (
                Err(anyhow!(
                    "Rune 编译失败：{e}\n{text}\n下一步：修脚本语法（入口 pub async fn main()，不支持返回类型注解；错误用 ? 传播、回值用 Ok(..)/Err(..)）"
                )),
                Some(cleanup),
            );
        }
    };

    use rune::alloc::clone::TryClone;
    let vm = match (|| -> Result<rune::Vm> {
        let rt = Arc::new(context.runtime().map_err(|e| anyhow!("{e}"))?);
        let vm = rune::Vm::new(rt, Arc::new(unit));
        vm.try_clone().map_err(|e| anyhow!("{e}"))
    })() {
        Ok(v) => v,
        Err(e) => return (Err(e), Some(cleanup)),
    };
    let exec = match vm.send_execute(["__browse_rune_wrap__"], ()) {
        Ok(e) => e,
        Err(e) => return (Err(anyhow!("Rune 入口派发失败：{e}")), Some(cleanup)),
    };
    // 双限额：budget 管指令数，limit 管分配；两层都是 Future 包装可组合
    let budgeted = rune::runtime::budget::with(caps.instructions, exec.async_complete());
    let limited = rune::alloc::limit::with(caps.memory, budgeted);
    let result: rune::runtime::VmResult<rune::runtime::Value> = limited.await;
    if let Err(e) = result.into_result() {
        // #113 诊断道：BROWSE_RUNE_VMDEBUG=1 时附全量 Debug（指令流/帧栈，
        // 定位 VM 级缺陷用；Display 只有一句不够）
        let dbg = if std::env::var("BROWSE_RUNE_VMDEBUG")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            format!("\n[vmdebug] {e:?}")
        } else {
            String::new()
        };
        return (
            Err(anyhow!(
                "Rune VM 终态错误（不重试）：{}；下一步：看脚本逻辑与限额（指令 {}、分配 {}）{dbg}",
                enrich_missing_fn(&e.to_string()),
                caps.instructions,
                caps.memory
            )),
            Some(cleanup),
        );
    }

    let payload = result_slot.lock().ok().and_then(|mut g| g.take());
    let Some(payload) = payload else {
        // 评审 #67 一轮 F1：最常见根因是 main 未按契约返回 Result（裸值
        // 入口使回传包装匹配失效），文案点名入口形
        return (
            Err(anyhow!(
                "脚本 main 未按契约返回 Result（入口形要 pub async fn main() 返回 Ok(..)/Err(..)；裸值入口的回传包装不生效）；下一步：修入口形后重跑"
            )),
            Some(cleanup),
        );
    };
    let Some((tag, body)) = payload.split_once(':') else {
        return (
            Err(anyhow!(
                "结果回传道异常（载荷无标签形：{payload}）；下一步：报告 browse issue"
            )),
            Some(cleanup),
        );
    };
    let (tag, body) = (tag.to_string(), body.to_string());
    let value = serde_json::from_str(&body).unwrap_or_else(|_| Value::String(body.to_string()));
    let out = match tag.as_str() {
        "ok" => Ok(json!({ "ok": value })),
        "err" => Ok(json!({ "error": value })),
        other => Err(anyhow!(
            "结果回传道异常（未知标签 {other}）；下一步：报告 browse issue"
        )),
    };
    (out, Some(cleanup))
}

/// 缺失实例函数的哈希→名字表（#70 补名腿）：Rune 的
/// 「Missing instance function `0x…`」报错只带确定散列哈希（战役台两枚
/// 实测锚：to_string=0x871c12da43b176d2、splitn=0x7d6f3bd0775df4c8，跨
/// 机逐位一致）；本表在首次遇到该类错误时用一次性 VM 探针收割（在
/// String/i64/Vec 上逐个调常用方法名，收缺失错的哈希），OneLock 缓存。
/// 命中即在报错里补名（哈希保留可溯源）；未命中维持原文。
/// 建表后台预热（REQ-020 G-3 收口）：daemon 启动时调一次，独立线程建
/// 哈希表，用户首个缺失函数错误不再付约 2 秒收割时延。
pub fn preheat_missing_fn_table() {
    std::thread::spawn(|| {
        missing_fn_table();
    });
}

fn missing_fn_table() -> &'static std::collections::HashMap<u64, &'static str> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<u64, &'static str>> =
        std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        for ty in ["String::from(\"x\")", "1", "Vec::new()"] {
            for name in [
                "to_string",
                "to_owned",
                "as_str",
                "chars",
                "lines",
                "bytes",
                "splitn",
                "rsplit",
                "rsplit_once",
                "to_uppercase",
                "to_lowercase",
                "is_empty",
                "len",
                "trim",
                "trim_start",
                "trim_end",
                "replace",
                "contains",
                "starts_with",
                "ends_with",
                "find",
                "parse",
                "push",
                "pop",
                "join",
                "split",
                "get",
                "iter",
            ] {
                if let Some(h) = probe_missing_fn_hash(ty, name) {
                    // 同名不同型不同哈希：都收（entry.or_insert 同哈希首写胜）
                    m.entry(h).or_insert(name);
                }
            }
        }
        m
    })
}

/// 单名探针：在一次性 VM（仅默认模块，零 cdp）里调 `ty.name()`，错误是
/// 缺失实例函数时解析其哈希返回；方法存在或其他错误返回 None。
fn probe_missing_fn_hash(ty: &str, name: &str) -> Option<u64> {
    let src = format!("pub fn main() {{ let v = {ty}; v.{name}() }}");
    let context = rune::Context::with_default_modules().ok()?;
    let mut sources = rune::Sources::new();
    sources.insert(rune::Source::new("probe", src).ok()?).ok()?;
    let unit = rune::prepare(&mut sources)
        .with_context(&context)
        .build()
        .ok()?;
    let rt = context.runtime().ok()?;
    let mut vm = rune::Vm::new(std::sync::Arc::new(rt), std::sync::Arc::new(unit));
    let out = vm.execute(["main"], ()).ok()?.complete().into_result();
    if let Err(e) = out {
        let (h, _, _) = parse_missing_fn_hash(&e.to_string())?;
        return Some(h);
    }
    None
}

/// 解析 VmError 文本里的缺失实例函数哈希（探针与终态错误包装共用）。
fn parse_missing_fn_hash(msg: &str) -> Option<(u64, usize, usize)> {
    let marker = "Missing instance function `0x";
    let start = msg.find(marker)? + marker.len();
    let end = msg[start..].find('`')? + start;
    u64::from_str_radix(&msg[start..end], 16)
        .ok()
        .map(|h| (h, start - 3, end + 1))
}

/// 终态错误文本补名（#70）：哈希命中查表即在报错里带方法名（保留哈希
/// 可溯源）；不命中原文返回。
fn enrich_missing_fn(msg: &str) -> String {
    let Some((hash, from, to)) = parse_missing_fn_hash(msg) else {
        return msg.to_string();
    };
    // 评审 G-3：一次性建表约 2 秒（84 探针），仅在首个缺失函数错误时
    // 发生（OnceLock 后零成本）；block_in_place 在 current-thread 运行时
    // 会 panic（vm_face 测试即该形态），故直取并记档（后台预热是后续
    // 可选优化）
    match missing_fn_table().get(&hash) {
        Some(name) => {
            let mut out = String::with_capacity(msg.len() + 32);
            out.push_str(&msg[..from]);
            out.push_str(&format!("{name}（哈希 0x{hash:x}）"));
            out.push_str(&msg[to..]);
            out
        }
        None => msg.to_string(),
    }
}

#[cfg(test)]
mod missing_fn_tests {
    use super::*;

    /// 战役台实测锚（跨机逐位一致）：表必须含 to_string 与 splitn。
    #[test]
    fn table_has_field_anchors() {
        let t = missing_fn_table();
        assert_eq!(t.get(&0x871c12da43b176d2), Some(&"to_string"), "锚一");
        assert_eq!(t.get(&0x7d6f3bd0775df4c8), Some(&"splitn"), "锚二");
    }

    /// 补名文本：命中带名保留哈希；普通错误原文。
    #[test]
    fn enrich_names_known_hash() {
        let out = enrich_missing_fn(
            "Missing instance function `0x871c12da43b176d2` for `::std::string::String`",
        );
        assert!(
            out.contains("to_string（哈希 0x871c12da43b176d2）"),
            "{out}"
        );
        assert_eq!(enrich_missing_fn("别的错"), "别的错");
    }
}
