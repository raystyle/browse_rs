# 2026-10-02 注释惰性化对齐全模板道（v0.30.0）

## 背景

0.29.0 后总台令「继续」：backlog 首项 G-1 对齐批启动（评审 G-1 披露 --js/方言模板道与 js_lib 注释仍参与渲染，战役台坑一在两面仍可复现）。同轮递 omc 验收锚（#69/#70，ai_ccoe 工位）。

## 落地

- rawstr 词法族分叉：Family::Rune（r# 加注释）/Family::Js（仅注释加正则感知）；正则启发式（前一显著字节属运算开括号类或关键字后或行首起正则；转义对与字符类吞 /）；prev_sig/last_word 全字面量闭口接线
- 消费方：CLI render_js_template（方言/--js 模板道）；js_lib 渲染前抽占位回填；rune 面不动
- 文档：http-api 范围句更新（三面同规）、surface eval 与 js-flag 条目补注释惰性半句、CHANGELOG、REQ-019、README 行

## 验收

- 单测：JS 族三件（注释惰性字符串活、正则与除法、模板插值内扫描）＋CLI js_template_comments_are_inert
- e2e 真引擎：rune_js_lib_comments_inert（库内注释含 {{ }} 不破 Strict 渲染，字符串槽照渲染回 [2,1]）
- 门禁与评审见发版回执

## 记档

- 误判安全网：启发式任何误判都输出中性（逐字回填），只影响惰性区间不影响字节
- omc 锚已递（#69 修于 0.28.0、#70 披露腿落 0.28.0 补名腿在排）
