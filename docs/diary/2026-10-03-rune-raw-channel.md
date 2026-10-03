# 2026-10-03 rn 面通用 CDP 命令通道与糖皮回收（账本 #73 升级案）

## 背景

#72 报 mouse 与网络拦截原语缺口；#73 现状补正实测收窄：方言面七件 mouse 与 route 三件引擎侧全在，缺口精准为 rn 面绑定。原案逐件绑 `cdp::mouse` 与 `cdp::route` 系；用户裁定升级（2026-10-03）：rn 面缺的不是个别原语，是通用 CDP 命令通道，事件侧 `wait_event` 已开任意 method 名，命令侧对称开放即闭环。实施中再接修订令：raw 落地后回收纯透传糖皮。批次经 herdr-review 两轮门禁后推 main。

## 设计弯路（记档）

- 逐件绑定案升级令到即弃：单通用绑定替代，回收令把「缺什么补什么」翻转为「只留不可透传件」
- params 形选型：预设「rune 对象字面量无 null 直写法」被 vm_face 探针推翻（`()` 单位值过桥即 JSON null），按令实测后写实改口径：对象形可行，选串形是协议文档参数形原样粘贴（r# 免转义，与 wait_event 任意 method 名对偶）加避开外部类型序列化错面
- rune 的 `format!` 无字面大括号转义（不支持 `{{`），params 拼接走字符串相加；e2e 三轮踩实（外层 Rust format! 的花括号双写、内层 rune format! 报 unsupported char、拼进完整对象形非法）
- `wait_event` 回的是事件 params 层，e2e 首版取 `ev["params"]["requestId"]` 撞 Missing field 终态错

## 落地

- `cdp::raw(method, params_json)`：`parse_raw_params` 预检（空串等价空对象、非对象形拦、坏 JSON 带 CTA）透传 `Session::call_with_deadline`（同守卫同 30 秒界），响应整体过桥；与全部内置函数同一道域策略与提交屏障
- 糖皮回收五件：click、type_text、wait_sel、cookies、set_cookie；wait_sel 工位裁定回收（纯轮询壳，`time::sleep` 在 rune-modules time 特性内，无引擎侧状态）
- raw 导航类边界（评审 G-6）：Page.navigate/Page.reload 成功后与 goto 同款废隔离世界；Target.createTarget 不登记 own 文档点明
- 自检纪律四面（评审 G-5）：旧 set_cookie 的 success 报错与 type_text 的未命中守卫回收后归脚本侧，ADR/http-api/surface/pillars.rn 示范加 roundtrip e2e 填值组合形段
- ADR-0011 立件（通用通道与糖皮回收，件数口径经评审二轮纠偏：脚本可见件 18 收到 14，删五加一）；ADR-0010 加增补段指路部分替代；CHANGELOG Unreleased 段（semver 判据预定：raw 取 minor、回收属契约破裂按判据取 major，终版号落封版 REQ 属总台令）
- http-api.md 的 POST /rune 契约段随新面重写（评审 F-1：该文件非生成物且守卫扫不到，一轮漏检的静默漂移面）；surface 三投影与 aidoc 重生成

## 验收

- 门禁全套 exit 0：fmt、clippy -D warnings、test 全家（含 doc）、cargo doc、aidoc --check --strict、PEVO
- e2e 门控真 chrome rune 族 9/9：拟人鼠标回执 `{"ok":[["mm",true,30,30],["mm",true,35,34],["mm",true,41,39],["mm",true,48,45],["md",true,48,45],["mm",true,60,58],["mu",true,60,58],["wh",true,60,58]]` 全 isTrusted=true；断流对账 `{"ok":{"baseline":3,"blocked_run":1,"media_font_blocked":2,"others_passed":1}}`（本地 127.0.0.1 测试源，Fetch.enable 加 wait_event 循环按 resourceType 分流，Fetch.disable 收尾）
- CLI 全链实弹（daemon、POST /rune 道过）：Browser.getVersion 加四类鼠标事件 isTrusted 全 true，exit 0
- 评审闸门（browse-codex-review 格，本 tab 右分驻场）：一轮 F-1 加 G-1 至 G-7 全修，二轮快核三 G 加可选项收尾，终审放行后推 main（259f18f..cb2e1c4）；CI ci 与 docs 双 success

## 记档

- pentest_rs 战役件用量（cookies 4、set_cookie 3、click 1、wait_sel 2）由总台迁 raw 组合形，不阻塞本批
- 封版判据与版本号：总台令时落封版 REQ
- 命名实例解析不到托管 pinned chromium（chromium_root 随 BROWSE_NAME 走子目录，部署在默认 state 根）：随批提 issue #74，待修；CLI 实弹经 BROWSE_CHROME 显式道绕行
- 两个评审 nit 记档不采纳：errorText 不检查（透传面保守废世界偏安全）；navigateToHistoryEntry 不入特判集（维持与 goto 对齐最小集）
