# 2026-09-29 JS 命令小程序批（@ 实参 --vars 模板参数化）

背景：总台派单「JS 模板参数化执行」（snippets/@file 的自然延伸：存片段解决了，同片段不同参数没解决），随后两道补单定向：snippets 库即 JS 模板的家，特性定型「JS 命令小程序」（代码驻库，CLI 只传参）；参数走专用旗标 --vars 不占位置位。用户另给 minijinja 定向（会话内给链 github.com/mitsuhiko/minijinja）。

## 设计定案（三句）

- 占位符语法：Jinja（minijinja v2.24，ADR-0009；Strict undefined 加零 autoescape，`{{ 键 }}` 直插、`| default()` 可选参、`{% for %}` 生成面；与 JS 字面常态不冲突靠激活规则兜底）
- 传参 CLI 面：`--vars 键=值` 重复旗标累积（与 --engine-arg 同族，不收逗号分隔：值可含逗号有歧义）；与 @ 实参组合，方言与 --js 两形态同用，位置位语义零改动
- 模板激活规则：给 --vars 即模板道（源先按 snippets 根三序解析：`根/<名>` 原样与补 .js 两试、越根候选跳过守卫同 snippets show，未命中回落字面路径即 #64 语义）；不给 --vars 即 #64 纯透传，字面大括号对天然不误伤（零静默漂移，既有 #64 契约测试原样绿即锁）

## 实现

- client.rs：template_env（OnceLock 单例）、parse_vars_arg（形状）、missing_bare_placeholders（裸标识符占位符带名预检：引擎 Strict 错误不保证带键名，验收要求点名；带过滤器/表达式的占位交给引擎，可选参由此通行）、render_template（BTreeMap 收拢重复键后值胜）、load_template（根道两试 + 越根跳过 + 字面回落，错误列试过的候选位）
- main.rs：--vars 与 --root 全局旗标（--root 升全局：eval 面模板道与 snippets 子命令共用一根，子命令内环改赋同一变量）；@ 实参双道分流（给 --vars 走模板管线：形状校验、只配 @ 形态守卫、逐 @ 解析渲染 exit 2 直出；不给走原 expand_at_args 纯透传，代码路径不动）
- 披露：surface eval/js-flag 加 vars 参数行与模板道长句、snippets-list/show 升格「参数化 JS 命令小程序库」措辞（补单一定向）、意图路由行、帮助面 Options 加 --vars 伴生行（--root 在豁免表不得加行，行数锁）、README 例一条、CHANGELOG 0.23.0
- 新依赖 minijinja v2.24（传递 memo-map 一件）：本仓首个模板引擎依赖，选型与受控配置在册 ADR-0009（含被否备选：手写替换、tera/handlebars）

## 失误与轮换（如实记）

立项查钥匙态时误跑 `browse ledger keygen --force`，把 2026-09-20 在册私钥覆写销毁（守卫设计本来就在防这个：非 force 即拒、force 先带出旧 kid，21:13 首误带出 replacedKid 30dfd88d；builtinKid 5f6664e4 即原在册 kid）。恢复走仓内既定轮换道：二次 --force 定轮换目标 keyId 81ca8826（pubkeyJwk 重烧进 ledger.rs PUBKEY_JWK 常量，baked_jwk_pairs_with_local_keyfile 单测锁钥文与常量配对），随本批发布后总台在册新 kid 方恢复写入道。立项 issue（内容已备）在册前不可写入，账本面待总台协调。

## 实证

单测 template_resolution_root_first_then_fallback（根道 .js 重试、原样命中、字面回落、越根走字面道、全未命中列候选；坑：越根探针初版 `../` 落共享 /tmp 撞历史残留，cwd 收子目录后确定性）与 template_render_values_and_missing（重复键后值胜、空串与等号引号原样直插、未填点名、default 参不点名、语法错带 CTA）；argv 契约 template_vars_arg_contract（--vars 只配 @、键=值 形、未填键 exit 2 点名已给键不点名、根第一等判别：库内同名未填错 vs 字面先解析会到引擎连接、渲染成臂经 --ws 死口证填充毕进求值道零 daemon）。实弹：--js @x.test/mul --vars n=6 --root 库 出 60（default 参 10 兜底）；env 根缺位回落 cwd 字面道渲染 cwd；--js @文件 不带 --vars 出 42（#64 零回退）。[实证: fmt、clippy -D warnings、test --locked 13 套 216 用例、doc-test、doc、aidoc --check --strict、metadata --locked、surface_contract 8（--llms 仍 120 行贴帽）、PEVO 11 pass（ADR-0009 登记索引后）、e2e 真 chrome 5 passed]

