# browse-rune::exec

Rune 执行器（账本 #67）：一套 `Context` 编译一次，VM 经
`try_clone` 派发，`send_execute` 跨线程执行体包
[`rune::runtime::budget`]（指令数）与 [`rune::alloc::limit`]（分配）
双限额后 `async_complete`。

结果桥形（探针定案）：`send_execute` 对逃逸值包一层外部引用（serde
与 `FromValue` 都穿不透），唯脚本到原生函数的参数方向畅通——执行器
在源码尾自动追加 `__browse_rune_wrap__` 包装：VM 内拆 `main` 的
`Result`、经 `json::to_string` 序列化成字符串、调 `cdp::__result`
回传 Rust 侧槽位。回执形恒 `{"ok": <json>}` 或 `{"error": <json>}`。

错误面（方案约定）：脚本可见失败（`cdp::CdpError`、脚本自造
`Timeout` 等）走 `{"error": …}` 属正常回执；`VmError`（含双限额击穿）
是终态——映射为 [`crate::run`] 的 `Err`，不让脚本重试。

## Functions

- `run` — 编译并执行一段 Rune 编排脚本（入口恒 `pub async fn main()`）。

## Types

- `Caps` — 双限额（方案条款：执行时套两层；原生函数自己尊重预算——本模块的
- `RunOpts` — 执行选项（三形态批）：snippets 根（`js_lib` 解析位）与管道参数串

## Constants

- `DEFAULT_CAPS` — 缺省限额：指令一千万（编排脚本量级富余），分配 16MB。

