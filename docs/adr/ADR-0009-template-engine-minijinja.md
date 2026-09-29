---
id: ADR-0009
status: accepted
date: 2026-09-29
deciders: 用户（minijinja 定向，2026-09-29 会话内给链）、raystyle
superseded_by: null
---

# ADR-0009：JS 命令小程序模板引擎选 minijinja

## Context

#66 把 snippets 定位升格为「参数化 JS 命令小程序库」：代码驻库，`--vars` 传参，一条命令一个可复用小程序。模板面需要：占位符取值直插（值按源码文本，引号由模板管）、可选参（不填不报错）、生成面（循环展开重复 JS 结构）；未填必填键必须网络前拦 exit 2 且报键名（对齐 #64 读失败报错形态）。本仓此前零模板依赖（changelog 多批以「零新依赖」为纪律），引入引擎属不可逆技术选择。

## Decision

选 minijinja（v2.24，默认特性；传递依赖仅 memo-map 一件），受控配置单点封装在 `browse_cli::client::template_env`：`UndefinedBehavior::Strict`（未定义值即错，对齐「未填硬错误」验收）加零 autoescape（产物是 JS 源码不是 HTML，转义即破坏源码；minijinja 缺省不转义，不另开 HTML loader）。因为 Strict 错误不保证带键名，渲染前先扫裸标识符占位符（`{{ 键 }}` 形）做带名预检，引擎兜底复杂表达式面的未定义值。

## Consequences

- 好：Jinja 语法生态成熟（过滤器、`| default()`、`{% for %}`），命令小程序的表达面一步到位；Strict undefined 语义与验收天然对齐；mitsuhiko 维护活跃
- 坏：破零新依赖纪律（本仓首个模板引擎依赖，二进制体积增加）；模板文件被激活（给 `--vars`）后整文件属 Jinja 语法域，字面 `{{ }}` 须靠「不给 --vars 即纯透传」的激活规则规避；Strict 预检是双道（扫描器与引擎各一层），扫描器漏掉的复杂表达式错误不带键名
- 缓解：激活规则零静默漂移（不给 `--vars` 走 #64 纯透传道，既有测试锁）；预检与渲染同收口 `render_template` 单函数

## Alternatives

- 手写 `{{ 键 }}` 字符串替换（零依赖）：被否。只撑得住平替取值，可选参与生成面撑不住，命令小程序定位下表达力不足；且自研语法解析的边角（空白、嵌套、逃逸）长期维护成本高于引一个成熟引擎
- tera / handlebars 等 Rust 模板引擎：被否。Jinja 语法面 minijinja 实现最轻（单 crate 主件），tera 依赖面更重，handlebars 语法与用户定向不符
