//! 找到 Chrome 可执行文件并拉起一个带调试口的专属实例。
//!
//! 只服务于「引擎自起」路径：独立 user-data 目录、调试端口自动分配
//! （`--remote-debugging-port=0`，真实端口写进 profile 的
//! `DevToolsActivePort`）、`browse down` 时才终结，绝不碰用户默认 profile。

use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

/// 依优先序探测 Chrome 可执行文件，找不到返回 `None`。
///
/// 优先序：`BROWSE_CHROME` 环境变量 -> 可执行文件同目录与当前目录下的
/// `chromium-*/chrome.exe`（clean-chrome SxS 部署形态）-> 常规安装路径。
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// // 显式路径原样直通（不校验存在性）
/// assert_eq!(
///     cdp::spawn::find_chrome(Some(Path::new("/x/chrome"))),
///     Some("/x/chrome".into()),
/// );
/// // 无显式时按环境与部署探测，结果随机器而变，不在此断言
/// ```
pub fn find_chrome(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    if let Some(env) = std::env::var_os("BROWSE_CHROME") {
        let p = PathBuf::from(env);
        if p.is_file() {
            return Some(p);
        }
    }
    // 从 exe 目录与当前目录沿祖先向上爬，覆盖三种跑法：
    // 仓库内 cargo（target\debug\deps）、cargo test（cwd=包根）、装进 PATH 后在仓库里跑
    let mut roots = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        roots.push(dir.to_path_buf());
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    for root in roots {
        for dir in root.ancestors().take(6) {
            if let Some(p) = newest_chromium_under(dir) {
                return chrome_binary_in_dir(&p);
            }
        }
    }
    fallback_paths().into_iter().find(|p| p.is_file())
}

fn newest_chromium_under(root: &Path) -> Option<PathBuf> {
    /// 版本数值键（`chromium-152.0.7xxx.y` -> (152, 0, 7xxx, y)）：字典序
    /// 会让 `chromium-99.x` 赢过 `chromium-152.x`（评审 G-24），按数值序
    const VER_KEY: fn(&PathBuf) -> (u64, u64, u64, u64) = |p: &PathBuf| {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .trim_start_matches("chromium-");
        let mut it = name.split('.');
        let seg =
            |i: &mut std::str::Split<'_, char>| i.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        (seg(&mut it), seg(&mut it), seg(&mut it), seg(&mut it))
    };
    let mut hits: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("chromium-"))
        })
        .filter(|p| chrome_binary_in_dir(p).is_some())
        .collect();
    hits.sort_by_key(VER_KEY);
    hits.pop()
}

/// 返回本平台的 chrome 二进制名（Windows `chrome.exe`，其余 `chrome`）。
pub fn chrome_binary_name() -> &'static str {
    if cfg!(windows) {
        "chrome.exe"
    } else {
        "chrome"
    }
}

/// 在部署目录里解析 chrome 可执行文件（布局感知）：Windows `chrome.exe`、
/// Linux 裸 `chrome`、macOS `.app` 束内 `Chromium.app/Contents/MacOS/Chromium`
/// （mac 也接受裸 unix 形）。托管部署校验、pin 解析与祖先发现共用此口径。
///
/// # Examples
///
/// ```
/// # use std::path::Path;
/// // 目录里没有二进制时返回 None
/// assert!(cdp::spawn::chrome_binary_in_dir(Path::new("/nonexistent-dir")).is_none());
/// ```
pub fn chrome_binary_in_dir(dir: &Path) -> Option<PathBuf> {
    let mut candidates = vec![dir.join(chrome_binary_name())];
    if cfg!(target_os = "macos") {
        // 束在目录里（版本目录形）与目录本身是束（.app 直指形）都认
        candidates.push(
            dir.join("Chromium.app")
                .join("Contents")
                .join("MacOS")
                .join("Chromium"),
        );
        candidates.push(dir.join("Contents").join("MacOS").join("Chromium"));
    }
    candidates.into_iter().find(|p| p.is_file())
}

