# browse-core::fetch_core

fetch 的纯函数面（REQ-028 单源）：HTTP 腿的正文粗抽与升级判定、页内
抽取 JS 构造、markdown 转换。何处分流：CLI `browse fetch`（零 daemon
快路径）与方言全局 `fetch(url, opts?)`（daemon 侧，rn 经 dialect 桥
直达）两道编排共用本面；两道编排各自持腿逻辑（HTTP 客户端与引擎腿
拼装不同：CLI 引擎腿走 /eval、全局直调本进程 session），属库内双轨
（ADR-0009 同款注记，漂移由各自测试锁）。

## Functions

- `attach_optional_fields` — fetch 回执的可选字段拼装（两道编排共用）：`html_enroll` 是 `html`
- `extract_js` — 引擎腿的页内抽取 JS（主世界 returnByValue 求值体）：标题、正文
- `needs_upgrade` — 升级判定的三条件（#50）：正文空、墙词、少于 20 词。
- `strip_html` — HTML 粗抽正文文本（HTTP 腿的降级面）：剥 script/style 与标签，压空白。
- `title_of` — 标题抽取（HTTP 腿）：开标记 ASCII 不敏感、闭标记大小写敏感（原行为），
- `to_markdown` — markdown 转换（REQ-028，mdka）：字符串转换不可失败，空串安全。

