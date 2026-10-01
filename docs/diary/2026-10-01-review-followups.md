# 2026-10-01 #67 评审遗留收口批（G10 尾置旗标 + G2 超时清账）

背景：v0.24.0 收轮后工位清点评审记档项，开工 0.24.1 收尾批（用户令「开工」）。0bb0 旧键吊销属总台待裁不入本批；G3 symlink list 过滤与备份 prune 维持记档。

## 修法

- G10（三轮低项升批）：main.rs 增 KNOWN_GLOBAL_FLAGS 名册与 bail_flag_in_subcommand：子命令收集环（rune 与 snippets list/show 三处 bail 臂）遇认识的全局旗标尾置报位置 CTA，未知旗标走 bail_arg 原面。权衡：codex 建议的「内环 break 回外层」改道动解析架构（尾置位置位回灌外层需 pushback 缓冲与 Mode 感知 catch-all），对低项过重；位置 CTA 消措辞矛盾即达验收。名册与解析臂手工同步（旗标增删时同步），surface_contract 旗标抽取测试旁证
- G2（一轮记档升批）：Cleanup 增 Drop 兜底：settled 原子闸（显式 run() 先置位防双跑），Drop 借 tokio Handle::try_current 排程清账任务（无运行时上下文静默让位）；script_id 从 WorldCtx（tokio 锁）移 CdpState 独立 std 锁（Drop 面不可 await/blocking_lock）
- 坑实录：cleanup_inner 首版 std 锁守卫跨 await，future 变 !Send 直接打穿 axum Handler 界（browse-core 编译红）。修法：守卫先取值即弃再 await。std 与 tokio 锁的守卫纪律：std 守卫恒不跨 await

## 实证

实弹三面：rune x.rn --headless 与 snippets list x --json 出位置 CTA、rune x.rn --bogus 保持参数不认识；arg_contract 增 tail_global_flag_gets_position_cta 锁双面加对照。[实证: fmt、clippy -D warnings、test --locked 全绿、arg_contract 13、aidoc --check --strict、metadata --locked、surface_contract 8、PEVO、e2e 真 chrome 6 过]

## 发版 v0.24.1

修复批取 patch;评审快核 CONFIRM 后推 main 发版（回执随发版补记）。
