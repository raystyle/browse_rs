# 2026-10-01 Rune 编排面批（browse rune 与自注册 cdp 模块，账本 #67）

背景：用户给完整方案（2026-09-29）：Rune 做编排、自注册 cdp 模块做效果、Chrome 只经隔离世界 Runtime.evaluate 碰页面；不装 rune-modules full、脚本无 block_on、双限额、错误二分、参数字面量注入、句柄留 Rust 侧、对象组记账。三叉经 AskUserQuestion 裁定：自有 cdp crate 栈（非 chromiumoxide）、browse_rs 仓内新 crate（crates/browse-rune）、`browse rune` 并存新入口且同享 --vars 模板道。立项 #67（dry-run 先行实发）。

## 设计定案

- 方案的「browser handler 泵/会话路由/VM 任务」三任务结构，前两个是 cdp crate 既有结构（session.rs 常驻 WS + flatten attach + sessionId 路由），本面只新增 VM 执行任务；chromiumoxide 被否（自有栈 652 方法与 Session::call 层域策略守卫复用）
- API 面 spike 亲证：rune 0.14.2 的 `Vm::send_execute` 返回 `VmSendExecution`（`async_complete()` 收值）；双限额真实路径是 `rune::runtime::budget::with` 加 `rune::alloc::limit::with`（非方案原文的 `rune::budget`/`rune::limit`），两层都实现 Future 可组合
- 结果桥形（探针定案，ADR-0010 在册）：`send_execute` 对逃逸值包外部引用，serde 与 FromValue 都穿不透、唯脚本到原生函数的参数方向畅通。执行器自动追加 `__browse_rune_wrap__` 包装，VM 内拆 main 的 Result、json 序列化成字符串经 `cdp::__result` 回传 Rust 槽位；回执恒 `{"ok":…}`/`{"error":…}`；脚本自造错误型 json 序列化不了落 Debug 兜底
- rune 0.14 语言坑实录：`let mut` 不支持（全可变）；`use json;` 与 `json::path` 同用撞 import 环（去 use 直呼路径）；原生函数注册 `Module::function` 返回 builder 要 `.build()`；`m.function` 已统一替代 async_function；自定义类型 CLONE 协议要显式 `#[rune::function(protocol = CLONE)]` 加 function_meta 注册（句柄按值消费两次即撞 move 守卫）

## 实现

- crates/browse-rune：module.rs（八函数 cdp 模块：隔离世界 ensure（Page.getFrameTree 取主 frame、createIsolatedWorld worldName browse-rune、addScriptToEvaluateOnNewDocument 同名引导幂等注册、导航即废世界重建）、eval_in_world 收口所有碰页面道（awaitPromise 加 returnByValue，handle 形走 objectGroup）、serde_json 字面量注入选择器、click 走 Input.dispatchMouseEvent、type_text 走 focus 加 Input.insertText、wait_sel 百毫秒轮询有界）；exec.rs（一套 Context 编译一次、Vm::try_clone、send_execute 包双限额、诊断渲染进错误 CTA 带 span、收尾 releaseObjectGroup）
- daemon：POST /rune（与 /eval 同槽串行同超时同 envelope，Rune 恒碰页面故无条件懒起引擎）
- CLI：`browse rune <script.rn|@名>`（@ 走 snippets 根三序与 .rn 两试（load_template_ext 变体）、字面路径回落、--vars 同命令小程序模板道）；--vars 只配 @ 守卫与 --root 求值面守卫均放行 Rune 形
- 披露：surface rune 条目（求值族归组）、意图路由加 rune 半句（120 帽内合并三行腾位）、http-api.md 增 /rune 节、README 例一条、CHANGELOG 0.24.0、ADR-0010 加索引

## 实证

单测 vm_face 四件（ok 载荷过桥、脚本错误型 Debug 兜底、编译错 CTA 带诊断 span、千条指令预算击穿终态）；arg_contract rune 四臂（缺实参、@ 未命中列候选零 daemon、--vars 坏形、模板未填键点名零 daemon）；e2e rune_orchestration_roundtrip（真 chrome：goto 加 wait_sel 加 click 触发页面 handler 加 eval_handle 加 call（function 形 this）加 release，回执三键锁定，BROWSE_E2E 门控 SEQ 互斥）。实弹冒烟：模板参出 `{"ok":"vars-ok"}`、@名加根出 `{"ok":"plain"}`、type_text 全链出 `{"ok":"hello rune"}`、click 链 out=clicked 加 body=BODY。坑实录：本机 9880 被装机 daemon 反复占位（旧码 404 空体 EOF 假象）与 engine-profile 单实例锁，冒烟改 BROWSE_NAME 隔离实例加 BROWSE_CHROME 显式递托管 chrome 后全通；debug 二进制改 browse-rune 后须重链 browse-cli 才带新模块（常驻 daemon 也要收割重启才吃新码）。[实证: fmt、clippy -D warnings 零告警、test --workspace 全绿、doc-test、doc、aidoc --check --strict、metadata --locked、surface_contract 8（--llms 恰 120）、PEVO、e2e 6 过（真 chrome）]

## 待办

评审闸门走 browse-codex-review；放行后推 main 发 0.24.0；账本 #67 留 open（关单归 omc）。
