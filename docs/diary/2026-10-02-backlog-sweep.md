# 2026-10-02 余量清扫批（v0.30.4）

总台裁「二、三不做」（上游待复与总台域搁置件永搁）后令「做一封 v0.30.4」：余量第一层四件一次清。

## 做

- js_lib 值注入理论形守卫：rawstr 的 PLACEHOLDER_PREFIX 升 pub(crate)，module.rs 新 json_contains_placeholder 递归扫 vars_json，js_lib 臂渲染前拦（值含 `__BROWSE_RN_RAW_` 前缀报错带 CTA）。定位判据：daemon 侧只有 js_lib 过 extract_comment_regions+subst 占位道（cdp::render 纯 render_str 无占位），守卫只挂这一臂。三单测：顶层命中/嵌套命中/零误伤
- surface：G-A6 publish signature 补三旗标、G-A9 -V 显名与同名不同位句；G-A8/G-A7 考古后判已闭（读序句 surface.rs:1678、双向守卫测试在册），REQ-023 引证收口不重复造
- bh 94 站迁移（REQ-015 残留）：上游考古 browser-harness assets/domain-skills 94 站 107 件；映射表经 URL 域族聚类 + 逐站编务（垃圾段子域不采用、域族并档、聚合簇保名）；逐字保真 + 源注横幅迁 browse_workspace（c5140a4）；种子四站 bh- 前缀让名保位。实弹：方言 goto musicbrainz.org 回 `domain_skills:["scraping.md"]`
- e2e flake：不适用面裁定（REQ-022 已记观察态，未复发不预改）

## 坑

- G-A6/A7/A8/A9 原文不在 REQ-017 展开面（只一句「记档后续批」），考古走 codex 会话 jsonl（hst trace 无 codex 命中，直接 grep ~/.codex/sessions 才中）；教训：评审「记档后续批」的低项要么当场展开入 REQ，要么留检索锚
- rune 面的 cdp::goto 是裸原语（回 frameId 串），域名命中回执键是方言 goto 面的；实弹验证走错面先踩 String[index] 型错
- 迁移件数对账：find -type f 计 107 含一个 .py 伴件（claude-ai），.md 106 + .py 1 分道迁移（.py 用 # 注释横幅）

## 裁量

- 无实弹不重写：bh 件逐字保真迁移，browse 原生化改写按需小批（每站先真站验证）；种子四站不碰
- 垃圾段子域不采用：news./store./admin./my./mail./web./hotels./sfbay. 等跨站通用首标签做目录名会误命中他站（如 news.google.com 撞 hackernews），九站宁无自动命中（MIGRATION.md 全表留痕）
