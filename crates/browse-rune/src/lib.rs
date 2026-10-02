//! Rune 编排面（账本 #67，ADR-0010）：`browse rune` 的 VM 构建、自注册
//! `cdp` 模块与执行器。
//!
//! 分工（用户方案 2026-09-29）：Rune 做编排（宿主侧全控制流，填方言
//! 刻意不做的真空），[`cdp`] 模块做效果（全部经自有 cdp crate 的
//! `Session::call`，域策略守卫与 652 方法面复用），Chrome 只通过隔离
//! 世界里的 `Runtime.evaluate` 碰页面。宿主只装 json/time/cdp 三模块
//! （不装 `rune-modules` 的 `full`，不装 fs/process/http——页面请求走
//! 浏览器自己的网络栈），脚本里无 `block_on`（VM 挂起，CDP 往返发生在
//! 注册的 async 原生函数里）。
//!
//! 进程三任务中 browser handler 泵与会话路由是 cdp crate 既有结构，
//! 本 crate 只新增 VM 执行任务（`Vm::send_execute` 跨线程执行体加
//! [`rune::runtime::budget`] 与 [`rune::alloc::limit`] 双限额包裹）。
//!
//! 错误约定：脚本可见失败只有 `cdp::CdpError`（带 `op` 与 `message`）
//! 与脚本自造的 `Timeout` 型；`VmError` 是终态（虚拟机错误直接终止
//! 本次执行，不让脚本重试）。

pub mod exec;
pub mod module;
pub mod rawstr;

pub use exec::{Caps, DEFAULT_CAPS, RunOpts, preheat_missing_fn_table, run};
pub use module::{CdpError, HostFn, HostFns, ModuleCtx, OpProbe, cdp_module};
