# browse

[![CI](https://github.com/raystyle/browse_rs/actions/workflows/ci.yml/badge.svg)](https://github.com/raystyle/browse_rs/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/raystyle/browse_rs)](https://github.com/raystyle/browse-rs/releases)
[![License](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-blue)](LICENSE-MIT)

业界唯一把自研 Chromium 内核和引擎内完整油猴接口做成同一条驾驶面的浏览器 CLI。一条 JS 方言驱动定制 Chrome 核；油猴跑在引擎里，不是商店扩展。常驻 daemon 保存会话、变量、活动 tab 与元素引用。绝不关闭用户自己的浏览器。

- 定制 Chrome 核：`browse chrome install/update/use/remove/list/doctor`
- 完整油猴接口：GM_*、`@require`/`@resource`、document-start/idle
- 方言直调 CDP 652 方法（`session.<Domain>.<method>`）
- 页面原语：snapshot/screenshot/pdf、clickRef/fillRef、路由拦截与假应答
- workspace：油猴探针 + rune 编排，goto/fetch/detect 回执点名
- 附着优先：有本机浏览器就附着，否则 spawn 隔离实例
- `--llms` 手册、错误带可照抄下一步、`browse update` 自升级

不做 MCP；控制流不进方言；不做 Cloud browser。

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

当前 latest：`155.0.8059.39-gen8-r5`（指针 https://chrome.ohmygh.com/latest）。三平台 zip 与 `.sha256` 边车均在位。

```bash
browse chrome update
browse chrome install 155.0.8059.39-gen8-r5
```

| 平台 | 包 |
| --- | --- |
| Linux x86_64 | https://chrome.ohmygh.com/155.0.8059.39-gen8-r5/chromium-155.0.8059.39-gen8-r5-x86_64-unknown-linux-gnu.zip |
| Windows x86_64 | https://chrome.ohmygh.com/155.0.8059.39-gen8-r5/chromium-155.0.8059.39-gen8-r5-x86_64-pc-windows-msvc.zip |
| macOS arm64 | https://chrome.ohmygh.com/155.0.8059.39-gen8-r5/chromium-155.0.8059.39-gen8-r5-aarch64-apple-darwin.zip |

边车：同一 URL 加 `.sha256`。

## 配置

| 变量 | 作用 |
| --- | --- |
| `BROWSE_PORT` | daemon 端口（默认 9880） |
| `BROWSE_NAME` | 命名实例：状态目录 + 派生端口 9900-9999 |
| `BROWSE_CHROME` | chrome 路径（发现序：显式 → 托管 pin → 祖先部署 → 常规路径） |
| `BROWSE_PROFILE` | spawn 的 user-data-dir（默认 engine-profile，down 不删） |
| `BROWSE_CDP_WS` | 钉死 WS URL |
| `BROWSE_NO_ATTACH=1` | 跳过附着，强制 spawn |
| `BROWSE_ENGINE_ARGS` | 附加旗标；无 DISPLAY/WAYLAND_DISPLAY 时有头 spawn 自动补 `--headless` |
| `BROWSE_NO_AUTO_DIALOG=1` | 关掉 alert 自动接受 |
| `BROWSE_EVAL_TIMEOUT` | 单次求值超时秒（默认 60） |
| `BROWSE_DENY_DOMAINS` / `BROWSE_ALLOW_DOMAINS` | 域策略，deny 优先 |
| `BROWSE_CHROME_MIRROR` / `BROWSE_CHROME_ASSET` | 引擎镜像与资产名 |
| `BROWSE_CHROME_LATEST` | `chrome update` 发现源钉 |
| `BROWSE_RELEASE_MIRROR` | `browse update` 镜像覆写 |
| `BROWSE_WORKSPACE` | 技能仓根（默认 `~/.browse-rs/workspace`） |
| `BROWSE_DOMAIN_SKILLS=0` / `BROWSE_PAGE_SKILLS=0` | 关掉域名 / 页面技能点名 |

状态目录 `~/.browse-rs/`：`daemon.log`、`engine.log`、`engine-profile`、`chromium/`。

## 使用方法

```bash
browse up --headless
browse 'await newTab("https://example.com")'
browse 'return await snapshot()'
browse 'await fillRef("e2", "hello")'
browse 'await clickRef("e3")'
browse --js @snips/title.js
browse rune @x.test/fill-form --vars q=hello
browse 'return await screenshot()'
browse 'await routeBlock("*://ads.example.com/*")'
browse workspace install
browse chrome update
browse issue new "标题" --body "复现步骤"
browse down
```

手册：`browse --llms`。方言无运算符与控制流；页面逻辑放 `Runtime.evaluate`。退出码 0/1/2。

`cargo test --workspace`。真 chrome：`BROWSE_E2E=1 BROWSE_NO_ATTACH=1 cargo test -p browse-core --test e2e`。契约见 AGENTS.md。
