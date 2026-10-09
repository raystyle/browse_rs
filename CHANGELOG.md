# Changelog

版本级里程碑；逐批过程见 docs/diary。semver 判据在册 docs/requirements/REQ-004。

## Unreleased

- **探针事件 P1（grok 评审候补落地）**：服务端 2 秒同 `(slug,action)` 下限（see 重复回 `throttled` 不入环,stop 不设下限）；环容量纪律（驱逐只逐 see 不逐 stop,单 slug 至多 20 条 see——失控探针不许独占环）；`GET /events` 加 `next`（全环按 `CTA_PRIORITY` 冻结序取最高优先的合法 cta 建议,安全类先于爬虫类,同级取最新,agent 可只跟 next）；runtime `R.see` 去重默认键升级 `slug|action|pathname` + `R.route` 路由变化重武装（SPA 换页同建议可再报,workspace 仓）。环单测锁四件;E2E 7/7。

## 0.33.1 - 2026-10-10

- **cta 安全收口（REQ-037，P0，grok X 检索安全评审，/tmp/grok-cta-review.md）**：自由串 cta 是页面可控注入面（间接提示注入向量，Chrome WebMCP 反面教材对照 Stagehand/UiPath/Slack action_id 的「受信侧定目录」先例）:请求体 `cta: String` 废弃，改 `action: 'read'|'rune'|'export'` 枚举；cta 句子由 daemon 模板生成（`FROZEN_PAGE_SLUGS` × 三动作 = 27 条闭集，`cta_for` 单源）；非法组合事件保留、cta 键不出现；stop 恒不带 cta；runtime `R.see` 参数同步收口（workspace 仓 87193dc）；环单测锁闭集；E2E 7/7 复绿。零消费者窗口的契约更正（grok 代码审计证实无一探针发过自由串 cta）。评审其余裁定在册：轮询不做 ws、P1 候补（去重键 pathname/服务端下限/环 slug 封顶/events next 冻结序）、P2 宿主侧 ack；cta 永是建议不自动执行（混淆代理残余白名单消不掉）。
- semver 判据：安全修复批取 patch（REQ-004）。

## 0.33.0 - 2026-10-10

- **探针框架起步（REQ-036，用户令「开发庞大的油猴探针框架」+「油猴探针去启发式地触发 cta」）**：事件流加 `cta` 字段（可选,截 512;探针按页况启发式给下一步动作,`browse events` 直读,不是静态表）;`page_skills_hint` 双指（读+跑:`browse workspace page <slug>(跑:browse rune page-skills/<slug>/mech.rn)`,真仓实弹在回执）;拼批包裹层 facts 改 var（prelude 自带 var 守卫撞 const 是 SyntaxError,整批复解析失败表现为零键,真仓实测修;坑入册）。
- **probe-kit v1 运行时落 workspace 仓**（c27354c）:`_prelude.js` 装 `window.__probe`（see/stop/tap/route/onFlush;fetch/XHR/history 单包约定、节流去重、%%BROWSE_PORT%% 自替换回落 9880、单探针错误隔离）+判定 facts 双职责;README 载创作约定与装载时序（document-start 探针用 waitProbe 等待形）。仓侧迭代零 browse 发版,新 slug 攒批进白名单才发版。
- semver 判据:能力新增（cta 启发式面）取 minor（REQ-004）。

## 0.32.0 - 2026-10-09

- **workspace 双脚本对重建（REQ-035，用户令「全面重写/以前 skill 全部删除重新写/rn 脚本和油猴 js 为主/爬虫与网络安全类」；grok 两轮设计案在 /tmp）**：技能本体变为 `page-skills/<slug>/(mech.rn + probe.user.js + README?)` 双脚本对（判定热路径只消费各对 `/* @detect */` 块，按多根序枚举拼**一次**主世界求值（`_prelude.js` 供 facts）；内置大探针（PAGE_PROBE_JS）与 `probe.rn` 隐式加载退役，无对零键零求值；白名单换首批九对（response-tap/scroll-until-end/next-page/spa-route/link-table/challenge-stop/antibot-vendor/fingerprint-watch/policy-surface），detect 判读 challenged 映射 challenge-stop；`browse workspace list/page/site` 改对读（README 全文或路径摘要，半对不算技能）。
- **stop api（用户令「油猴 js 检测到什么发送请求到这个 api 就停止进程输出什么」）**：`POST /userscript/stop` 打断在飞 rune，被停回执 `stopped: {op, elapsed_ms, cause: "userscript_stop", capture: {seq, type, slug, url, data}}`；不拿槽锁、不停浏览器；无在跑 rune 回 `stopped: false`。E2E 实弹：页内晚到挑战 2.4 秒打断 60 秒编排。
- **API 标准形（用户令「使用标准型/us- 不是标准形有歧义」）**：事件 create/read 同径对称（`POST /events` 与 `GET /events`）；`/us-event`/`/us-stop` 端点名废弃（发布数小时的零消费者面，直接换）。stringly-typed `name: "see.<slug>"` 约定改结构化 `{type: "see"|"stop", slug}`（旧 name 前缀形兼容解析一段）。
- **snippets 剔除淘汰（用户令，breaking）**：`browse snippets list/show` 命令族、`BROWSE_SNIPPETS_ROOT` env、状态目录 `<state>/snippets` 内置根全部移除（脚本数据目录只留 workspace 一家；`@实参` 具名解析与 rune `js_lib` 缺省根落 workspace 序首根 `scripts/`（`--root` 旗标保留覆写）；rune RunOpts 字段更名 scripts_root。
- 指南 userscripts.md：事件回调节全面标准形 + 条件停止节；手册 108/120 行。
- semver 判据：能力新增（stop api/双脚本对）+ 行为变化（判定入口重写、snippets 破除）取 minor；数小时前面的一批端点更名属零消费者窗口的更正不入 major。

## 0.31.0 - 2026-10-09

- **油猴事件回调面（账本 #77，用户令两连）**：daemon HTTP API 新增 `POST /us-event`（userscript 经 `GM_xmlhttpRequest` 上报 `{name, url, data}`，GM 面绕 CORS；name 必填，环容量 1000 满丢最旧，data 序列化超 32KB 截断成 `__truncated` 标记）与 `GET /events`（`since` 游标 / `last` 条数窗，回 `{ok, events, nextSeq}`）、`DELETE /events`（清环序号不回卷）；CLI 新命令 `browse events [--since <seq>] [--last <n>] [--clear]`（daemon 不在跑不算错，环随 daemon 生命周期）；指南 userscripts.md 第 6 节（完整可照抄 reporter 脚本 + 消费面 + 与 `Userscript.*` CDP 域的正交边界裁定：本面管事件上报消费，脚本安装管理归引擎面）。全链 E2E 实弹：reporter 脚本 page-ready/dom-click 双事件经 GM xhr 落环、CLI 双证收。
- **显式引擎路径快败（账本 #76 修法）**：`up` 显式 `--chrome` 路径在 daemon 侧不存在时入口即报错（带跨宿主指引：mirrored localhost 下 Windows CLI 可连到 WSL daemon，异宿主路径互不可见），不再走到 ENOENT 裸报或更隐蔽的慢路径。#76 定性同步在册：所谓「0.30.10 win 制品缺陷」实为跨宿主 daemon 串扰（Windows CLI 命中 WSL 侧 daemon，引擎操作在异宿主路径上排队），rebuild==dist 字节证制品无辜；env 形（BROWSE_CHROME）不动，其「指向文件不在则静默跳过续走发现序」契约保留。
- 手册帽收敛：`--llms` 常用例精选规则收紧（示例与命令显示名同文者零信息量不进，如 `browse chrome update`），events 新增两行后仍 119/120。
- semver 判据：能力新增取 minor（REQ-004）。

## 0.30.10 - 2026-10-09

- **油猴脚本帮助与文档面（REQ-033，纯文档批）**：`--help` 与裸调用帮助面加「油猴脚本」节（引擎 clean-chrome 155+ 内置 userscript 运行时、match 自动注入、管理页 chrome://userscripts、置位 `browse chrome install`、指南指路；browse userscript 命令面待引擎 Userscript.\* 协议域落地）；新指南 docs/guides/userscripts.md（引擎置位、脚本安装两道、match 语法与注入档、GM API 面 155 验收基线、验收方法论：探针 DOM 主断言+盘上 values 比对+settle-单读铁律）；getting-started 第 11 节指路；`--llms` 索引引言补能力行（surface 投影重生成）。门禁文档对齐：PEVO 骨架门禁 check.py 路径随 project-evo 0.5.x 重构迁 marketplaces `.tools/`（AGENTS 与 docs/README 双处），消 CHANGELOG 既有 em-dash 禁字一处。无行为变化。

## 0.30.9 - 2026-10-08

