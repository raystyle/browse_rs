# browse

[![CI](https://github.com/raystyle/browse_rs/actions/workflows/ci.yml/badge.svg)](https://github.com/raystyle/browse_rs/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/raystyle/browse_rs)](https://github.com/raystyle/browse_rs/releases)
[![License](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-blue)](LICENSE-MIT)

业界唯一把自研 Chromium 内核、按油猴脚本标准实现的 userscript 运行时、以及 Rune 图灵完备编排做成同一条驾驶面的浏览器 CLI。常驻 daemon 保存会话、变量、活动 tab 与元素引用。绝不关闭用户自己的浏览器。

自研内核为 clean-chrome（源码级补丁，不提交上游，不是日常浏览器用的 Chrome）：

- 按油猴脚本标准实现的运行时：`// ==UserScript==` 元数据与 GM_*，按 `@match` 注入用户数据目录
- 远程调试免确认（`AcceptDebugging` 直接放行），默认调试口 9222
- 调试通道可选端口、管道或二者；`--remote-debugging-address` 绑定监听地址，缺省 `127.0.0.1`
- 去掉不受支持的命令行黄条，`--no-sandbox` 启动无提示条
- Console / 未捕获异常经 CDP 上报时不急切序列化对象预览
- auto-attach 下 `window.open` 新标签不等调试器（`ShouldThrottlePopups` 恒 false，gen8-r7 起；配合 browse 0.35.6 客户端 resume 补发双道闭合）
- 由 `browse chrome install/update/use/list/remove/doctor` 装管，镜像 https://chrome.ohmygh.com

驾驶面分三层：

- **命令方言**：`browse '<片段>'`。单步原语，不含运算符与控制流。原生 CDP：`session.<Domain>.<method>`（652 个方法）。封装：`goto`、`snapshot`、`clickRef` 等。
- **Rune**：`browse rune`。图灵完备编排（循环、分支、异步等待）。入口 `pub async fn main()`（Rune 语言不支持返回类型注解，错误传播用 `?`、回值用 `Ok(..)`/`Err(..)`）。原生 CDP：`cdp::raw("Domain.method", params)`。封装：`cdp::goto`、`cdp::js`、`cdp::dialect`（方言全量桥，含等待与错误处理）；另有 `eval`/`eval_handle`/`call`/`release`（隔离世界句柄族）、`js_lib`（库内小程序）、`wait_event`（事件驱动等待）、`requests`/`snapshot`/`request_detail`（三支柱同源）、`render`/`stdin`。宿主模块 `json` 与 `time`（`time::sleep` 可用）。模板变量：`--vars k=v`（可重复），与 `@` 脚本同用时按 Jinja 渲染，默认定界 `<%= 键 %>`。
- **管道**：stdin 作为输入。无实参时 `cat snippet.js | browse --js`、`cat plan.rn | browse rune` 将管道内容作为脚本；有实参时管道内容作为 rune 的 `cdp::stdin()`。

原生 CDP 与封装并存。列出原生方法：`browse cdpMethods`。发送一条原生 CDP：`browse cdp <Domain.method> [--params JSON]`。封装在原生调用上补齐导航等待、`errorText`、无障碍短 ref，不替代原生面。

其余能力：

- userscript：`browse userscript install` 写入引擎用户数据目录；更改后加 `--restart`
- 页面原语：snapshot/screenshot/pdf、clickRef/fillRef、路由拦截与假应答
- workspace：域名段与页面 slug 在 `goto` / `fetch` / `detect` 回执中附加 `domain_skills_hint` / `page_skills_hint`；探针 `see` 事件另生成 `cta`
- 附着优先：有本机浏览器就附着，否则 spawn 隔离实例
- `--llms` 手册、错误带可照抄下一步、`browse update` 自升级

不做 MCP；不做 Cloud browser。

## 部署

- GitHub Releases：https://github.com/raystyle/browse_rs/releases
- 镜像：`https://browse.ohmygh.com/browse/<版本>/<资产>`，同名 `.sha256`
- 源码：`cargo install --path crates/browse-cli --force`
- 已装：`browse update`

| 平台 | 资产 |
| --- | --- |
| Linux x86_64 | `browse-v<版本>-x86_64-unknown-linux-gnu.tar.gz` |
| Windows x86_64 | `browse-v<版本>-x86_64-pc-windows-gnu.zip` |
| macOS arm64 | `browse-v<版本>-aarch64-apple-darwin.tar.gz` |

Windows 为 win-gnu 静态 CRT。Linux 需 `libnss3 libasound2t64` 族。

## clean-chrome

当前 latest：`155.0.8059.39-gen8-r7`

指针：https://chrome.ohmygh.com/latest

三平台 zip 与 `.sha256` 边车均在位。

```bash
browse chrome update
browse chrome install 155.0.8059.39-gen8-r7
```

| 平台 | 包 |
| --- | --- |
| Linux x86_64 | https://chrome.ohmygh.com/155.0.8059.39-gen8-r7/chromium-155.0.8059.39-gen8-r7-x86_64-unknown-linux-gnu.zip |
| Windows x86_64 | https://chrome.ohmygh.com/155.0.8059.39-gen8-r7/chromium-155.0.8059.39-gen8-r7-x86_64-pc-windows-msvc.zip |
| macOS arm64 | https://chrome.ohmygh.com/155.0.8059.39-gen8-r7/chromium-155.0.8059.39-gen8-r7-aarch64-apple-darwin.zip |

边车：同一 URL 加 `.sha256`。

## 配置

| 变量 | 作用 |
| --- | --- |
| `BROWSE_PORT` | daemon 端口（默认 9880） |
| `BROWSE_NAME` | 命名实例：状态目录 + 派生端口 9900-9999 |
| `BROWSE_CHROME` | chrome 路径（发现序：显式、托管 pin、祖先部署、常规路径） |
| `BROWSE_PROFILE` | spawn 的 user-data-dir（默认 engine-profile，down 不删） |
| `BROWSE_CDP_WS` | 钉死 WS URL |
| `BROWSE_NO_ATTACH=1` | 跳过附着，强制 spawn |
| `BROWSE_ENGINE_ARGS` | 附加旗标；无 DISPLAY/WAYLAND_DISPLAY 时有头 spawn 自动补 `--headless` |
| `BROWSE_NO_AUTO_DIALOG=1` | 关掉 alert 自动接受 |
| `BROWSE_EVAL_TIMEOUT` | 单次求值超时秒（默认 60） |
| `BROWSE_ENGINE_CONTEXT=1/0` | 强制开/关操作回执的引擎身份行（stderr 的「引擎：实例…」行，与 status 同一词汇）；默认只在 stderr 是终端时显示，管道与 agent 安静，机器面在回执信封 `engineContext` |
| `BROWSE_DENY_DOMAINS` / `BROWSE_ALLOW_DOMAINS` | 域策略，deny 优先 |
| `BROWSE_CHROME_MIRROR` / `BROWSE_CHROME_ASSET` | 引擎镜像与资产名 |
| `BROWSE_CHROME_LATEST` | `chrome update` 发现源钉 |
| `BROWSE_RELEASE_MIRROR` | `browse update` 镜像覆写 |
| `BROWSE_WORKSPACE` | 技能仓根（默认 `~/.browse-rs/workspace`） |
| `BROWSE_DOMAIN_SKILLS=0` / `BROWSE_PAGE_SKILLS=0` | 关闭回执中的域名 / 页面技能提示 |

状态目录 `~/.browse-rs/`：`daemon.log`、`engine.log`、`engine-profile`、`chromium/`。

## 使用方法

```bash
browse up --headless
browse 'await newTab("https://example.com")'
browse 'return await snapshot()'
browse 'await fillRef("e2", "hello")'
browse 'await clickRef("e3")'
browse --js @snips/title.js
cat title.js | browse --js
browse rune page-skills/x-search/mech.rn --vars q=OpenAI --vars rounds=2
cat plan.rn | browse rune
browse rune @x.test/fill-form --vars q=hello
browse 'return await screenshot()'
browse 'await routeBlock("*://ads.example.com/*")'
browse fetch https://example.com
browse fetch https://example.com --markdown
browse cdpMethods
browse cdpMethods Network
browse cdp Runtime.evaluate --params '{"expression":"1+1"}'
browse workspace install
browse chrome update
browse issue new "标题" --body "复现步骤"
browse down
```

手册：`browse --llms`。命令方言无运算符与控制流；页面逻辑用 `browse --js` 或 `Runtime.evaluate`；多步流程用 Rune。退出码 0/1/2。

`cargo test --workspace`。真 chrome：`BROWSE_E2E=1 BROWSE_NO_ATTACH=1 cargo test -p browse-core --test e2e`。契约见 AGENTS.md。
