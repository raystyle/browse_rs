---
id: REQ-036
title: 探针框架起步（封 v0.33.0；cta 启发式 + probe-kit v1）
status: implemented
priority: must
trace: 待封版树回填
---

# REQ-036：探针框架起步

## Scenario

用户令（2026-10-10）：「所以后面的精力都是聚焦在 workspace 的开发」「现在有油猴脚本探针后 page-skills cta 指针触发方式更多了」「对 需要油猴探针去启发式的触发 cta」「我们要开发庞大的油猴探针框架」「都解决 让可以发新版」。战略裁定：browse 底座定形做最小托底，框架主体在 workspace 仓生长。

## Criteria

- [x] 事件流 `cta` 字段（可选,截 512）：探针按页况启发式给下一步动作（跑编排/读配方/导出），`browse events` 直读；请求体/环/事件形/两 handler 全链贯通；环单测覆盖
- [x] `page_skills_hint` 双指：读 + 跑（`browse workspace page <slug>（跑:browse rune page-skills/<slug>/mech.rn）`）
- [x] 拼批包裹层 facts 改 var：prelude 的 `var facts` 守卫撞 `const` 是 SyntaxError，整批复解析失败表现为零键（文件单测过、组合才炸）；真仓实测修 + 复验 CONFIRMED 点名
- [x] probe-kit v1 运行时落 workspace 仓（c27354c）：window.__probe（see 带 cta/once/throttle、stop、tap 单包钩注册表、route 单包、onFlush）+ 判定 facts 双职责；README 创作约定与装载时序（waitProbe 等待形）
- [x] E2E 七腿绿 + 真仓实弹绿（新 prelude + 双指 hint 在回执）
- [x] 门禁全套绿

## Notes

- 框架开发纪律在册：改 prelude/加对/改 cta 零 browse 发版；新 slug 进白名单才攒批发版
- 后续：九对迁移到 R.* 运行时、类目扩容（监控/表单/测试探针）、P2 arm/disarm,全仓侧
