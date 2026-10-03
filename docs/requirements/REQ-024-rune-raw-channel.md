---
id: REQ-024
title: rn 面通用 CDP 命令通道与糖皮回收（v0.30.5，总台令升级案）
status: implemented
priority: must
trace: vm_face 三件（raw 可达/预检先于连接/串形选型探针）＋module 单测四件（params 边界含 null）＋e2e 门控真 chrome 9/9（拟人鼠标 isTrusted 全 true、断流对账 baseline 3 降 blocked_run 1、roundtrip 组合形含填值回读）＋CLI 全链实弹 exit 0；评审两轮（F-1 加 G-1 至 G-7 全修，二轮三 G 收尾）终审放行后发版 v0.30.5
---

# REQ-024：rn 面通用 CDP 命令通道与糖皮回收

## Scenario

#72 报 mouse 与网络拦截原语缺口；#73 现状补正实测收窄缺口为 rn 面绑定（方言面引擎侧全在）。原案逐件绑 `cdp::mouse` 与 `cdp::route` 系；总台裁定升级（2026-10-03）：rn 面缺的是通用 CDP 命令通道，事件侧 `wait_event` 已开任意 method 名，命令侧对称开放即闭环。实施中接修订令：raw 落地后回收纯透传糖皮。决策与理由见 ADR-0011。

## Criteria

- [x] `cdp::raw(method, params_json)`：JSON 对象串参数（空串等价空对象、非对象形预检拦、坏 JSON 带 CTA），透传 `Session::call_with_deadline` 同守卫同 30 秒界，响应整体过桥
- [x] 回收五件纯透传糖皮：click、type_text、wait_sel、cookies、set_cookie；wait_sel 工位裁定回收（纯轮询壳，`time::sleep` 在面，无引擎侧状态）；保留件各有不可透传语义（goto/eval 族/句柄三件/js 族/wait_event/三支柱）
- [x] 导航类边界（评审 G-6）：raw 的 Page.navigate/Page.reload 成功后与 goto 同款废隔离世界；Target.createTarget 不登记 own 文档点明
- [x] 组合验证两件（真 chrome）：拟人鼠标（wiggle/拖拽/滚轮双发，isTrusted 全 true）；Fetch.enable 加 wait_event(requestPaused) 循环按 resourceType 分流（Media/Font 断、其余放行），requests() 对账基线 3 降 1、Fetch.disable 收尾。引擎侧零改
- [x] 自检纪律四面（评审 G-5）：旧 set_cookie/type_text 内建自检归脚本侧（ADR/http-api/surface/pillars 示范/roundtrip 填值段）
- [x] 文档四面同步：surface 三投影、aidoc、http-api 的 /rune 契约段重写（评审 F-1）、--llms 行描述与意图路由句

## Notes

- semver 张力：raw 能力新增按 REQ-004 判据取 minor、五件回收属契约破裂按判据取 major；总台明令封 0.30.5（patch），从令并在此留痕（同 REQ-022 先例形）
- 下游迁移：pentest_rs 战役件用量（cookies 4、set_cookie 3、click 1、wait_sel 2）由总台迁 raw 组合形；组合形样例见 http-api.md 与 crates/browse-rune/examples/pillars.rn
- 附带入账：issue #74（命名实例解析不到托管 pinned chromium，CLI 实弹中发现）
- 评审 nit 记档不采纳两条：errorText 不检查（透传面保守废世界偏安全）；navigateToHistoryEntry 不入特判集（维持与 goto 对齐最小集）
