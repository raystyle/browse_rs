---
version: 1
links:
  - target: sources/diary/2026-10-10-con02三层建与六代同步
    relation: derived_from
  - target: sources/diary/2026-10-09-155发布线与油猴文档批
    relation: derived_from
---

# R103 模式:E2E 环境互扰,跨宿主抢 daemon 与嵌套 job 残留与慢起方差

## PROBLEM

多套 E2E 并行或连跑时,同机/跨机资源互抢产生时红时绿:win CLI 经 mirrored localhost 打进 WSL 的默认 daemon 抢走引擎(linux 门 17 案 7 败、win 门 14 败同源);嵌套 Start-Job 起的源服退出不回收子进程,残留 listener 占端口让下轮服起失败;rune 第二段 evaluate 挂 Promise 占单飞槽时外部 --js dispatch 排队互等死锁。

## ROOT CAUSE

共享资源默认态无隔离:daemon 端口(9880/9222/887x)与引擎 profile 是全局名,并行方各以为是自己的;进程树清理(pwsh job 嵌套)与事件时序(WSL mirrored 网络、job 冷启动)在不同负载下表现不同,方差被误读为产物回归。

## FIX

门跑一律 BROWSE_NAME 隔离加串行,源服平铺 job 加 TcpClient 就绪等待,页面触发一律页面自治(setTimeout 自 dispatch),产物嫌疑先换旧代产物对照定罪。

## Trace

- sources/diary/2026-10-10-con02三层建与六代同步(坑节:三案实档+定罪链)
- sources/diary/2026-10-09-155发布线与油猴文档批(跨宿主首实证)
- commit:ad7f7ce(第五代验收定罪批)、bb87ac2(死锁修批)
