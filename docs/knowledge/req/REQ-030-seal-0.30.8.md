---
id: REQ-030
title: 封版 v0.30.8（fetch 渲染 DOM 与判据 rn 脚本化两批，总台明令 patch）
status: implemented
priority: must
trace: 封版树 a97f6b2（评审三轮 CONFIRM 放行；CI ci+docs 双岗绿；门禁全套 exit 0 含 BROWSE_E2E=1 e2e 16 件）；release.ps1 -Tag v0.30.8 发布流水回执
---

# REQ-030：封版 0.30.8

## Scenario

总台令「封v0.30.8」（2026-10-03）。本版载荷：REQ-028（fetch 渲染 DOM HTML 与 mdka markdown，方言全局 fetch(url, opts?) rn 经桥直达）加 REQ-029（页面特征判据 rn 脚本化，workspace scripts 隐式加载，探针主世界化加 8 秒界与 pending 按龄回收）。

## Criteria

- [x] 版本 bump 0.30.7 至 0.30.8（Cargo.toml workspace 唯一权威；Cargo.lock 随刷）
- [x] CHANGELOG 定段 0.30.8 加判据句；aidoc 投影随版本重生成提交
- [x] 封版树门禁全套绿（fmt/clippy -D warnings/test 全家/doc/aidoc --check --strict/PEVO/BROWSE_E2E=1 e2e 16 件）
- [x] release.ps1 正式发布道（tag 一致性闸、测试闸、三目标打包边车、三端解包冒烟、gh 直发 --latest；CI release.yml 播种接力）

## Notes

- semver 判据偏差留痕：两批按 REQ-004 判据均应取 minor；总台明令 patch 封 0.30.8，从令（同 REQ-022/REQ-024/REQ-025/REQ-026 先例形）
- 非阻塞账随批在册：账本 #73/#74/#75 关单归 omc 委托道；封后四远端装机刷；pentest_rs 迁 raw 组合形（外仓）
