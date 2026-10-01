---
id: ADR-0010
status: accepted
date: 2026-10-01
deciders: 用户（方案定稿与三叉裁定 2026-09-29：自有 cdp 栈、仓内新 crate、并存新入口加 --vars 同道）、raystyle
superseded_by: null
---

# ADR-0010：Rune 编排面（browse rune 与自注册 cdp 模块）

## Context

方言刻意无控制流（ADR-0002 忠实移植），`--js` 的控制流在页面内（导航即清、跨步编排不可携）；「宿主侧全控制流编排」是空缺。用户给完整方案（2026-09-29）：Rune 做编排，自注册 `cdp` 模块做效果，Chrome 只经隔离世界 `Runtime.evaluate` 碰页面；不装 `rune-modules` 的 `full`，脚本无 `block_on`；双限额（指令与分配）；错误二分（`Timeout`/`CdpError` 脚本可见、`VmError` 终态）；参数 `serde_json` 字面量注入；句柄留 Rust 侧；对象组随会话记账。三叉经裁定：自有 cdp crate 栈（非 chromiumoxide，652 方法与域策略守卫复用）、browse_rs 仓内新 crate、`browse rune` 并存新入口且同享 `--vars` 模板道。

## Decision

新增 crates/browse-rune：`cdp_module` 八函数（goto/eval/eval_handle/call/release/click/type_text/wait_sel）全走自有 `cdp::Session::call`（browser handler 泵与会话路由是 cdp crate 既有结构，本面只新增 VM 执行任务）；每 tab `Page.createIsolatedWorld`（worldName `browse-rune`）加 `addScriptToEvaluateOnNewDocument` 同名引导，导航即废世界重建；执行器 `Vm::send_execute` 包 `runtime::budget::with`（指令一千万）与 `alloc::limit::with`（16MB）。结果桥形（探针定案）：`send_execute` 的逃逸包装连 `FromValue` 都穿不透，唯脚本到原生参数方向畅通，包装脚本在 VM 内拆 `main` 的 `Result` 序列化成字符串经 `cdp::__result` 回传 Rust 槽位，回执恒 `{"ok":…}`/`{"error":…}`。CLI `browse rune <script.rn|@名>`（`@` 走 snippets 根三序与 `.rn` 两试，`--vars` 同命令小程序模板道）；daemon 增 `POST /rune`（与 `/eval` 同槽同超时同 envelope）。方言与 `--js` 契约零动。

## Consequences

- 好：编排层获得全控制流（select 超时、分支、循环）且逻辑驻宿主不惧导航；隔离世界与主世界页面脚本互不污染；双限额给脚本执行上界；新依赖只有 rune/rune-modules（json/time 两特性，不装 full）
- 坏：第三语言面（方言/`--js`/Rune）带来学习与手册面成本（`--llms` 120 帽内已挤行）；结果回传走字符串桥（逃逸包装所迫，VM 内序列化对脚本自造错误型落到 Debug 兜底）；`call` 的 this 绑定要求 `function(){…}` 形（箭头函数词法 this，CDP 语义如实披露）
- 缓解：三面分工写进手册（方言管受限编排、`--js` 管页面逻辑、rune 管宿主侧全控制流）；隔离世界只属 rune 面（方言/`--js` 主世界契约不变）

## Alternatives

- chromiumoxide（方案原文）：被否。自有 cdp crate 协议完整（652 方法）、生产验证、域策略守卫在 `Session::call` 层；引第二套 CDP 栈重复且守卫面要重建
- 方言加控制流：被否。破 ADR-0002 忠实移植契约，方言的「无控制流」正是其可审计性来源
- Rune 作方言后继（替换）：被否。迁移面 major 级，并存新入口零回退
