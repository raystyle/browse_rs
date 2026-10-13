---
name: browse
description: >-
  browse 浏览器 harness 研究与自动化:google-search 双脚本对研究道(SERP 判读+结果页
  渲染内容采集+AI Overview)、多步浏览器自动化编排(rune/CDP 直通)、油猴脚本装管、
  引擎起停与托管 Chromium。命令面活权威是 `browse --llms`(本技能是路由与纪律面,不是命令字典)。
  Use when 多步网页研究(要搜索+取多个结果页内容)、多步浏览器编排、油猴脚本管理、
  引擎起停换版时。**不要为单步任务加载本技能**:读单个 URL 直接 browse fetch,单条求值直接
  browse --js(近失负例,见 When NOT)。
---

# browse:浏览器研究与自动化

> 候选轮(con-06 件5,2026-10-10):依 distil-skill 一案提案落未提交工作区,闸门三证据备齐停候人裁;知识底座 [[R104-pattern-serp-selector-drift]]、[[R105-pattern-csp-safe-vendored-injection]](diary trace:2026-10-10-evo-explore-google-search-phase1)。

## When to Apply

- 网页研究:搜一下 + 取目标页内容(google-search 双脚本对,含 AI Overview)
- 浏览器自动化:goto、页内求值、快照点击、rune 编排(多步流程、CDP 直通)
- 油猴脚本:装/列/启停/更新/路径,双脚本对(page-skills)的装用与升级
- 引擎面:起停、headless/headful、托管 Chromium 装版/pin/体检、代理

## When NOT to Apply(近失负例:这些直接跑命令,不加载本技能)

- 单纯读一个已知 URL 的内容:`browse fetch <url> [--markdown]` 一步即得,不起引擎不装对,**无需本技能**
- 单条页内求值:`browse --js '<片段>'` 直跑,**无需本技能**
- 非 browser 面(网络诊断、HTML 离线解析):不是本技能辖域

## 活权威铁律

- 命令语法与旗标以 `browse --llms` 为活权威(本仓投影 docs/surface/ 派生,tests/surface_contract.rs 锁漂移);本技能不复制命令字典,只给道路由与纪律
- 技能对与站点知识在 workspace 仓(github.com/raystyle/browse_workspace),升级走 `browse workspace update`

## 命令道路由

| 要做什么 | 走哪条道 |
| --- | --- |
| 研究:搜索+取结果页 | 双脚本对两步(见下节) |
| 读单页 | `browse fetch <url> [--html] [--markdown]` |
| 页内求值 | `browse --js '<片段>'`(跨调用变量持久) |
| 多步编排/CDP 直通 | `browse rune <script.rn> --vars k=v`(cdp::js/dialect/raw/wait_event) |
| 起引擎 | `browse up [--headless] [--proxy …]`;关 `browse down`(不关用户浏览器) |
| 引擎体检/换版 | `browse chrome doctor / list / use / update` |
| 油猴管理 | `browse userscript install|list|enable|disable|remove|update|path` |
| 事件消费 | `browse events [--since N]`(探针回调入环,含技能建议 cta) |

## 核心用例:google-search 研究(两步)

```bash
browse userscript install page-skills/google-search/probe.user.js --restart
browse rune page-skills/google-search/mech.rn --vars q=<查询词> --vars top=5 > research.json
```

- 返回单 JSON:`probe`(verdict/drift/results/markdown/ai_overview/probe_debug)+ `pages`(finalUrl/title/text/html,渲染后内容)+ `errors`
- verdict 语义:ok 正常;consent 同意墙(自动接受后仍卡即保持);challenged 挑战终态(如实回执不假绿,不设 HTTP 抓 google 兜底);empty 零结果
- drift=suspected = 主选择器漂移(兜底道顶上),连续 2 次该升选择器(SELECTORS_VERSION 在 probe.v)
- 机制:指纹收齐事件驱动(链接在页即见,收齐即走);结果页 cdp 开标签并行访问取渲染内容,goto 包裹真址由访问落定(finalUrl);AI Overview 流式块探针事件道补捕
- 细节手册:references/research-google.md

## 纪律红线

- **三要素铁律(用户令 2026-10-10)**:脚本与插件核心编写逻辑恒守——**事件驱动**(观察器/事件回调即走,不 sleep 轮询)、**原子错误**(失败面回单 JSON 证据面,不落部分垃圾)、**超时兜底**(所有等待段有帽,帽尽原子错早退);逐等待段自查:事件源是什么?帽多少?帽尽回什么?
- 不关用户浏览器:守卫在 cdp Session::call 层,绕不过是特性
- 判定点名 gate-verdict:门禁/验收退出码判定纪律见该标准页
- 挑战/失败终态如实回执,不假绿
