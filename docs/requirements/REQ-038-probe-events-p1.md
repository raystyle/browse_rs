---
id: REQ-038
title: 探针事件 P1（封 v0.33.2；下限/驱逐律/next 冻结序/去重键升级）
status: implemented
priority: must
trace: 待封版树回填
---

# REQ-038：探针事件 P1

## Scenario

用户令(2026-10-10):「继续」进 P1(grok cta 评审候补四件)、「.2 封」。评审依据 /tmp/grok-cta-review.md 第 4c/4d 节。

## Criteria

- [x] 服务端 2 秒下限：see 同 `(slug,action)` 重复回 `throttled` 不入环;stop 不设下限（打断信号不容丢）;首见键不查
- [x] 环驱逐律：只逐 see 不逐 stop;单 slug 至多 20 条 see（EVENT_PER_SLUG_CAP）
- [x] `GET /events` 加 `next`：CTA_PRIORITY 冻结序（安全类先于爬虫类）取最高优先合法 cta,同级取最新;stop 不参与
- [x] runtime：R.see 去重默认键 `slug|action|pathname` + R.route 重武装（显式 once 覆盖保留;workspace 仓 cca6f2e）
- [x] 环单测锁四件;E2E 7/7;门禁全套绿

## Notes

- P2 留候：宿主侧 ack（ran/skipped/failed 只影响 next,不回页面）
- 评审原话要点：客户端节流是省请求,服务端下限才是界;电平不进环只边沿;页面可写的优先级必被写成最大故排序表在 daemon
