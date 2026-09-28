# S001 网络安全 skill 吸收研究计划（Burp/PortSwigger 生态转 browse skill 面）

立项 2026-09-28，状态 active。研究问题：Burp 生态（BApp、BChecks、Bambdas、
浏览器侧与中文圈工具）的网络安全能力，哪些能吸收为 browse 的 skill 资产，
哪些需要原语立项，哪些结构性不收。本档是计划与分型判定，证据细节后续
批次回填。

## 语境与边界（先立于技术之前）

授权安全测试语境：pentest 委托、CTF、自有资产与防御研究。产出物是
方法论知识与判读配方（agent 照做的人读流程），不是武器化自动攻击器。
三条红线贯穿全计划：不做 MITM 代理监听面；不做高速率 mass fuzz；
不对未授权目标提供批量自动化。browse 域策略（`BROWSE_DENY_DOMAINS`）
在 skill 层之下继续兜底。

## 源地形盘点（官方入口四类）

| 类 | 源 | 定性 |
| --- | --- | --- |
| 官方面 | PortSwiggerBApp 组织（几乎所有 BApp 源码）、extension-portal 提交门户、BApp 商店页 | Java 扩展生态总库；机制参考源 |
| 声明式 | PortSwigger/BChecks（YAML 扫描定义合集）、PortSwigger/bambdas（Java lambda 片段合集） | 最接近「skill 即数据」的两种形态，吸收价值最高 |
| 浏览器侧 | FoxyProxy（v8+ 跨浏览器）、SwitchyOmega、FastBurp | 代理切换 UX；browse 引擎已有 `BROWSE_ENGINE_ARGS` 直通 `--proxy-server`，映射弱 |
| 机制型 BApp | Autorize、Param Miner、Turbo Intruder、ActiveScan++、HTTP Request Smuggler、Backslash Powered Scanner、Hackvertor、Collaborator Everywhere、Upload Scanner、Logger++、Sharpener、Autovader | 检测回路参考（下节解剖） |
| 中文圈 | ShiroScan、FastjsonScan、sqlmap4burp++ | 特定组件检测与外桥；多不在官方商店，吸收只取检测逻辑不引实现 |

[实证: 上表入口清单来自用户 2026-09-28 立项指令；BChecks 与 bambdas 仓
组织结构经 GitHub 页面核验（LGPL-3.0、examples/vulnerabilities-CVEd/
vulnerability-classes/other/archived 目录、Bambda 五类挂点目录
CustomAction/CustomColumn/CustomScanChecks/Filter/MatchAndReplace）]

## 生态解剖（实证锚）

### BChecks：两形各抓一份真样例

主动形（改请求加 OOB 回调，`examples/blind-ssrf.bcheck`）：

```yaml
metadata:
    language: v1-beta
    name: "Request-level collaborator based"
    description: "Blind SSRF with out-of-band detection"
    author: "Carlos Montoya"

given request then
    send request:
        headers:
            "Referer": {generate_collaborator_address()}

    if http interactions then
        report issue:
            severity: high
            confidence: firm
            detail: "This site fetches arbitrary URLs specified in the Referer header."
            remediation: "Ensure that the site does not directly request URLs from the Referer header."
    end if
```

被动形（响应正则，`examples/leaked-aws-token.bcheck`）：

```yaml
metadata:
    language: v1-beta
    name: "Response-level (passive)"
    description: "Looks in responses to see if there are leaked AWS Access Key IDs"
    author: "Carlos Montoya"
    tags: "passive"

given response then
    if {latest.response} matches "AKIA[0-9A-Z]{16}" then
        report issue:
            severity: high
            confidence: firm
            detail: "Leaked AWS Access Key ID."
            remediation: "Replace your keys and ensure keys are no longer revealed."
    end if
```

[实证: 两样例 2026-09-28 自 raw.githubusercontent.com 主分支抓取，逐字入档]

