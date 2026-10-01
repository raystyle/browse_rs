# 2026-10-01 Rune 三形态运行时批（js/cdp/jinja 集成与输入三通道，0.25.0）

背景：用户连发定向令（当日下午）：rn 脚本要可内嵌 js 小程序；rn 脚本小程序能力集成 js 与 cdp；三种形态运行时（rn 脚本小程序 js+cdp / 方言命令 cdp / js 小程序 js）；要考虑单双引号转义；支持 js 代码参数与 rn 代码的管道输入；文件、参数和管道输入都支持。0.24.1 收尾批先行落（G10 加 G2），本批继之。

## 设计定案

- 三形态定位：rn 脚本小程序是唯一同时握 js+cdp 双能力的编排层；方言与 js 既有面零动（js 的 eval 参数与文件输入在册）
- 两世界分工修订（ADR-0010 增补）：eval 族走隔离世界轻探针，js 族（js/js_lib）走主世界全量页面逻辑（--js 同语义，页面函数可调）；rn 形态按需双世界选用
- 转义纪律：js 体原样入包裹器零拼接；args/vars 对象经 serde_json::to_string 做 JSON 字面量注入，单双引号反引号天然安全（结构化注入取代手工拼接道）
- 输入三通道：文件/实参道（@ 名走 snippets 根 .rn 两试加 --vars CLI 侧 Jinja）；管道源码道（无实参时 cat plan.rn | browse rune，对齐 --js 管道先例）；管道参数道（有实参时管道串经 RuneRequest.stdin 进 cdp::stdin()）；--root 双生效面（@ 名解析 CLI 侧加 js_lib 经 daemon 三序，RuneRequest.root 递入）

## 实现

- module.rs：ModuleCtx 上下文束（session/result_slot/snippets_root/stdin）；jinja_env（ADR-0009 同配置双持注记）；js_wrapper 纯函数；eval_main_world（主世界求值腿）；resolve_js_lib（库内 .js 两试加越根跳过）；四函数注册（js/js_lib/render/stdin）加 __describe 错误载荷降形
- exec.rs：RunOpts（snippets_root 加 stdin）；wrapper Err 臂消费 __describe
- server.rs：RuneRequest 增 root/stdin 可选字段；daemon 侧根三序收口
- CLI：管道双义消歧（有实参管道即参数、无实参管道即源码）；--root 递 daemon；提示语更新（--root 生效面）
- 披露：surface rune 描述重写（三形态定位加四函数加三通道）、http-api 签名表加四行加输入三通道注记、ADR-0010 增补节、README 两行例、CHANGELOG 0.25.0

## 实证

vm_face 增 render_and_js_wrapper_face（render 过桥含双引号值、Strict 未填报 op 结构化降形）与六件全绿；e2e 增 rune_three_channel_integration（真 chrome：js 主世界页面函数 triple(7)=21、引号值往返、js_lib 库内 Jinja 21 乘 2=42、render 5x、stdin 参数道，五键回执锁定）。实弹装机面（BROWSE_NAME 隔离实例）：管道参数道 js=42 加 lib=40 加 stdin=hello-pipe；管道源码道同脚本同果 stdin 空串；--root 经 daemon 喂到 js_lib 实证。坑实录：json 模块与 Debug 协议对外部类型都出「object at 0x」占位（CdpError 的 ? 传播载荷丢 op/message），宿主侧 __describe 专降 JSON 形、脚本自造型回落 VM 内 Debug 双轨；旧 daemon 残留两吃缺函数假象（收割重启即愈）；一次过宽 pkill 可能波及 9902 他人实例（自愈面在，对方下次调用自动重拉）。[实证: fmt、clippy -D warnings、test --workspace 全绿、vm_face 6、e2e 真 chrome 7 件（含三通道）、aidoc --check --strict、metadata --locked、surface_contract 8（--llms 恰 120）、PEVO]

## 发版 v0.25.0

能力新增取 minor；评审闸门走 browse-codex-review，与 0.24.1（已本地 commit 4851c4d，e2e 6/6 后台补验绿）同场送审，双 tag 双发。
