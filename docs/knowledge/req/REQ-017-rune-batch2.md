---
id: REQ-017
title: rune 面批次二与看板移除（#69、停止三道、wait_event、r# 裸串、#70 披露、API-only）
status: implemented
priority: should
trace: 单测（server 超时/停止回执面、engine_lost_watch、rawstr 词法十件、vm_face r# 端到端、stdin 读界集成双件）；e2e 四件（timeout 探针保留、r# 真引擎、wait_event 双臂、既有 rune 回归族）；CLI 实弹三证据（stop 回执/victim stderr 一行 JSON/daemon 日志行）；评审三轮 CONFIRM 后发版 v0.28.0
---

> 口径修订注记（2026-10-03）：本文的 45/300 秒数值与 daemon 级同界口径由 REQ-027/ADR-0012 统一兜底超时修订（统一缺省 60 秒、--timeout 请求级覆写）。历史记录冻结。


# REQ-017：rune 面批次二

## Scenario

战役台在 pentest_rs rn 化切换中连续暴露四类缺口：非 TTY 开着不关的 stdin 让 `browse rune` 无上界挂死（#69，「双 eval 必挂」真凶归因）；外部 GNU timeout 被 session-tool-guard 硬拦后引擎内缺少主动停止方法；rune 面只有轮询等待（wait_sel/readyState），CDP 丰富的 wait 事件面没直出；rn 件内嵌页面 JS 的双引号逐个转义体操（裸串研究单定谳 A 案）。总台连发指令收敛设计：结合事件驱动、主动条件停止与超时机制集成 CTA；自定义检测用现成 rn 原语；**不自封装（原生 CDP 能实现就行）**；验收回归 rn 脚本加 CTA 回执即可。

## Criteria

- [x] #69：rune 面 stdin 读上界（3 秒静默界线程读，过界按无参数道放行加 stderr 提示）；EOF 管道零回归（集成测试双件：持开 stdin 20 秒内 exit 2、EOF 管道同面快出）
- [x] `BROWSE_RUNE_TIMEOUT` 独立旋钮（默认 45 秒，`/eval` 的 300 秒零动，daemon 级上界披露）；超时回执 CTA 点名旋钮
- [x] 主动停止三道同一 CTA 面：`POST /rune/stop` 与 `browse rune stop`（幂等，回 running/requested/note）；引擎失联联动（两次连续探测断连即停，cause=engine_lost）；脚本侧原生自停不自封装（select/分支加 Err 返回）。`stopped{op, elapsed_ms, cause}` 结构面、CLI stderr 一行 JSON 非零退出、daemon 日志行
- [x] `cdp::wait_event(method, ms)`：原生 `Session::wait_for` 直出（零新语义）；超时 `Err(CdpError)` 带「下一步」CTA；脚本 `match` 条件分支模式（e2e 锁命中/未中双臂）
- [x] r# 裸串（A 案）：`r#+"…"+#` 编译前重写为转义普通串（只转 `\`/`"`/CR，换行原样行号不漂）；模板道先抽后渲再回填（r# 区免渲染，与 `{% raw %}` 各管一层）；单实现于 browse-rune（CLI 渲染道依赖，daemon run() 入口兜底，防双轨）；词法器跳过注释/普通串/模板串（`${}` 递归）/char，标识符 run 内不误判；未闭合报起行 CTA
- [x] #70 披露口径：Rune 与 Rust std 差异入册（to_string/splitn 缺席、`split_once` 正形、M-… 移交报错与 `clone(h)` 正形、直桥原生类型无 Value 方法面）；哈希查表补名留后续批
- [x] HTML 看板移除（总台令「不做 html 看板 只保留 api」，评审二轮后并入）：GET / 与 GET /dashboard/sse 退役（路由、两 handler、内嵌 HTML、dashboard_tests），daemon 只保留 API 面；serve 条目与 http-api 同步；REQ-011 注记；契约破裂判 major（0.27.0 改判 0.28.0）
- [x] 文档：surface rune 条目与 env 节（BROWSE_RUNE_TIMEOUT）、http-api（stop 路由行、停止与检测联动节、r# 与 std 差异节）、ADR-0010 增补（r# 语言扩展与上游对齐、停止三道）

## Notes

- 评审二轮全目录审计（总台授权「review 所有命令的合理性完整性」）：零 F 九 G；G-A1（env 对账补 6 条含 BROWSE_NO_AUTO_DIALOG 行为开关与内部/镜像类尾注）、G-A2（session CTA 补 waitJs）、G-A3（rune 退出码三态入条目）、G-A4（--cookies 入 Options 伴生表）、G-A5（路由表补 GET 行，后随看板移除令改撤两行）随批；G-A6/A7/A8/A9（低）记档后续批（G-A7 随 REQ-021 双向守卫闭、G-A8 随读序句闭、G-A6/G-A9 随 REQ-023 闭）
- 设计纪律（总台指令链 2026-10-02）：不自封装原语（曾实现 `cdp::stop_self` 后按令回撤，脚本自停走原生控制流）；`wait_event` 是原生 `Session::wait_for` 的直出面不算自封装；验收口径＝回归 rn 脚本＋CTA 回执
- 停止道与超时同构：都丢弃执行 future，清账走 Cleanup Drop 兜底，op 探针保留在飞面（v0.26.1 机制复用）
- 评审一轮 F1（必修已修）：rawstr 字面量面逐字节 as char 是非 ASCII mojibake 静默变形（run() 无条件过道，全脚本面回归）；修为 UTF-8 整段推进，回归锁＝非 ASCII 词法单测（五面）＋run() 全道非 ASCII 串测试＋r# e2e 中文载荷
- 评审一轮 G2 口径：stdin 界语义是「3 秒内 read_to_string 完成」非「首字节」；超界慢/大生产者数据不候（提示文案与 http-api 如实）
- 评审一轮 G3 披露：fail-fast 先到即走：CDP 等待腿悬挂时单调用 30 秒界先到（脚本错误面），engine_lost 只兜非 CDP 等待段（长 wait_event、纯计算循环）
- 评审一轮 G1/G4/G5/G6 随批：subst 占位恰一次校验加残留前缀拦截；stop 动词位撞名披露（surface/http-api）；stop note 收尾竞态句；stopped.elapsed_ms 实测口径注明
- r# 上游对齐：rune-rs #537（2023-05 开册零动作）；上游将来采纳同形语法则预处理幂等退役（ADR-0010 增补条款）；发版后去 #537 贴需求与薄词法重写参考
- 残留后续批：哈希查表补名（#70 增强腿）、探针并发多槽（G1 披露已入档）、`--vars` 值注入通道文档化（B 通道）
