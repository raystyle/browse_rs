# 2026-10-02 注释区 Jinja 惰性化（v0.29.0）

## 背景

总台 r# 增益面实测对齐后问「需要修复优化么」；工位判定无必修缺陷，建议两项小优化（注释惰性化：战役台坑一实战实证；format! 三层正交文档句：坑二误解面），总台令「优化」「优化 review 封一个 0.1 小版本」。

## 落地

- rawstr.rs：注释两分支（行注释到 EOL 不含换行、块注释含定界符）改占位抽取（与 r# 区同道），逐字回填行数不变；EOF 无换行与未闭合块注释整段惰性；未填键点名扫描随之看不到注释内占位
- 免渲染道幂等：daemon run() 入口对注释是逐字回填空转
- 文档：surface rune 条目 r# 句补注释惰性；http-api r# 节补注释免渲染句与三层正交句；CHANGELOG 0.29.0；REQ-018
- 测试：rawstr comments_are_jinja_inert 与 comment_edges_eof_and_unterminated_block；CLI 渲染道 rune_template_comments_are_inert

## 记档

- 行为收窄判 minor：注释从可渲染变字面，无在册用例依赖注释内替换
