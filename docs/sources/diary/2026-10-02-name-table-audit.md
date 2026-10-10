# 2026-10-02 #70 补名腿与目录审计收尾（v0.30.1）

## 背景

总台令「其他继续」后启动：#70 补名腿（哈希查表）+ 目录审计 G-A6/A7/A8/A9 收尾。总台域与上游令本批不做。

## 落地

- 补名表：exec.rs missing_fn_table（OnceLock 收割：String/i64/Vec × 28 方法名的一次性 VM 探针，仅默认模块）；enrich_missing_fn 在终态错误文本命中时补名保哈希
- 踩坑三连（记档）：探针入口要 pub fn（非 pub 是 MissingEntry）；VmResult 模式匹配要 into_result()；哈希解析要 from_str_radix(16)（十进制 parse 对十六进制串静默 None，单点诊断定位）
- G-A7：roster_flags_all_recognized_in_rune_tail_position（31 旗标旁证；--vars/--root 在 rune 环是合法子旗标不入名单，首跑抓出此差异即注记）
- G-A6/A8/A9：artifact publish 签名补三旗标；-V 入 version 签名；手册退出码节前补旗标位置口径句
- 版本 0.30.1（错误信息增强加文档/守卫测试取 patch）；REQ-020、CHANGELOG、README 行、本日记

## 验收

- exec 单测双件：锚点锁（to_string=0x871c12da43b176d2、splitn=0x7d6f3bd0775df4c8）＋补名文本形
- vm_face 端到端：to_string 缺席终态错误带「to_string（哈希 0x871c…）」
- 门禁与评审见发版回执
