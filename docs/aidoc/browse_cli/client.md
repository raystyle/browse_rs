# browse-cli::client

daemon 客户端：本地 HTTP 调用 + 首次使用自动拉起 detached daemon。

## Functions

- `cdp_call` — /cdp 直通（#80）：任意 CDP 域方法经 daemon 转发引擎会话，回
- `daemon_alive` — 探测 daemon 是否在跑：GET /health 通即为在。
- `daemon_bind` — 返回 daemon 监听的 `host:port` 串；多实例由 `BROWSE_NAME` 派生端口（ADR-0006）。
- `daemon_spec_profile` — daemon 记录 spec 的 profile 面（`/health` 的 `spec.profile`，#84 下腿）：
- `decode_arg_b64` — 解码 `-b/--b64` 通道的片段实参（#22）：标准 base64 解码为 UTF-8
- `engine_restart` — /engine/restart（#84）：引擎级重启——daemon 按「最近一次 up 的记录
- `engine_up` — 拉起引擎（`browse up` 面）：POST /engine/up 并校验 ok 回包。
- `ensure_daemon` — daemon 不在跑时 detached 拉起 `browse --serve` 并探活到通；已在跑则直接返回。
- `ensure_daemon_with_env` — 同 [`ensure_daemon`]，但给新拉起的 daemon 进程注入环境变量
- `eval` — 把方言片段 POST 到 daemon 的 /eval 求值。
- `events_clear` — 清空油猴事件环（#77）：`DELETE /events`（序号不回卷，消费游标不失位）。
- `events_ring` — 消费油猴事件回调（#77）：`GET /events`，`since` 游标与 `last` 条数
- `expand_at_args` — `@` 文件实参展开（#64）：片段实参以 `@` 开头即读文件内容为片段体，
- `health` — GET /health 取 daemon 状态面；daemon 不在时报可照抄的拉起提示。
- `load_template` — 模板源解析（#66）：`--vars` 模板道的 @ 实参解析序——scripts 根
- `load_template_ext` — [`load_template`] 的扩展名与语境标签变体（#67：Rune 脚本用 `.rn` 与
- `parse_vars_arg` — 解析 `--vars` 单值（`键=值`）：键是标识符（字母或下划线开头，字母
- `quit` — POST /quit 退 daemon，daemon 侧顺带只终结自起引擎（附着来源不动）。
- `render_js_template` — JS 族模板渲染（方言片段与 `--js` 小程序，G-1 对齐批）：先抽出注释
- `render_rune_template` — rune 面模板渲染（批次二 r# 裸串、批次三注释惰性化）：先抽出 r# 区
- `render_template` — `--vars` 模板渲染（#66 命令小程序）：Jinja 语法渲染模板源，`{{ 键 }}`
- `render_template_with` — [`render_template`] 的定界形态参数版（0.30.3：四面同源语法选择）。
- `rune` — 把 Rune 编排脚本 POST 到 daemon 的 /rune 执行（账本 #67）。
- `rune_stop` — 主动停止在跑的 rune 执行（批次二总台指令）：POST /rune/stop，回
- `skills_passthrough_env` — 技能层透传环境（#50/#51）：拉起新 daemon 时把 CLI 进程里的 workspace
- `state_dir` — 返回 daemon 日志与运行面目录（`%USERPROFILE%\.browse-rs[\<name>]`，多实例各一份）。
- `userscript_eval` — 拉起引擎（`browse up` 面）：POST /engine/up 并校验 ok 回包。

## Types

- `UpParams` — `engine_up` 的参数面：CLI up 旗标直通（字段名即 /engine/up 请求键）。

## Constants

- `DEFAULT_PORT` — daemon 的缺省端口，`BROWSE_PORT` 与 `BROWSE_NAME` 派生端口可覆盖。