- **全量 code review 修复批（REQ-031，grok+kimi 双工位七轮对线至三方一致）**：
- 正确性十件：rune 隔离世界死 context 错误驱动自愈（eval 撞 Cannot find context 置空重建重试；raw/dialect 导航置空名单补 navigateToHistoryEntry 与 fetch）；录制 ack 取 params.sessionId 帧号（帧流不再 3 帧停摆）；waitIdle 改 requestId 集合对账（重定向同 id 重发不再恒在飞、WebSocket 无终态对剔除、活动 session 过滤）；方言解析器关键字词边界（`letter = 5` 不再静默成 `let ter`）；会话 cookie expires<=0 省略透传加 setCookie success 入账；方言数组负数/浮点下标落 null；`issue new` stdin 有界读（--dry-run 不再被开管道挂死）；解析器截断标识符报错；importStorageState 失败面（failed/firstError）
- 安全一族：域策略 url_host 对齐 WHATWG 可构造形全消（剥 userinfo、`\` 全斜杠位归一、Tab/LF/CR 剔除、首尾 C0/空格剔除、scheme 大小写不敏感、斜杠数不限六轮六形，deny 不可绕；域规则小写归一）
- 韧性六件：附着探测命中但连接失败回落自起（幻影 DevToolsActivePort 不再击穿 ADR-0003）；Fetch 域互抢守卫补 session.call 面与成员面 disable；路由 watcher 连接纪元重同步（连败十次挂起可恢复）；自更新锁收割道回读校验；管道 pipe2(O_CLOEXEC) 加同位空转清位（断管即关浏览器承诺成立）；eval/rune 超时与停止臂 pending 僵尸回收
- fetch 面：引擎腿页内抽取异常显式报错（CLI 与全局两腿，不再吞成空回执；信封解析提纯函数带回归锁）；goto 预算与 daemon 硬界随 --timeout 耦合修正（+12s 余量）；dialect 桥上界随请求级界（不再恒 60 秒截死长预算方言腿）；可选字段求而未得 null 占位；surface 两份摘要统一可选键
- CLI 面：尾随裸词报错、chrome install 旗标位显式报、--timeout/--idle-timeout 病态值拒（env 旋钮 0 值同拒）、收集环 CTA 统一、eval 腿结构化 timeout 行、HTTP 状态码入错误上下文、drops 同毫秒不互覆
- spawn/chrome 面：chromium-* 版本数值序、下载腿总超时、manifest 损坏留痕、engine.log 开不了留痕、强杀收尸、shutdown 锁收窄加 pid 复核、grantPermissions 缺 origin 拒、实例名相对穿越拒、daemon 端口坏值留痕、connect 端口坏值留痕、自更新解压 64MB 上界加版本尾数数值序
- rune 面：goto 提交等待（跨文档先等主框架 frameNavigated 越水位）；Cleanup 换靶回原会话清账；rune_running 恐慌守卫；vars_syntax BOM 剥离；probe 脚本可见失败首败留痕与不可读告警
- 文档对齐：architecture 三层图收 browse-rune/fetch_core 与 /rune 路由、workspace 两层定位；AGENTS 发现序补托管 pin 档与附着回落语义；README EVAL_TIMEOUT 60 秒与 BROWSE_CHROME 坏值语义；ADR-0008 成本句 rn 化；schema 派生面消非法 type（int/id/any 映射与省键）
- 记档不做六项与轮次账见 REQ-031 Notes（修复自伤三件留痕：信封类型链、G3 谎报、pipe2 空转臂；实证「每轮修复可能引入新缺陷，轮次到 CONFIRM 为止」）

## 0.30.8 - 2026-10-03

- **fetch 渲染 DOM 与 markdown 面（REQ-028，总台令两连 2026-10-03；mdka-rs 指定选型）**：
- CLI `--html`：恒走引擎腿回渲染后 DOM HTML（`documentElement.outerHTML`，JS 执行后的面），响应加 `html` 字段；`--markdown/-m` 从 v1 no-op 升 mdka 真转换（mdka 3.2，Apache-2.0，关 default 免 rayon），回 `markdown` 字段
- 方言全局 `fetch(url, opts?)`：完整两腿（HTTP 直取加三条件升级引擎腿），**rn 脚本经 `cdp::dialect("fetch", ...)` 直达零新绑定**（dialect 桥的首次实战）；域名知识点名与 goto 同口径
- 纯函数单源 `browse_core::fetch_core`（粗抽/升级判定/标题/抽取/markdown），CLI 与全局两道编排共用；CLI 零 daemon HTTP 快路径不变（腿编排双轨注记，ADR-0009 形）
- e2e：本地源 JS 注入节点的渲染证明（html 字段含 raw body 没有的节点、markdown 含标题）；rn 经桥 fetch 断言（data: URL 升级引擎腿）
- **页面特征判据 rn 脚本化（REQ-029，总台令三连 2026-10-03）**：判据移 workspace `page-skills/scripts/probe.rn`（标准 scripts 目录），browse 隐式加载（多根序首命中覆写内置缺省，缺省与历史 PROBE 同判据），经 rune VM 跑（cdp::js 主世界探页：框架全局与元素 expando 隔离世界不可见，评审 F-1 裁定；对象直桥，外层 8 秒界超时静默降级零键，在飞调用 pending 僵尸由带时间戳的 reap_pending_older_than 32 秒后回收；失败首败留痕一次；取舍：放弃 #52 时代隔离世界对页面属性覆写的挂死免疫，换回框架/expando 信号）；白名单与 confidence 校验留本仓不变；workspace 仓收窄两层定位（intent-skills 层删除，81cbf14）；判据增改走 workspace 仓免 browse 发版
- 评审闭环：合并轮 F-1（隔离世界缩窄探针信号集）加二轮 F-1（fetch 可选字段入册源与转换源耦合，升级腿 markdown 单置两雷）全修，三轮 CONFIRM 放行
- semver 判据结论（0.30.8 裁定）：两批按判据均应取 minor（fetch 渲染 DOM 与 markdown 面、判据 rn 脚本化均属能力新增，后者兼行为变化）；总台明令封 0.30.8（patch），从令（REQ-030 在册留痕，同 REQ-024/REQ-026 先例形）

## 0.30.7 - 2026-10-03

- **方言全量桥（账本 #75，总台令「提 实现全量桥 封v0.30.7」；ADR-0011 增补）**：
- `cdp::dialect(fn, args_json)`：方言全局函数按名直达方言面同一实现（js_host `global_json` 按名派发：currentTab/waitLoad/waitForResponse/clickRef/routeBlock 等 100+ 件，含三支柱同源）；args 是 JSON 数组串（位置参序，空串等价空数组，非数组形预检拦），与 raw 的 `(method, params_json)` 对象串形对称
- 与 raw 对偶：raw=CDP 层通用通道，dialect=方言层通用通道，同为单通道替代逐件绑定；守卫在 Session 层照拦（域策略同拦）；60 秒界与同源腿同律；薄封装件经桥可达不等于 rn 面内建（调用形自带 dialect 前缀）
- 动机：方言面有态聚合件（waitForResponse 的界内等体、waitIdle 的窗口语义）与既有战役片段逻辑在 rn 侧原要手组等价物或 CLI 层混合编排
- 实测：e2e 三形（零参 currentTab、数字参 waitLoad、对象参 requests 与三支柱同源对账 1==1）；评审无 F 终审放行（三低 G 随批收：载荷缺 name 补 CTA、HostFns 构造纪律注记、manual 意图路由句两通道）
- **统一兜底超时（REQ-027，用户令「统一标准统一命令统一函数」+「兜底硬指标与等待窗不冲突可命名区别」+「默认 60 秒」；ADR-0012）**：
- 统一标准：兜底硬指标（执行层，到期即结构化退出，**统一缺省 60 秒**）与等待窗（wait 族命中型参数，自命中即返）两类概念正交、命名恒区别；缺省沿革收口（rune 前 45、eval 前 300，统一 60）
- 统一命令：全局旗标 `--timeout <s>`（尾置合法，值经统一秒口径守卫：>=1000 按毫秒误写换算并告警），作用于 rune 与 --js/方言求值两道（请求级 `timeout_s` 覆写 env 缺省）；`browse fetch` 的 `--timeout` 收编同一守卫；env 旋钮语义从「daemon 级定格」改为「实例内缺省、逐次可覆写」
- 统一函数：秒口径换算器单一实现公开（`js_host::secs_to_ms`，旗标/env/wait 族共用）；在飞面探针统一（/eval 侧 JsHost `current_op` 挂 `call_global` 收口，片段道与编程道同覆盖）；/eval 超时回执新增结构化 `timeout` 对象（`op` 在飞方言函数面/`elapsed_ms`/`bound_s`）与 /rune 同形，错误文案带卡点 CTA
- 行为变化面：eval 默认界 300 收紧到 60（超 60 秒无参长跑需显式 `--timeout` 或调 env）；rune 默认 45 放宽到 60；env 旋钮值 >=1000 旧语义秒直解（如 1200=1200 秒）现按毫秒误写守卫换算（1200=1 秒并告警，每旋钮每进程一次）
- 测试：探针清空单测、统一界缺省与回执形单测、旗标名册双向守卫同步、surface/aidoc 投影随批
- semver 判据结论（0.30.7 裁定）：能力新增（dialect 桥）与行为变化（超时统一）按判据取 minor；总台明令封 0.30.7（patch），从令（REQ-026/REQ-027 在册留痕，同 REQ-022/REQ-024 先例形）

## 0.30.6 - 2026-10-03

- **余账清扫批（总台令「开工 review 态后封 v0.30.6」）**：
- #74 命名实例解析不到托管 pinned chromium：`chromium_root()` 恒指默认 state 根（`state_dir_for(&None)`），托管部署是机器级共享资产不随 `BROWSE_NAME` 走子目录；命名实例的 pinned 解析不再恒 miss，`browse chrome install/use` 托管语义对命名实例恢复；单测 `chromium_root_ignores_instance_name` 锁，实弹命名实例无 `BROWSE_CHROME` 直起引擎过（此前必报「找不到 chrome」）
- #71 三缺口考古收口（零代码缺口，三态账落 diary）：对话框阻塞处置路径在且 e2e 锁（dialogStatus 本地态直读加 dialogAccept/Dismiss 清障，alert/beforeunload 自动接受）；页签目标错位处置路径在（listPageTargets/currentTab 加 switchTab 显式切活动路由）；提交后尾等悬挂引擎面已修（全部 CDP 调用 30 秒上界，丢响应快错不永挂）；验收达成（各有一条可复现处置路径或引擎面修复）
- 卫生清：验收残留目录与命名 daemon（rawsmoke 系、issue74live）、`dist/` 旧 v0.30.4 六件本地中转件
- semver 判据结论（0.30.6 裁定）：缺陷修复与考古收口取 patch，判据一致（REQ-004；REQ-025 在册）

## 0.30.5 - 2026-10-03

- **Rune 面通用 CDP 命令通道（账本 #73，用户裁定方案升级 2026-10-03；ADR-0011）**：
- `cdp::raw(method, params_json)`：任意 CDP 命令直发到当前页面会话（params 是 JSON 对象串，空串等价空对象，非对象形预检拦；与内置函数同一道 `Session::call_with_deadline` 守卫与 30 秒上界）；与 `wait_event` 的任意 method 名对偶（事件侧已开，命令侧闭环）
- 糖皮回收（契约破裂面）：`click`/`type_text`/`wait_sel`/`cookies`/`set_cookie` 五件纯透传绑定移除，raw 加 eval/js/time 组合全覆盖（组合形：拟人鼠标、断流分流、cookie 面）；旧 set_cookie/type_text 的内建自检（success 报错、选择器未命中报错）归脚本侧；raw 的导航类边界：Page.navigate/Page.reload 成功后与 goto 同款废隔离世界，Target.createTarget 不登记 own（closeTarget 被守卫拦，新页走 goto）；保留 goto/eval 族/句柄三件/js 族/wait_event/三支柱（各有不可透传语义）；wait_sel 裁定回收（纯轮询壳，无引擎侧状态）
- 组合验证（真 chrome e2e）：a) raw 串 Input.dispatchMouseEvent（wiggle 序列、拖拽 down-move-up、滚轮双发），页内 isTrusted 全 true；b) raw Fetch.enable 加 wait_event(Fetch.requestPaused) 循环按 resourceType 分流（Media/Font 走 failRequest、其余 continueRequest），requests() 对账基线 3 降到 1、收尾 Fetch.disable 恢复。引擎侧零改，resourceType 过滤与放行动作升为 rn 侧脚本逻辑（优于 #73 原案的引擎侧落点）
- 下游迁移注记：pentest_rs 战役件用量（cookies 4、set_cookie 3、click 1、wait_sel 2）由总台迁 raw 组合形
- semver 判据结论（0.30.5 裁定）：raw 能力新增按判据取 minor、五件回收属契约破裂按判据取 major；总台明令封 0.30.5（patch），从令（REQ-024 在册留痕，同 REQ-022 先例形）

## 0.30.4 - 2026-10-02

- **余量清扫批（总台令「做一封v0.30.4」；二三层永搁从令）**：
- js_lib 值注入理论形守卫（v0.30.2 评审 G-2 记档件）：运行时 vars 绕过 CLI 的 vars_value_guard（直 POST /rune 或脚本内拼串），值含内部占位前缀会在 subst 回填段被当作占位记号；js_lib 臂渲染前递归扫 vars_json（嵌套对象/数组深处串值同拦）报错带 CTA；cdp::render 不经占位道无需同守卫
- 目录审计低项收尾：G-A6 artifact publish signature 补列 `--version`/`--git-range`/`--note`；G-A9 version-flag 显 `-V` 同义短参与「同名不同位」一句；G-A8/G-A7 已闭引证（读序句在册、名册双向守卫在册）
- REQ-015 残留清账：bh 94 站 domain-skills 迁 workspace 仓（逐字保真+源注横幅+段名改位+域族并档+MIGRATION.md 清单；browse_workspace c5140a4），goto 自动命中面实弹过（musicbrainz.org 回 domain_skills）；种子四站原生件保位
- e2e flake（spawn_port_channel）：不适用面（REQ-022 已记，未复发不预改）

## 0.30.3 - 2026-10-02

- **模板定界缺省 ERB（总台令「js rn 的 jinja 全部默认 erb」）**：四面（rune 模板道、方言/--js 片段道、js_lib 库体、cdp::render 运行时）槽形一律 `<%= 键 %>`、块 `<% %>`、注释 `<%# %>`：`<%` 邻接在合法 JS 中不存在（`a<%b` 即 `a < (%b)`，JS 无 `%` 一元），`{{`（format! 转义/JS 对象字面量相邻）整类撞面消。回退道：`--vars-syntax classic` 旗标或件首行 `// browse-vars: classic` 指令（指令优先）；minijinja custom_syntax 双引擎单例。**行为变化**：存量 `{{ 键 }}` 件需加首行指令或迁 `<%= %>`
- 传播链：CLI 旗标/件指令收口 至 render_template_with(syntax)（rune/JS 两 wrapper + 未填键扫描器双形）至 RuneRequest.vars_syntax 至 RunOpts 至 ModuleCtx 至 js_lib（件级指令覆写）与 cdp::render 跟随
- 实弹：`<%= url %>` 渲染 + `{{ safe_literal }}` 字面 + format! 双花括号安全全链过；classic 指令回退过；指令优先旗标实证
- semver 判据结论（0.30.3 裁定）：定界缺省翻转属行为变化按判据应取 minor；总台明令封 0.30.3（patch），从令（REQ-022 在册）

