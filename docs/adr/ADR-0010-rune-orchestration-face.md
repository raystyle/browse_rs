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

## 增补（2026-10-01，三形态运行时批）

小程序三形态定位（用户定谳）：rn 脚本小程序（js+cdp 双能力）、方言命令（cdp）、js 小程序（js）。rn 形态增 `cdp::js`/`cdp::js_lib`/`cdp::render`/`cdp::stdin` 四函数与输入三通道（文件、管道源码、管道参数）。**两世界分工修订**：原文「Chrome 只经隔离世界碰页面」修订为 eval 族（eval/eval_handle/click 等）走隔离世界轻探针，js 族（js/js_lib）走主世界全量页面逻辑（`--js` 同语义，页面函数可调），rn 形态按需双世界选用。minijinja 引擎配置（Strict 加零 autoescape）在 browse-cli 与 browse-rune 各持一份，由 ADR-0009 钉住同构；js_lib 的库解析与 CLI 侧 load_template_ext 属库内双轨（同一语义两处实现，漂移由各自测试锁）。

## 增补（2026-10-02，超时回执与专属旋钮终裁）

战役台 agent 壳退役外部 timeout 兜底后引擎内 CTA 成唯一安全网，v0.26.1 先行结构化超时回执（`timeout` 对象：op 在飞调用面/elapsed_ms/bound_s，daemon 日志行，CLI stderr 一行 JSON；探针函数级置名清空律见 REQ-016）。**旋钮终裁**：`BROWSE_RUNE_TIMEOUT` 独立旋钮，rune 面整体上界默认 45 秒（战役台诉求口径）；`/eval` 的 `BROWSE_EVAL_TIMEOUT` 300 秒契约零动（长跑方言片段正当面）。上界是 daemon 级（实例内全部 rune 执行同界，env 在 daemon 进程定格）：长爬编排（逐页 goto 累计分钟级）在 45 秒界触顶，按需调大或拆专用命名实例；超时报错 CTA 点名旋钮。实现随批次二（v0.28.0）。

## 增补（2026-10-02，批次二：停止三道、事件等待直出、r# 裸串语言扩展）

总台指令链定谳（同日）：「结合事件驱动、主动条件停止和超时机制集成 CTA」＋「不自封装，原生 CDP 能实现就行」。**停止三道**：`POST /rune/stop`/`browse rune stop`（请求停）、引擎失联联动（两次连续探测断连即停，检测驱动主动停止的 daemon 侧实例）、脚本侧原生自停（select/分支加 Err 返回；曾按「现成 rn 原语」实现 `cdp::stop_self` 后依「不自封装」令回撤）；三道与超时同构丢弃执行 future，stopped 面带 op/elapsed_ms/cause。**事件等待**：`cdp::wait_event(method, ms)` 是自有 cdp crate `Session::wait_for` 的直出面（方言 session.waitFor 同源，零新语义），脚本 match 即条件分支。**r# 裸串（browse 侧语言扩展）**：Rune 上游无 r#"…"# 形（rune-rs #537 自 2023-05 在册零动作），browse 在编译前把 r# 裸串重写为转义普通串（词法边界五点案见战役台回执；模板道先抽后渲再回填，r# 区免渲染与 {% raw %} 各管一层）；**上游对齐条款**：上游将来采纳同形语法（# + N×# + " 对）则预处理幂等空转、无缝退役；browse-rs 单实现（CLI 渲染道与 daemon 入口共用），发版后去 #537 贴实战需求与薄词法重写参考。#67 G7 收口注记（0.30.2）：`__describe` 的降形判定用 `!s.contains("object at")` 嗅探：脚本自造错误型的 Debug 输出若恰好含该子串会被误判「不可序列化」回落 Debug 全文（仅影响载荷形，不影响成败语义）；上游 FromValue 面不开就无法根除，属已知边界。
#69 stdin 读上界与 #70 披露口径随批（REQ-017）。

## Alternatives

- chromiumoxide（方案原文）：被否。自有 cdp crate 协议完整（652 方法）、生产验证、域策略守卫在 `Session::call` 层；引第二套 CDP 栈重复且守卫面要重建
- 方言加控制流：被否。破 ADR-0002 忠实移植契约，方言的「无控制流」正是其可审计性来源
- Rune 作方言后继（替换）：被否。迁移面 major 级，并存新入口零回退