fn fallback_paths() -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![
            PathBuf::from(r"C:\Program Files\Chromium\Application\chrome.exe"),
            PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
        ]
    } else {
        vec![
            PathBuf::from("/usr/bin/chromium"),
            PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
        ]
    }
}

/// 引擎 chrome 的 stdio 三路全显式：stderr 落 profile 旁的 `engine.log`，
/// stdin/stdout 置空。
///
/// 绝不继承父进程句柄；引擎常比单次调用方（CLI/测试）活得久，继承的
/// stderr 管道会让调用方管道永不 EOF（实测挂死过整条流水）。
fn engine_stdio(profile_dir: &Path) -> std::process::Stdio {
    let log = profile_dir
        .parent()
        .map(|p| p.join("engine.log"))
        .unwrap_or_else(|| PathBuf::from("engine.log"));
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
    {
        Ok(f) => std::process::Stdio::from(f),
        // 开不了就留痕（评审 G-28）：静默退 null 会让「engine.log 里什么
        // 都没有」与「日志腿没开成」不可分辨
        Err(e) => {
            eprintln!(
                "[browse] engine.log 开不了（{}：{e}），引擎 stderr 置空",
                log.display()
            );
            std::process::Stdio::null()
        }
    }
}

/// #57 无显示会话的 headless 自动回退判定（纯函数）：有头意图下，若
/// extra_args 未显式带 headless 旗标且环境无显示（`display_present` 为
/// false），返回应补的 `--headless`；已无头、用户已显式给旗标、有显示
/// 三种情形都返回 `None`（spawn 行为不变）。何时用：三个 spawn 位的
/// 公共缝（端口态与管道态两实现共用同一口径）。边界：只判 argv 元素
/// 前缀（`--headless` 与 `--headless=*`），不解析语义。
fn headless_fallback(
    headless: bool,
    extra_args: &[String],
    display_present: bool,
) -> Option<&'static str> {
    if headless
        || display_present
        || extra_args
            .iter()
            .any(|a| a == "--headless" || a.starts_with("--headless="))
    {
        return None;
    }
    Some("--headless")
}

/// 显示可用性探测（#57）：Windows 会话制无 X 概念、macOS 是 Cocoa 非
/// X11/Wayland（环境恒无这两变量但有桌面），两者恒真不在回退范围
/// （有头默认与 ADR-0003 口径不动；macOS 误判无显示会把有头 spawn 静默
/// 改无头，评审 F1 回归锁见 tests）；其余 unix（X11/Wayland 面，实跑
/// 矩阵是 linux）看 `DISPLAY` 或 `WAYLAND_DISPLAY` 任一非空即真
/// （WSLg 恒供 `DISPLAY`，ssh 会话恒无）。
fn display_present() -> bool {
    if cfg!(windows) || cfg!(target_os = "macos") {
        return true;
    }
    std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty())
        || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|w| !w.is_empty())
}