## 0.30.2 - 2026-10-02

- **休眠项与 REQ-020 记档全清（总台令，版本定 0.30.2）**：
- 探针多槽化（0.26.1 G1 收口）：`OpProbe` 改多重集（set 入列、clear 消自己一份），`future::join` 并发在飞时 timeout/stopped 的 `op` 全列（1 个仍字符串、多个数组、0 为 null），单槽盲区消
- G-3 建表后台预热：daemon 启动 `preheat_missing_fn_table()`（独立线程），用户首个缺失函数错误不再付约 2 秒收割时延
- G-2 双向名册守卫：源抽取比对 KNOWN_GLOBAL_FLAGS 与全局解析臂（顶层臂缩进判定），正反两向漂移即红
- #67 G6：js_wrapper 单测两件（包裹形状、体 verbatim 含反引号与 ${）
- #67 G7：管道空读提示（有实参且空白，消误 </dev/null 排障面）；Drop 兜底清账整体 10 秒上界；object-at 嗅探边界 ADR-0010 注记
- G3 存量收口：snippets list 跳符号链接（与 show 越根守卫同口径）；keygen 备份修剪（同前缀旧备份先清，只保最新一代）
- REQ-020 注入理论形收口：--vars 值含占位前缀即 exit 2（nonce 等价防护，确定性更优）
- B 通道文档化：http-api r# 节补 --vars 值注入 JS 的受支持口径与边界
- semver 判据结论（0.30.2 裁定）：探针 op 多槽属超时回执面加法（1 个时形不变），其余为修复/守卫/文档；总台定版 patch

## 0.30.1 - 2026-10-02

- **#70 补名腿（缺失实例函数报错带名）**：哈希查名字表在首次遇错时以一次性 VM 探针收割（String/i64/Vec 上逐调常用方法名收缺失错哈希，OnceLock 缓存）；终态错误文本命中即在报错里补方法名并保留哈希可溯源（战役台两枚实测锚 to_string=0x871c12da43b176d2、splitn=0x7d6f3bd0775df4c8 已进单测锁）
- **目录审计收尾（G-A6/A7/A8/A9）**：artifact publish 签名补 --version/--note/--git-range；-V 入 version 条目签名；手册补「旗标位置口径」句（全局旗标尾置合法、无关子命令静默忽略、未知旗标用法错三分面）；G-A7 名册旁证测试（KNOWN_GLOBAL_FLAGS 名册 32 项、尾置旁证 30 项逐个过 rune 收集环，漏同步解析臂即红；--vars/--root 是 rune 环合法子旗标不入名单）
- semver 判据结论（0.30.1 裁定）：错误信息增强与文档/守卫测试取 patch（REQ-004 判据行；REQ-020 在册）

## 0.30.0 - 2026-10-02

- **注释惰性化对齐全模板道（G-1 对齐批，总台令「继续」）**：方言片段与 `--js` 小程序的模板道（@ 实参加 --vars）与 js_lib 库渲染三面同规（0.29.0 只 rune 面）；JS 族词法分叉：只抽注释不认 r#（r# 是 Rune 语法），正则字面量感知（前一显著字符启发式与关键字后可起正则、`\` 转义与 `[...]` 字符类吞 `/`）防正则内 `//` 误判；字符串/模板串照旧参与渲染。CLI 增 `render_js_template`（方言/--js 面走它，rune 面仍 `render_rune_template`）；js_lib 渲染前抽占位渲染毕回填
- 测试：rawstr JS 族三件（注释惰性字符串活/正则与除法/模板插值内扫描）＋CLI `js_template_comments_are_inert`＋e2e `rune_js_lib_comments_inert`（真引擎，库内注释含 `{{ }}` 不破 Strict 渲染、字符串槽照渲染）
- semver 判据结论（0.30.0 裁定）：方言/--js/js_lib 三面注释渲染行为收窄取 minor（REQ-004 判据行；REQ-019 在册）

## 0.29.0 - 2026-10-02

- **注释区 Jinja 惰性化（r# 增益面对齐轮，总台令「优化」）**：`//` 与 `/* */` 注释整段纳入「抽取、渲染、回填」同道（逐字回填、行数不变）：注释里的 `{{ }}`、字面 `{% raw %}` 标签、`r#` 形文本不再破渲染也不被替换，并绕开未填键点名扫描；战役台坑一（注释字面 raw 标签致渲染区间错乱）整类消除；行为收窄（注释从可渲染变字面）取 minor
- **三层正交文档句（http-api r# 节）**：Jinja 层（--vars 槽）、format! 层（运行时槽单 `{}`、JS 字面大括号双写）、转义层（r# 已免）互不相关：防迁移误双写（战役台坑二的误解面）
- 测试：rawstr 单测 14 件（新增注释惰性双件：注释内槽位/raw 标签字面回填、EOF 无换行与未闭合块注释边界）＋CLI 渲染道 `rune_template_comments_are_inert`
- semver 判据结论（0.29.0 裁定）：注释渲染行为收窄取 minor（REQ-004 判据行；REQ-018 在册）

