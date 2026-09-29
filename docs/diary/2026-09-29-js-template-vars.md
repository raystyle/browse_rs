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

## 待办（总台协调面）

本地 commit 不 push（派单令）；评审闸门走 browse-codex-review；立项 issue 写入道等新 kid 在册；发布 0.23.0 待评审放行后总台协调。
