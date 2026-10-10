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

## 评审一轮（browse-codex-review，发版后补闸）

门禁语句后置说明：本批随总台工单令直推 main 并发 v0.22.0，评审是发版后补闸（工单含发布令，闸门语义由二轮快核接管增量面）。回执 1 F + 7 G + 四核 CONFIRM。

- F1 必修（采纳）：snippets 解析臂收集循环吞未知旗标，`snippets list x --bogus` 从基线 exit 2 变静默 exit 0（`--root=` 形同踩），坏旗标契约破口。修复：两臂加 `other if starts_with('-') => bail_arg`，基线复位；arg_contract 此前对 snippets 族坏旗标零覆盖，补 snippets_arg_face_after_review_round1 锁（评审方指出的测试盲区属实）
- G1 采纳（随 F1 并入）：首位置位胜出，多余位置位用法错（旧形余参静默落外层 catch-all 也是暗坑）
- G2 采纳：显式配置根不存在报配置错 exit 1（拼错根与真空库可分）；内置缺省根不在册维持「片段库为空」exit 0（#44 bootstrap 契约，既有测试钉着）
- G4 采纳：paths.rs 文档句澄清 env 根不分舱与内置缺省根随实例分舱的区别（workspace 缺省也不分舱，三者勿混读）
- G5 采纳：裸 @ 特例文案（不再落「读文件失败 ：」悬空冒号形）
- G6 采纳：空串 --root 视同未给（与 env 空串回落同规）
- G7 采纳：js-flag 手册补「字面前缀，前导空白即不触发」半句
- G3 记档不修：list 面符号链接不过滤（存量，#44 时代行为），越根守卫在 show 面（读内容道）是安全边界，list 只印首行 // 摘要；#65 导向外部仓后若要收口另立 issue
- 四核 CONFIRM：@ 展开边界（REPL/stdin 确不经此道、序与文案同构）、根三序与越根守卫（canonicalize 随根走、env 根不受 BROWSE_NAME 影响）、解析臂回归（即 F1/G1，余项成立）、surface 披露无过诺无漏诺

## 评审二轮（browse-codex-review 快核）

回执：F1/G1-G7 全验收，另出 F2 必修 + G8/G9/G10 + 三问答复。

- F2 必修（采纳）：修复批把版本 bump 放在门禁与 aidoc 之后，b0f0fd0 提交里 Cargo.lock 与 aidoc 投影仍 0.22.0（metadata --locked 101、aidoc --check exit 2，评审方实弹双红）。本批序复位：代码改、build 刷锁、aidoc 重生成、全门禁、提交依序走完再落笔；--locked 两条进门禁循环（教训与「doc 后补未 regen aidoc」同族：投影与锁都是派生物，bump 后必须重生成再提交）
- G8 采纳：谓词单源。paths 层新增 snippets_root_explicit（env 非空判定唯一裁定点），CLI 层 snippets_root 不再自判 env，防 list 配置错面与 show 路径面日后漂移
- G9 采纳：裸 @ CTA 覆盖方言与 --js 双形
- G10 采纳：显式根存在但不是目录分面「片段库根不是目录」exit 1，arg_contract 增 snippets_explicit_root_not_a_dir
- 三问：负首字符位置位属病态可弃（评审方同意，基线 0.12.1 虽 exit 0 但仓级口径本就 - 前缀 bail，是拉回不是新不一致）；G2 explicit 判定经 G8 收敛后无第二真相；G1 无新回归（show '' x.js 的空串哨兵形记档，病态）
- 评审方另注：其跑 aidoc 时顺带刷了 Cargo.lock，已还原回提交态并留期望投影于 /tmp 供比对（工作树交还干净）

## 封版 v0.22.1

修复批取 patch（REQ-004 判据行）；三行快核 CONFIRM 后推 main、tag、release.ps1 直发。
