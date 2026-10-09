# browse-core::skills

技能触发层（#50/#51/#56 + 清仓重建总台令 2026-10-09）：goto 导航回执
与 fetch/detect 回执的条件附加面。技能本体存 workspace 单仓
（[`crate::workspace`]，github.com/raystyle/browse_workspace），形是
「宿主侧 `mech.rn` + 页内侧 `probe.user.js`」**双脚本对**——本模块在
回执里「点名」：热路径只消费各对 `probe.user.js` 的 `/* @detect */`
块，按多根序收集拼成**一次**主世界求值，命中附加清单与 hint，未命中、
无对或关闭时零键，回执不加页面技能键。

## Functions

- `build_batch_js` — 拼批判定 JS（纯函数，单测锁形）：prelude 在 facts 上填事实（约定可写
- `collect_pairs_multi` — 枚举各根 `page-skills/` 的成对目录（纯函数，单测用）：目录同时有
- `domain_hint` — 域名层 hint 字段值（#50）：读全文命令字串。
- `domain_segment` — URL 到域名段（#50，纯函数）：先做 http(s) 门禁（[`cdp::session::url_host`]
- `domain_skills_enabled` — #50 域名层是否开启：`BROWSE_DOMAIN_SKILLS` 未设或非 0/false 即开。
- `list_domain_skills` — 列 `<ws>/domain-skills/<段>/` 的对文件清单（纯函数）：成对（mech.rn
- `load_prelude_multi` — 序首 `_prelude.js`（facts 游走脚本，page-skills 层）：先命中根整胜，
- `page_fields` — 把探测结果变成回执附加键对（#51，纯函数）：framework 形合法出
- `page_skills_enabled` — #51 页面层是否开启：`BROWSE_PAGE_SKILLS` 未设或非 0/false 即开。
- `url_domain_fields` — URL 到域名层键对（#56，fetch 腿的口径源）：与 goto 同口径的域名段
- `url_domain_fields_multi` — #63 多根域名层键对：序首命中段（有文件）的根整胜，未命中零键。
- `verdict_fields` — detect() 回执的技能附加入口（#56）：判读经 [`verdict_page_slugs`]
- `verdict_page_slugs` — detect() 判读到 page-skill slug 的映射表（#56，纯函数）：challenged

## Types

- `SkillPair` — 一个成对的页面技能（清仓重建总台令 2026-10-09）：多根序首胜的原子

## Constants

- `FROZEN_PAGE_SLUGS` — 清仓重建后的冻结 slug 名单（页面可控串注入面的白名单，评审 F3 律

