# browse-cli::fetch

一次性只读抓取（#50/#56）：HTTP 优先，三条件升级引擎，markdown 直出。

- HTTP 腿：reqwest 直取（零浏览器成本），判升级三条件：正文空、命中
  墙词、正文少于 20 词。
- 引擎腿：经 daemon POST /eval 走 goto 加页内抽取（启发式标题加段落，
  非 Readability；v1 口径已在 surface 披露）。
- 单源纪律（REQ-028）：纯函数（粗抽/升级判定/标题/markdown 转换）在
  `browse_core::fetch_core`，与方言全局 `fetch(url, opts?)`（rn 经
  dialect 桥直达）共用；本文件的腿编排（CLI 进程内 reqwest + /eval
  引擎腿）与全局的 daemon 侧编排双轨（ADR-0009 注记形，零 daemon 快
  路径是 CLI 既有契约）。
- 域名层点名（#56）：两腿回执都按 URL 做 goto 同口径匹配，命中
  workspace 站点知识附 domain_skills 与 hint；不依赖引擎（引擎腿
  daemon 侧 goto 的点名回执被抽取串替换，CLI 侧补点）。

## Functions

- `fetch` — 抓取入口：HTTP 优先，命中升级条件转引擎腿。

