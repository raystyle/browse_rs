---
id: ADR-0011
status: accepted
date: 2026-10-03
deciders: 用户（方案升级与糖皮回收裁定 2026-10-03：单通用绑定替代逐件绑定）、raystyle
superseded_by: null
---

# ADR-0011：Rune 面通用 CDP 命令通道（cdp::raw）与糖皮回收

## Context

账本 #72/#73 追 rn 面缺鼠标与网络拦截原语；#73 现状补正实测（2026-10-03）：方言 js 全局面已有 mouseMove 七件与 routeBlock 三件（引擎侧 semantic 层与 js_host Fetch 接线全在），缺口精准为 rn 面绑定。原案逐件绑 `cdp::mouse` 与 `cdp::route` 系；用户裁定升级（2026-10-03）：rn 面缺的不是个别原语，是通用 CDP 命令通道。事件侧 `wait_event(method, ms)` 已开放任意 method 名，命令侧对称开放即闭环，逐件绑定会随协议 652 方法不可收敛。引擎侧 `Session::call_with_deadline` 管线现成（goto/eval/click/set_cookie/cookies 都走它），绑定是薄透传。

## Decision

- 新增 `cdp::raw(method, params_json)`：params_json 是 JSON 对象串（空串等价空对象，非对象形预检拦），透传 `Session::call_with_deadline`（30 秒上界与域策略守卫同享），响应对象整体过桥回脚本。会话作用域：活动 tab 的 session target（Browser.* 前缀走 browser 级端点）。导航类边界（评审 G-6）：Page.navigate/Page.reload 成功后与 goto 同款废隔离世界待重建；Target.createTarget 不登记 own（后续 Target.closeTarget 被守卫拦，新页开档走 goto）。
- 回收五件纯透传糖皮：`click`/`type_text`/`wait_sel`/`cookies`/`set_cookie`（raw 加 eval/js/time 组合全覆盖）。wait_sel 裁定回收：纯轮询壳（eval 探测加睡步进，`time::sleep` 在 rune-modules time 特性内），无引擎侧状态，留它即唯一被豁免的糖皮、无判据。注：旧 set_cookie 内建 `success:false` 报错、旧 type_text 内建选择器未命中守卫，回收后这两道自检归脚本侧（组合形示例与 e2e 在册，见 http-api.md 的组合形段）。
- 保留件与理由（各有不可透传语义）：goto（有界等加载与会话世界重建）、eval 族（隔离世界探针语义）、eval_handle/call/release（JS 句柄生命周期，组合件）、js/js_lib/render/stdin（编排面非 CDP 透传）、wait_event（事件对偶件）、requests/snapshot/request_detail（引擎侧有态聚合，js_host 同源注入）。
- params 选串形而非 rune 对象：对象形实测可行（嵌套组合与 null 经 `()` 都过桥，vm_face 探针在册），选串形是协议文档参数形原样粘贴（r# 裸串免转义，与 wait_event 任意 method 名对偶）加避开外部类型的序列化错面（js 的「args 不可 JSON 化」先例），不是对象形坏了。

## Consequences

- 好：rn 面获得全协议面（652 方法一道通道），策略逻辑（如 resourceType 分流断流）归脚本层自由表达；函数面收缩（脚本可见件 18 收到 14：删五加一）手册与 --llms 行减负；rn 面与方言面仍是并列消费面（raw 走同一 Session 守卫，不另造第二实现）
- 坏：存量 rn 脚本破（click/type_text/wait_sel/cookies/set_cookie 五件用法编译红；pentest_rs 战役件用量由总台对齐轮迁 raw 组合形，2026-10-03 毕）；常见操作样板变长（click 由一件变 eval 量点加两发 raw）；旧 set_cookie/type_text 的内建自检（success 报错、未命中报错）不再内建，脚本须自检；rune format! 无字面大括号转义，params 拼接要走字符串相加（e2e 实测记档）；外部桥接值的子项 Debug 取形报 Missing runtime information for type hash（战役实证 0.30.6，判值走 json::from_string 字面等值比较）
- 缓解：组合形三例（拟人鼠标、断流分流、cookie 面）进 --llms 行描述与 e2e 用例；迁移样板见 crates/browse-rune/examples/pillars.rn

## 增补（2026-10-03，方言全量桥：issue #75）

rn 面再开通用通道第二件 `cdp::dialect(fn, args_json)`：方言全局函数按名直达 js_host `global_json` 派发面（与方言面同一实现同一守卫），args 取 JSON 数组串（位置参序，与 raw 的对象串形对称）。动机：方言面的有态聚合件（waitForResponse 的界内等体、waitIdle 的窗口语义）与既有战役片段逻辑在 rn 侧原要手组等价物或 CLI 层混合编排；桥后一条通道直达。边界判据：桥的是方言层既有实现，不在 rn 面新造糖皮（薄封装件经桥可达不等于 rn 面内建，调用形自带 dialect 前缀，契约面仍是 raw/dialect 两通道）；三支柱独立件保留（高频直用 + opts 对象形更顺手）。实现：HostFns 加通用腿（载荷 `{name, args}` 由 daemon 闭包解包），进程内测试态报未注入。

## Alternatives

- 逐件绑定（#73 原案）：被否。面随协议方法数增长不可收敛，且每件都要重复接线与文档维护
- 对象形 params（`raw(method, #{…})`）：被否（弱否）。序列化面可行但协议字面量要经 rune 字面量二次转写，外部类型引入报错面；见 Decision 末条
- 五件糖皮保留并行：被否。与通用通道并存即第二真相面（同一操作两种写法），回收令明确
