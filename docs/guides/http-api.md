# /eval HTTP API 公开契约（#45）

> browse 的 daemon 本就是一个 HTTP 面（ADR-0001）。本档是它的公开契约：方言做线协议，门外语言不限（curl、PowerShell、Bash、Python、Node、agent 本体都可以直接驱动），零 SDK 依赖；各语言薄封装只是可选便利层，不是官方路线。

## 端点

| 方法 | 路径 | 用途 |
|---|---|---|
| POST | `/eval` | 求值一段方言片段（或 `js: true` 时全量 JS） |
| POST | `/rune` | 执行一段 Rune 编排脚本（宿主侧全控制流，隔离世界碰页面） |
| POST | `/rune/stop` | 主动停止在跑的 rune 执行（幂等；stopped 面回执见下） |
| GET | `/health` | 探活与状态（就绪即 200，响应形见下） |
| POST | `/engine/up` | 显式起引擎 |
| POST | `/quit` | 退 daemon（只杀自起引擎） |

## GET /health 响应形

```json
{ "ok": true, "name": "default", "uptime": 1656, "connected": true,
  "activeTargetId": "46BC…", "activeSessionId": "FF6D…",
  "daemon": { "os": "linux", "hostname": "AI-LAB", "pid": 2085366,
      "name": "default", "startedAt": 1790097761849 },
  "engine": { "Spawned": { "pid": 267718, "ws_url": "ws://127.0.0.1:43877/…",
      "profile_dir": "…", "chrome": "…", "headless": false, "channel": "port",
      "spawned_at": 1790097761849 } },
  "engineProvenance": { "origin": "managed-spawn", "hostContext": "linux/AI-LAB",
      "spawnedBy": "daemon", "spawnedAt": 1790097761849 } }
```

`engine` 三态（单键包裹的枚举形）：`{"Spawned":{pid,ws_url,profile_dir,chrome,headless,channel,spawned_at}}`（自起）、`{"Attached":{ws_url}}`（附着）、`{"NotConnected":null}`。`daemon` 是 daemon 自描述（os/hostname/pid/name/startedAt）；`engineProvenance` 是来源语义面（origin 枚举 attached/managed-spawn/isolated-spawn、hostContext、spawnedBy、spawnedAt；attached 态 loopback 宿主显式降级标注）。门外 agent 用它判「引擎在不在、活动 tab 是哪个、引擎在谁的宿主上」。

## POST /rune 请求体与响应（Rune 编排面）

```json
{ "code": "pub async fn main() { cdp::goto(\"data:text/html,<title>t</title>\").await?; Ok(cdp::eval(\"document.title\").await?) }" }
```

脚本在 daemon 侧独立 VM 执行（Rune 语言，入口恒 `pub async fn main()` 且返回 `Result`；`?` 传播 `cdp::CdpError`）。宿主只装 json/time/cdp 三模块；cdp 模块八函数 `goto`/`eval`/`eval_handle`/`call`/`release`/`click`/`type_text`/`wait_sel`，全走自有 CDP 会话，Chrome 只经隔离世界（worldName `browse-rune`）`Runtime.evaluate` 碰页面。双限额（指令一千万、分配 16MB）；对象组执行收尾整组释放。

八函数签名（`Result` 的 `E` 恒 `cdp::CdpError`）：

