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
/// 长动作都有界：goto 等 15s、wait_sel 按参）。
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
    pub snippets_root: std::path::PathBuf,
    /// 管道参数道捕获串（无管道即空串）。
    pub stdin: String,
    /// 三支柱同源宿主函数束（daemon 注入；缺省时对应 cdp 函数报未注入）。
    pub host_fns: crate::module::HostFns,
    /// 在飞调用面探针（v0.26.1 超时 CTA）：调用方持克隆，整体超时丢弃
    /// 执行 future 后读在飞面。
    pub op_probe: crate::module::OpProbe,
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
        snippets_root: opts.snippets_root,
        stdin: opts.stdin,
        host_fns: opts.host_fns,
        op_probe: opts.op_probe,
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
                    "Rune 编译失败：{e}\n{text}\n下一步：修脚本语法（入口 pub async fn main() 返回 Result）"
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
        return (
            Err(anyhow!(
                "Rune VM 终态错误（不重试）：{e}；下一步：看脚本逻辑与限额（指令 {}、分配 {}）",
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
