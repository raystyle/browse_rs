---
id: REQ-034
title: 油猴事件回调面与显式路径快败（封 v0.31.0；账本 #77/#76）
status: implemented
priority: must
trace: 待封版树回填
---

# REQ-034：油猴事件回调面与显式路径快败

## Scenario

用户令两连（2026-10-09）：「油猴脚本的正确用法应该是事件回调 可以知道什么网页和js事件发送请求给我们接口回调通知」「我们的http api应该给油猴一个回调申事件接口」（账本 #77）；随后「解决2个issue」令把 #76 一并收口。#76 在 O6 第三换代终验窗报为「0.30.10 win-gnu CLI up--pipe×有状态 profile×前台挂起」，本轮定性实为跨宿主 daemon 串扰（Windows CLI 经 mirrored localhost 命中 WSL 侧 daemon，并发会话的默认实例 daemon 被共享抢占，异宿主引擎路径上排队成挂），rebuild==dist 字节证制品无辜；修法取显式路径快败守卫。

## Criteria

- [x] daemon HTTP API：`POST /us-event`（name 必填非空；环容量 1000 满丢最旧；name/url 截 512 字符；data 序列化超 32KB 截成 `{__truncated, bytes}`）+ `GET /events`（since/last 手解 query，axum 无 query feature 不新开）+ `DELETE /events`（序号不回卷）；回 `{ok, seq}` / `{ok, events, nextSeq}`
- [x] CLI `browse events [--since <seq>] [--last <n>] [--clear]`（目录+帮助+llms 全投影；daemon 不在跑不算错）；surface 契约/帮助守卫全绿（分组表、旗标豁免表、内部编号零痕、手册 120 帽收敛）
- [x] 全链 E2E 实弹：us-e-reporter 脚本（@connect 127.0.0.1）装载即报 page-ready、页内 click 报 dom-click，经 GM_xmlhttpRequest 落环，CLI `browse events` 双证收（/tmp/us-acceptance/us-event-e2e.ps1，幂等）
- [x] 边界裁定在册：事件面（HTTP 回调，browse daemon 面，引擎无关今日即用）与 `Userscript.*` CDP 域（脚本安装管理，clean-chrome 批 4）正交；指南第 6 节明文
- [x] #76 修法：`ensure_explicit_chrome` 守卫挂 spawn/spawn_pipes 双入口（显式 `--chrome` 不在 daemon 侧即错，带跨宿主指引）；env 契约（BROWSE_CHROME 坏值静默跳过）不动；单测锁形
- [x] 版本 bump 0.30.10 至 0.31.0（能力新增取 minor）；门禁全套绿

## Notes

- 安全面：daemon 缺省绑 127.0.0.1，/us-event 天然只收本机；环与截断上限防页面侧失控上报打爆 daemon
- #76 定性更正入 CHANGELOG：撤「制品缺陷」诉（字节级 rebuild==dist + 清净窗全绿 + 完成形回执带跨宿主横幅三证）；守卫是防再犯的入口硬化，非缺陷本体修复
- 手册帽：events +2 行后以「示例与显示名同文者不进常用例」规则收敛到 119/120