| 函数 | 形 | 说明 |
|---|---|---|
| `goto(url)` | `-> Result<String>` | `Page.navigate` 加等 readyState 脱 loading（上限 15s）；返回 frameId |
| `eval(expr)` | `-> Result<Value>` | 隔离世界求值（`awaitPromise` 加 `returnByValue`）；`() ↔ null`、对象/数组直桥 |
| `eval_handle(expr)` | `-> Result<ObjHandle>` | 同上不取值，返回不透明句柄（对象组记账） |
| `call(handle, fn_src)` | `-> Result<Value>` | `Runtime.callFunctionOn`；`fn_src` 用 `function(){…}` 形收 `this`（箭头函数是词法 this） |
| `release(handle)` | `-> Result<()>` | 释放单个句柄 |
| `click(sel)` | `-> Result<()>` | 隔离世界定位取中心点，`Input.dispatchMouseEvent` 真实点击；未命中报错 |
| `type_text(sel, text)` | `-> Result<()>` | 聚焦后 `Input.insertText` 插在插入点（不清空原值）；未命中报错 |
| `wait_sel(sel, ms)` | `-> Result<bool>` | 百毫秒轮询，超时返 `false` 不报错（脚本自裁 `Timeout`） |
| `js(src, args)` | `-> Result<Value>` | 内联 JS 小程序体（**主世界**执行、页面函数可调）；args 对象 JSON 字面量注入（引号天然安全）；体须用 `return` 出值（裸表达式得 null，与 `--js` 的完成值语义不同） |
| `js_lib(name, vars)` | `-> Result<Value>` | 直调 snippets 库内 `.js` 小程序（名原样与补 `.js` 两试），vars 做 Jinja 渲染（#66 全同管线）后主世界执行 |
| `render(tpl, vars)` | `-> Result<String>` | 运行时 Jinja（Strict；未填键走 `CdpError{op:"render"}`） |
| `stdin()` | `-> String` | 管道参数道捕获串（CLI 有实参时管道串经请求体 stdin 字段递入；无管道空串） |
| `requests(opts)` | `-> Result<Value>` | 交互请求对账（与方言 `requests()` 同源同参：since/filter；Network 事件环形 1000 条） |
| `snapshot(opts)` | `-> Result<Value>` | ax 树快照（与方言 `snapshot()` 同源同参：ref/depth/pierce；返回 url/title/nodes） |
| `request_detail(sel, opts)` | `-> Result<Value>` | 单请求详情（index 或 requestId；响应头含 Set-Cookie 可取） |
| `cookies()` | `-> Result<Value>` | `Network.getCookies`（当前会话 cookie 全量） |
| `set_cookie(name, value, domain)` | `-> Result<()>` | `Network.setCookie`（path 恒 /；拒设报错） |
| `wait_event(method, ms)` | `-> Result<Value>` | 事件驱动等待（原生 `Session::wait_for` 直出：活动 tab 与 browser 级事件，取出即移除，含缓冲内历史；超时 `Err(CdpError)`，脚本 `match` 即条件分支） |

边界：函数都在主 frame（iframe 内不可编排，frame 形后续批）；执行器会自动追加 `__browse_rune_wrap__` 包装函数（用户脚本请勿用同名）。输入三通道：`browse rune <script|@名>`（文件/实参道）、`cat plan.rn | browse rune`（管道即源码）、`browse rune @名 < data`（管道即参数，脚本内 `cdp::stdin()` 读）。请求体可选字段 `root`（js_lib 的片段库根，旗标 > env > 内置）与 `stdin`。快错面（修复批）：全部 CDP 调用 30 秒上界、同源宿主腿 60 秒上界、WS 握手 15 秒上界；引擎忙（另一请求持执行槽）排队 10 秒过界即报错不无界等。单请求理论最坏 ≈ 引擎就绪 + 10 秒队列 + min(300 秒总超时, N×30 秒调用)；外部判挂建议 ≥60 秒，长编排调 `BROWSE_RUNE_TIMEOUT`（rune 面专属）。

响应 envelope 与 `/eval` 同形（`ok`/`value`/`engineContext` 或 `ok:false`/`error`）；`value` 是 rune 回执：`{"ok": <main 成功载荷>}` 或 `{"error": <脚本可见失败载荷>}`（脚本自造 `Timeout` 型、`cdp::CdpError{op,message}` 都属后者）。编译错、VM 终态（含限额击穿）、超时走 `error` 面。细节与脚本写法见 `browse --llms` 的 rune 条目与 crates/browse-rune 文档。

超时回执（v0.26.1 起，批次二换 rune 专属旋钮）：整体超时的 `ok:false` 帧除 `error` 文案（带下一步 CTA）外，附机器可读 `timeout` 对象：`op`（在飞调用面，如 `"js"`/`"goto"`；执行间隙为 null）、`elapsed_ms`（界值回显毫秒，恒等于 bound_s×1000，非独立秒表）、`bound_s`（上界秒，即 `BROWSE_RUNE_TIMEOUT`，默认 45，`/eval` 的 `BROWSE_EVAL_TIMEOUT` 300 秒契约零动）；daemon 同字段写一行 JSON 进日志。CLI 侧 `browse rune` 把该对象以一行 JSON 打到 stderr 后非零退出，上层 agent 可直接解析换通道重试，无需外部 timeout 兜底。探针是单槽：脚本经 `future::join` 并发在飞多个 cdp 调用时 `op` 可能为 null 或非卡点面（默认串行 await 面不受影响）。

