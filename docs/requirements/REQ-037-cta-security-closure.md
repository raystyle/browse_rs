---
id: REQ-037
title: cta 安全收口（封 v0.33.1；自由串→动作枚举+daemon 模板闭集）
status: implemented
priority: must
trace: 待封版树回填
---

# REQ-037：cta 安全收口

## Scenario

用户令（2026-10-10）：「和grok讨论 让grok用x api搜索研究 看油猴探针启发式触发CTA是否需要优化和改进」与「发」。grok 评审报告 /tmp/grok-cta-review.md（X+web 实溯：Chrome WebMCP 反面教材、Stagehand/UiPath/Slack action_id 受信目录先例、间接提示注入文献）定自由串 cta 为页面可控注入面，P0 收口。

## Criteria

- [x] 请求体 `cta: String` 废弃，改 `action: 'read'|'rune'|'export'` 枚举
- [x] `cta_for` 单源模板生成：`FROZEN_PAGE_SLUGS` × 三动作 = 27 条闭集；名单外 slug 或非法动作返回 None（事件保留，cta 键不出现）；stop 恒不带 cta
- [x] runtime `R.see` 参数收口（workspace 仓 87193dc）；README 消费契约（cta 是候选命令，data/url 是观测）
- [x] 环单测锁闭集（三模板/非法动作/名单外/无 action/stop 全断言）；E2E 7/7 复绿；门禁全套绿

## Notes

- 评审裁定在册：轮询够用不做 ws；静态双指与 cta 并存；P1 候补=去重键 pathname+服务端 2 秒下限+环按 slug 封顶+GET /events 加 next（冻结序安全类>爬虫类）；P2=宿主侧 ack(ran/skipped/failed 只影响 next,不回页面)
- 混淆代理残余：页面仍可伪造事实（摆挑战框）触发合法建议，白名单消不掉,故 cta 永是建议不自动执行
