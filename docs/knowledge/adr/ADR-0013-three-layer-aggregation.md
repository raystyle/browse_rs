# ADR-0013:三层聚合与归档统一,docs 中心形

- Status: Accepted(2026-10-10)
- Supersedes: 无(承接 ADR-0008 知识外置的两层定位,workspace 仓运行时知识不受影响)
- 背景裁定:project-evo 四仓标准合流 v2 冻结(con-02 施工单;distil-skill 三层聚合标准)

## Context

本项目参与 evo-skills distil-skill 标准合流为灯塔判例仓(distil-03 回执六坑三提案断言并入)。标准要求三层聚合(sources/knowledge/operations 单向支撑)与归档统一字母表(ADR至knowledge/adr、REQ至knowledge/req、R至knowledge/references、G至operations、diary至sources)。现状:docs/ 平铺(diary 41 件=sources 等价形、adr 13 件、requirements 38 REQ、guides 4 件混态、research 2 件),无 knowledge/operations 层。

## Decision

1. **层根 docs/ 中心形**(v2 条 13 层根仓自选,本仓近零迁移):
   - `docs/sources/`:diary(整目录迁入)、external/(外档信源)、evidence/(判例证据)、INDEX
   - `docs/knowledge/`:adr、req、references(使用指南+模式页)、research、mistakes、INDEX,**独立 git**(分目录分 git 形,v2 条 5 二选:主仓 .gitignore 登记该目录,PEVO 骨架门禁豁免正则同步 `^docs/knowledge/`)
   - `docs/operations/`:G 手册族(治理态 how-to 手册)+标准技能(agentskills 形,经 .claude/skills 加载视图)+index;只收过闸产物(归档迁移件人裁授权,页头注迁移来源)
2. **归档映射**:requirements至knowledge/req(REQ 文件名不变);adr至knowledge/adr;diary至sources/diary;research至knowledge/research;guides 分流,userscripts.md 与 apps.md(治理态契约手册)升 G001/G002 入 operations,getting-started/http-api 入 knowledge/references(R001/R002);docs/skills 指路碑并入 references(R003)
3. **知识外置边界不变**:workspace 仓(ADR-0008)是运行时机制知识库,不属本三层辖域;aidoc/surface 是代码投影生成物,留特色面白名单不进辖域
4. **闸门与台账**:点名义务载体=ledger issue 台账(--acceptance 判据,v2 条 12);标准页头闸门记录段(v2 条 11);operations 候选先落未提交工作区、接受才 commit

## Consequences

- 代码内文档路径引用(surface.rs 五处、server.rs 一处)与 AGENTS/README 同步改;PEVO_CHECK_ALLOW 扩 knowledge 豁免
- 主仓 git 保 rename 史;knowledge 独立 git 从剥离点起史(主仓 log 全程可溯)
- 迁移动作属归档统一人裁授权,不逐件过闸;此后一切 operations 新案走合取闸门(首轮 gate-verdict 停人裁)
