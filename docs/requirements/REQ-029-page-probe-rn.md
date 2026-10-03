---
id: REQ-029
title: 页面特征判据 rn 脚本化（workspace scripts 目录隐式加载）与 workspace 收窄两层
status: implemented
priority: must
trace: skills 单测（probe_rn_load_override_and_default 加 frozen_slugs_match_probe 既有锁）＋e2e 既有特征页断言过 rn 路径（部署态 workspace probe.rn 实跑）；workspace 仓 81cbf14（scripts 目录加去 intent-skills）
---

# REQ-029：特征判据 rn 脚本化与 workspace 收窄

## Scenario

总台令链（2026-10-03）：「特征判据改为隐式加载的rn脚本」；「rn脚本在page-skills中维护 去掉intent-skills workspace专心维护域名触发和页面触发的domain和page skill」；「page-skills 下建立标准的scripts目录存放页面特征检测rn脚本」。动机：判据属机制配方,改特征原要动 browse_rs 发版;rn 化后判据增改走 workspace 仓热更。

## Criteria

- [x] 判据脚本：workspace `page-skills/scripts/probe.rn`（标准 scripts 目录），browse 隐式加载（goto 回执 page-skill 附着时），多根序首命中覆写、全根未命中回内置缺省（与历史 PROBE 同判据，离线行为稳定）
- [x] 执行道：browse_rune::run 跑脚本（rune VM 双限额与 30 秒单调用界缺省；cdp::eval 隔离世界探页，对象直桥）；失败/环境错/坏脚本静默降级零键（不让 goto 失败）
- [x] 信任模型不变：FROZEN_PAGE_SLUGS 白名单与 confidence 枚举校验留本仓 page_fields（页面可伪造判据回执，滤在本仓不在脚本）
- [x] workspace 收窄：intent-skills 层删除，README 定位收窄两层（domain-skills 域名触发加 page-skills 页面触发含 scripts 判据）
- [x] semantic::goto 收 &Arc<Session>（rune VM 要 Arc 克隆；两调用点均 Arc 态零ripple）

## Notes

- parse_probe(字符串解析腿)随对象直桥移除;page_fields doctest 改 serde_json 直构
- e2e 特征页断言（skill trigger 层）实跑部署态 workspace 的 probe.rn:隐式加载路径有真实覆盖；判据改动后跑该 e2e 即验
- 旧 8 秒 deadline 注记(#52)随 call_with_deadline 直调腿退役;rune 面自带 30 秒单调用界与 /eval 兜底界
