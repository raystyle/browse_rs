# browse-rune::rawstr

r# 裸串预处理（战役台裸串研究单定谳 2026-10-02，批次二落地）：Rune
上游无 r#"…"# 形（rune-rs #537 自 2023-05 在册零动作，#802 重复件已
关），browse 侧在编译前把 r# 裸串重写为转义普通串：任意引号与反
斜杠原样直达 `cdp::js`/`cdp::eval`，多行保真（换行原样入串，编译诊
断行号不漂移）。

管道序（ADR-0010 增补）：CLI 模板道先 [`extract_raw_strings`] 把 r#
区换占位标识符，Jinja 只渲染余下源（r# 区天然免渲染，`{{`/`{%` 原
样），渲染毕 [`subst_raw_strings`] 回填；免渲染道（无 --vars、管道源
码、实参）由 [`rewrite_raw_strings`] 在 `run()` 入口兜底。两道单实现
在本 crate，CLI 直接依赖防双轨。

词法器是单遍状态机（近似面）：跳过 `//` 与 `/* */` 注释、普通串、
模板串（`${}` 插值内递归按表达式扫）、char 字面量，只重写表达式位
的 r# 记号；非 ASCII 标识符紧贴 r# 的罕见形可能误判，报错即回退普
通串写法。形与 Rust 全同：`r` + N 个 `#` + `"` 开、`"` + N 个 `#`
闭（N 大于等于 1），内容含 `"#` 时升 N。

## Functions

- `extract_raw_strings` — 抽出源内全部 r# 裸串区：每处 `r#+"…"+#` 记号替换为占位标识符
- `rewrite_raw_strings` — 全量重写（免渲染道兜底）：源内全部 r# 裸串直接换转义普通串。
- `subst_raw_strings` — 渲染后回填：占位源里第 n 个占位换 literals 第 n 项的转义普通串字面量。

## Types

- `RawStrError` — r# 词法错：一行 CTA 形（带行号与下一步）。

