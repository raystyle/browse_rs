# browse-core::workspace

workspace 单仓管理（#50/#51 配套 + 清仓重建总台令 2026-10-09）：站点
与机制知识仓 `github.com/raystyle/browse_workspace` 的安装、更新与仓内
读取。技能本体是「宿主侧 `mech.rn` + 页内侧 `probe.user.js`」双脚本对
（`page-skills/<slug>/` 与 `domain-skills/<段>/` 同形），README 只作人读
说明——list/site/page 只认成对目录，缺一的目录不算技能。

何时用：`browse workspace` 命令族的全部后端（status/install/update/
list/site/page）。git 维护走 shell-out `git`（不引 git crate）：install
是 clone、update 是 `pull --ff-only`，本地修改可手工 commit/push 回推
同一远端。仓根定位见 [`crate::paths::workspace_dir`]。

## Functions

- `domain_segment_files` — 列 `<root>/domain-skills/<段>/` 的对文件清单（成对才有,排序保回执
- `domain_segment_files_multi` — #63 多根段文件列（技能点名口径）：序首有文件的根整胜（不跨根混拼，
- `install` — `browse workspace install`：git clone 种子仓到 `root`。clone 不带
- `list_json` — `browse workspace list` 的机器面：`{domains: [{segment, files}],
- `list_json_multi` — #63 多根清单：domain 段与 page slug 跨根并集（首见序，段名重复以
- `probe_files` — 探针文件形（REQ-041 P0 两段式）：单件 `probe.user.js`，或
- `read_page` — `browse workspace page <slug>`：交出该对（有 README 附全文，无给
- `read_page_multi` — #63 多根 page 读：序首命中根优先。
- `read_site` — `browse workspace site <段>[/<文件>]`：只给段时打该段清单（每行
- `read_site_multi` — #63 多根 site 读：序首命中根优先（根内逻辑同 [`read_site`]）；全根
- `status_json` — `browse workspace status` 的机器面：`{installed, root, gitPresent,
- `update` — `browse workspace update`：脏树先拒（本地修改未提交会挡 fast-forward），

## Constants

- `SEED_REMOTE` — 种子仓远端（install 的缺省源；本地修改 push 回推同一远端）。

