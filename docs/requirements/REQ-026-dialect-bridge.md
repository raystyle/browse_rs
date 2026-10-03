---
id: REQ-026
title: 方言全量桥 cdp::dialect(fn, args_json)（v0.30.7，账本 #75）
status: implemented
priority: must
trace: module 单测三件（args 边界）＋vm_face 一件（未注入面与预检先于注入检查）＋e2e rune_dialect_bridge 三形实弹（零参 currentTab、数字参 waitLoad、对象参 requests 与三支柱同源对账 1==1）；评审无 F 终审放行（三低 G 随批收）
---

# REQ-026：方言全量桥

## Scenario

rn 面回收五件糖皮后（ADR-0011），方言面有态聚合件（waitForResponse 的界内等体、waitIdle 的窗口语义）与既有战役片段逻辑在 rn 侧不可直达，战役只能 raw 手组或 CLI 层混合编排。用户令「提 实现全量桥 封v0.30.7」（2026-10-03）；账本 #75 在册。决策见 ADR-0011 增补节。

## Criteria

- [x] `cdp::dialect(fn, args_json)`：方言全局函数按名直达 js_host `global_json` 派发面（100+ 件，与方言同一实现同一守卫）；args JSON 数组串（位置参序、空串等价空数组、非数组形预检拦 `parse_dialect_args`）
- [x] HostFns 通用腿（载荷 `{name, args}` 由 daemon 闭包解包，60 秒界与同源腿同律）；进程内测试态报 op=dialect 未注入
- [x] 与 raw 对偶关系写明（CDP 层与方言层两通道；薄封装件经桥可达不等于 rn 面内建）
- [x] 文档四面：http-api 表行、surface rune 行、cdp_module 文档、ADR-0011 增补

## Notes

- HostFns 字段加法对外部穷举构造者是 source-breaking（评审核：工作区内部 API 无跨仓直构，可接受）；构造纪律注记在册（用 `..Default::default()`）
- 三支柱独立件保留（高频直用 + opts 对象形顺手），不并入桥
