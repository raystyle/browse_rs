# 2026-10-02 rune 超时回执结构化（v0.26.1 最小 CTA 批）

## 背景

战役台 CTA 沟通单三轮定形后总台下开工令：v0.26.1 patch = rune 整体超时回执结构化最小集（调用面 op/elapsed_ms/上界秒 + CLI stderr 一行 JSON + 非零退出 + daemon 日志行），零默认值零契约变更；`BROWSE_RUNE_TIMEOUT` 45 秒终裁同步写 REQ-016 与 ADR-0010 增补（实现留批次二）。

同日归因实录（本批的 Scenario 源）：战役台报「双 eval 必挂」，三组对照实测翻案：平凡脚本（零 cdp 调用）加开而不关 stdin 同样必挂（#69 CLI stdin 无界读，读点先于一切 daemon 接触故无日志）；双 eval 原形加 `</dev/null` 在我方实例 3/3、战役台实环境 4/4 通。真凶是 runner 的 stdin 形，不是 eval 次数；战役台已全量加 `</dev/null` 并硬拦外部 GNU timeout（session-tool-guard 0.3），引擎内 CTA 成为唯一安全网。

## 落地

- browse-rune：`OpProbe`（`Arc<Mutex<Option<&'static str>>>`，std 锁与 script_id 同律）经 `RunOpts`/`ModuleCtx` 装配进 `CdpState`；cdp 模块十五个异步函数（goto/eval/eval_handle/call/release/click/type_text/wait_sel/js/js_lib/requests/snapshot/request_detail/cookies/set_cookie）入口 `set_op`、正常退出（含快错）`clear_op`、future 被丢弃不清空即保留在飞面；粒度定谳函数级（goto 导航与 readyState 轮询之间不闪 None）
- browse-core server：`/rune` 超时臂读探针、`rune_timeout_response` 构造 `ok:false` + `error`（CTA 文案）+ `timeout`{op,elapsed_ms,bound_s}；`eprintln!` 一行 JSON `{"runeTimeout":{...}}` 进 daemon.log
- browse-cli client：rune 错误道见响应含 `timeout` 对象即一行 JSON 打 stderr，非零退出照旧
- 文档：surface rune 条目补整体上界与 timeout 面句（重生成投影）、http-api /rune 节超时回执段、CHANGELOG 0.26.1、REQ-016（含旋钮终裁 Notes）、ADR-0010 增补（2026-10-02 节）

## 验收

- 单测：server `rune_timeout_envelope_has_structured_face`（三字段与 null 臂）；browse-rune `op_probe_clears_after_fast_error`（无连接快错 0.07 秒清空）
- e2e 真引擎：`rune_timeout_probe_retains_inflight_op`：悬挂 js（`await new Promise(() => {})`）外层 3 秒界（< 单调用 30 秒界）超时丢弃后探针保留 `Some("js")`，6.74 秒过
- 门禁与发版：见本批回执（评审 browse-codex-review 门禁后推 main）

## 记档

- 批次二队列（时判 v0.27.0；后因看板移除契约破裂改判 0.28.0）：#69 stdin 读上界、`BROWSE_RUNE_TIMEOUT` 旋钮实现、raw 转义与触发边界文档、#70（哈希查表补名或披露）、r# 裸串预处理（词法边界五点案已在战役台回执定形）、std 差异文档（to_string/splitn 缺席、split_once 正形）；发版后去上游 rune-rs #537 贴需求
- 环境注记：默认实例引擎换显式 `--headless`（WSLg 有头 spawn 起得极慢，另三命名实例同因三具 chrome 僵尸）；战役台 psw-campaign 实例健康