/// headless 在 Linux 的自持加固组（clean-chrome 二分定案，油猴验收轮发现
/// 一二同根）：裸 `--headless` 在 WSL/ssh 等环境会进 GPU 初始化崩溃环
/// （EGL/Vulkan/SwANGLE 三连失败反复），消息泵被拖垮——timer/mojo 敏感
/// 面（通知终态回调、注入调度）被饿死且非确定。已知好形长期靠调用方
/// BROWSE_ENGINE_ARGS 兜底，pwsh 等不继承 bash rc env 的调用路径即裸奔
/// ——收进二进制自持，不再赖调用方环境。`--disable-gpu` 无条件入组：
/// DISPLAY 在不代表 GPU 栈可用（WSLg 恒供 DISPLAY 但图形通道可带伤，
/// 本机实证），headless 自动化语义本就稳定优先于 GPU 渲染（screenshot
/// 走软件光栅不受影响）。`extra_args` 已带同前缀旗标时不叠（用户显式
/// 值优先）。Windows/macOS 的 headless 无此面，返回空。
fn headless_hardening(extra_args: &[String]) -> Vec<&'static str> {
    if cfg!(windows) || cfg!(target_os = "macos") {
        return Vec::new();
    }
    let taken = |prefix: &str| {
        extra_args
            .iter()
            .any(|a| a == prefix || a.starts_with(&format!("{prefix}=")))
    };
    let mut out = Vec::new();
    for (flag, prefix) in [
        ("--disable-gpu", "--disable-gpu"),
        ("--noerrdialogs", "--noerrdialogs"),
        ("--ozone-platform=headless", "--ozone-platform"),
        (
            "--ozone-override-screen-size=800,600",
            "--ozone-override-screen-size",
        ),
        ("--use-angle=swiftshader-webgl", "--use-angle"),
    ] {
        if !taken(prefix) {
            out.push(flag);
        }
    }
    out
}

/// 拉起一个带调试口的专属引擎实例（独立 profile、端口自动分配）。
///
/// 参数：可执行文件、独立 profile 目录、是否无头。
///
/// 命令行：`--user-data-dir <dir> --remote-debugging-port=0 --no-first-run
/// --no-default-browser-check --no-sandbox [--headless] about:blank`；
/// 无显示会话（如 ssh 无 DISPLAY）有头意图自动补 `--headless`（#57 的
/// headless_fallback 判定），extra_args 显式 headless 旗标优先不叠补。
///
/// `--no-sandbox`：SxS 部署的 clean-chrome 在本机沙箱进程打不开自身 exe
/// （`Sandbox cannot access executable`，0x5）；引擎是自动化专属隔离实例，
/// 不承载用户浏览面，关沙箱与 clean-chrome 验收实践一致（linux 同款）。
/// chrome 诊断写 `<state>/engine.log`。
///
/// # Errors
///
/// spawn 失败（路径不是可执行文件等）。
pub fn spawn_engine(
    chrome: &Path,
    profile_dir: &Path,
    headless: bool,
    extra_args: &[String],
) -> Result<Child> {
    if let Some(parent) = profile_dir.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    // 清上次 run 的残留端口文件，防 wait 读到死端口（专属 profile，删除安全）
    std::fs::remove_file(profile_dir.join("DevToolsActivePort")).ok();
    let mut cmd = Command::new(chrome);
    cmd.arg(format!("--user-data-dir={}", profile_dir.display()))
        .arg("--remote-debugging-port=0")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--no-sandbox")
        .arg("about:blank")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(engine_stdio(profile_dir));
    if headless {
        cmd.arg("--headless");
        cmd.args(headless_hardening(extra_args));
    } else if let Some(h) = headless_fallback(headless, extra_args, display_present()) {
        // #57：无显示会话有头必挂（ozone 初始化失败即退，DevToolsActivePort
        // 永不落盘），自动回退无头（回退形同享加固组）
        cmd.arg(h);
        cmd.args(headless_hardening(extra_args));
    }
    cmd.args(extra_args);
    cmd.spawn()
        .with_context(|| format!("spawn {}", chrome.display()))
}

/// 等 spawn 出来的实例调试口就绪，返回 WS URL（读它 profile 下的
/// `DevToolsActivePort`）。
///
/// 冷启动首跑（建 profile）可能要几秒，默认上限 15 秒。
///
/// [`wait_devtools_ready_pinned`] 的取料腿（S009 修复批）：extra_args
/// 同时带 `--remote-debugging-port=N`（N 非 0）与
/// `--remote-debugging-address=<非回环>` 时返回 `(host, port)`——此形
/// chrome 不落 DevToolsActivePort，等待腿需按端点探测。只钉端口未绑址
/// 或绑回环按缺省文件道，返回 `None`。
pub fn pinned_debug_endpoint(extra_args: &[String]) -> Option<(&str, u16)> {
    let mut port: Option<u16> = None;
    let mut host: Option<&str> = None;
    for a in extra_args {
        if let Some(v) = a.strip_prefix("--remote-debugging-port=")
            && let Ok(p) = v.parse::<u16>()
            && p != 0
        {
            port = Some(p);
        }
        if let Some(v) = a.strip_prefix("--remote-debugging-address=") {
            host = Some(v);
        }
    }
    let port = port?;
    let host = host?;
    if host == "127.0.0.1" || host == "localhost" || host == "::1" {
        return None;
    }
    Some((host, port))
}