## 0.28.0 - 2026-10-02

- **#69 修复：rune 面 stdin 读上界**：非 TTY stdin 恒读原形在「开着不关」的调用壳（CI/agent）里无上界挂死（战役台实锤「双 eval 必挂」真凶）；读腿移线程加 3 秒静默界，过界按无管道参数道放行并 stderr 提示（cat 文件管道零额外时延）；集成测试双件（持开 stdin 不挂、EOF 管道零回归）
- **BROWSE_RUNE_TIMEOUT 专属旋钮落地（ADR-0010 增补终裁）**：rune 面整体上界独立旋钮默认 45 秒（战役台诉求口径），`/eval` 的 BROWSE_EVAL_TIMEOUT 300 秒契约零动；daemon 级上界（实例内全部 rune 执行同界），长爬按需调大或拆命名实例；超时 CTA 点名旋钮
- **主动停止三道 + 检测联动（总台指令，同一 CTA 面）**：`POST /rune/stop` 与 CLI 动词位 `browse rune stop`（幂等）；引擎失联联动（执行期两次连续连通性探测失败即停，cause=engine_lost）；脚本侧原生自停不自封装（select/分支加 Err 返回，error 面自带 CTA）。stopped 回执与超时同构：`op`（在飞调用面）/`elapsed_ms`（实测）/`cause`，CLI stderr 一行 JSON 非零退出，daemon 日志行
- **`cdp::wait_event(method, ms)`：事件驱动等待原语**：原生 `Session::wait_for` 直出（活动 tab 与 browser 级事件、取出即移除、含缓冲历史），超时 `Err(CdpError)` 带 CTA 提示，脚本 `match` 即条件分支；e2e 锁命中/未中双臂
- **r# 裸串（战役台裸串研究单 A 案）**：`r#"…"#`/`r##"…"##`（Rust 同形）编译前重写为转义普通串：任意引号、反斜杠、多行 JS 原样直达 `cdp::js`/`cdp::eval`；模板道先抽后渲再回填（r# 区天然免渲染，与 `{% raw %}` 各管一层）；CLI 渲染道与 daemon `run()` 入口单实现于 browse-rune；词法器跳过注释/普通串/模板串（`${}` 递归）/char，多行保真行号不漂；单测十件加 e2e 真引擎例
- **HTML 看板移除（总台令「不做 html 看板 只保留 api」）**：GET / 与 GET /dashboard/sse 路由退役，daemon 只保留 API 面（/eval、/rune、/rune/stop、/engine/up、/quit、/health）；serve 条目与手册同步；REQ-011 注记
- **#70 披露口径落地**：Rune 与 Rust std 差异入册 http-api（to_string/splitn 缺席与 `split_once` 正形、M-… 移交报错与 `clone(h)` 正形、直桥原生类型无 Value 方法面）；哈希查表补名留后续批
- semver 判据结论（0.28.0 裁定）：含公开路由移除（契约破裂）取 major（REQ-004 判据行；REQ-017 在册）

## 0.26.1 - 2026-10-02

- **rune 整体超时回执结构化（最小 CTA，战役台需求）**：`/rune` 整体超时（`BROWSE_EVAL_TIMEOUT`）的 `ok:false` 帧附机器可读 `timeout` 对象：`op`（在飞调用面，browse-rune cdp 函数入口置名/正常退出清空/future 被丢弃即保留）、`elapsed_ms`、`bound_s`；daemon 同字段写一行 JSON 日志；CLI `browse rune` 把 `timeout` 对象一行 JSON 打到 stderr 后非零退出（上层 agent 直接解析换通道重试，不依赖外部 timeout 兜底）。browse-rune 增 `OpProbe`（RunOpts/ModuleCtx 装配，daemon 持克隆）。默认值与既有契约零动（45 秒专属旋钮 `BROWSE_RUNE_TIMEOUT` 已终裁进 ADR-0010 增补，实现随批次二）；配套裁决：reqs #69 stdin 无界读（战役台实锤「双 eval 必挂」的真凶）与 #70 报错可读性同批在排
- semver 判据结论（0.26.1 裁定）：错误信息增强零契约变更取 patch（REQ-004 判据行；REQ-016 在册）

## 0.26.0 - 2026-10-01

- **#68 修复批：rune 面挂死根修（三段有界）与三支柱薄封装**：根因三段闭合：①cdp `Session` 的 `open_ws` 裸 `connect_async` 无超时（冷引擎半开口上 WS 握手可挂到 OS TCP 时限，冷挂暖过竞态的真根因），包 15 秒上界快错带 CTA；②cdp 模块全部 CDP 调用（八函数加 js/js_lib 主世界腿加 Cleanup 清账）换 `call_with_deadline`（30 秒上界，丢响应快错 `CdpError{op,message}` 不永挂）；③同源宿主腿（js_host `global_json` 闭包）整体 60 秒上界。连坐修复：`/rune` 与 `/eval` 的 `eval_lock` 获取排队 10 秒过界快错「引擎忙」（执行串行不变，不无界排队）。验收：规范四形（canon 三连冷跑、json::from_string、递归 async 携 cdp::eval）60 秒内回执 ok，真 chrome e2e 锁定；三冷循环 pillars 全过
- **三支柱薄封装（战役台需求）**：`cdp::requests(opts)`（交互请求对账，方言同源）、`cdp::snapshot(opts)`（ax 树快照同源）、`cdp::request_detail(sel,opts)`（单请求详情，响应头含 Set-Cookie）、`cdp::cookies()`/`cdp::set_cookie(name,value,domain)`（会话收割面）；js_host 增 pub `global_json` 同源入口（方言与 rune 零第二真相）；示例落 crates/browse-rune/examples/（repro68.rs 加 pillars.rn）
- semver 判据结论（0.26.0 裁定）：修复加能力新增取 minor（REQ-004 判据行）

## 0.25.0 - 2026-10-01

- **Rune 三形态运行时定位与 js/cdp/jinja 集成（ADR-0010 增补）**：小程序三形态定谳：rn 脚本小程序（js+cdp 双能力，宿主编排）、方言命令（cdp）、js 小程序（js）；rn 形态新增四函数：`cdp::js(src, args)`（内联 JS 小程序体，`--js` 同语义主世界执行、页面函数可调；args 对象经 `serde_json::to_string` 做 JSON 字面量注入，单双引号反引号天然安全，js 体原样入包裹器零拼接）、`cdp::js_lib(name, vars)`（直调 snippets 库内 `.js` 小程序，#66 全同 Jinja 渲染管线后主世界执行，名原样与补 .js 两试、越根候选跳过）、`cdp::render(tpl, vars)`（运行时 Jinja，ADR-0009 同配置 Strict 加零 autoescape）、`cdp::stdin()`（管道参数道）。输入三通道：文件/实参道、管道源码道（无实参时 `cat plan.rn | browse rune`）、管道参数道（有实参时管道串进 `cdp::stdin()`）；`--root` 双生效面（@ 名解析 CLI 侧 + js_lib 经 daemon 三序）；POST /rune 增 root 与 stdin 可选字段。两世界分工定谳：eval 族隔离世界轻探针，js 族主世界全量页面逻辑（ADR-0010 原文修订）
- **错误载荷降形（vm_face 实弹抓出的坑）**：json 模块与 Debug 协议对外部类型都出「object at 0x」占位，`?` 传播的 `CdpError` 载荷丢 op/message：宿主侧 `__describe` 专降 JSON 形（`{"op","message"}`），脚本自造型回落 VM 内 Debug（Timeout 等保真）
- semver 判据结论（0.25.0 裁定）：能力新增取 minor（REQ-004 判据行）

## 0.24.1 - 2026-10-01

- **#67 评审遗留收口（三轮 G10 + 一轮 G2）**：G10 尾置全局旗标分面文案：子命令收集环（rune/snippets 族）遇到认识的全局旗标尾置报「是全局旗标，要置于子命令与脚本位之前」（消「认识却说参数不认识」的措辞矛盾；未知旗标保持原 bail 面），名册常量与解析臂同步纪律在册；G2 超时丢弃清账 drop-guard：`Cleanup` 增 Drop 兜底（主 future 被 daemon 超时 drop 时，借当前 tokio 运行时排程 releaseObjectGroup 与引导脚本注销；显式 run() 先置 settled 防双跑），script_id 移出 tokio 锁成独立 std 锁（Drop 面不可 await/blocking_lock），并修 std 锁守卫跨 await 的 !Send 面。测试：arg_contract 增 tail_global_flag_gets_position_cta（rune 与 snippets 双面 + 未知旗标对照）
- semver 判据结论（0.24.1 裁定）：修复批取 patch（REQ-004 判据行）；未单独发版，随 0.25.0 合并发布（同日小批，minor 全含）

## 0.24.0 - 2026-10-01

