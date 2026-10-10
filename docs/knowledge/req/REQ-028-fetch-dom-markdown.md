---
id: REQ-028
title: fetch 渲染 DOM HTML 与 mdka markdown 转换,并暴露为方言全局（rn 经桥直达）
status: implemented
priority: must
trace: fetch_core 单测四件（含 H1/F-1 回归锁随迁与 mdka 冒烟）＋e2e fetch_global_face（本地源 JS 注入节点渲染证明＋markdown 含标题）＋rune_dialect_bridge 扩 fetch 经桥断言＋CLI 实弹；门禁全套 exit 0
---

# REQ-028：fetch 渲染 DOM 与 markdown 面

## Scenario

总台令两连（2026-10-03）：「fetch功能可以取渲染后的dom html内容和将domhtml转换为markdown 转换markdown使用 mdka-rs」；「fetch功能应该暴露给rn脚本」。browse fetch 此前只有 HTTP raw body 粗抽与引擎腿 innerText，`--markdown` 是 v1 no-op；rn 面无 fetch。

## Criteria

- [x] CLI `--html`：恒走引擎腿回渲染后 DOM HTML（`documentElement.outerHTML`，JS 执行后的面），响应加 `html` 字段
- [x] CLI `--markdown/-m`：从 v1 no-op 升 mdka 真转换（mdka-rs 3.2，Apache-2.0，关 default 特性免 rayon），对本腿 html（渲染形为渲染 DOM、HTTP 直通为 raw body）转 `markdown` 字段
- [x] 方言全局 `fetch(url, opts?)`：完整两腿（daemon 侧 reqwest 直取＋三条件升级引擎腿直调 session），rn 经既有 `cdp::dialect` 桥直达零新绑定（桥的首次实战）
- [x] 纯函数单源：粗抽/升级判定/标题/抽取 JS/markdown 转换收口 `browse_core::fetch_core`，CLI 与全局两道编排共用；腿编排双轨注记（CLI 零 daemon 快路径是既有契约，ADR-0009 注记形）
- [x] 测试与文档：fetch_core 单测、e2e 双件（渲染证明＋桥直达）、surface 双条目（CLI 行加旗标、Global 登记新条）、GLOBALS_CTA 入册

## Notes

- mdka 选型依据：nabbisen/mdka-rs（用户指定），crates.io 名 `mdka` 3.2.0，Apache-2.0，`html_to_markdown(&str) -> String` 字符串转换不可失败；`default-features = false` 免 rayon 依赖
- 方言全局的 fetch 受调用道兜底硬界约束（/eval 60 秒缺省），HTTP 腿自带 timeout 参（秒口径守卫）；CLI 的零 daemon HTTP 快路径不变
- CLI fetch 引擎腿仍走 /eval POST（daemon 侧 goto），与全局的直调 session 引擎腿是两道编排;后续批可评估换轨收敛（双轨注记在 fetch_core 模块头）
