# userscripts：油猴脚本面（引擎 clean-chrome 155+）

油猴（userscript）能力在引擎侧：clean-chrome 155 起内置运行时，browse 只做驾驶。装进引擎 profile 的 `.user.js` 按 match 规则在导航后自动注入，browse 的 `goto`/`--js` 照常工作，注入产物落在同一 DOM 里主世界可读。本指南讲置位、安装、API 面、验收口径与边界；能力判基是 155.0.8059.39 上的 17 案验收全线收口（探针 DOM + 盘上 values 双证，2026-10-09）。

## 1. 引擎置位

```bash
browse chrome install 155.0.8059.39   # 镜像下载（sha256 边车锚校验），缺省即 pin
browse chrome list                    # 验 pin 指到 155
```

- 托管集是机器级共享（不随 `BROWSE_NAME` 走）；旧版保留可 `browse chrome use <版本>` 回退。
- 无头自动化（`--headless`）在 linux 自带加固组（disable-gpu / ozone headless 等，browse 自动叠加），注入稳定性不赖调用方环境。

## 2. 脚本安装两道

1. **引擎管理页** `chrome://userscripts`：列表 / 编辑 / 删除 / 菜单 / 日志五区（有头态手操）。
2. **clean-chrome 置场 CLI** `tools/install-userscripts.ps1`（clean-chrome 仓，pwsh 7）：

```bash
pwsh install-userscripts.ps1 -ChromeProfile <引擎 user-data-dir> -UserJs <脚本路径>
pwsh install-userscripts.ps1 -ChromeProfile <…> -ListOnly     # 只列
pwsh install-userscripts.ps1 -ChromeProfile <…> -Remove <id>  # 卸载
```

脚本 id 同构引擎 `UserscriptStore::MakeId`（脚本源 SHA256 前 6 字节 hex）。勿手构 store JSON，id 与登记形以置场工具为准。

## 3. match 与注入档

- match 是 http/https 的 URLPattern 官方语法；本地断言页直配 `http://127.0.0.1:<port>/*`。
- `data:` / `file:` scheme 不中（严格语义，与域策略 allow-list 同口径）。
- 注入档 document-idle：页面起后到 idle 之间注入，断言前要给 settle 窗（见第 5 节）。

## 4. GM API 面（155 验收基线）

| 面 | 形 |
| --- | --- |
| GM_info | `scriptHandler` 是字符串 `"Chromium"`；`version` 即引擎版本 |
| 存储族 | GM_setValue/getValue/listValues/deleteValue；盘面 `<profile>/Default/Userscripts/values/<id>.json`；down 不删 profile，重启持久 |
| GM_xmlhttpRequest | 跨域 XHR 通道 |
| GM_openInTab | 返回 handle；新页同受注入 |
| GM_registerMenuCommand | 注册项进管理页菜单区 |
| GM_getTab/saveTab | 页签数据往返（含 `__tabId`） |
| GM_setClipboard | 两参 fire-and-forget 契约（第三参回调非契约面） |
| GM_notification | ondone 回调可达 |

OS 效果（剪贴板真写入、通知真弹出）归带桌面端手验；无头端只验 API 契约。

## 5. 验收口径（四轮对线沉淀）

- **探针 DOM 属性主断言**：脚本体 `document.documentElement.setAttribute('probe', JSON.stringify(…))`，`browse --js "return document.documentElement.getAttribute('probe')"` 直读（跨世界共享）。断言目标物本身，不数随服务端 body 漂移的页面形态属性。
- **盘上 values 比对管存储族**：页面读数与 `values/<id>.json` 双证。
- **settle-单读铁律**：禁止高频 `browse --js` 轮询等注入，每次求值即 DevTools attach 自扰，拖慢 timer/mojo 与 document-idle 注入。settle 静默（约 3s）后单读，至多补读一次。
- console 仅辅助（daemon 缓冲有开域前丢消息 caveat）。

## 6. 事件回调（上报给 browse）

脚本可以做页面内的**事件探针**：什么网页、什么 js 事件、什么请求载荷，主动回调通知 browse，agent 侧 `browse events` 消费（区别于轮询/快照拉取）。链路：脚本 `GM_xmlhttpRequest` POST daemon 的 `/us-event`（GM 面绕 CORS，页面 `fetch` 直调会被 CORS 拦，走 GM 是文档口径；daemon 缺省 `127.0.0.1:9880`，`BROWSE_PORT`/命名实例派生端口注意）。

```js
// ==UserScript==
// @name         page-event-reporter
// @match        http://127.0.0.1:8871/*   // 端口级锚定,勿宽配到全部本机页
// @connect      127.0.0.1          // 必须与上报 URL 的主机一致(127.0.0.1 ≠ localhost)
// @grant        GM_xmlhttpRequest  // clean-chrome 运行时可省,移植到扩展管理器必带
// @run-at       document-idle
// ==/UserScript==
(function () {
  function report(name, data) {
    GM_xmlhttpRequest({
      method: 'POST',
      url: 'http://127.0.0.1:9880/us-event',   // 命名实例注意换派生端口,主机不变
      headers: { 'Content-Type': 'application/json' },
      data: JSON.stringify({ name: name, url: location.href, data: data }),  // data 恒字符串
      onerror: function () { /* daemon 不在跑时静默失败,需要感知就打 console */ }
    });
  }
  report('page-ready', { title: document.title });
  document.addEventListener('click', function (e) {
    report('dom-click', { tag: e.target.tagName });
  }, true);
})();
```

实践要点(X 上真实踩坑提炼，2026-10 调研)：

- **探针选形**：耐用的形是「一次 `page-ready` + 钩 `fetch`/`XMLHttpRequest` + `MutationObserver`」,不是全页 click 捕获:环只有 1000 条/单条 32KB,高频事件会把旧记录挤掉；去重/节流放脚本侧做(本地缓存记已上报键)，消费端不设批量接口。
- **通道纪律**：走 `GM_xmlhttpRequest` 不走页面 `fetch`(后者被 CORS 拦，给 daemon 加 CORS 头是错方);`@connect` 按主机授权，与 URL 主机不一致仍被拦；不要把本机回环配进 HTTP 代理(代理带走回环表现为「daemon 明明在却上报失败」)。
- **Worker 边界**：Web Worker 里没有 GM 函数，要 `postMessage` 回主线程再上报。

消费面：

```bash
browse events                    # 最近 20 条(回 {ok, events, nextSeq})
browse events --since 41         # 增量游标(nextSeq 作下一轮 since)
browse events --last 0           # 全部
browse events --clear            # 清环(序号不回卷,游标不失位)
```

事件形 `{seq, ts_ms, name, url, data}`；环容量 1000 满丢最旧，单条 data 超 32KB 截断（`__truncated` 标记），daemon 不在跑时 `browse events` 不算错（环随 daemon 生命周期）。**边界**：本面只承载事件上报与消费；脚本安装管理归引擎面（`Userscript.*` CDP 域，clean-chrome 批 4），两者正交不重复。

## 7. 边界与后续

- `browse userscript` 命令面**尚无**：挂引擎 `Userscript.*` CDP 协议域（clean-chrome 批 4 在账），域落地后 browse 开命令面 REQ 薄封装；当前脚本置场走上两道。
- 脚本面随 profile 走：`--profile <dir>` 换 user-data-dir 时 Userscripts 目录随迁，固定引擎 profile 的脚本对自定义 profile 不生效。
