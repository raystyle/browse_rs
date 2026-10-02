---
id: REQ-016
title: rune 整体超时回执结构化（最小 CTA）
status: implemented
priority: should
trace: 单测三件（server 超时回执结构面、browse-rune 探针快错清空、client 侧行为经实弹 CLI 冒烟）；e2e rune_timeout_probe_retains_inflight_op（真引擎，悬挂 js 的 awaitPromise 腿上外层 3 秒界超时后探针保留 op=js）；战役台沟通单三轮定形（字段集/45 秒裁定/最小批先行）与 #68 验收环境同源
---

# REQ-016：rune 整体超时回执结构化（最小 CTA）

## Scenario

战役台 agent 壳退役外部 GNU timeout 兜底后，引擎内部超时回执成为唯一安全网：原超时报错是纯文案（无在飞调用面、无结构化字段），上层拿到后无法程序化甄别卡点与换通道重试。同窗口期排查坐实 #69（CLI stdin 无界读）才是当日「双 eval 必挂」真凶（平凡脚本加开而不关 stdin 同样必挂；双 eval 原形加 </dev/null 在战役台实环境 4/4 通），本批是引擎内 CTA 先行件。

## Criteria

- [x] `/rune` 整体超时（`BROWSE_EVAL_TIMEOUT`，默认 300 秒不变）的 `ok:false` 帧附 `timeout` 对象：`op`（在飞调用面；执行间隙为 null）、`elapsed_ms`（已等待毫秒）、`bound_s`（上界秒）；`error` 文案保持 CTA 形（前缀律同 err_response）
- [x] 在飞面探针：browse-rune cdp 模块十五个异步函数入口置名、正常退出（含快错）清空、future 被丢弃（整体超时/中止）不清空即保留；`OpProbe` 经 `RunOpts`/`ModuleCtx` 装配，daemon 持克隆在超时臂读取
- [x] daemon 日志行：超时臂以一行 JSON 写 `{"runeTimeout":{op,elapsedMs,boundS}}`（与 daemon.log 既有 JSON 行同形）
- [x] CLI `browse rune` 错误道：响应含 `timeout` 对象时一行 JSON 打 stderr，进程非零退出；人读错误行照旧
- [x] 默认值与既有契约零动：不新增旋钮、不改 `/eval`、不改任何超时数值
- [x] 测试：server 单测锁回执形（op/elapsed_ms/bound_s 与 null 臂）；browse-rune 单测锁快错清空；e2e 真引擎锁「超时丢弃后探针保留在飞面」（悬挂 js awaitPromise，外层 3 秒界 < 单调用 30 秒界）

## Notes

- 终裁（2026-10-02，ADR-0010 增补在册）：`BROWSE_RUNE_TIMEOUT` 独立旋钮，rune 面默认 45 秒；`/eval` 的 `BROWSE_EVAL_TIMEOUT` 300 秒契约零动；上界是 daemon 级（实例内全部 rune 执行同界），长爬（逐页 goto 累计分钟级）会触顶，按需调大或拆专用命名实例。实现随批次二（v0.27.0），本批不改数值
- 探针粒度定谳：函数级（不是单 CDP 调用级）：goto 的导航与 readyState 轮询之间不闪 None，超时报的「卡在哪个调用面」语义稳定
- 探针是 std 锁（与 script_id 同律：drop 兜底道不可 await/blocking_lock）；锁中毒即读 None，不掩盖脚本结果
- 本批不覆盖 #69（CLI stdin 无界读，战役台已 </dev/null 缓解）与 #70（哈希无名报错），两件随批次二
