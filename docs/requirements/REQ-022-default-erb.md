---
id: REQ-022
title: 模板定界缺省 ERB（v0.30.3，总台令）
status: implemented
priority: must
trace: vars_syntax 单测（指令解析/双引擎/块渲染）＋client 全族 ERB 迁移＋e2e 三通道与 js_lib 迁移＋实弹四证（ERB 渲染/字面免疫/format! 安全/classic 回退与指令优先）；评审快核 CONFIRM 后发版 v0.30.3
---

# REQ-022：模板定界缺省 ERB

## Scenario

战役台 fetch-text.rn 踩「Jinja 渲染时 JS 的 {{ 双写被当模板块」；工位提四方案（B 旗标 opt-in/C 件指令/D 默认切/A 纪律），总台先令「开工」实现 opt-in，中途改令「默认开启 erb」再明「js rn 的 jinja 全部默认 erb」并封 0.30.3。

## Criteria

- [x] 缺省 ERB：四面（rune 模板道、方言/--js、js_lib、cdp::render）槽 `<%= %>`、块 `<% %>`、注释 `<%# %>`；`VarsSyntax::default() = Erb`
- [x] 回退道：`--vars-syntax classic` 旗标（全局位）与件首行 `// browse-vars: classic` 指令；指令优先旗标
- [x] 传播链：CLI 收口 至 RuneRequest.vars_syntax 至 RunOpts 至 ModuleCtx；js_lib 件级指令覆写本执行选择；cdp::render 跟随
- [x] 未填键点名扫描器双形（经典扫 `{{ k }}`、ERB 扫 `<%= k %>`）
- [x] minijinja custom_syntax 特性启用，双引擎 OnceLock 单例
- [x] 测试：单测（指令/引擎/块）＋client/e2e 全族 ERB 迁移＋实弹四证

## Notes

- semver 张力：定界缺省翻转按 REQ-004 判据属行为变化应取 minor；总台明令 patch 封版，从令并在此留痕
- 存量迁移：`{{ 键 }}` 件加首行 `// browse-vars: classic` 或迁 `<%= %>`；战役台 pillar 系待迁清单由其自查
- `<%` 免撞证明：JS 无 `%` 一元运算符，`a<%b` 非法；五十年 ERB/EJS 实证
