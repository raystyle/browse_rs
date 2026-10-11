# 2026-10-10 evo-explore Phase 1:google-search 双脚本对(con-06 件1-3)

con-06 施工单(project-evo 总台派发,全案 /home/ray/.claude/plans/binary-orbiting-kitten.md)。本篇是过程轨迹与蒸馏素材;产物事实在 workspace 仓 README 与 receipt。

## 裁定链(用户令,按序)

1. **con-06-add1 并单**:mdream 弃 esbuild/@mdream/js ESM 转译线(那是为绕 google.com CSP 的 cdp::js 主世界道),改官方 IIFE 发行形(WASM base64 内嵌)经油猴接口加载:userscript 上下文不吃页面 CSP。实测钉 2.0.1:`window.mdream.htmlToMarkdown(html, opts)`,npm tarball `dist/iife.js` 325KB。
2. **事件回调不等**:90s/25s 等待不可接受 ->「拿到完整 url 列表就停」;检测机制归探针(油猴),rn 拿到回调即走。
3. **正确流程**:快取结果 url -> 逐条访问取内容;后放宽「可以并行」。
4. **访问道修正**:不是 gmFetch(探针 GM XHR;@connect * 实测与守卫/预热竞态纠缠),是 **cdp 开标签访问**取**渲染内容**(HTML 渲染后),事件回调(readystatechange-complete)取到即走。
5. **AI Overview 采集**:AI 分析块有流式加载渲染过程,探针事件道检测,不瞎等。

## 机制沉淀(蒸馏素材,R104/R105 候选)

- **收齐信号(指纹稳定)**:Google SERP 动画变更持续不断,「DOM 静默」判据会被饿死;正确判据 = **结果指纹稳定**(url 列表两拍不变 ≥500ms)。document-idle 注入太晚(Google idle 尾 ~8s),**document-start + interval 轻采样**(250ms 指纹;变更只记时不排程:去抖排程被变更流饿死是另一坑)。settled attr + `/events` 回调一次携 url 列表;mech 侧单次求值内挂 attr 的 MutationObserver,写入即返。
- **新 SERP goto 包裹(2026-10)**:结果锚 href 全是 `google.com/goto?url=<opaque>`(实测 num=10 页 12 h3 中 6 真结果全包裹,0 直链);服务端跟跳,**真址只能访问后得知**:cdp 标签访问后 `location.href` 即真址(输出记 finalUrl)。内链过滤器须把 goto 识别为结果包装而非内链,否则全灭(verdict=empty 假象)。
- **Google num 参数**:num<10 实测缩自然结果(num=6 只出 3 条):恒请求 num=10,top 只管跟进条数。
- **无头与降级**:A/B 实测有头/无头同形(12 h3/12 goto/同 UA);「3 条结果」不是降级是 num=3 生效;无头 UA 干净(Chrome 155 新无头自身不带 Headless 标记,browse 未做 UA 改写;加固组=disable-gpu/noerrdialogs/ozone-platform=headless+800x600/swiftshader-webgl,crates/cdp/src/spawn.rs:206)。
- **mdream IIFE 陷阱**:d.ts 声 `htmlToMarkdown(): string`,实测返 **Promise**(必须 await;不 await 被 String() 化成 "[object Promise]");minimal 形(isolateMain)对 SERP 常取空,降级链 minimal->clean->plain;单次 wasm 转换 ~67ms(SERP 全页 16K 字输出)。
- **多标签 cdp 编排**:关签守卫只认方言道自建(newTab/switchTab/closeTab,raw createTarget 建的签 closeTab 被拦「不是本会话自建」);**closeTab 后活动路由仍指死 session**:参数构建 js 会撞 "Session with given id not found",修法 = 提取循环不关签,回原签后统一关(closeTab 浏览器级,不必目标签活动)。空签批量建+逐签点火导航 = 加载并行,轮签事件取内容,温机 top=5 全程 ~8s(初版串行+load 尾等待 25-30s)。
- **AI Overview 流式**:settled 时块常未到/未稳;探针挂 MutationObserver(characterData+subtree)事件道:变更重置待稳,文本两拍稳(≥1.2s 不变)即收,10s 帽;块级 mdream 摘要异步补。
- **pwsh 流重定向怪癖**:native 命令 stderr 提示行(browse 的「管道读入为空串」)混进 `>` 重定向的 json 文件头且 `2>` 目标文件未建;解析侧从首个 `{` 截断兜底,勿赖流 plumbing。
- **验收脚本判定**:pwsh ConvertFrom-Json 对带前缀行文件硬红;退出码纪律照 gate-verdict(脚本自身退出码即门禁)。

## 验收实录(件3)

- 载体:`tests/google-search.accept.ps1`(pwsh 7;隔离=引擎每跑 browse down 重起,串行;判据 results≥5 ∧ verdict=ok ∧ drift=none,≥8/10)
- 第二轮(修复解析后打捞):**8/10 OK**(两败 = 引擎 down/up 竞态快败 1.5s 错形,非判读失败);定局轮(含 AI Overview 终形)实录见 receipt
- 时耗:温机单跑 ~8s(top=5),冷机 ~17s(引擎冷起 ~8s 为主)

## 工程账

- workspace 仓提交 712268a(google-search 三件 + vendor 三件套 + _prelude isGoogleSerp + check 十二对与 vendor 守卫);browse_rs 零代码改动(白名单不吃显式 rune 道的判断经实弹证实)
- mdream vendored:sha256 90a8bada4c1ad1f2d592a380eb198b2393fd2216a788df0fafaf80fd03dd91a3,probe.user.js 字节等内嵌(check.ts 守卫)
- 返工量并单点:add1 时零;v3 标签道改写时 gmFetch 通道整段剥离(约 -60 行死码)