/// 等调试口就绪并返回 WS URL（S009 修复批带绑址回退）：先走
/// `DevToolsActivePort` 文件（回环绑定形）；文件超时且 `pinned_hint`
/// 给了钉死端点（`--engine-arg --remote-debugging-port=N
/// --remote-debugging-address=<非回环>` 形——此形 chrome 不落端口文件，
/// clean-chrome r5 实测 0.0.0.0 与字面 IP 都不落），转 `/json/version`
/// 探测该端点。两路都败报原文件路的错（文案指向 profile 诊断位）。
///
/// # Errors
///
/// 两路超时都未就绪。
pub async fn wait_devtools_ready_pinned(
    profile_dir: &Path,
    pinned_hint: Option<(&str, u16)>,
) -> Result<String> {
    match crate::discovery::wait_active_port_file(profile_dir, Duration::from_secs(15)).await {
        Ok(ws) => Ok(ws),
        Err(file_err) => {
            if let Some((host, port)) = pinned_hint
                && let Ok(ws) = crate::discovery::http_version_ws_url(
                    // 0.0.0.0 绑定的本地可达面是 127.0.0.1（Windows 连
                    // 0.0.0.0 不通）；字面 IP 绑定本机接口直连即达
                    &format!(
                        "http://{}:{port}",
                        if host == "0.0.0.0" { "127.0.0.1" } else { host }
                    ),
                    Duration::from_secs(10),
                )
                .await
            {
                return Ok(ws);
            }
            Err(file_err)
        }
    }
}

/// [`wait_devtools_ready_pinned`] 的无钉死形（回环绑定缺省道）。
///
/// # Errors
///
/// 超时内端口文件没就绪。
pub async fn wait_devtools_ready(profile_dir: &Path) -> Result<String> {
    wait_devtools_ready_pinned(profile_dir, None).await
}

/// 管道态引擎的句柄：chrome 子进程加留在启动器侧的两条 CDP 管道端。
pub struct PipeEngine {
    /// chrome 子进程（drop 不杀；`Engine::shutdown` 负责）。
    pub child: Child,
    /// 从 chrome 读 CDP（out 管道的本侧）。
    pub read: crate::pipe::PipeReader,
    /// 往 chrome 写 CDP（in 管道的本侧）。
    pub write: crate::pipe::PipeWriter,
}

/// 管道通道的开关取值，即 `CLEAN_CHROME_DEBUG` 环境变量的值域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeMode {
    /// 只开 CDP 管道，不开 9222 端口。
    Pipe,
    /// 管道与 9222 端口双通道。
    Both,
}

impl PipeMode {
    /// 返回喂给 `CLEAN_CHROME_DEBUG` 的环境变量值。
    pub fn as_env(self) -> &'static str {
        match self {
            PipeMode::Pipe => "pipe",
            PipeMode::Both => "both",
        }
    }
}

