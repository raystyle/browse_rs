---
id: REQ-035
title: workspace 双脚本对重建与 stop api（封 v0.32.0；snippets 剔除）
status: implemented
priority: must
trace: 待封版树回填
---

# REQ-035：workspace 双脚本对重建与 stop api

## Scenario

用户令系列（2026-10-09 晚）：「和grok讨论 全面重写workspace skill」、「先我们是rn脚本和油猴js为主」、「以前的skill全部删除 重新写」、「主要是增加爬虫和网络安全类的 和grok讨论 让grok用x api和搜索类似代码」、「开始」,交流中三连补令「使用标准型」「us-xx us_xx不是标准形 有歧义」「snippets代码和功能剔除淘汰」。grok 两轮设计案（/tmp/grok-workspace-rewrite.md 双脚本对架构+清仓重建+stop api+断点分层；/tmp/grok-crawler-security-skills.md X/web 检索的爬虫五对+安全四对候选与许可证表）为蓝本。

## Criteria

- [x] 判定入口重写：`page-skills/<slug>/(mech.rn+probe.user.js+README?)` 成对才算技能；枚举各根 @detect 块拼一次主世界 cdp::js（_prelude.js 供 facts，无对零键零求值）；PAGE_PROBE_JS/probe.rn 隐式加载退役；白名单换首批九对；verdict challenged 映射 challenge-stop
- [x] workspace list/page/site 对读（README 全文或路径摘要；半对不算；md 清单与 10 篇帽退役）
- [x] stop api：`POST /userscript/stop` 打断在飞 rune，`stopped: {op, elapsed_ms, cause: "userscript_stop", capture}`；捕获槽只留最后一次、新 rune 清槽；不拿 eval_lock、不停浏览器；E2E 实弹（晚到挑战 2.4s 打断 60s 编排，环同 seq 双证）
- [x] API 标准形：POST/GET /events 同径对称；us-event/us-stop 端点名废弃；name 前缀约定改结构化 {type, slug}（旧形兼容解析）
- [x] snippets 剔除（breaking）：snippets list/show 命令族、BROWSE_SNIPPETS_ROOT、<state>/snippets 全移除；脚本数据目录只留 workspace（scripts/ 子树）；@实参与 rune js_lib 缺省根落 workspace scripts/；--root 旗标保留
- [x] E2E：/tmp/us-acceptance/pairs-e2e.ps1 七腿全绿（list/page 对读、plain 零键、challenge CONFIRMED、antibot PLAUSIBLE、stop 打断、环双证）
- [x] 门禁全套绿（303+50+clippy+aidoc+surface+PEVO 11；手册 108/120）

## Notes

- 设计裁定在册：事件上报归 HTTP 面（/events），脚本安装管理归引擎面（Userscript.* 批 4）正交；采集对大结果走 rune 返回值不走 32KB 环；断点分层=脚本侧条件停 vs rune/cdp Debugger（P0 无断点配方）
- 清仓重建时序：browse 新读法先行（本版），workspace 仓清仓+首批对同窗口跟进（93 域站/12 slug 全删，旧文只作素材）
- 兼容解析一段后旧 name 形可移除（下个 minor 攒批）
- workspace 仓侧动作（清仓+九对+scripts/）见批次批记；本 REQ 只锁 browse 侧
