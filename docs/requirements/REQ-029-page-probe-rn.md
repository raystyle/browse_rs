---
id: REQ-029
title: 页面特征判据 rn 脚本化（workspace scripts 目录隐式加载）与 workspace 收窄两层
status: implemented
priority: must
trace: skills 单测（probe_rn_load_override_and_default 加 frozen_slugs_match_probe 既有锁）＋cdp 单测 reap_pending_removes_only_aged_entries＋e2e 挂死块（8 秒界墙钟加 pending<=2）与既有特征页断言过 rn 路径（部署态 workspace probe.rn 实跑）；workspace 仓 81cbf14 加 4849251/5b32d5b（probe.rn 主世界化加 README）
---

# REQ-029：特征判据 rn 脚本化与 workspace 收窄

## Scenario

总台令链（2026-10-03）：「特征判据改为隐式加载的rn脚本」；「rn脚本在page-skills中维护 去掉intent-skills workspace专心维护域名触发和页面触发的domain和page skill」；「page-skills 下建立标准的scripts目录存放页面特征检测rn脚本」。动机：判据属机制配方,改特征原要动 browse_rs 发版;rn 化后判据增改走 workspace 仓热更。

## Criteria

- [x] 判据脚本：workspace `page-skills/scripts/probe.rn`（标准 scripts 目录），browse 隐式加载（goto 回执 page-skill 附着时），多根序首命中覆写、全根未命中回内置缺省（与历史 PROBE 同判据，离线行为稳定）
- [x] 执行道：browse_rune::run 跑脚本（rune VM 双限额与 30 秒单调用界缺省；**cdp::js 主世界探页**（评审 F-1 裁定：框架全局 __NUXT__/__NEXT_DATA__/window.Vue 与元素 expando __reactFiber$/__svelte* 隔离世界一律不可见，eval 形会静默废掉框架与 hydration 腿；对象直桥）；外层 8 秒界（超时静默降级零键，在飞调用留 pending 僵尸由 32 秒后 reap_pending_older_than 回收）；失败/环境错/坏脚本静默降级零键且首败 daemon.log 留痕一次（G-C）
- [x] 信任模型不变：FROZEN_PAGE_SLUGS 白名单与 confidence 枚举校验留本仓 page_fields（页面可伪造判据回执，滤在本仓不在脚本）
- [x] workspace 收窄：intent-skills 层删除，README 定位收窄两层（domain-skills 域名触发加 page-skills 页面触发含 scripts 判据）
- [x] semantic::goto 收 &Arc<Session>（rune VM 要 Arc 克隆；两调用点均 Arc 态零ripple）

## Notes

- parse_probe(字符串解析腿)随对象直桥移除;page_fields doctest 改 serde_json 直构
- e2e 特征页断言（skill trigger 层）实跑部署态 workspace 的 probe.rn:隐式加载路径有真实覆盖；判据改动后跑该 e2e 即验
- 旧 8 秒 deadline 注记(#52)随 call_with_deadline 直调腿退役;run_probe_rn 外层 8 秒界接棒（挂死页兜底）,rune 面自带 30 秒单调用界与 /eval 兜底界;外层界丢弃在飞 future 会丢内层清登记路径,pending 表带登记时刻（Instant）,reap_pending_older_than 按时刻扫老项（e2e 挂死块断言 pending<=2,reap 时效由 cdp 单测 reap_pending_removes_only_aged_entries 锁）
- G-E 加二轮 G-2:default_probe_rn 判据 JS 内嵌 r## 裸串（外层 r###）,debug_assert 加单测真断言双锁不含 "## 定界;G-F:workspace README 点明 probe.rn 无模板道
- 二轮 G-3 取舍明记:主世界化**放弃** #52 时代隔离世界对页面属性覆写（expando/defineProperty）的挂死免疫,换回框架全局与元素 expando 信号;挂死风险由 8 秒外层界加 pending 按龄回收兜底,勿误读为免疫仍在
- 二轮 F-1:fetch 可选字段 helper 补 include 语义（入册源与转换源两参独立）,CLI 与全局两腿接线修,升级腿 markdown 单置的 html 漏入册与空串两雷清,CLI shape_engine_receipt 纯函数单测加全局升级腿 e2e 断言锁