- **Rune 编排面（`browse rune`，ADR-0010）**：宿主侧全控制流编排脚本（Rune 语言，入口 `pub async fn main()` 返回 `Result`），经自注册 `cdp` 模块做效果：goto/eval/eval_handle/call/release/click/type_text/wait_sel 八函数全走自有 CDP 会话（非 chromiumoxide，652 方法与域策略守卫复用），Chrome 只经隔离世界（worldName `browse-rune`，每 tab `createIsolatedWorld` 加导航前同名引导脚本）`Runtime.evaluate` 碰页面；宿主只装 json/time/cdp 三模块（不装 rune-modules full，页面请求走浏览器网络栈），脚本无 `block_on`；双限额（指令一千万加分配 16MB，`runtime::budget` 与 `alloc::limit`）；错误二分（`cdp::CdpError{op,message}` 与脚本自造 `Timeout` 属脚本可见失败走 `{"error":…}` 回执，VM 错误含限额击穿是终态不重试）；参数 `serde_json` 字面量注入（禁拼选择器进 JS 字符串）；DOM 只留不透明 `ObjHandle`（CLONE 协议可复用，`call` 用 `function(){…}` 形收 this）；对象组执行收尾整组 `releaseObjectGroup`。CLI `browse rune <script.rn|@名>`（`@` 走 snippets 根三序与 `.rn` 两试回落字面路径，`--vars` 同命令小程序模板道）；daemon 增 `POST /rune`（与 `/eval` 同槽同超时同 envelope）。方言与 `--js` 契约零动（隔离世界只属 rune 面）
- **新 crate crates/browse-rune 与新依赖 rune v0.14 加 rune-modules（json/time 特性）**：结果桥形为字符串回传道（`send_execute` 逃逸包装穿不透 serde 与 FromValue，探针定案在册 ADR-0010）
- **评审一轮收口（F1 + G1 至 G7）**：F1 非 Result 入口文案点名契约（回传包装对裸值 main 不生效，vm_face 断言锁）；G1 世界引导脚本捕获 identifier 收尾注销（防每执行重复注册无界积压）；G2 对象组每执行随机组名（并行纵深；超时 drop 面记档）；G3 rune 臂接 engine_up 全旗标族（--cookies 显式报不支持）；G4 rune 解析臂对齐 snippets 族口径（未知旗标 bail、多余位用法错）；G5 http-api 八函数签名表加 surface --root 参数行；G6 Ok 载荷 Debug 兜底对称化加 type_text 未命中守卫；G7 文案语境化（脚本/模板）、--vars 先校验后读文件、字面路径 --root 提示、保留名与 iframe 边界披露
- semver 判据结论（0.24.0 裁定）：能力新增取 minor（REQ-004 判据行）

## 0.23.0 - 2026-09-29

- **JS 模板参数化执行（命令小程序）**：@ 实参加 `--vars 键=值`（可重复累积，不收逗号分隔）同给即模板道。模板源先按 snippets 根解析（`--root` 旗标 > `BROWSE_SNIPPETS_ROOT` env > 内置缺省三序；`根/<名>` 原样与补 `.js` 两试，越根候选跳过守卫语义同 snippets show），未命中回落字面路径（#64 语义：绝对/相对皆可）；按 Jinja 语法渲染（minijinja，ADR-0009：Strict undefined、零 autoescape；`{{ 键 }}` 直插值、`| default()` 可选参、`{% for %}` 生成面），未填键网络前拦 exit 2 报全部未填键名并给 `--vars` 补齐 CTA。不给 `--vars` 即 #64 纯透传，字面大括号对天然不误伤（零静默漂移）；方言与 `--js` 两形态同用；`--root` 升为全局旗标（eval 面模板道与 snippets 子命令共用一根）。snippets 定位升格：从「可复用 JS 片段」升为「参数化 JS 命令小程序库」（代码驻库，一条命令一个可复用小程序）；workspace 存编排知识、snippets 存命令小程序，两层各司其职
- **账本身份轮换**：ledger 签名私钥轮换（新 kid `81ca88268ec8142dea86b032f884abeeef8a24ec28180df8bb5762ede48c1395` 重烧进 PUBKEY_JWK 常量；过程与在册协调见 diary 当日批记）。总台在册新 kid 前，issue 写入道不可用
- **新依赖 minijinja v2.24**（memo-map 传递一件）：本仓首个模板引擎依赖，选型与受控配置在册 ADR-0009
- **评审一轮收口（F1 + G 面）**：F1 渲染权威序反转（引擎先渲染、带名扫描只在未定义类报错后富化：`{% if false %}`/`{% raw %}` 死分支占位符不再误报未填，与 `{% for %}` 生成面披露自洽）；autoescape 显式钉死 `AutoEscape::None`（原属隐式不变量）；G1 `--root` 求值面漏 `--vars` 显式用法错（不再静默失效被误读成文件不存在）；G2 `--root` 撤豁免补 Options 行；G3/G4/G6 文档句（show 全名 vs 模板道省 .js、--vars 值恒字符串循环按字符、敏感值走模板内 secrets 道）；G5 `keygen --force` 覆盖前自动落 0600 备份件并在回执点名（本日 5f66 私钥不可逆损失的直防）；G7（写入前配对告警）记档。轮换终态：新键 81ca8826 经 omc 键面在册（active），死面旧键 5f66 吊销（revoked），0bb0 待裁原样；新键首发实弹 = 账本 #66 立项
- semver 判据结论（0.23.0 裁定）：能力新增取 minor（REQ-004 判据行）

## 0.22.1 - 2026-09-29

- **#64/#65 评审一轮修复（F1 + G 收口）**：F1 snippets 族解析臂吞未知旗标（`snippets list x --bogus` 旧形 exit 2，0.22.0 静默吃成位置位出误导性成功），`--` 前缀未知旗标（含 `--root=` 形）回 bail exit 2 基线契约复位；G1 首位置位胜出、多余位置位用法错（`snippets list a b` exit 2）；G2 显式配置根（`--root`/env）不存在时报配置错 exit 1（拼错根与真空库可分），内置缺省根不在册维持「片段库为空」引导面（#44 契约不破）；G5 裸 `@` 特例文案「@ 后缺文件路径」（二轮 G9 覆盖方言与 `--js` 双形）；G6 空串 `--root` 视同未给（与 env 空串回落同规）；G4 paths 文档句澄清（env 根不分 BROWSE_NAME，内置缺省根随实例分舱，与 workspace 缺省不同勿混读）；G7 js-flag 手册补「字面前缀，前导空白即不触发」。G3（list 面符号链接不过滤，存量）记档不修：越根守卫在 show 面（读内容道），list 只印首行摘要。测试：arg_contract 新增 snippets_arg_face_after_review_round1（F1 双臂、G1 双形、G2 显式/缺省对照、G5）
- **评审二轮收口（F2 + G8/G9/G10）**：F2 版本一致性随批（0.22.0 批的锁与 aidoc 投影在 bump 后未重生成即提交的漂移，本批起构建刷锁与投影重生成在门禁之后、提交之前完成，`cargo metadata --locked` 与 `cargo aidoc --check --strict` 进门禁循环）；G8 「谁算显式配置」谓词单源化（paths 层新增 `snippets_root_explicit`，CLI 层不再自判 env 空串）；G10 显式根存在但不是目录分面报「片段库根不是目录」exit 1（arg_contract 增 snippets_explicit_root_not_a_dir）
- semver 判据结论（0.22.1 裁定）：修复批取 patch（REQ-004 判据行）

## 0.22.0 - 2026-09-29

- **#64 eval 面文件实参形态（`--js @<路径>` 直读执行）**：片段实参以 `@` 开头即读文件内容为片段体（方言与 `--js` 两形态通用；绝对/相对路径皆可），读失败报错带 CTA 直出 exit 2（网络前拦截，零 daemon）；先于 `--b64` 解码展开，`--b64 @<存 base64 的文件>` 组合形自然成立；stdin 管道与 REPL 形态不经此道，共存不互踩。展开收口在 `browse_cli::client::expand_at_args`（与 `decode_arg_b64` 同族单点）
- **#65 snippets 库可配置外部根**：片段根三序 `--root` 旗标 > `BROWSE_SNIPPETS_ROOT` env > 内置缺省 `<state_dir>/snippets`（缺省态不变；env 根刻意不分 BROWSE_NAME 命名空间，与 workspace 同口径：片段是跨实例共享资产，配 prs_c2coe 仓内使用）；`browse snippets list/show` 读配置根，`<site>/<task>.js` 分层与首行 `//` 摘要约定不变，越出配置根的相对/绝对路径仍拦截（canonicalize 守卫随根走）。语义定位：workspace 存编排知识（域/页/意图技能），snippets 存编排功能脚本（可复用 JS 片段），两层各司其职
- semver 判据结论（0.22.0 裁定）：能力新增取 minor（REQ-004 判据行）

## 0.21.0 - 2026-09-23

- **#63 workspace 多仓加载（自定义技能仓）**：`browse workspace add <路径>`/`remove` 登记自定义技能仓，配置固化到应用状态目录 `~/.browse-rs/workspaces.json`（跨会话持久，不分 BROWSE_NAME 与仓本体跨实例口径一致）；多根序：`BROWSE_WORKSPACE` env 钉死单根最高优先（既有语义不变）> 配置清单序首优先 > 缺省仓垫底（同段被自定义仓覆盖时以自定义仓为准，未覆盖段默认仓仍可用）；技能格式同默认仓（domain-skills/<段>/ 与 page-skills/<slug>.md）；goto/fetch 点名与 site/page/list 读全吃多根（段命中序首仓整胜不混拼、清单跨根并集首见去重、全文读序首命中）；list 回执新增 roots 面
- semver 判据结论（0.21.0 裁定）：能力新增取 minor（REQ-004 判据行）

## 0.20.0 - 2026-09-23

- **#62 显式连接意图强制切换（--connect 暖态假绿修复）**：`Engine::ensure` 对 Attach/Port 显式意图不再被已连接态幂等短路，目标不同即切换：旧引擎是 browse spawn 的走优雅关（down 所有权，含隔离 profile 清理），是附着来源的零触碰只断本侧连接；同目标幂等免抖（重复 up 不次次断重连）；Auto 缺省意图维持原幂等（懒起与发现序不动）。修复 --connect 在暖 daemon 上静默跑既有引擎的假绿（本日两踩同根：mac 串台误判、lan-ubuntu 首连假绿）
- semver 判据结论（0.20.0 裁定）：行为变化取 minor（REQ-004 判据行）