语义要点：metadata 卡（name/description/author/tags）加 `given request
then`（主动入口，改包重发）或 `given response then`（被动入口，纯判读），
内嵌 `{表达式}`（内置函数如 `generate_collaborator_address()`）与
`matches` 正则判定，收口 `report issue`（severity 加 confidence 双档）。
confidence 有 firm/certain 等档位，与 browse page_skills 置信档
（CONFIRMED/PLAUSIBLE）同构，报告语义可直译。

### Bambdas：五类挂点

Filter（历史过滤）、CustomColumn（自定义列）、CustomAction（Repeater
动作）、MatchAndReplace（改写规则）、CustomScanChecks（Java 扫描件）。
[实证: README 五类挂点核验；「scripts can run arbitrary code」官方告警在册，
吸收只取逻辑不执行第三方件]

### 机制型 BApp 核心回路（一句话解剖）

| BApp | 核心回路 | browse 面对应 |
| --- | --- | --- |
| Autorize | 取特权态请求，剥 cookie/授权头重发，比对响应判越权 | 缺改写重放原语（B 层） |
| Param Miner | 字典变参重放加响应差分，挖隐藏参数与缓存投毒 | 缺同原语；差分判读是知识（A 层） |
| Turbo Intruder | 高并发 HTTP fuzz 引擎 | 红线不收（C 层） |
| HTTP Request Smuggler | 畸形 CL/TE 字节序列探反代差异 | 引擎发不出畸形字节（C 层） |
| Collaborator Everywhere | 出站请求注入载荷，等 OOB 回调 | 无协作服务器基建（C 层缓议） |
| Autovader | 自动扫 DOM source/sink 与 postMessage | Runtime.evaluate 加 DOMDebugger 可达（A 层，已落 dom-xss-sink-map） |
| Sharpener | Burp 内 UI 增强（右键菜单、标签整理） | 无检测面，UI 皮不吸收（清单对账补记） |
| Logger++ | 流量录制加过滤加 grep | `requests()`/`requestDetail`/`responseBody` 已覆盖摘要与单条全文（R2 修正原判）；批量导出面缺（B 层收窄） |
| Hackvertor | 标签式编解码转换语言 | 方言页内是真 V8，编解码知识直转（A 层） |
| Upload Scanner | multipart 上传面 fuzz | A 面已落 upload-attack-surface（维度清单加双路配方）；批量 fuzz 部分不收 |
| Backslash Powered Scanner | 意外后缀探测触发报错差异 | A 面已落 error-based-probing（三档探针加变体族）；改写重放原语落地后升级 |

[记忆: 上表回路是行业通识级理解，个别 BApp 细节建议 R1 批次开源码复核]

## 吸收分型（核心产出）

三层判定：A 层知识直转（写 skill 零代码）、B 层原语缺口（需立 REQ）、
C 层结构性不收（留档理由）。

### A 层：知识直转 workspace skill

| 检测面 | 源 | skill 形态 |
| --- | --- | --- |
| 响应敏感信息（ leaked key、token） | BChecks 被动形 | intent-skill：正则清单加判读（goto/fetch 回执文本上跑） |
| DOM XSS source/sink 地图 | Autovader/DOM Invader | intent-skill：sink 地图加页内扫描配方（R2 修正：原判 page-skill，但 slug 面是冻结白名单（#51 F3 十值）无安全配方挂点，落 intent 反查） |
| 编解码与转换套路 | Hackvertor | intent-skill：V8 表达式配方集 |
| 越权差分方法论 | Autorize | intent-skill：剥头重放比对流程（等 B 层原语落地后实弹化） |
| 隐藏参数挖掘流程 | Param Miner | intent-skill：字典源加差分判读 |
| 组件特征判读（Shiro rememberMe、Fastjson autotype） | 中文圈工具 | intent-skill：探测请求形加回执判读 |

### B 层：原语缺口（REQ 候选）

