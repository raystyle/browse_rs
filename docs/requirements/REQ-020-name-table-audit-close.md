---
id: REQ-020
title: #70 补名腿与目录审计收尾（v0.30.1）
status: implemented
priority: should
trace: exec 单测双件（战役台锚点锁）＋vm_face missing_fn_error_carries_name＋arg_contract roster_flags_all_recognized_in_rune_tail_position（31 旗标旁证）；评审快核 CONFIRM 后发版 v0.30.1
---

# REQ-020：#70 补名腿与目录审计收尾

## Scenario

#70 披露口径已落（0.28.0），补名腿（常用 std 方法名哈希查表翻译）随待办批落地；总台令「其他继续」后启动本批（#70 补名 + 目录审计 G-A6/A7/A8/A9 收尾）。

## Criteria

- [x] 补名表：首次遇缺失实例函数错误时以一次性 VM 探针收割（String/i64/Vec × 28 常用方法名，OnceLock 缓存，仅默认模块零 cdp 依赖）
- [x] 终态错误补名：命中即在报错里带方法名并保留哈希（`to_string（哈希 0x871c…）` 形可溯源）；未命中原文
- [x] G-A7 名册旁证：KNOWN_GLOBAL_FLAGS 名册 32 项、尾置旁证 30 项（--vars/--root 是 rune 环合法子旗标除外）（--vars/--root 是 rune 环合法子旗标不入名单），漏同步解析臂即红
- [x] G-A6/A9：artifact publish 签名补 --version/--note/--git-range；-V 入 version 条目
- [x] G-A8：手册补旗标位置口径句（全局旗标尾置合法、无关子命令静默忽略、未知旗标用法错三分面）
- [x] 测试：锚点双锁（to_string=0x871c12da43b176d2、splitn=0x7d6f3bd0775df4c8）＋vm_face 端到端＋名册旁证

## Notes

- 探针入口形：`pub fn main()`（非 pub 的 MissingEntry，非 async 的 complete 即收错）；哈希解析 u64::from_str_radix(16)
- 表是收割非预计算：哈希与 rune 版本绑定，升级依赖自动重收割（正确性免维护）
- 评审 G-2/G-3 记档：roster 守卫是名单单向（解析臂到名册的漏同步验不到，双向需源抽取，后续批）；建表首错约 2 秒同步（84 探针一次性，block_in_place 在 current-thread 运行时会 panic 故直取；后台预热是后续可选）
- 残留后续批：探针多槽（并发在飞 op 精确化）、B 通道文档化、注入理论形 nonce 化