## 0.19.1 - 2026-09-23

- **#61 引擎懒重 spawn 丢 up --headless 意图**：/engine/up 成功且是 Auto（spawn）意图时回写 daemon 缺省 spec（Mutex 形，eval 读侧克隆快照），引擎意外退出后的自动重拉沿用最近一次显式 up 的形态（headless 等），不再回退 daemon 启动时的 from_env 有头缺省；Attach 是连接意图不是重生意图不回写。实弹验收：up --headless 后 kill -9 引擎，重拉后 headless True 对 True，#60 告警面仅「引擎换新」一句零翻转（修复前同场景双句含翻转）。surface up 述补形态记忆口径
- semver 判据结论（0.19.1 裁定）：修复批取 patch（REQ-004 判据行）

## 0.19.0 - 2026-09-23

- **#60 操作回执的引擎上下文 CTA**：/eval 信封加 engineContext 键（provenance 与 daemon 自描述同源 #59，加 changes 数组；方言 value 不动，向后兼容）；变化检测在 daemon 侧（上次引擎指纹存 Mutex，逐操作比对，CLI 无状态）；指纹面是 origin/pid/有头无头/通道/profile 形态/附着 host；CLI 渲染纪律对齐技能触发层先例：正常态零输出，异常（引擎换新、有头无头翻转、形态翻转、来源变化）出 stderr 告警行，跨宿主与同 OS 异机告警每进程一次防批处理刷屏；BROWSE_ENGINE_CONTEXT 开紧凑行（引擎：origin @ hostContext）。fetch 引擎腿同款渲染。实弹：daemon 内存活期杀引擎逼重 spawn，次发操作精确告警「引擎换新 pid A -> B」加「有头无头翻转」，顺带照出懒重 spawn 丢 up --headless 意图的隐性翻转（告警面设计目的的当场实证，另立 issue）
- semver 判据结论（0.19.0 裁定）：能力新增取 minor（REQ-004 判据行）

## 0.18.0 - 2026-09-23

- **#59 引擎来源与所有权信息面**：status 与 /health 内嵌 daemon 自描述（`daemon` 键：os/hostname/pid/name/startedAt，hostname 走命令采值零新依赖）与引擎来源语义面（`engineProvenance` 键：origin 枚举 attached/managed-spawn/isolated-spawn、hostContext、spawnedBy、spawnedAt）；Spawned 快照加 `spawned_at` 时间锚（序列化加法向后兼容）；人读面补「daemon 宿主」行与引擎行的 `@ origin hostContext since` 段，CLI 与 daemon 跨宿主（Windows CLI 经 localhost 转发命中 WSL daemon）时显式告警标注；attached 态 loopback 宿主显式降级标注（本机或隧道不可辨，不冒充确定）。#60 操作回执告警面同源消费 provenance 结构（一份事实两处展示）
- semver 判据结论（0.18.0 裁定）：信息面能力新增取 minor（REQ-004 判据行）

## 0.17.0 - 2026-09-23

- **#58 批 G 尾随收加 chrome 下载进度面（用户令 2026-09-23）**：G1 stall 看门狗落地：下载腿独立线程监控零字节停顿，每满 10 秒告警一行 `仍在等数据（已收 N 字节，X 秒无进展）`，纯 stall（连接活但零字节）与慢速推进可辨，读结束后看门狗最多再活一个节拍即退；下载核心抽 `stream_with_progress` 通用缝（update 收 Vec、`chrome install` 写文件加哈希共用同一心跳/看门狗/断流上下文口径），chrome 下载（150MB 级）同享进度面（大件字节闸放宽 4MB 约 40 行不刷屏）；心跳与告警文案抽 `heartbeat_line`/`stall_line` 纯函数带调用方语境 label，单测锁形；G3 预分配帽 256MB 收紧 64MB。测试：stall 看门狗 mock（64KB 后长睡）告警字节锚与续传全量锁、文案锁补全
- semver 判据结论（0.17.0 裁定）：chrome 下载获能力（进度面）取 minor（REQ-004 判据行）

## 0.16.0 - 2026-09-23

- **#58 update 下载腿进度面**：资产体改流式读（64KB 块）加心跳回显：收满 512KB 或满 1 秒打一行 `下载中 N/总量（%）`（总量缺失降级字节式），快源被字节闸限到每 512KB 一行、慢源被时长闸托底到每秒一行，慢源下「在下」与「死」可辨；取毕补一行收尾。读失败（超时、断流）错误带已收/总量/耗时上下文，超时行为可预期。零新依赖（reqwest blocking 的 std Read 流式）；判新腿与镜像选序不动；边车与判新标记是单行小件不走此道。测试：限速源 mock（服务端节流等价限速代理）多拍心跳与字节单调锁、断流错误上下文锁
- semver 判据结论（0.16.0 裁定）：能力新增取 minor（REQ-004 判据行）；随批发 0.15.2 的 h1 钉死与播种缓存头（未单独发版）

## 0.15.2 - 2026-09-23

- **下载腿钉死 HTTP/1.1（镜像域字节悬崖，总台令 2026-09-23）**：`browse update` 判新与下载腿统一走 `update_http_client`（`.http1_only()`），因 browse.ohmygh.com 前置链（openwrt 透明代理）对 HTTP/2 有约 1,720,320B 字节悬崖，h2 精确断流、强制 h1 同路径全通。注记：本仓 reqwest 现为 http1-only 编译面（workspace 关 `default-features` 且未开 `http2` feature，`h2` 不在依赖图），故本批是防漂移口径（锁住镜像腿不随依赖统一化漂回 h2）而非即时修速；对照测例 `mirror_download_leg_forces_http1_above_cliff` 以 mock 镜像取 2.5MB 资产断言镜像域请求首行恒 `HTTP/1.1`
- **播种腿 stable 段缓存头（总台令 2026-09-23）**：`release.yml` 播种 stable 滚动段 rclone 上传头由 `max-age=60` 改 `Cache-Control: public, max-age=3600, stale-while-revalidate=86400`（边缘免同步回源）；版本段保持 immutable 长缓存、latest 判新标记保持 `max-age=60`（判新新鲜度）
- semver 判据结论（0.15.2 裁定）：加固与流水线披露批取 patch（REQ-004 判据行）

## 0.15.1 - 2026-09-22

- **#57 无显示会话 headless 自动回退**：spawn 引擎（端口态与管道态两实现三落点）在有头意图且环境无 `DISPLAY`/`WAYLAND_DISPLAY`（典型 ssh 会话）且未显式给 headless 旗标时自动补 `--headless`（修复该场景 ozone 初始化失败即退、DevToolsActivePort 永不落盘的必挂）；有显示环境行为不变；`BROWSE_ENGINE_ARGS` 显式旗标优先不叠补；Windows 会话制与 macOS（Cocoa 非 X11，环境恒无这两变量但有桌面）恒不触发（ADR-0003 口径不动，macOS 有头默认不回退，评审 F1）；README/getting-started/surface 披露生效口径
- semver 判据结论（0.15.1 裁定）：修复批取 patch（REQ-004 判据行）

## 0.15.0 - 2026-09-22

- **#56 技能触发事件面扩展**：fetch 两腿（HTTP 直出与引擎升级）回执接入域名层点名（与 goto 同口径 `url_domain_fields`，CLI 侧补点不依赖引擎，命中附 `domain_skills` 与 hint）；detect() 判读映射 page-skill 点名（challenged 映 bot-shield/captcha、login-wall 映 login-wall，恒 PLAUSIBLE：判读是行为推断非 DOM 实证）；未命中或关闭回执零新增键（逐字节一致）；goto 域名段切统一口径，`page_skills` 键对构建两事件源共用
- semver 判据结论（0.15.0 裁定）：能力新增取 minor（REQ-004 判据行）

## 0.14.0 - 2026-09-22

- **artifact publish 结构化字段面**：ledger-client 升 v0.1.3，`browse artifact publish` 增 `--summary`/`--outcome`/`--git-sha` 复旗标（summary 非空限长与 git_sha 十六进制形本地预检，免配额损耗）；总台令三件在册产物补载荷
- semver 判据结论（0.14.0 裁定）：能力新增取 minor（REQ-004 判据行）

## 0.13.1 - 2026-09-22

- **#55 帮助面专业化重排**：Commands 按语义五组分类（求值与抓取、引擎管理、片段与知识仓、账本与产物、自更新），组内名对齐、描述改首子句短述（从 description 机械派生，单一真源零手维护）；Options 短述化并自动回填默认值；帮助面、llms 手册与 schema 三面清零内部台账编号（#NN/REQ/ADR/评审/裁定日期，溯源归 CHANGELOG 与 diary）；片段方言节紧凑化；环境变量块对齐修正。守卫三件随卷：命令树全归组锁（HELP_GROUPS 恰一覆盖）、帮助面零内部编号锁、既有全覆盖锁随分组结构更新
- semver 判据结论（0.13.1 裁定）：帮助面文档批取 patch（REQ-004 判据行）

## 0.13.0 - 2026-09-21

- **#54 判新镜像面（update 全程默认零 GitHub 依赖）**：播种流水新增 `stable/latest` 判新标记步（单行纯版本号，rclone rcat 恒在 sync 后重写防 `--delete-excluded` 洗掉，回读红灯锚定）；`latest_browse_version` 改镜像 `latest` 优先（404、超时、垃圾文本静默回落 GitHub Releases API），下载腿本就镜像优先，`browse update` 全程默认走自家镜像（browse.ohmygh.com），GitHub 仅剩镜像故障兜底（此时 GH_TOKEN 才有用）。触发面：判新腿原恒走 GitHub API，匿名 60/h 机队共用易撞（403 即整个 update 退场）
- semver 判据结论（0.13.0 裁定）：行为变化取 minor（REQ-004 判据行）