1. 改写重放：取当前会话请求，改头（剥 cookie、换 method）重发取响应。
   CDP 路径是 `Fetch.enable` 拦截加 `Fetch.continueRequest`（url/method/
   headers/postData 可改），CLI 路径是 `Network.getAllCookies` 导出后
   原生 HTTP 客户端重放。Autorize/Param Miner/Backslash 全家桶的底座。
   [推断: 两条路径技术可达，选型是 R3 批次 REQ 的核心议题]
2. 流量导出：R2 修订（2026-09-28）：方言已有 `requests()`（活动 tab
   环形 1000 摘要）、`requestDetail`/`responseBody`（单条全文），录制
   面大半已在；真缺口收窄为批量导出（raw 请求文件形与 HAR 面一键
   落盘），是 sqlmap 桥接的提效项。
3. 响应差分助手：双响应结构化比对（状态码、长度、正文相似度）。方言
   页内可 JS 自写，原语化是为 skill 简写与一致判读。[直觉: 优先级最低，
   先用页内 JS 验证需求]

B 层共同边界检查：新增均为原语函数调用，不进控制流，不破 ADR-0002。

### C 层：结构性不收（留档理由）

| 面 | 理由 |
| --- | --- |
| MITM 代理监听（外工具指向 browse） | browse 是浏览器驾驶器非代理；`--proxy-server` 直通已覆盖引擎侧需求 |
| 高速率 mass fuzz（Turbo Intruder 形） | 定位差异加红线（不做 DoS 面能力） |
| 畸形字节走私探测 | 浏览器引擎协议栈会规范化，发不出 CL.TE 级畸形；raw socket 面不在形态内 |
| OOB 协作服务器自建 | 基建超范围；对接公共 OOB 服务的隐私口径未裁定，R3 一并议 |

## BChecks 构造到 browse 映射表

| BChecks 构造 | 语义 | browse 表达 |
| --- | --- | --- |
| `metadata` 卡 | 检查元信息 | skill 文件标题与首段（同构既有 skill 面） |
| `tags: passive` | 执行形态分类 | intent-skill（反查触发）或 page-skill（特征触发）选型 |
| `given response then` | 被动入口 | goto/fetch 回执文本上跑配方 |
| `given request then` | 主动入口 | B 层改写重放原语 |
| `send request` 加 headers 改写 | 改包重发 | 同上 |
| `{latest.response} matches "regex"` | 响应匹配 | V8 正则或 skill 判读行 |
| `generate_collaborator_address()` | OOB 载荷 | C 层（无协作基建） |
| `if http interactions then` | 回调判读 | C 层 |
| `report issue` 的 severity/confidence | 报告双档 | skill 回执判读加置信档（CONFIRMED/PLAUSIBLE 同构） |

[推断: 表为构造级映射，BChecks 语言全构造清单（loop、变量、`given site`
入口等）R1 批次对着官方 definition reference 扩全]

## 分阶段计划

| 批 | 内容 | 验收判据 |
| --- | --- | --- |
| R1 | BChecks 全构造解剖（对 definition reference 逐项）加 Bambdas 五挂点对照扩全；机制型 BApp 源码复核回路表 | 映射表覆盖全构造且每行有样例或文档锚；回路表 [记忆] 项清零 |
| R2 | 安全 skill 试点仓：独立自定义仓走 #63 多根通道（`browse workspace add`），首批 5 篇（响应敏感信息扫、DOM XSS sink、越权差分方法论、隐藏参数流程、组件判读 shiro/fastjson） | add 后 list 在册；goto/fetch 实弹点名命中至少 2 篇；intent 反查可用；默认仓零污染 |
| R3 | B 层原语立项评估：改写重放与流量录制走 REQ（含 semver 判据）；OOB 对接口径单独裁定 | REQ 在册带验收判据，或裁定不做留档理由；ADR 触发判据明确（若动方言面） |
| R4 | 实弹验收：自建靶场走端到端检测流（例：authz 差分在靶场报出越权差异）；红线复核 | 一条全链绿；红线三条复核无破口；结论择要升 ADR 或 REQ trace |

