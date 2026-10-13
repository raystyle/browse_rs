# 双脚本对与油猴面(G001 细则配套)

> 权威页:docs/operations/G001-userscripts.md(置位/安装/API 面/验收口径/事件回调全档);本页收 agent 高频面与三要素交点。

## 双脚本对形态(REQ-035)

`page-skills/<slug>/{mech.rn, probe.user.js, README?}` 成对才算技能:

- **probe.user.js**(油猴世界):`==UserScript==` 头(@match 收窄、@run-at、用 GM 必带 @grant+@connect 127.0.0.1)+ `/* @detect */` 块(facts-only 判读,vm 可测)+ 本体 IIFE
- **mech.rn**(宿主编排):导航、跨标签、收果;轻编排重数据(数据整形在 js 小程序)
- 主断言 = `documentElement.setAttribute('probe', JSON)`(跨世界共享,G001 §5)
- workspace tests/check.ts 门:对册在册数、detect 标记成对、GM 纪律、vendor 钉版守卫

## 通道纪律

- settle 单读铁律:注入等待静默后单读至多补读一次,禁高频 `--js` 轮询(每次求值即 DevTools attach 自扰);内容稳定窗用**指纹稳定**判据(SERP 类动画页「DOM 静默」永不到来)
- 事件回调:探针 `GM_xmlhttpRequest` POST daemon `/events`(type 两档 see/stop;@connect 与 URL 主机一致,127.0.0.1 ≠ localhost);消费 `browse events`
- vendored 库进探针:官方 IIFE 单文件字节等内嵌 + sha256 边车 + LICENSE 三件套,测试守卫字节比对;调用侧三防(Promise 兼形/降级链/竞速帽,见 [[R105-pattern-csp-safe-vendored-injection]])

## 三要素交点

- 事件驱动:探针是页面侧事件源(observer/回调);mech 是消费方(观察器等待,拿到即走)
- 原子错误:探针判读终态(consent/challenged/empty)与 mech 各等待段错面统一单 JSON
- 超时兜底:双侧各有帽;探针帽尽给 settled=false 面或 absent,mech 帽尽原子错早退