## 0.12.2 - 2026-09-21

- **#53 子命令旗标收集循环假报用法错（0.12.1 G-F 收口回归）**：`next` 取值口改直出 exit 2 后，七处按「参数尽返 Err 收尾」旧契约写的旗标循环（fetch / artifact publish·attest·list / ledger keygen / issue new / issue list）的 Err 分支成死代码，任意调用在旗标耗尽时假报「旗标 需要一个值」exit 2；`snippets list` 可选位同碎，`workspace site`/`page` 的 Mode 层缺参处理器被解析层先拦成死代码（`snippets show` 从无该守卫，评审 F1 本批补齐同款）。修法：CLI 实参游标双口分面，`Args::next` 必值口缺值直出 exit 2（G-F 口径不变），`Args::next_opt` 收集口参数尽返 None 即收尾；bail_arg 措辞中性化（非 issue 子命令的坏旗标不再误报「issue 参数不认识」）。browse-cli 首建 argv 契约测试 `tests/arg_contract.rs`（`CARGO_BIN_EXE_browse` 真二进制四测，零网络；argv 面此前零锁是回归漏网主因）；win-gnu 交叉岗 check 补 `--all-targets`（集成测试此前在 windows 面零编译闸，评审 G1）
- semver 判据结论（0.12.2 裁定）：修复批取 patch（REQ-004 判据行）

## 0.12.1 - 2026-09-21

- **#52 timeout 包 session.call 丢 pending 登记**：cdp 新增 `call_with_deadline(method, params, deadline)` 窄接口（deadline 收进内层，超时清登记路径不再被外层 drop 丢掉）；技能探测腿（8 秒）与 Input 派发短超时（8 秒自愈档）两处同型点切窄接口。e2e 验收锁：cookie getter 死循环挂死页上 goto 在 8 秒档返回零 page 键、pending 表归零（`pending_len` 诊断面）、browser 级调用健在、newTab 切走楔死渲染器复活（页内 crash/navigate 都进不去楔死页属 CDP 结构性，busy-loop 测试解法在册）。server.rs 求值 300 秒外层超时 drop 复合求值的边角同型留档不扩批
- **用法错 exit 2 直出收口（上批评审 G-F）**：next 闭包缺值、--port 非数字、workspace site/page 缺参四处从 anyhow exit 1 改 eprintln + exit(2)（bail_arg 族同口径，文案不再撒谎）
- **fetch 引擎腿 CTA 换日志指路（#49 评审随记下批项）**：daemon 不可达时指 daemon.log 与 browse status，不再盲指 browse up
- semver 判据结论（0.12.1 裁定）：修复批取 patch（REQ-004 判据行）

## 0.12.0 - 2026-09-21

- **domain skill 触发层（#50，REQ-015）**：goto 回执按 URL 域名段（hostname 去 www 首段）点名 workspace 站点知识，命中附 `domain_skills`（封顶 10）与 `domain_skills_hint`（读全文命令）；未命中回执零新增键（BTreeMap 键序下逐字节一致，e2e 键集锁）；`BROWSE_DOMAIN_SKILLS=0` 关闭（opt-out 镜像前代 bh）；daemon 三变量显式透传属防御性（现状与继承等效，改配置重启 daemon 口径同 BROWSE_SECRETS）
- **page skill 启发式触发层（#51，REQ-015）**：goto 后一次有界 pageEval（TreeWalker 前 800 节点封顶、iframe 判定源封顶 50、8 秒超时静默降级）检出 10 特征 slug 两档置信（spa/hydration/shadow-dom/iframe/iframe-cross-origin/lazy-scroll/bot-shield/login-wall/captcha/service-worker）附 `page_skills`/`page_skills_hint`；框架名与版本单列回执 `framework:{name,version}` 不占 slug；`BROWSE_PAGE_SKILLS=0` 独立关闭
- **workspace 单仓命令族**：`browse workspace` 六件（status/install/update/list/site/page），git shell-out（clone / pull --ff-only 脏树拒绝、本地修改可回推）；种子仓 github.com/raystyle/browse_workspace（三平台用户目录部署 `~/.browse-rs/workspace`，BROWSE_WORKSPACE 覆盖，跨实例共享不分 BROWSE_NAME）；仓骨架已推（page-skills 10 篇配方 + domain-skills 样例 + intent-skills 索引）；仓内 interaction 机制知识迁 workspace 统一维护（docs/skills/ 留指路碑）
- semver 判据结论（0.12.0 裁定）：能力新增取 minor（REQ-004 判据行）；#50/#51 关单走 omc 委托道

## 0.11.0 - 2026-09-21

- **fetch 引擎腿懒拉起（#49）**：三条件升级的引擎腿 POST /eval 前补 ensure_daemon，冷启动 `browse fetch` 与方言缺省形态同构（daemon 与引擎自动拉起，无需先手工 `browse up`）；win 0.10.0 首报、wsl 复现，修复后冷启动直达 via=engine。评审一轮 CONFIRM（隔离实例冷启动独立复验）
- semver 判据结论（0.11.0 裁定）：行为变化取 minor（REQ-004 判据行）

## 0.10.0 - 2026-09-21

- **引擎附加旗标直通道（#48，REQ-014）**：`--engine-arg <a>`（可叠加）与 `BROWSE_ENGINE_ARGS`（空格分隔）双通道直通 spawn argv，托管 pin 形态下 clean-chrome 扩展域（Browse.* 四函数）零包装脚本启用；求值前置形态直达。`/engine/up` 并集 env 先行、显式随后（chrome 重复旗标后值胜）加同值去重；撞车旗标慎叠注记与六处指路文案随迁。评审两轮（F1/G1/G2/G3 全修后 CONFIRM）
- **r4 引擎验收窗（clean-chrome 联动，152.0.7977.84-r4）**：五端装齐验收全绿（导入、驱动、真实环境面）；waitForResponse 事件面缺陷（clean-chrome c0bbcfd 三段根因修复：桥存活、wait 注册表进程级、接缝冲排）五端实弹关闭；真跨站换渲染器 wait 不迁移边界入档
- semver 判据结论（0.10.0 裁定）：能力新增取 minor（REQ-004 判据行）；#48 关单走 omc 委托道

## 0.9.0 - 2026-09-20

- **账本收口（总台修正令 2026-09-20）**：自研 ledger 客户端网络签名道移除，全权委托标准 crate ledger-client v0.1.1（github.com/raystyle/ledger-rs，全舰队唯一实现；v0.1.0 有 URL 拼接舰队级缺陷已避，总台追注）
- **CLI 只增不关不删（删面清单）**：`issue close` 与 `artifact promote` 子命令面移除（exit 2 指路）；attest 收三型 attest_dev/attest_prod/verification_failed；publish 参数面随标准 crate 收窄（--summary/--outcome/--git-sha 撤，--note 进）。关闭与删除唯一道 = omc 工位经 herdr 委托
- **semver 判据结论（0.9.0 裁定）**：删面按 REQ-004「契约破裂取 major」字面可解读为 1.0.0，但本仓 0.x 序列以 minor 位为 breaking 位（semver 0.x 惯例，历史 0.5 至 0.8 全 minor 滚动）；major 位解读为 1.0 稳定承诺后生效（用户裁定 2026-09-20）
- 薄适配层只留：身份面（PUBKEY_JWK 与 kid 派生，keygen 回执带 builtinKid 自证对账）、密档管理（base64url seed，在册密钥与 kid 不受收口影响）、本地校验（值域/形错统一 exit 2）、issue new --dry-run、#52 家族截断提示
- 工程：reqwest blocking 在 async main 的嵌套 runtime drop 雷以 ledger_call（spawn_blocking）适配（实弹红转绿）；crate 静默默认守卫（issue_new 回 0 / artifact_publish 回空串即拒假成功）；dry-run 签名基七行对账断言；REQ-013 立项回填 + diary 2026-09-20 补账
- 实弹：v0.1.1 真账本 GET issue list（47 条）与 artifact list 全通；移除面三拒 exit 2；实发腿同规预检（title/kind/name 本地拦零网络，收口批评审 F1）；ledger 测试 8 绿
- 旗总台上游缺口：ledger-client BASE_URL 不可覆写（灰度与本地捕获测试面需要，建议 v0.1.2 开 base 注入）；attest payload 形漂移（本仓 v0.8.0 旧事件 payload.note 与 crate 新形 payload.checks+body 并存，消费方按 payload.note 取证据会漏读新事件，建议统一或迁移注记）

## 0.8.0 - 2026-09-20

- **账本迁移（REQ-063，总台令 2026-09-20）**：issue/artifact 客户端面整体切真源 ledger.ohmygh.com（契约全文 ohmycloud REQ-063，实装对读范本 hst_rs v2.6.0）；写入 Ed25519 五头签名道（签名基 v1 六行换行连，公钥 JWK 常量内置 kid=sha256hex(JWK) 按舰队派生约定），GET 免签；私钥 env `BROWSE_LEDGER_PRIVATE_KEY`/密档 `~/.browse-rs/ledger/ed25519.key` 双通道，零入仓零 argv零打印
- issue 流四命令：new（--acceptance 必填 + --dry-run 载荷与签名基预览零网络，#57 G6 承继）/list（limit 100 钳制 + before 游标 + has_more 权威 + 截断提示）/show/close（result 引 digest 先行 + status=done 收尾，确定性幂等键首段成次段败重跑不重复追加）
- artifact 流四命令：publish（kind 十五枚举必填 + --dep 依赖链可重复 + --outcome + --git-sha）/attest（六型 + artifact_id 36 字 UUID 形校验截断本地报）/promote（糖）/list（current/env 过滤）
- 旧 REQ-057 issues.ohmygh.com 客户端通道摘除（零调用实证；服务只读保役）；AGENTS.md 与 docs/requirements/README.md 台账口径随迁
- 工程面：TEST_ENV_LOCK 进程级锁 + 测试 env 自封闭（评审 F1，强制 env 16 线程 800 轮压测 0 失败）；评审五轮 F1-F6 全修 G1-G9 采纳 CONFIRM 在卷；实弹回执 issue new 201 + artifact publish/attest_dev/attest_prod 双绿（artifact 66857bb2-d1ae-4977-97df-8ab41cc37399，seq 20/21/24）；此前批次（#61 收官批 30、意图路由表、REQ-012 引擎面四批）随本版卷入

