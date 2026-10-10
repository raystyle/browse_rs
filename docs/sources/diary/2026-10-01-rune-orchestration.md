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

## 评审一轮（browse-codex-review）

回执 F1 必修 + G1 至 G7 + 五核 CONFIRM（隔离世界并发面、双限额线程迁移不架空【评审方源码加实弹双证：Budget/Memory 逐 poll 装回 TLS，探针 I/J 分配帽与多线程指令帽真拦】、结果回传道兜底链、__wrap__ 注入边界、与既有面零互踩）。二轮探针面评审方自建四线程 runtime 实弹，未复跑 e2e 属其口径声明。

- F1 必修（采纳）：非 Result 入口的裸值 main 使回传包装失效，旧文案「结果回传道异常」误导。修：文案点名入口契约（「脚本 main 未按契约返回 Result（入口形要 pub async fn main() 返回 Ok(..)/Err(..)）」），vm_face 增 non_result_main_gets_entry_form_cta 断言
- G1 采纳：世界引导脚本原是每执行注册无注销（identifier 丢弃，跑 N 次积 N 条）。修：捕获 identifier，收尾 Cleanup 注销（removeScriptToEvaluateOnNewDocument）
- G2 采纳半：对象组改每执行随机组名 browse-rune-<序号>（并行纵深）；超时 drop future 时收尾不执行、句柄待下次 rune 或 target 销毁回收（记档：daemon 超时面的 drop-guard 属后续批）
- G3 采纳：rune 臂接 engine_up 全旗标族（--ws/--port/--chrome/--headless/--pipe/--profile/--proxy/--proxy-bypass/--isolated/--engine-arg/--secrets 不再静默失效）；--cookies 是 eval 前置热迁流程，rune 面显式 exit 2 报不支持
- G4 采纳：rune 解析臂对齐 snippets 族口径（- 前缀未知旗标 bail、多余脚本位用法错；旧形余位静默落外层 catch-all），arg_contract 增双臂
- G5 采纳：http-api.md 增八函数签名表（参数序、返回形、错误与边界：call 的 function 形 this、type_text 插入点不清空、wait_sel 超时返 false）；surface rune 条目补 --root 参数行
- G6 采纳：Ok 载荷 json 化失败改 Debug 兜底（与 Err 侧对称，不再静默 null）；type_text 增选择器未命中守卫（与 click 同款报错，防文本插错焦点元素）
- G7 采纳：@ 名未命中文案语境化（「脚本 X 未命中」，load_template_ext 增 kind 标签）；--vars 形状校验先于读文件；字面路径形 --root 给 stderr 提示（不生效但不拦）；__browse_rune_wrap__ 保留名与 iframe 边界写进 http-api
- rune e2e 全量套曾单红一次：同命令双跑 e2e 互踩 profile（我方操作面），复跑 6/6 绿

## 评审二轮与三行快核

二轮回执 F2 必修：上批修复回执有三项声称落地实未进提交（G4 解析臂与断言、type_text 守卫、surface --root 参数行）。根因是我用 python 字符串替换改码且未断言命中，三个「ok」打印骗了回执。本批改 Edit 工具（失配即红）逐项落并 grep 验证；G8（--new-tab 显式不支持）、G9（空名文案语境化）随批。三行快核 **CONFIRM**（评审方 git show --stat 加 grep 加三实弹复验）。

- G10 记档（下批候选）：尾置全局旗标（rune s.rn --headless）报「参数不认识」与 snippets 族口径一致不拦；更整齐道是内环收完脚本位即 break 回外层（外层认全家族），一改收拢 G1/G3 两面
- 评审方附记采纳进批纪律：回执里每个「已落」项附一条 grep 或实弹输出（本轮三次失实都被 git show --stat 抓出）；python 静默替换禁用，改 Edit 断言制
- 评审方探针残留披露在案（/tmp/rev67*，进程已收；我方两个 dev daemon 按名未碰）

## 发版 v0.24.0

三笔 e391c8b 加 3279a11 加 af6344a 推 main（88b92fa..af6344a），tag v0.24.0；release.ps1 三平台构建冒烟直发六件；播种 run 36848738661、main ci run 36848391900、docs run 36848391914 三绿全挂 af6344a。评审格收轮回执（资产名 + run id）已发。账本 #67 留 open（关单归 omc）。
