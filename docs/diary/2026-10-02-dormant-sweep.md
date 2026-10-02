# 2026-10-02 休眠项与记档全清（v0.30.2）

## 背景

总台令清完休眠项与 REQ-020 记档后发 0.30.2。九项：探针多槽、建表预热、双向名册守卫、#67 G6/G7、G3 存量两件、注入收口、B 通道文档。

## 落地

- 探针多槽：OpProbe 改 Arc<Mutex<Vec<&'static str>>>；set_op 入列、clear_op 带名消己份（16 调用点配对）；server probe_snapshot_json 三态（null/str/array）；响应签名 Value 化
- preheat_missing_fn_table（独立线程）daemon 启动调
- G-2 源抽取守卫：顶层臂按 12 空格缩进判（子命令内环更深不入），首版全文臂抽取抓到 --markdown 误入后改缩进法
- G7：空管道提示（Script/blank/有实参三条件）；Drop 清账包 10s timeout；ADR-0010 object-at 边界注记
- G3：list 跳 symlink（DirEntry::file_type 不随链接）＋集成件；keygen 备份修剪（同前缀先清）
- 注入收口：vars_value_guard 两 wrapper 入口（值含 __BROWSE_RN_RAW_ 即 exit 2）：nonce 等价但确定性
- B 通道：http-api r# 节段

## 踩坑

- clear_op 初版 position(|x| *x == x) 恒真（自比较），改带名参数
- js_wrapper 期望串的三层转义笔误两次（r# 内 \\\" 与 Rust 源 \" 混淆），改 span 替换法

## 验收

- 单测：多槽三态、双向守卫、js_wrapper 两件、值守卫、symlink 集成件
- e2e 断言迁移（first().copied）；门禁与评审见发版回执