## 0.7.0 - 2026-09-18

- `browse update` 自更新面（REQ-005，家族统一标准件，对齐 build-release 公共契约第六节双通道）：GitHub latest 判新（semver 只升不降，本地领先报 localNewer 不动）；下载镜像 stable 滚动段优先、GitHub 回落，资产与边车恒同源，`.sha256` 锚校验硬拒不回落；自替换同目录暂存（防跨文件系统 rename）加 pid 备份加更新锁（陈旧收割）加 `--version` 自证五次重试加回滚复核；管理方布局（ark 落痕或用户面链接入口）让位走 ark；`BROWSE_RELEASE_MIRROR` 覆写加 `GH_TOKEN` 提限流
- 评审三轮闭环入卷（F 全修含跨文件系统 rename 实弹雷、锁陈旧收割、回滚复核共用体；G 全收口含仓内双通道三态 mock 锁）；新依赖 tar/flate2（发布包 tar.gz 腿）

## 0.6.1 - 2026-09-18

- 评审闸门三轮修复收口（3F 加 F1′ 加 16G）：chrome 子命令 CTA 随迁 update/remove；remove 残留态（登记在册目录已失）幂等清登记不再死结，回执带 `alreadyGone` 且释放量如实归零；`use_version` 加版本校验与部署在位闸，拦「登记在册目录已失」的假绿切空位 pin；release.ps1 自引用正名
- 守卫硬化：派发守卫改调用形真派发（原裸名断言空转假绿）；帮助面行首精确匹配加 Commands/Options 行数双向对称断言加旗标双向反查守卫；r2 兼容与残留态四条新回归锁
- README：部署节升四通道（新增 GitHub Releases 直下）、r2 补丁族版本说明（实测口径归因）、命令清单补全 update/remove
- 文档：五端舰队滚动与评审闭环入册（diary）

## 0.6.0 - 2026-09-18

- `browse chrome update`：发现镜像最新版（桶根 `latest` 纯文本指针，`BROWSE_CHROME_LATEST` 可钉）加未装则镜像安装（sha256 锚校验链）加 pin 切最新；幂等，回执带 previousPin
- `browse chrome remove <版本>`：删已装版本（目录与登记一起清，回执带释放文件数与字节数；pin 指向的拒删保「pin 永远指向在位版本」不变量）
- 修 #15：方言 `.length` 成员访问不再静默丢（数组元素数；字符串按 UTF-16 单元同 JS），回归锁三态
- r2 补丁形资产全链（152.0.7977.84-r2，clean-chrome 补丁版；上游主张 51 锚源码级恒 false，本仓实测口径为三端 spawn 与 flat 附着态全 false，pipe 态未验）：版本号收连字符后缀（兼容锁在册）；三端真桶实测 update 拉新、Google 登录面进 v3/signin 表单页。stealth 注入按用户裁定撤批不自带，r2 即终态。追正 2026-09-19：pipe 态后补进本仓 e2e 断言（tests/e2e.rs 通道参数化），spawn 的 port 与 pipe 两通道口径闭合（both 态未接，属 clean-chrome 侧矩阵）

## 0.5.1 - 2026-09-18

- 帮助面重排进 cli-docs 标准节序：头行 `browse@版本` 注入、Usage synopsis、Commands/Options 分家列对齐（CJK 展宽）、环境变量逐条 default 后缀；`print_help` 改 `surface::render_help` 活树派生（curated 加守卫形），`help_covers_catalog` 守卫锁命令树全覆盖与版本注入
- `--llms` 手册补「读序」与「退出码」节（57 升 69 行，帽 120 内）；目录描述清内册引用两处（`--llms` 与 `--serve` 条目）
- 帮助与命令面清内部细节：`--help` 删镜像域孤行，issue new 目录描述去 issues 域与 REQ 内册后缀（`rg ohmygh` 三面零命中）
- README 甲面补徽章三枚（CI/Release/License）与特性短句列表

## 0.5.0 - 2026-09-18

- issue 命令集成（REQ-057 对齐单）：`browse issue new/list/show` 一键缺陷反馈入统一入口 issues.ohmygh.com，自动署名 tool/version/platform/host，`BROWSE_ISSUES_API` 覆写；agent 一键反馈纪律入 AGENTS 合同
- `--llms` 三面统一升册（REQ-060）：裸形升 markdown 紧凑 agent 手册（活树派生，行数帽 120 契约锁）、`--json` 机器形 Schema、`--full` 完整目录；README 四节重排（166 压 89 行）加镜像直下 URL 入册
- 裸调用面对齐：裸跑（TTY 与空管道）出本仓帮助体 exit 0，REPL 收显式 `--repl`，stdin 管道批处理不回归
- 发布流水自播上线（自本版起）：CI 退编译改 seed-only（release published 触发双段播种加零上传红灯），本地发布面 `tools/release.ps1`（预检五件加测试闸加三目标打包边车加三端解包冒烟加 gh 直发）为唯一正式发布口；评审回执终审修（三目标显式内层名、锚链预检五件、prerelease 过滤）随卷
- 帮助面清理：删外部冗余注记（browser-harness-js 对齐、clean-chrome 专属），节标题独立一行、正文统一两格缩进

## 0.4.1 - 2026-09-17

- R2 先验窗三端 happy-path 实证：wsl（linux-gnu 包 567 文件）、lan-win（msvc 包 499 文件）、lan-mac（arm64 包 331 文件束形）镜像装通即 spawn 驱动，doctor 健康，干净退场；REQ-003 trace 回填
- 边车解析对齐 `sha256sum -c` 兼容格式（hex 双空格文件名，取首 token 为锚；mock 测试同步锁真格式）
- 资产名升定标三元组形 `chromium-<版本>-<三元组>.zip`（总台裁二；win 对 msvc 资产名实相符）

## 0.4.0 - 2026-09-17

- macOS 平台适配：`.app` 束形导入（直指 `Chromium.app` 导入保留束形落版本目录）与布局感知二进制解析（`chrome_binary_in_dir`：认束内 `Contents/MacOS/Chromium`、版本目录包装形与裸 unix 形）；托管 pin 解析与祖先发现共用同口径
- mac 真机全绿：导入 331 文件/711MB、无头 spawn 加 evaluate、有头 spawn（ssh 直启 GUI 会话）；五端测试矩阵全活（wsl/lan-linux 无头，lan-ubuntu/lan-win/lan-mac 有头加附着）

## 0.3.0 - 2026-09-17

- 自定义引擎 profile：`browse up --profile <dir>` 与 `BROWSE_PROFILE`（spawn 引擎的 user-data-dir；显式旗标顶掉环境缺省）。默认仍是固定 `<state>/engine-profile`，站点状态与会话跨跑持久（down 不删）
- EngineSpec::Auto 增 profile 字段（HTTP `/engine/up` 面同构透传）；WS 与管道两通道 spawn 同支持

## 0.2.1 - 2026-09-17

- 修 attach 断线不愈：Session 存活旗原先只由读循环写进局部对象，连接断开后 `is_connected` 永远谎报活着，引擎懒 ensure 短路不重连，一切求值 `CDP socket closed` 直到重启 daemon；改共享旗后断线即翻 false，下一次求值自动重连或重起引擎（WS 与管道两通道同修，附回归测试与杀引擎自愈实证）
- 观察入账：懒 ensure 重起引擎用缺省形态（headless 随缺省），显式 headless 需再 `browse up --headless`

## 0.2.0 - 2026-09-17

- Chromium 版本管理器 R2 下载腿：`browse chrome install <版本>`（部署目录缺省）与 `chromeInstall({version})` 从 chrome.ohmygh.com 版本段下载，`.sha256` 边车锚校验后 zip 解包原子落位，manifest 登记自动 pin；`BROWSE_CHROME_MIRROR` / `BROWSE_CHROME_ASSET` 覆写（资产名暂定约定，候首版资产定标）
- `browse --version`（资产解包冒烟用；原计划 0.1.1 patch 并入本批）

## 0.1.0 - 2026-09-16

- 首版封口：三层 crate（cdp 协议层、browse-core 引擎与方言宿主、browse-cli）加常驻 daemon（HTTP 默认 9880 与命名实例派生端口）
- 方言片段三形态求值（参数、stdin、TTY REPL），附着优先引擎策略（探测 9222 缺则 spawn clean-chrome 隔离实例，down 只杀自起）
- 命令面单一真相源目录与三投影（schema、llms、llms-full），agent 发现通道 `browse --llms` 三形态直出（--full/--json）
- 内嵌 Chromium 版本管理器本地导入面（chrome install/use/list/doctor，ADR-0007；R2 下载腿候 omc 端点热验）
- 域策略防线（BROWSE_DENY_DOMAINS/ALLOW_DOMAINS）、CDP 管道通道（--pipe 零 TCP 面）、652 方法清单派生
- CI 三岗（linux、win-gnu 交叉、mac）加文档岗；Windows 编译面定标 win-gnu（非 msvc）
