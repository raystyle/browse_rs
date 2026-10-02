# browse-rune::module

自注册 `cdp` 模块（账本 #67）：八个效果函数全走自有 [`cdp::Session`]
的 `call`，Chrome 只经隔离世界 `Runtime.evaluate` 碰页面。

纪律：参数经 `serde_json::to_string` 做成 JS 字面量再传入（禁止把
选择器拼进 JS 字符串）；小结果走 JSON 过桥（`() ↔ null`、`#{ } ↔
object`、数组对数组），DOM 节点只留 [`ObjHandle`] 不透明句柄；对象
组随会话记账（本组恒名 `browse-rune`，执行收尾整组
`Runtime.releaseObjectGroup`）；会话句柄留 Rust 侧，不进脚本全局。

## Functions

- `cdp_module` — 注册 `cdp` 模块（方案八函数面 + 三形态批四函数）：`goto`/`eval`/

## Types

- `CdpError` — 脚本可见的 CDP 失败型（方案错误约定）：`op` 是失败操作名，`message`
- `Cleanup` — 执行收尾（方案边界条款 + 评审 #67 一轮 G1/G2）：整组释放本执行
- `HostFn` — 同源宿主面（#68 随批三薄封装）：snapshot/requests/requestDetail 经
- `HostFns` — 三支柱宿主函数注入束（Debug 手写：闭包无派生）。
- `ModuleCtx` — 模块构建上下文（#67 三形态运行时批）：会话与两条输入通道（snippets
- `ObjHandle` — DOM 节点不透明句柄：`eval_handle` 产出，`call`/`release` 消费；脚本
- `OpProbe` — 在飞调用面探针（v0.26.1 超时 CTA；0.31.0 多槽化）：cdp 函数入口置

