# browse-rune::vars_syntax

模板语法选择（总台令「{{ 与 jinja 不冲突」批）：四面（rune 源模板
道、方言/--js 片段模板道、js_lib 库体、cdp::render 运行时）同源的
定界形态。缺省 Jinja 经典形 `{{ }}`；ERB 形 `<%= %>` opt-in——`<%`
邻接在合法 JS 中不存在（`a<%b` 即 `a < (%b)`，JS 无 `%` 一元），模
板工程五十年实证的 JS 免撞定界。入口双轨：件首行指令
`// browse-vars: erb`（库件自描述，优先）与 CLI `--vars-syntax erb`
旗标（调用方）；同值同效。

## Functions

- `directive_of` — 件首行指令：`// browse-vars: erb`（或 `classic`）。返回 None 表示无
- `env_for` — 按形态取 Jinja 引擎（ADR-0009 同配置：Strict 加零 autoescape；两形

## Types

- `VarsSyntax` — 模板定界形态。**缺省 ERB**（总台令「默认开启 erb」，0.30.3）：`<%`