载体裁定：安全 skill 不进默认仓 browse_workspace（公开站点知识面），
独立仓（名暂定 browse-security-skills）走多根加载，正是 #63 通道的
首个自用案例。[推断: 隔离理由是受众与审核口径不同，安全配方需要
授权语境声明前置]

### R2 完成（2026-09-28）

仓 github.com/raystyle/browse-security-skills（root 40c6ac4，12 文件
596 行）：8 篇 intent（越权差分旗舰、业务逻辑、隐藏参数、敏感信息、
编解码、组件判读、DOM sink、sqlmap 桥）加 2 篇 domain 靶站（httpbin
自验、portswigger labs）加双 README。业务逻辑与越权两篇按用户指令
（2026-09-28「特别是业务逻辑漏洞越权类」）加深：剥授权三法（页内
`credentials:"omit"` 零侵入单发、exportStorageState 态往返、
BROWSE_NAME 双实例）与四类业务手法（价格篡改、跳步、越赋、重复
提交）全量方言化。[实证: 验收实测四条全过：`browse workspace add`
登记后 list 并集在册（roots 面双根）；`browse fetch` 对 httpbin.org
与 portswigger.net 双靶站域名点名命中（domain_skills 各回执附
echo-selftest.md 与 labs.md 加 hint）；`browse workspace site httpbin`
全文读通；`rg -il 越权` 反查命中 authz-diffing.md；默认仓零改动]
全部配方走今日原语，无空等原语的死文。

### R2 收口与 R4 预演（2026-09-28 同日续）

对照用户源清单逐项盘点补三篇 A 面（commit 49d0d99，加 158 行）：
upload-attack-surface（Upload Scanner 的六维探测清单）、
error-based-probing（Backslash 的三档探针）、insertion-points
（ActiveScan++ 的入口枚举，其他配方的共用前置）。至此清单全部条目
各有落点：A 面 skill（11 篇 intent 加 2 靶站）、B 层 REQ 候选三项或
C 层留档四面，无悬空条目。

R4 预演（旗舰配方实弹，httpbin 自验靶）：
[实证: 态往返回路全通（newTab 设 cookie 后 before=1、cookiesClear 后
anon=0、importStorageState 后 restored=1）；零侵入单发探针全通
（`fetch("/cookies", {credentials:"omit"})` 回空 cookies，
include 回 {selftest:"1"}，匿名重放与带权重放的差分肉眼可见）。
引擎跨宿主 daemon（wsl CLI 到 windows 引擎）spawn 隔离实例，
验后 down 幂等退场]
[实证: 两个口径坑入档：现公共实例 `/get` 不回 cookies 字段，cookie
探针打 `/cookies`；方言对象字面量键名 with 前缀（withCreds）被解析器
拒，改名即过]

部署注记：goto 回执的引擎侧域名点名在跨宿主 daemon（windows）下不
吃 wsl 侧 workspaces.json 登记（fetch 两腿是 CLI 侧点名故命中）。
各端要用 goto 点名安全仓，须在该端的 browse 侧 `workspace add` 登记
本端可达路径。[推断: 五端部署时安全仓随端克隆登记，走各端自己的
workspaces.json；不共享配置是既有多根语义]

## 风险

- skill 面被误用为攻击自动化：配方以判读与流程知识为体，速率与批量
  控制不进 skill 面；授权语境声明前置每篇。
- B 层原语过界变方言控制流：REQ 评审锚 ADR-0002。
- 中文圈工具实现不可信：只吸收检测逻辑（请求形与判读），不引代码与
  二进制。

## 信源

- 立项入口清单：用户指令 2026-09-28（四类官方入口全录在案）
- BChecks 仓与样例：<https://github.com/PortSwigger/BChecks>；
  `examples/blind-ssrf.bcheck`、`examples/leaked-aws-token.bcheck`（主分支
  2026-09-28 抓取）
- Bambdas 仓：<https://github.com/PortSwigger/bambdas>
- BChecks 语言定义参考（R1 用）：
  <https://portswigger.net/burp/documentation/scanner/bchecks/bcheck-definition-reference>
