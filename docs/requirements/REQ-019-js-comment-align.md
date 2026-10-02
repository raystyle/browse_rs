---
id: REQ-019
title: 注释惰性化对齐全模板道（方言/--js/js_lib，G-1 对齐批，v0.30.0）
status: implemented
priority: should
trace: rawstr JS 族单测三件＋CLI js_template_comments_are_inert＋e2e rune_js_lib_comments_inert（真引擎）＋既有全族回归；评审快核 CONFIRM 后发版 v0.30.0
---

# REQ-019：注释惰性化对齐全模板道

## Scenario

0.29.0 注释惰性化只落 rune 脚本模板道，评审 G-1 披露 `--js`/方言片段模板道与 js_lib 库渲染的注释仍参与渲染与未填键点名（战役台坑一在这两个面仍可复现）；总台令「继续」启动对齐批。

## Criteria

- [x] 三面同规：方言片段模板道、`--js` 小程序模板道（CLI `render_js_template`）、js_lib 库渲染（daemon 侧抽占位回填）
- [x] JS 族词法分叉：只抽注释、不认 r#；字符串（含单引号与模板串）照旧参与渲染；正则字面量感知（启发式前一显著字符/关键字后起正则、`\` 转义与 `[...]` 字符类吞 `/`）防正则内 `//` 误判
- [x] 未填键点名扫描不再看注释（三面一致）
- [x] 测试：JS 族单测三件＋CLI 渲染道一件＋e2e 真引擎一件＋全族回归

## Notes

- 词法族判据：r# 属 Rune 语法（Rune 族重写、JS 族不动）；正则字面量属 JS 语法（JS 族感知、Rune 族无此面）；注释两形两族同构
- 误判安全网：正则/除法启发式的任何误判都输出中性（逐字回填），只影响惰性区间划分不影响源码字节
