---
id: REQ-032
title: 封版 v0.30.9（全量评审修复批，REQ-031 判据 patch）
status: implemented
priority: must
trace: 封版树 2f30b0e（= ac20013 加 apple 补救一笔：libc 无 pipe2 绑定，anon_pair cfg 分流 pipe+fcntl 补 CLOEXEC；mac 构建实机绿）；三方一致：grok/kimi 双工位七轮对线双 CONFIRM，F 清零；门禁全套 exit 0（fmt/clippy -D warnings/test 305+doc 51/aidoc --check --strict/surface 契约/BROWSE_E2E=1 e2e 16 件 125s 含管道态实弹）；release.ps1 -Tag v0.30.9 发布毕（三平台包加 sha256 边车六件、三端解包冒烟、gh 直发 --latest，播种流水接力）
---

# REQ-032：封版 0.30.9

## Scenario

总台令「和ai ccoe交流集成 然后封新版本」（2026-10-08）。本版载荷：REQ-031（全量 code review 修复批，grok+kimi 双工位七轮对线至三方一致，F19/P6/G47+ 处置）。集成交流轮先于封版：ai_ccoe/actl 总台回执确认 rn 集成面已闭合（vt.rn 单脚本双页抓取实弹绿，@nsm/vt 加 --root 库形，信封面判成败），零缺口，升 0.30.9 加回归是其侧唯一动作项（timeout 耦合修正为正收益项）。

## Criteria

- [x] 版本 bump 0.30.8 至 0.30.9（Cargo.toml workspace 唯一权威；Cargo.lock 四 crate 随刷）
- [x] CHANGELOG 定段 0.30.9 加批次全貌；aidoc 投影随版本重生成提交
- [x] 封版树门禁全套绿（fmt/clippy -D warnings/test 全家/doc/aidoc --check --strict/surface 契约/BROWSE_E2E=1 e2e 16 件）
- [x] release.ps1 正式发布道（tag 一致性闸、测试闸、三目标打包边车、三端解包冒烟、gh 直发 --latest；CI release.yml 播种接力）

## Notes

- semver 判据在册无偏差：修复批加文档对齐按 REQ-004 判据取 patch（对照 REQ-030 的 minor 批令 patch 留痕形）
- ai_ccoe 集成回执要点留痕：成品在 ai-cloud 仓 rune/nsm/vt.rn（框架仓 e763afa）；骨架件不需要；装位 wsl browse@0.30.8，升级加回归（actl nsm intel 实弹加 vitest 340 基线）回执见其批记
- 评审轮次资产：双工位回执产物 /tmp/browse-rs-review/（七轮齐全）；工位 browse-grok-review/browse-kimi-review 留驻 w2X:p2/p3 可复用