### 停止与检测联动（批次二：事件等待、主动停止、超时同一 CTA 面）

主动停止三道，回执同构（`stopped` 对象：`op` 在飞调用面、`elapsed_ms` 实测毫秒、`cause` 二分流）：

- **请求停**：`POST /rune/stop` 或 CLI 动词位 `browse rune stop`（回 `{ok, running, requested, note}`，幂等：无在跑执行空放）；被停执行回 `ok:false` + `stopped{cause:"stop_request"}`，CLI stderr 一行 JSON 后非零退出
- **引擎失联联动停**：执行期两次连续连通性探测失败（各隔 300 毫秒，容忍瞬时重连窗）即停，`stopped{cause:"engine_lost"}`，CTA 指 `browse status` 看引擎态重试；**可达面**：fail-fast 先到即走：CDP 等待腿悬挂时单调用 30 秒界先到（走脚本错误面），engine_lost 兜非 CDP 等待段（长 `wait_event`、纯计算循环等）
- **脚本侧原生自停**：不自封装原语，脚本用语言原生控制流（`select`/分支 + `Err` 返回或早退），error 面自带 CTA；自定义检测代码的典型形：`match cdp::wait_event(m, ms).await { Ok(ev) => …, Err(_e) => 停止分支 }`
- **动词位撞名披露**：脚本名恰为 `stop` 且无 `--vars` 时会被当停止动词；用 `@stop` 或加 `--vars` 走脚本道
- `stopped.elapsed_ms` 是**实测**毫秒（起跑计时；对照 timeout 面的 `elapsed_ms` 是界值回显）

事件驱动等待：`cdp::wait_event(method, ms)`（上表）直出原生事件面；方言的 `session.waitFor`/`waitForResponse`/`waitLoad` 家族同源。超时机制：整体 `BROWSE_RUNE_TIMEOUT`（45 秒）＋单 CDP 调用 30 秒＋同源腿 60 秒＋引擎忙 10 秒快错，全部结构化回执。

```bash
# 主动停在跑执行
curl -s localhost:9880/rune/stop -X POST
# -> {"ok":true,"running":true,"requested":true,"note":"停止信号已投递…"}
```

### r# 裸串与 Rune std 差异（批次二）

- **r# 裸串**：`r#"…"#`（Rust 同形；内容含 `"#` 时升 `r##"…"##`，逐级类推）编译前重写为转义普通串：任意引号、反斜杠、多行 JS 原样直达 `cdp::js`/`cdp::eval`；模板道（`--vars`）下 r# 区**免渲染**（`{{`/`{%` 原样，与 `{% raw %}` 各管一层：raw 管 Jinja 面任意区，r# 管 Rune 串字面量面）；换行原样保真（编译诊断行号不漂）
- **Rune 与 Rust std 差异（#70 披露口径）**：String 无 `to_string`（字面量已是 String：直返或 `String::from`）；无 `splitn`（`"a=b".split_once("=")` 回 `Option<(String,String)>` 是 kv 形正形；`split` 全拆可用）；值被移交后再读报 `Cannot read/take, value is M-…`（哈希址无变量名，上游诊断限制；正形是 `clone(h)` 复用句柄）；实例函数缺失报哈希无名（常见名查表翻译留后续批）；直桥返回的原生类型（i64/String/bool）不带 Value 实例方法面（`as_i64`/`as_str` 只在 Value 上），要方法面在脚本内保持对象形

```bash
curl -s localhost:9880/rune -H 'Content-Type: application/json' \
  -d '{"code":"pub async fn main() { Ok(42) }"}'   # -> {"ok":true,"value":{"ok":42},…}
```

## POST /engine/up 请求体

字段全可省（走发现序）：`headless`（bool）、`chrome`（路径）、`ws`（钉死 WS URL）、`port`、`pipe`（bool）、`profile`、`proxy`、`proxy_bypass`、`isolated`。失败同 `/eval` 的 500 加错误形。

## POST /eval

请求体（JSON）：

```json
{ "code": "return await currentTab()", "new_tab": false, "js": false }
```

- `code`：方言片段（缺省）或 JS 源码（`js: true`，等价 CLI 的 `--js` 面）
- `new_tab`：先开 about:blank 新 tab 再求值（等价 `--new-tab`）
- `js`：走全量 JS 受限旁路（#22 口径）

响应（成功，HTTP 200）：

