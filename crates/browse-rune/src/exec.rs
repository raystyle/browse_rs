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

use crate::module::{cdp_module, release_object_group};

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

/// 自动追加的结果回传包装（入口恒 `pub async fn main()`）。
const WRAPPER: &str = r#"
pub async fn __browse_rune_wrap__() {
    let r = main().await;
    match r {
        Ok(v) => cdp::__result(format!("ok:{}", match json::to_string(v) { Ok(s) => s, Err(_) => "null" })),
        Err(e) => cdp::__result(format!("err:{}", match json::to_string(e) { Ok(s) => s, Err(_) => format!("{:?}", e) })),
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
pub async fn run(session: Arc<cdp::Session>, source: &str, caps: Caps) -> Result<Value> {
    let out = run_raw(session.clone(), source, caps).await;
    release_object_group(&session).await;
    out
}

async fn run_raw(session: Arc<cdp::Session>, source: &str, caps: Caps) -> Result<Value> {
    let result_slot = Arc::new(std::sync::Mutex::new(None::<String>));
    let mut context = rune::Context::with_default_modules()?;
    context.install(rune_modules::json::module(true)?)?;
    context.install(rune_modules::time::module(true)?)?;
    context.install(cdp_module(session, result_slot.clone())?)?;

    let mut sources = rune::Sources::new();
    sources.insert(rune::Source::new("script", format!("{source}\n{WRAPPER}"))?)?;
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
            anyhow::bail!(
                "Rune 编译失败：{e}\n{text}\n下一步：修脚本语法（入口 pub async fn main()）"
            );
        }
    };

    use rune::alloc::clone::TryClone;
    let rt = Arc::new(context.runtime()?);
    let vm = rune::Vm::new(rt, Arc::new(unit)).try_clone()?;
    let exec = vm.send_execute(["__browse_rune_wrap__"], ())?;
    // 双限额：budget 管指令数，limit 管分配；两层都是 Future 包装可组合
    let budgeted = rune::runtime::budget::with(caps.instructions, exec.async_complete());
    let limited = rune::alloc::limit::with(caps.memory, budgeted);
    let result: rune::runtime::VmResult<rune::runtime::Value> = limited.await;
    result.into_result().map_err(|e| {
        anyhow!(
            "Rune VM 终态错误（不重试）：{e}；下一步：看脚本逻辑与限额（指令 {}、分配 {}）",
            caps.instructions,
            caps.memory
        )
    })?;

    let payload = result_slot
        .lock()
        .ok()
        .and_then(|mut g| g.take())
        .ok_or_else(|| {
            anyhow!(
                "结果回传道异常（脚本未走 __result 包装）；下一步：报告 browse issue，附脚本与回执"
            )
        })?;
    let (tag, body) = payload.split_once(':').ok_or_else(|| {
        anyhow!("结果回传道异常（载荷无标签形：{payload}）；下一步：报告 browse issue")
    })?;
    let value = serde_json::from_str(body).unwrap_or_else(|_| Value::String(body.to_string()));
    match tag {
        "ok" => Ok(json!({ "ok": value })),
        "err" => Ok(json!({ "error": value })),
        other => anyhow::bail!("结果回传道异常（未知标签 {other}）；下一步：报告 browse issue"),
    }
}