## 评审一轮（browse-codex-review）

回执 1 F + 7 G + 五核 CONFIRM（轮换面评审方独立验证五项：kid 数学、钥文配对经 cryptography 独立派生、账本 #66 签名者即新键、全机密档枚举证旧钥确无副本、omc 服务端态采信我方回执）。

- F1 必修（采纳，权威序反转）：带名预检先于引擎跑，`{% if false %}{{ x }}{% endif %}`、`{% raw %}`、`{# #}` 死分支占位符被误报「未填」，凡循环体打印循环变量的模板全过不了，与披露的 `{% for %}` 生成面自相矛盾。修法照评审方推荐：引擎先 render_str，未定义类报错后才跑扫描器富化键名；误报类自然消失
- 零转义显式钉死（三核建议）：`set_auto_escape_callback(|_| AutoEscape::None)`，原属 render_str 匿名名落 None 的隐式不变量，写死防引擎升级改道
- G1 采纳：--root 求值面漏 --vars 显式用法错（原静默失效，@读失败文案误导「文件不存在」）
- G2 采纳：--root 撤 surface_contract 豁免（「子命令私有」理由已过期）补 COMPANION_FLAGS 行，行数锁两侧同步
- G3/G4/G6 采纳（文档句）：show 面要全名 vs 模板道省 .js 两试；--vars 值恒字符串循环遍历按字符；敏感值走模板内 secrets/dotenv 道（argv 可见于 ps/历史）
- G5 采纳：keygen --force 覆盖前自动落 0600 备份件 ed25519.key.bak-<epoch>（内容逐字节同旧 seed，单测锁），回执 backupKey 点名，即本日 5f66 不可逆损失的直防
- G7 记档不修：写入前配对非阻塞告警（pairing_ok 仅测试用面，存量）
- 五核一采纳补强：字面大括号 + 不给 --vars 的透传从推论升直断言（arg_contract：报 b64 字母表错而非模板错即证未过模板道）；根序、越根、--vars 位置位、--root 全局化四核 CONFIRM

## 键面收口（飞轮跨工位，ai_ccoe 工位办结）

omc 键面三件套回执全绿：list 事前 8 键无冲突；register 81ca8826 预览 kid 派生分毫不差后 --yes 在册（active，count 8 到 9）；revoke 5f66（私钥已毁死面，卫生动作）翻 revoked。看板终态 = browse-rs 两把 active（0bb0 待裁 + 81ca）加一把 revoked（5f66）。新键首发实弹 = 账本 #66 立项单写入成功（dry-run 先行零入账预览）。

## 评审二轮（browse-codex-review 快核）

F1 反转验收（碰撞形实测未触发漏网）、G1/G2/G5/G7/五核一逐条过，三问答复（错误分面、G1 边界与基线同口径、G5 无丢钥窗）全对。**结论 CONFIRM：同意停手待总台协调推 main 与发 0.23.0**。随批加修二轮低 G：

- G8 采纳：keygen 备份道从「静默 best-effort」改「不成即拒」：备份 open/write 失败报错中止覆盖，落盘后回读与旧 seed 逐字节比对不一致也中止（覆盖是不可逆动作，这正是 5f66 损失的同类窗口）
- G9 采纳：F1 富化判据从措辞启发式（msg contains "undefined"）换结构化 `e.kind() == ErrorKind::UndefinedError`（耐引擎版本变化）
- G10 记档：备份件 ed25519.key.bak-* 无自动清理（每次 --force 留一件，属有意的保守；要 prune 另立 issue）；0bb0 旧键吊销属总台待裁项，跟踪口在 ai_ccoe 09-22 键面日记与本节（悬空引用防呆：0bb0 = browse-rs 更名前疑似旧钥，active）

## 发版 v0.23.0（用户令「继续 直到发版」）

三笔 1eec525 加 09e6f29 加 174908b 推 main（2a8f898..174908b），tag v0.23.0；release.ps1 三平台构建冒烟直发六件（linux tar.gz / win-gnu zip / mac arm64 tar.gz 各带 sha256 边车）；播种 run 36578267235、main ci run 36578104447、docs run 36578104892 三绿全挂 174908b。评审格收轮回执（资产名 + run id）已发。账本 #66 留 open（关单归 omc）。
