# browse-core::userscript_store

油猴脚本盘上契约管理（REQ #79）：对引擎 `<profile>/Default/Userscripts/`
的同构读写——安装、列表、启停、删除、换源、路径输出。

引擎（clean-chrome `UserscriptStore`）启动时读盘，装改后需引擎重启生效
（CLI `--restart` 自动 down/up）；批 4 `Userscript.*` CDP 域落地后升级为
域优先、盘上兜底。id 与引擎 `MakeId` 同构：`SHA256("namespace@name")`
前 6 字节 hex（12 字符）。

## Functions

- `install` — 安装一个或多个 `.user.js`：写 `scripts/<id>/{meta.json, script.user.js}`，
- `list` — 列表：对读 index.json,并按 workspace `page-skills/<slug>/probe.user.js`
- `make_id` — 脚本 id：`SHA256("namespace@name")` 前 6 字节 hex，与引擎 `MakeId` 同构。
- `parse_header` — 解析 `==UserScript==` 头；无头块时按全缺省 meta 返回（仍可安装，等于
- `remove` — 删除一个脚本：index 条目与 `scripts/<id>/` 目录同删。
- `script_body` — 剥头后的脚本体（头块连同收尾标记剔除；无头块返回原文）。
- `script_path` — 输出脚本体文件路径（编辑入口：改后 `update` 同 id 回写或直接编辑该文件）。
- `set_enabled` — 启停一个脚本（改 index 条目 `enabled`）。
- `update` — 同 id 换源：以新文件内容重写 meta 与脚本体（name/namespace 由新头决定，
- `userscripts_root` — 引擎侧 Userscripts 目录（Chrome profile 下的固定相对位）。

## Types

- `ScriptMeta` — `==UserScript==` 头解析结果（缺省语义同置场 CLI：无 `@name` 用文件基名，

