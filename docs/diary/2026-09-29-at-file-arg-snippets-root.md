# 2026-09-29 #64/#65 批（@ 文件实参与 snippets 可配置根）

背景：总台（prs_c2coe 操作台）实弹发现两缺口，账本开单 #64/#65，派单现场改加封版发布。

## 修法（在哪找改了，怎么找没动）

- #64 eval 面文件实参形态：client.rs 新增 `expand_at_args`（实参以 `@` 开头即读文件内容为片段体；方言与 --js 两形态通用；绝对/相对路径皆可；读失败报错带 CTA）；main.rs 在 --b64 解码前接线展开（`--b64 @<存 base64 的文件>` 组合形自然成立），stdin 管道与 REPL 不经此道。收口单点与 decode_arg_b64 同族
- #65 snippets 可配置外部根：paths.rs 新增 `snippets_root`（BROWSE_SNIPPETS_ROOT env 显式优先，缺省内置 `<state_dir>/snippets`；env 根不分 BROWSE_NAME，与 workspace 同口径，配 prs_c2coe 仓内使用）；CLI `snippets list/show [--root <dir>]`（旗标 > env > 内置三序，main.rs snippets_root 收口）；`<site>/<task>.js` 分层与首行 `//` 摘要约定不动；越根守卫（canonicalize 后必须在库根内）随配置根走，相对 `../` 与绝对路径两形仍拦
- 披露：surface 四条目（eval/js-flag/snippets-list/snippets-show）与意图路由一行、帮助面 env 节一行；README 例一条（`browse --js @snips/title.js`）；CHANGELOG 0.22.0

## 实证

单测 at_args_expand_from_file（@ 展开换内容、坏路径报错点名、非 @ 原样、相对路径 cwd 基准；cwd 全局态走 TEST_ENV_LOCK）与 doctest；argv 契约三件：读失败形 exit 2 带 CTA（方言与 --js 双形）、管道共存形（argv @ 先于 stdin 形态命中，两道不互踩；spawn 后 wait_with_output 须显式 piped stderr 才收得到子进程报错，首跑假绿即此坑）、根三序与越根拦截（旗标生效、env 生效、旗标盖 env、缺省内置不泄外部根、../ 与绝对路径 exit 1）。实弹：`--js @文件` 出 42；越根 ../outside.js 与绝对形双拦。[实证: fmt、clippy -D warnings、test --workspace 13 套全绿、doc、aidoc --check --strict、surface_contract 8、PEVO 全绿、e2e 真 chrome 5 passed]

## 坑与纪律

- surface 对外面零内部编号（faces_free_of_internal_ids 守卫）：首版描述带 #64/#65 全数被拦，台账编号的溯源属 CHANGELOG 与 diary，描述改裸文
- eval 条目 signature 不能带 `[--` 分段：HELP_GROUPS 按 `split(" [")` 前段归组，签名改 `browse '<片段|@<路径>>'` 会让归组名失配；@ 细节放 args 缺省与描述，签名回归原形
- --llms 手册 120 行帽：本批后 120 整（贴帽过）

## 封版 v0.22.0

能力新增取 minor（REQ-004 判据行）；release.ps1 三平台直发，播种流水自动接力。账本 #64/#65 留 open（只增不关不删，关闭归 omc）。
