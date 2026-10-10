---
id: REQ-033
title: 油猴脚本帮助与文档面（封 v0.30.10，纯文档批）
status: implemented
priority: must
trace: 封版树 10ea333（门禁全套绿：fmt/clippy -D warnings/test 306+doc 51/aidoc --check --strict/surface 契约/PEVO 骨架 11 过）；release.ps1 -Tag v0.30.10 发布毕（linux+win-gnu 交叉+mac 实机构建、dist 六件含 sha256 边车、三端解包冒烟、gh 直发 --latest，播种流水由 release published 事件接力）
---

# REQ-033：油猴脚本帮助与文档面

## Scenario

总台令（2026-10-09，三方部署更新验收同步轮中追加）「包括browse对于帮助和文档 针对油猴脚本应该发布新版」。背景：clean-chrome 155.0.8059.39 已发布（GWS prewarm 修复代，O6 终门 12/12×2 放行），userscript 运行时经本工位 17 案验收全线收口（批 0-3 零残留，2026-10-09）；browse 侧帮助与文档零覆盖（全仓 grep 仅 spawn.rs 一处加固组代码注释），令发新版补齐。

## Criteria

- [x] `render_help` 加「油猴脚本」节（片段方言后）：引擎 155+ 内置运行时、match 自动注入、管理页、置位命令、指南指路、命令面边界注；`render_llms` 引言补能力行；surface 投影重生成（llms/llms-full）+ aidoc 重生成
- [x] docs/guides/userscripts.md 新指南：引擎置位（chrome install/list、机器级共享、无头加固组）、安装两道（管理页 + install-userscripts.ps1、id 同构 MakeId 勿手构 JSON）、match 语法与注入档、GM API 面 155 验收基线表、验收方法论（探针 DOM 主断言、盘上 values 比对、settle-单读铁律、console 辅助）、边界（OS 效果归桌面端、命令面挂协议域）
- [x] getting-started 第 11 节指路；CHANGELOG 定段 0.30.10
- [x] 版本 bump 0.30.9 至 0.30.10（semver 判据：纯文档批取 patch，对照 REQ-004）
- [x] 门禁全套绿（fmt/clippy -D warnings/test 306+doc 51/aidoc --check --strict/surface 契约/PEVO 骨架 11 过）

## Notes

- `browse userscript` 命令面不在本 REQ：挂引擎 `Userscript.*` CDP 协议域（clean-chrome 批 4 在账）落地后另开 REQ（对齐轮已裁，2026-10-08）；指南与帮助面均留边界注，不虚构命令
- 顺手对齐：PEVO 骨架门禁 check.py 路径随 project-evo 0.5.x 重构从 `~/repos/project-evo/plugins/evo-adr/...` 迁 `~/.claude/plugins/marketplaces/project-evo/.tools/`（AGENTS 与 docs/README 双处改，旧径已死）；CHANGELOG 0.30.9 段一处 em-dash 禁字消除（新 check 版规 PE-11 收紧）
- 指南能力断言全部来自 17 案验收在档事实（批记 v6），无前瞻承诺
