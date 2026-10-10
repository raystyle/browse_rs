---
id: REQ-031
title: 全量 code review 修复批（grok 行为语义腿 + kimi 静态正确性腿，双工位交叉）
status: implemented
priority: must
trace: 双工位评审（herdr browse-grok-review / browse-kimi-review，产物 /tmp/browse-rs-review/{grok,kimi}.md）出 F 14 / P 6 / G 47 / CONFIRM 大面；总台逐项独立复核零误报后修；门禁全绿（fmt、clippy -D warnings、test --workspace 302 过、test --doc 51 过、aidoc --check --strict clean、surface 契约过）；新增回归锁：url_host_strips_userinfo、keyword_prefix_not_eaten、truncated_member_props_error、attach_optional_fields null 占位臂、version_suffix_numeric_order
---

# REQ-031：全量评审修复批

## Scenario

用户令（2026-10-07）：「让 grok kimi 全面 review」后接「review修复所有问题 直到三方一致」。grok（行为语义与契约漂移腿）出 F4/G6/P1/CONFIRM 8 面，kimi（静态正确性腿，增量 1828 行精读 + 三 crate 六路子代理深读）出 F10/P5/G41/CONFIRM 大面。总台独立复核 F 面 14/14 属实后成批修复。

## Criteria

- [x] **F 面 14 全修**：kimi F-1/F-10 rune 隔离世界死 context（eval 撞错自愈置空重建重试一次；raw 名单补 navigateToHistoryEntry、dialect 名单补 fetch 双保险）；F-2 录制 ack 取 /params/sessionId 帧号（顶层路由串不再顶帧号）；F-3 waitIdle requestId 集合对账（重定向同 id 重发不再恒在飞；WebSocket 无终态对剔除；活动 session 过滤）；F-4 解析器 eat_kw 词边界（`letter = 5` 不再静默成 `let ter`）；F-5 会话 cookie expires<=0 省略透传 + setCookie success 布尔入账；F-6 url_host 剥 userinfo（deny 不可绕）+ 域规则小写归一；F-7 附着探测命中但连接失败回落自起（幻影 DevToolsActivePort 不再击穿 ADR-0003）；F-8 数组负数/浮点下标落 null 不取元素 0；F-9 issue new stdin 有界读（--dry-run 不再被挂）；grok F1-F4 文档漂移（architecture 的 pageEval 旧口径/三层图缺 browse-rune/路由缺 /rune、意图反查旧面、AGENTS 发现序补 pin 档、README 的 EVAL_TIMEOUT 缺省 300 改 60）
- [x] **P 面 6 定谳或修复**：P-1 管道 fork 前关本侧两端子副本（CLOEXEC 窗口收口）；P-2 rn goto 提交等待（跨文档先等主框架 frameNavigated 越水位再轮询）；P-3 routeClear 放行缓冲里的在飞 requestPaused；P-4 route watcher 连接纪元观察重连重发 Fetch.enable；P-5 自更新锁收割道回读校验；grok P1 fetch 引擎腿 exceptionDetails 显式上抛（CLI/全局两腿）
- [x] **G 面 47 项处置**：修复 31 项（增量相关 G-1~G-8 全修；存量 G-9/G-10/G-11/G-12/G-13/G-14/G-15/G-16/G-17/G-18/G-19/G-20/G-21/G-22/G-23/G-24/G-25/G-26/G-27/G-28/G-30/G-31/G-32/G-33/G-34/G-35/G-36/G-37/G-38/G-40 与 G-41 碎项含 vars_syntax BOM、rune_running 恐慌守卫、Cleanup 换靶回原会话清账）；记档不做 3 项（见 Notes）
- [x] **文档同批**：AGENTS.md 发现序与附着回落、README 两行（EVAL_TIMEOUT 60、BROWSE_CHROME 坏值语义）、architecture.md 三层图收 browse-rune/fetch_core + workspace 两层定位 + /rune 路由、docs/skills/README 指路碑改口、ADR-0008 Consequences 成本句 rn 化、PAGE_PROBE_JS 与 attach_optional_fields 等 pub `///` 契约同步、surface 两份 fetch 摘要统一可选键、up 补 cookies 参、snapshot 补 pierce 参、ArgSpec.ty 值域全集
- [x] 门禁全绿后 surface 与 aidoc 投影重生成提交

## Notes

- 记档不做及理由：G-29 connect_pipes 线程 spawn 的 expect 属进程资源枯竭终态（# Panics 文档句随三轮补齐）；G-39 Windows 运行中映像不可删的 .bak 残骸是 OS 语义限制（下次 update 的 rename 链自然覆盖，清扫收益低于复杂度）；G-41 的 rawstr 多行普通串行号计数属错误消息美化（无行为面）；G-r6 server 无 CatchPanicLayer 记档不做（引 tower-http 依赖属 minor 批决策，与修复批分离；处理器 panic 最险的 rune_running 滞留位已由 RunningGuard 收口）；G-r10 自更新锁收割道回读后仍有微秒级理论双持窗（记档，超出可观测粒度）
- **G-4 的守卫面边界（kimi 二轮 G-r11 更正口径）**：Fetch 互抢守卫罩方言两道（`session.Fetch.enable/disable` 成员面与 `session.call("Fetch.enable"/"disable")` 面）；rn `cdp::raw` 面**有意不罩**（raw 是任意 CDP 命令显式逃生舱，http-api 与 --llms 已载「rn raw Fetch.enable 自行避让」纪律，罩它等于关逃生舱）
- G-22 shutdown 锁收窄采用「pid 复核后复位」防窗口内新引擎误伤；G-33 实例名净化只拒相对穿越组件（绝对路径是测试面既有能力：join 语义整体改向状态目录，首次实现过猛杀 ledger keygen 测试后收窄，教训留 diary）
- fetch 引擎腿 goto 预算随 --timeout（旧写死 20s）；/eval 请求体带 timeout_s（三轮起 +12s 余量：探针与抽取不吃导航预算）；dialect 桥上界随请求级兜底硬界（kimi G-5/G-6）
- 二轮对线：grok 抓出修复自伤 F 一件（CLI 引擎腿信封类型链：片段回对象、CLI 按串解析必空，391048f 收口并提 parse_engine_eval_receipt 纯函数加回归锁）；kimi 抓出 F-6 修复不完整一件（WHATWG 解析分歧：special scheme 的 `\` 等同 `/` 终止权威段、Tab/LF/CR 解析前剔除，deny 仍可绕，三轮补 `\\` 分隔符与控制符剔除加回归锁）
- 评审轮次：修复批提交后派两工位交叉复核（对方向为对方腿的修复面），F 清零且双 CONFIRM 才闭环（三方一致判据）
- 四轮记档：R4-G-3 非 special scheme 的 `\` 一刀切分歧记档（影响面近零：allow 模式非 http(s) 恒拦、deny 模式 Chrome 对非 special 不导航 host，函数文档已载非 http(s) 调用方自判）；G-r11 引据小出入更正（Fetch 避让纪律实载 http-api.md，--llms/surface 面只载能力未载纪律）；四轮 F（pipe2 同位空转不清 CLOEXEC）与 G（url_host scheme 斜杠位、schema any 省键、watcher 连败节流挂起）随五刀收口
