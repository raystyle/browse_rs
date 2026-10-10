---
id: REQ-021
title: 休眠项与 REQ-020 记档全清（v0.30.2，总台定版）
status: implemented
priority: should
trace: 探针多槽单测（probe_snapshot_multiset_face 三态）＋G-2 双向守卫＋js_wrapper 两件＋值守卫＋symlink 跳过集成件；e2e 全族回归（first().copied 断言迁移）；评审快核 CONFIRM 后发版 v0.30.2
---

# REQ-021：休眠项与 REQ-020 记档全清

## Scenario

总台令「休眠（旧批）REQ-020 记档 做完」后追加「解决完后发个 0.30.2」：九项一次清（探针多槽、预热、双向守卫、G6/G7 三件、G3 两件、注入收口、B 通道文档）。

## Criteria

- [x] 探针多槽：`OpProbe` 多重集语义（set 入列/clear 消己份），timeout/stopped 的 op 三态（null/字符串/数组）；e2e 断言迁移
- [x] G-3 预热：daemon 启动后台线程建表，首错零收割时延
- [x] G-2 双向守卫：源抽取（顶层臂缩进判）正反两向
- [x] G6：js_wrapper 两件（形状/verbatim）
- [x] G7：空管道提示、Drop 清账 10 秒上界、object-at 边界 ADR 注记
- [x] G3 存量：list 跳符号链接（集成件锁）、keygen 备份修剪
- [x] 注入收口：--vars 值占位前缀硬拦（exit 2）
- [x] B 通道：http-api 受支持口径与边界
- [x] 版本 0.30.2（总台定版）

## Notes

- 探针多槽的 clear_op 需带名（并发下只消自己那份）；16 处调用点配对迁移
- op JSON 形：单在飞字符串不变（既有消费者零感），多在飞数组是新面