/// 按管道契约拉起 clean-chrome（S005 / D02-6；Windows 句柄态与 POSIX
/// fd 3/4 布线两实现）。
///
/// 启动器建两条匿名管道，把「子读端,子写端」经
/// `--remote-debugging-io-pipes=<in>,<out>` 传入（十进制）。Windows：
/// 两枚句柄标可继承；POSIX：pre_exec 里 `dup2` 布到固定 fd 3/4（只用
/// async-signal-safe 的 dup2/close）后关原件。`CLEAN_CHROME_DEBUG=pipe|both`
/// 决定端口段开否。协议 ASCIIZ。断管即关浏览器（clean-chrome 行为），
/// 生命周期与本进程绑定。
///
/// # Errors
///
/// 建管道 / spawn 失败（POSIX：dup2 失败进 pre_exec 错误）。
pub fn spawn_engine_pipes(
    chrome: &Path,
    profile_dir: &Path,
    headless: bool,
    mode: PipeMode,
    extra_args: &[String],
) -> Result<PipeEngine> {
    #[cfg(unix)]
    {
        use crate::pipe::anon_pair;
        use std::os::unix::process::CommandExt;

        if let Some(parent) = profile_dir.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        // 对齐 Windows 侧：child 读 in、写 out；本侧持有 in_w / out_r
        let (in_r, in_w) = anon_pair().context("pipe(in)")?;
        let (out_r, out_w) = anon_pair().context("pipe(out)")?;
        let in_fd = in_r.h as i32;
        let out_fd = out_w.h as i32;
        // 泄漏面收口（评审 P-1）：本侧写端 in_w 与读端 out_r 的子进程副本
        // 必须在 exec 前关掉——chrome 读 fd 3 要见 EOF 需全部写端关闭，
        // 子进程自持 in_w 副本则 daemon 崩溃/SIGKILL 后 chrome 永不见
        // EOF，「断管即关浏览器」的承诺失效（孤儿 chrome 锁死 profile）
        let close_in_w = in_w.h as i32;
        let close_out_r = out_r.h as i32;

        let mut cmd = Command::new(chrome);
        cmd.arg(format!("--user-data-dir={}", profile_dir.display()))
            .arg("--remote-debugging-io-pipes=3,4")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--no-sandbox")
            .arg("about:blank")
            .env("CLEAN_CHROME_DEBUG", mode.as_env())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(engine_stdio(profile_dir));
        if headless {
            cmd.arg("--headless");
            cmd.args(headless_hardening(extra_args));
        } else if let Some(h) = headless_fallback(headless, extra_args, display_present()) {
            // #57：无显示会话自动回退（同端口态口径；同享加固组）
            cmd.arg(h);
            cmd.args(headless_hardening(extra_args));
        }
        cmd.args(extra_args);
        // fork 后 exec 前：把两端布到固定 fd 3/4（clean-chrome POSIX 契约），
        // 并关掉本侧两端在子进程的副本（评审 P-1）。闭包里只有
        // dup2/close/fcntl（async-signal-safe）
        unsafe {
            cmd.pre_exec(move || {
                // 外层 unsafe 已覆盖（edition 2024 的 unsafe_op_in_unsafe_fn 不适用于
                // 闭包体内的调用，这里嵌套块会被 clippy 判多余）
                let wire = |src: i32, dst: i32| -> std::io::Result<()> {
                    if src == dst {
                        // 同位空转也要清 CLOEXEC（评审四轮 F）：pipe2(O_CLOEXEC)
                        // 后本位带 CLOEXEC，POSIX dup2(fd,fd) 不改标志——
                        // 不清位则读端恰在 3（或写端在 4）时 exec 直接关掉
                        // 这端，chrome 丢管道
                        let flags = libc::fcntl(src, libc::F_GETFD);
                        if flags >= 0
                            && libc::fcntl(src, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0
                        {
                            return Err(std::io::Error::last_os_error());
                        }
                        return Ok(());
                    }
                    if libc::dup2(src, dst) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    libc::close(src);
                    Ok(())
                };
                // 先关泄漏端再 wire：wire 会移动 fd，先关防 3/4 撞号时序歧义
                libc::close(close_in_w);
                libc::close(close_out_r);
                wire(in_fd, 3)?;
                wire(out_fd, 4)?;
                Ok(())
            });
        }
        let child = cmd
            .spawn()
            .with_context(|| format!("spawn {} (pipes fd3/4)", chrome.display()))?;
        // 子进程已持有 3/4；本侧关掉子侧副本防泄漏
        drop(in_r);
        drop(out_w);
        Ok(PipeEngine {
            child,
            read: out_r,
            write: in_w,
        })
    }
    #[cfg(windows)]
    {
        use crate::pipe::anon_pair;
        use std::os::windows::io::AsRawHandle;

        if let Some(parent) = profile_dir.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        // 对齐 pipe-smoke.py：child 读 in_r、写 out_w；本侧持有 in_w / out_r
        let (in_r, in_w) = anon_pair().context("pipe(in)")?;
        let (out_r, out_w) = anon_pair().context("pipe(out)")?;

        crate::pipe::set_inheritable(in_r.h, true)?;
        crate::pipe::set_inheritable(out_w.h, true)?;

        let mut cmd = Command::new(chrome);
        cmd.arg(format!("--user-data-dir={}", profile_dir.display()))
            .arg(format!(
                "--remote-debugging-io-pipes={},{}",
                in_r.as_raw_handle() as usize,
                out_w.as_raw_handle() as usize
            ))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--no-sandbox")
            .arg("about:blank")
            .env("CLEAN_CHROME_DEBUG", mode.as_env())
            // chrome 诊断进 <state>/engine.log；不继承本进程句柄
            // （stderr(Stdio::inherit) 触发 bInheritHandles=TRUE 路径，
            // 但继承的 stderr 管道会让调用方流水永不 EOF）
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(engine_stdio(profile_dir));
        if headless {
            cmd.arg("--headless");
            cmd.args(headless_hardening(extra_args));
        } else if let Some(h) = headless_fallback(headless, extra_args, display_present()) {
            // #57：无显示会话自动回退（Windows 恒真不触发，口径同 unix 侧）
            cmd.arg(h);
        }
        cmd.args(extra_args);
        let child = cmd
            .spawn()
            .with_context(|| format!("spawn {} (pipes)", chrome.display()))?;
        // 子进程已收养两端（继承副本）；本侧关掉子侧副本防泄漏
        drop(in_r);
        drop(out_w);
        Ok(PipeEngine {
            child,
            read: out_r,
            write: in_w,
        })
    }
}

/// 强杀引擎进程（优雅退出 `Browser.close` 失败后的兜底）：Windows
/// `taskkill /T /F` 带子树；unix 对主 pid 发 SIGKILL——chrome 主进程退出
/// 后子进程随退，历史注释「进程树」在 unix 侧收窄为单 pid 口径（评审
/// G-23）。目标恰在杀前退出（kill 竞速 ESRCH）不算失败。
///
/// # Errors
///
/// 杀失败且目标仍存活（unix 经信号 0 复核）。
pub fn terminate_pid(pid: u32) -> Result<()> {
    let st = if cfg!(windows) {
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    } else {
        Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
    };
    match st {
        Ok(s) if s.success() => Ok(()),
        other => {
            // 已死即成功（评审 G-23）：shutdown 的 try_wait 轮询与 terminate
            // 之间进程恰退，ESRCH 不是「杀不掉」
            #[cfg(unix)]
            if unsafe { libc::kill(pid as i32, 0) } != 0 {
                return Ok(());
            }
            match other {
                Ok(s) => Err(anyhow!(
                    "terminate {pid}: exit {}（进程仍在）",
                    s.code().unwrap_or(-1)
                )),
                Err(e) => Err(anyhow!("terminate {pid}: {e}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// S009 修复批锁形：端口+非回环绑址同时钉死才出端点；只钉端口、
    /// 绑回环、缺任一都 None（回环形走 DevToolsActivePort 缺省道）。
    #[test]
    fn pinned_endpoint_shapes() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            pinned_debug_endpoint(&args(&[
                "--remote-debugging-port=9444",
                "--remote-debugging-address=0.0.0.0"
            ])),
            Some(("0.0.0.0", 9444))
        );
        assert_eq!(
            pinned_debug_endpoint(&args(&[
                "--remote-debugging-address=10.10.10.5",
                "--remote-debugging-port=9333"
            ])),
            Some(("10.10.10.5", 9333))
        );
        // 只钉端口（缺绑址）→ None（文件道）
        assert_eq!(
            pinned_debug_endpoint(&args(&["--remote-debugging-port=9444"])),
            None
        );
        // 绑回环 → None（文件道照常落盘）
        assert_eq!(
            pinned_debug_endpoint(&args(&[
                "--remote-debugging-port=9444",
                "--remote-debugging-address=127.0.0.1"
            ])),
            None
        );
        // port=0 随机口 → None（外部无从知口）
        assert_eq!(
            pinned_debug_endpoint(&args(&[
                "--remote-debugging-port=0",
                "--remote-debugging-address=0.0.0.0"
            ])),
            None
        );
    }

    /// headless 加固组自持(clean-chrome 二分定案回归锁):Linux 非空、
    /// 同前缀去重、用户显式值优先。
    #[test]
    fn headless_hardening_group() {
        let full = headless_hardening(&[]);
        if cfg!(windows) || cfg!(target_os = "macos") {
            assert!(full.is_empty(), "win/mac 无此面");
            return;
        }
        assert!(full.contains(&"--disable-gpu"), "无显示应叠: {full:?}");
        assert!(full.contains(&"--ozone-platform=headless"), "{full:?}");
        assert!(full.contains(&"--use-angle=swiftshader-webgl"), "{full:?}");
        // 同前缀去重:用户已显式给 ozone 平台时不叠
        let dedup = headless_hardening(&["--ozone-platform=x11".to_string()]);
        assert!(!dedup.contains(&"--ozone-platform=headless"), "{dedup:?}");
        assert!(
            dedup.contains(&"--use-angle=swiftshader-webgl"),
            "{dedup:?}"
        );
        // 等值形也不叠
        let same = headless_hardening(&["--noerrdialogs".to_string()]);
        assert!(!same.contains(&"--noerrdialogs"), "{same:?}");
    }

    /// #57 回退判定四象限：已无头/显式旗标/有显示都 None，唯「有头意图
    /// 加无显示加无显式旗标」补 --headless。
    #[test]
    fn headless_fallback_matrix() {
        let no_args: Vec<String> = Vec::new();
        // 已无头：不叠补
        assert_eq!(headless_fallback(true, &no_args, false), None);
        // 有显示：行为不变（有头默认不回退）
        assert_eq!(headless_fallback(false, &no_args, true), None);
        // 无显示 + 有头意图 + 无显式旗标：补 --headless
        assert_eq!(
            headless_fallback(false, &no_args, false),
            Some("--headless")
        );
        // 用户显式旗标（两种形）优先，不叠补
        for explicit in ["--headless", "--headless=new"] {
            let args: Vec<String> = vec![explicit.to_string()];
            assert_eq!(
                headless_fallback(false, &args, false),
                None,
                "显式 {explicit} 优先"
            );
        }
        // 显式旗标混在其他旗标里同样识别
        let mixed: Vec<String> = ["--enable-features=X", "--headless=new"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(headless_fallback(false, &mixed, false), None);
    }

    /// #57 F1 回归锁（只在 mac 岗跑）：macOS 是 Cocoa 非 X11/Wayland，
    /// 环境恒无 DISPLAY/WAYLAND_DISPLAY 但有桌面，display_present 必须
    /// 真——否则有头默认被静默改无头（lan-mac 有头测试面回归）。
    #[cfg(target_os = "macos")]
    #[test]
    fn display_present_true_on_macos() {
        assert!(display_present(), "macOS 有显示，不得误判回退");
    }
}
