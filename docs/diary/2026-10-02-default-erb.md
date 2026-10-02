# 2026-10-02 模板定界缺省 ERB（v0.30.3）

## 背景

战役台 fetch-text.rn 踩「JS 的 {{ 双写被 Jinja 当模板块」（format! 转义层与 Jinja 层撞面）；工位提四方案后总台令链：「开工」（opt-in）至「默认开启 erb」至「js rn 的 jinja 全部默认 erb」至「默认 erb 实现 review 后封 v0.30.3」。

## 落地

- browse-rune vars_syntax.rs：VarsSyntax（default=Erb）＋件首行指令解析＋双引擎 OnceLock（custom_syntax 特性）
- 传播链：client render_template_with(syntax) 三 wrapper；RuneRequest.vars_syntax；RunOpts/ModuleCtx；js_lib 件级指令覆写；cdp::render 跟随
- CLI：--vars-syntax 旗标（全局位，名册入册）＋ cli_vars_syntax 校验
- 未填键扫描器双形；全族测试迁 ERB（client 惰性/控制流/注入件、vm_face render、e2e 三通道与 js_lib、arg_contract 模板件）

## 踩坑

- scanner 重构中 bytes().len(body) 笔误（写成方法链），改 find(open) 循环
- e2e js_lib 件先改槽漏改块（{% if %} 残留）报 Unexpected token '%'：块与槽必须同迁
- 测试调用点 Classic 显式参与 ERB 件不匹配三处反复，根因是断言形随定界走

## 验收

- 单测：指令解析/双引擎渲染（含块死分支与 for 复制）/默认断言
- e2e 12/12 全绿（全 ERB 形）；实弹四证：erb1 件（ERB 渲染+字面免疫+format! 安全）、classic 指令回退、旗标全局位、指令优先旗标
