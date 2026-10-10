# docs 文档地图

> 契约在代码，文档是投影。本图只答一件事：想找某类信息先去哪。生成物勿手改。
> 三层聚合与归档统一(ADR-0013):sources 撑 knowledge,knowledge 撑 operations,单向支撑。

## 三层聚合,ADR-0013

| 层 | 目录 | 回答的问题 | 说明 |
|---|---|---|---|
| sources | [sources/](sources/) | 发生过什么、出处是什么 | [diary/](sources/diary/)(一天一篇轨迹,只增不改)、external/(外档原档)、evidence/(判例证据);索引 [sources/INDEX](sources/INDEX.md) |
| knowledge | [knowledge/](knowledge/) | 是什么、为什么 | **独立 git**(主仓 .gitignore 登记):[adr/](knowledge/adr/)(ADR-0001 至 0013)、[req/](knowledge/req/)(REQ 台账 trace 文档)、[references/](knowledge/references/)(R0xx 使用指南/R1xx 模式页带 trace)、[research/](knowledge/research/)(S 编号)、mistakes/、[防重提页](knowledge/skill-impact.md);索引与流水 [knowledge/INDEX](knowledge/INDEX.md) |
| operations | [operations/](operations/) | 怎么做、验收什么 | 只收过闸产物:[G001-userscripts](operations/G001-userscripts.md)、[G002-apps](operations/G002-apps.md) 与标准技能;[索引](operations/index.md) |

红线:diary 与 research 是保留核心结构,不可裁撤(用户裁定 2026-09-16);R0xx 指南按需生长;operations 一切新案走合取闸门(distil-skill gate)。

## 投影与专档,特色面白名单不进三层辖域

| 路径 | 性质 | 重生成 |
|---|---|---|
| [aidoc/](aidoc/) | Rust API 投影(生成物,勿手改) | cargo aidoc |
| [surface/](surface/) | CLI 命令面投影(生成物,勿手改) | cargo run -p browse-cli -- --gen-surface docs/surface |
| [architecture.md](architecture.md) | 结构专档(现在怎么拼,随结构同 PR 更新) | 手写 |

## 门禁

- 骨架合规:PEVO_CHECK_ALLOW="^docs/aidoc/|^docs/knowledge/" uv run ~/.claude/plugins/marketplaces/project-evo/.tools/check.py . 退出码 0(载体 project-evo 市场仓活 checkout)。豁免正则在册两处:aidoc 条目分隔符 em dash 是 cargo-aidoc 渲染格式(路径级放行 docs/aidoc/,真门禁是 cargo aidoc --check --strict);knowledge 是独立 git 层(归档统一字母表,ADR-0013)
- 投影漂移:cargo aidoc --check --strict 与 tests/surface_contract.rs
