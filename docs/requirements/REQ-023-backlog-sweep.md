---
id: REQ-023
title: 余量清扫批（总台令「做一封v0.30.4」：js_lib 值守卫、G-A6/A9 surface、bh 94 站迁移）
status: implemented
priority: should
trace: json_contains_placeholder 三单测（顶层/嵌套/零误伤）＋surface_contract 重生成锁＋workspace 仓 c5140a4（94 站 108 件）＋goto musicbrainz.org 实弹命中 domain_skills；评审 CONFIRM 后发版 v0.30.4
---

# REQ-023：余量清扫批

## Scenario

v0.30.3 终回执附余量三层清单，总台裁「二、三不做」（上游待复与总台域搁置件永搁），随即令「做一封 v0.30.4」：第一层四件一次清。

## Criteria

- [x] js_lib 值注入理论形守卫（v0.30.2 评审 G-2 记档）：运行时 vars 绕过 CLI 的 vars_value_guard（直 POST /rune 或脚本内拼串），值含内部占位前缀 `__BROWSE_RN_RAW_` 会在 subst 回填段被当作占位记号（拼接注释区文本）或触发误导性残留报错；module.rs js_lib 臂渲染前递归扫 vars_json（对象/数组深处串值同拦），PLACEHOLDER_PREFIX 升 pub(crate) 同源。cdp::render 不经占位道无需同守卫
- [x] G-A6（v0.28.0 目录审计低项）：artifact publish 单行 signature 补列 `--version <v>`/`--git-range <r>`/`--note <n>`（args 表本在册）
- [x] G-A9：version-flag 条目 signature 显 `-V` 同义短参；publish description 补「字段 --version 与全局版本旗标同名不同位」一句
- [x] G-A8 已闭引证：读序句「全局旗标尾置合法（无关子命令静默忽略；子命令环内未知旗标用法错）」surface.rs:1678 在册（审计建议的「文档一句」道先落，无需再动）
- [x] G-A7 已闭引证：名册双向守卫测试 roster_matches_global_parse_arms_bidirectionally（v0.30.2 交付）即审计所求
- [x] REQ-015 残留：bh 94 站 domain-skills 迁 workspace 仓（逐字保真+源注横幅+段名改位+域族并档+垃圾段子域不采用+MIGRATION.md 清单；commit c5140a4；goto musicbrainz.org 实弹回 domain_skills 命中）。种子四站 browse 原生件保位（bh 件 bh- 前缀让名）
- [x] e2e flake（spawn_port_channel）：不适用面：已记 REQ-022 Notes（再遇立 issue），本批未复发不预改

## Notes

- semver 判据：守卫收口与文档补列俱是既有面的完整性修复，取 patch（REQ-004 判据「文档/修复批取 patch」）；workspace 仓内容随本批记录但不随 browse 版本走（ADR-0008 知识与工具解耦）
- 迁移裁量三条（MIGRATION.md 全表）：垃圾段子域（news./store./admin./my./mail./web./hotels./sfbay.）不采用为目录名防跨站误命中（九站无自动命中，workspace list/site 仍可发现）；域族并档（archive-org+wayback-machine 至 archive、arxiv-bulk 至 arxiv）；聚合簇保留原名恒无自动命中（job-boards/news-aggregation/package-registries/weather/substack）
- 无实弹不重写：bh 件逐字保真，逐站 browse 原生化按需小批（每站先真站验证）
