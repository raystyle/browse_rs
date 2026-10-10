# 2026-10-03 全量 code review 发现批

## 背景

v0.30.7 发版后总台令全量 code review(herdr-review 任务协议:委派即产物、收件人模式)。codex 评审格三分钟产出发现清单(/tmp/browse-full-review/findings.md,基线 389560d):高 1、中 3、低 5,另已核无发现面七节 CONFIRM。

## 发现与处置,逐条核实再修

- H1(高,必修):fetch 的 strip_html 与标题抽取用 to_lowercase 后共用字节下标,U+0130(İ)/U+1E9E(ẞ) 等小写化变长字符使下标落字符中间直接 panic(İstanbul 页等价复现 rc=101)。修:html 本体 ASCII 不敏感扫描(HTML 标记名恒 ASCII),回归锁两件
- M1:cdp::dialect 导航/换靶族(goto/reload/goBack/goForward/newTab/switchTab/closeTab)经桥真执行不废 rune 隔离世界(后续 eval 撞死 context 或静默落旧 tab)。修:七名族成功后 world 置零(与 raw G-6 同律)
- M2:手动 session.Fetch.enable 与 routeBlock/routeMock 同会话互抢(手动开域覆盖路由 pattern 集、watcher 代答放行 requestPaused)。修:方言面 Fetch.enable 在路由在跑时 CTA 拦(先 routeClear);rn raw 面文档注互抢纪律
- M3:/eval 的 Not-connected 兜底重试腿裸 await 脱兜底界占槽。修:剩余界包裹,超时同走结构化回执
- L1 修(send_with 发送失败清 pending 积尸)、L2 修(CLI 请求 HTTP 超时随 --timeout 联动防 600s 缺省抢先);L3/L4/L5 记档(allow-list 拦非 http(s) scheme 点名、REQ-016/017 加 ADR-0012 指路、doc 同步)

## 修复本身过复核的又一实证

一轮修 H1 引入新越界(二轮 F-1):find_ascii 的饱和下标在 haystack 短于 needle 时退化 0..=0 后切片越界:空 html(HTTP 失败腿)的标题抽取与未闭合 script/title 的正文抽取实测崩,评审方两条路径独立复现。三轮守卫收口(两份拷贝同修),回归锁补七形;期间我方回归锁自身一断言错(闭标签查找本大小写敏感是原行为,断言按新行为写错)随修。教训:改扫描器要带短串/空串边界用例,断言先对原行为再对新行为。

## 终态

三轮全 CONFIRM 放行(无遗留必修);残余低项三项记档在 findings.md 二轮节(页面自发导航留死世界的根治需 ensure_world 比对 doc_generation、先手动 enable 后 routeBlock 的反向序、env 大界时 CLI 600s 缺省仍可能抢先)。门禁全套 exit 0;e2e rune 族 10/10。
