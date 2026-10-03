---
id: REQ-027
title: 统一兜底超时（--timeout 旗标、60 秒缺省、卡点回执同形；v0.30.7）
status: implemented
priority: must
trace: js_host 探针清空单测＋server 统一界缺省与回执形单测＋arg_contract 旗标名册双向守卫＋surface_contract（exempt 收紧）＋fetch 守卫收编；e2e rune 族 10/10；评审随批
---

# REQ-027：统一兜底超时

## Scenario

超时面沿革分叉（方言 wait 族 #51 秒口径、rune 45 秒、eval 300 秒、fetch 裸解析、/eval 无卡点面）。用户令统一（2026-10-03 三连）：「形成一个统一标准统一命令统一函数的timeout」；「timeout是兜底硬指标和各种事件wait和wait条件命中是不冲突的 可以命名区别」；「timeout 默认60秒」（中途曾拟 180，终裁 60）。决策记录见 ADR-0012。

## Criteria

- [x] 统一标准：兜底硬指标与等待窗正交、命名区别（timeout 字样专属兜底界）；统一缺省 60 秒（rune 45、eval 300 收口）
- [x] 统一命令：全局 `--timeout <s>`（名册与 Options 行入册；值经统一守卫）；rune 与 eval 两道请求级 `timeout_s` 覆写 env 缺省；fetch 的 `--timeout` 收编同守卫；env 语义改「实例内缺省、逐次可覆写」
- [x] 统一函数：`js_host::secs_to_ms` 公开为全域单一换算器；/eval 侧 `current_op` 探针挂 `call_global` 收口（片段道与编程道同覆盖）；`eval_timeout_response` 与 rune 超时回执同形（op/elapsed_ms/bound_s + 卡点 CTA）
- [x] 测试：探针清空单测（正常退出含报错退出清、超时丢弃保留）、统一界与回执形单测、旗标名册双向守卫同步、fetch 守卫

## Notes

- 行为变化面：eval 默认界 300 收紧 60（超 60 秒无参长跑需显式给界）；rune 默认 45 放宽 60；daemon 级「实例内同界」口径改为「env 缺省可逐次覆写」（ADR-0010 增补段的历史口径由本 REQ 修订）
- wait 族参数零动（正交裁定）；rn `cdp::wait_event(method, ms)` 维持毫秒原生形（等待窗面，不属兜底界）
- CLI 全局 `--timeout` 与 fetch 子旗标同名：fetch 分支自持解析（内环先消费），全局臂只作用于 rune/eval 道
- 实弹雷记档：CLI 臂首版把 `secs_to_ms` 的毫秒回值直接塞进秒形请求字段（`--timeout 3` 发 `timeout_s:3000`，兜底界成 50 分钟内窗恒赢）；本地 sink 捕获请求体抓出，修为 `ms/1000` 回秒；fetch 臂同款同修。教训：单测锁不到跨进程值形传递，CLI 到 daemon 的数值字段要 sink 实捕或 e2e 断言界值行为