```json
{ "ok": true, "value": { "targetId": "A1B2…", "title": "Example", "url": "https://example.com/" },
  "engineContext": { "provenance": { "origin": "managed-spawn", "hostContext": "linux/AI-LAB",
      "spawnedBy": "daemon", "spawnedAt": 1790097761849 },
    "daemon": { "os": "linux", "hostname": "AI-LAB", "pid": 2085366, "name": "default", "startedAt": 1790097700000 },
    "changes": [] } }
```

`engineContext` 是机器面信封键（方言 `value` 不受扰）：provenance 与 daemon 同 /health 的语义面（来源枚举、宿主上下文、spawn 时刻），changes 是 daemon 侧与上次操作引擎指纹比对出的变化句（引擎换新、有头无头翻转、形态翻转、来源变化；正常态空数组）。CLI 据此在 stderr 出告警行（正常态零输出），`BROWSE_ENGINE_CONTEXT=1` 显紧凑行。

响应（失败，HTTP 500 带错误形；判 `ok` 字段仍是推荐姿势，但客户端要允许从非 2xx 响应读出 body，别用 curl -f 一类「非 2xx 即抛」姿势把 CTA 吞掉）：

```json
{ "ok": false, "error": "方言不支持该语法（if/for/while…）；下一步：…" }
```

错误形带下一步 CTA（browse 全家错误面的统一口径），驱动方可以直接把它转给人或 agent。

## 语义要点

- **单飞槽**：daemon 同一时刻只跑一段求值（eval_lock），并发 POST 按到达序排队执行、不拒。`BROWSE_EVAL_TIMEOUT` 只管自己的求值段（拿到槽之后才起算），排队等待另计：第 N 个并发请求的总时延约等于前 N-1 个耗时之和加自己的求值，排队期间不会超时。客户端要按最坏 N 乘 timeout 设自己的 HTTP 超时，别把排队误判成 daemon 挂了。
- **超时**：单次求值上限 `BROWSE_EVAL_TIMEOUT`（秒，缺省 300），超时回错误形（`eval 超时（N 秒；BROWSE_EVAL_TIMEOUT 可调）`）。
- **懒引擎**：引擎未连且片段不含 connect 时自动拉起（懒拉起），冷启动首求值包含 spawn 时延。
- **状态跨请求保留**：`const` 变量与引用表在 daemon 生命期跨片段持久（多段工作流直接分段 POST）。
- **daemon 发现**：端口 `BROWSE_PORT`（缺省 9880）；命名实例 `BROWSE_NAME` 派生端口 9900-9999。客户端直接 POST 即可：没有 daemon 时 CLI 会自动拉起，但纯 HTTP 客户端需先 `browse up`（或自起 `browse --serve`）。

## 兼容承诺

契约即 HTTP + JSON：只加字段不减字段；错误响应当前恒为 HTTP 500，未来若细化状态码（如 400 参数错、503 引擎不可用），body 形仍恒为 `{ok:false, error}`，客户端只需认 body 不受状态码变化打穿。任意脚本语言可驱动。

## curl 三例

```bash
# 端口缺省 9880（BROWSE_PORT）；命名实例（BROWSE_NAME）派生 9900-9999，实际端口见 browse status

# 1. 探活
curl -s http://127.0.0.1:9880/health

# 2. 求值：navigate + 取 title
curl -s http://127.0.0.1:9880/eval \
  -H 'Content-Type: application/json' \
  -d '{"code": "await goto(\"https://example.com\"); return (await session.Runtime.evaluate({expression:\"document.title\", returnByValue:true})).result.value"}'

# 3. 取快照（限深省 token）
curl -s http://127.0.0.1:9880/eval \
  -H 'Content-Type: application/json' \
  -d '{"code": "const s = await snapshot({depth: 2}); return {url: s.url, nodes: s.nodes.length}"}'
```

## 与片段库的组合（#44）

外置驱动器读片段文件 POST `/eval`，即成「缺 helper 自己存」的闭环：方言做线协议，门外语言不限。

## 测试用例（验收面）

- curl 驱动全链路：`browse up` 后探活、navigate、snapshot 取回（三例各一）
- 错误形：POST 一段语法错误片段，回 HTTP 500 且 body `ok: false`、`error` 带 CTA
- 超时语义：`BROWSE_EVAL_TIMEOUT=1` 下 POST 一段 5 秒等待，1 秒级回错误形
