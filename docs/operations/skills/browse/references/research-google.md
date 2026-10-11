# google-search 研究道(Phase 1 产物手册)

> 双脚本对在 workspace 仓 `page-skills/google-search/`(REQ-035 形);知识底座 [[R104-pattern-serp-selector-drift]]、[[R105-pattern-csp-safe-vendored-injection]]。

## 两步调用

```bash
browse userscript install page-skills/google-search/probe.user.js --restart
browse rune page-skills/google-search/mech.rn --vars q=<查询词> --vars top=5 > research.json
```

- `q` 任意串(页面侧 encodeURIComponent);`top` = 跟进访问条数(请求恒 num=10,num<10 实测缩结果)
- 温机 top=5 全程约 8s;冷机 +8s 引擎冷起

## 返回 schema

```json
{
  "probe": { "verdict": "ok|consent|challenged|empty|nav-stall", "drift": "none|suspected",
    "results": [{"title","url","wrapped":"goto|null","snippet","via"}],
    "markdown": "SERP 摘要(mdream@2.0.1)", "converter": "…",
    "ai_overview": {"present":true,"text","markdown","converter"},
    "settled": true, "probe_debug": [{"t":0,"ev":"…"}], "v": "选择器版本" },
  "pages": [{"url","finalUrl","nav":"ok|failed","title","ready","text","html"}],
  "errors": [{"op","message"}]
}
```

## 三要素在机制里的落点(事件驱动/原子错误/超时兜底)

- **事件驱动**:probe(document-start)指纹稳定判收齐,settled attr + `/events` 回调一次;mech 单次求值内挂 attr 观察器写入即走;结果页 readystatechange-complete 事件取内容;AI Overview 块 MutationObserver(变更重置待稳,两拍稳即收)
- **原子错误**:nav-stall(9s 提交探测尽)/ probe 未现(20s 帽尽)/ 单页导航失败(nav:failed)/ 挑战终态(verdict=challenged 带证据)都回单 JSON 证据面,不假绿
- **超时兜底**:提交探测 9s、settled 等 20s、consent 重等 15s、单页 complete 等 12s、AI 块 10s、整跑 --timeout(缺省 60s,长跑显式给)

## 判读语义

- verdict=challenged:见 turnstile/captcha/sorry,如实回执,**不设 HTTP 抓 google 兜底**
- goto 包裹(2026-10 新形):结果锚 `google.com/goto?url=<opaque>`,真址只能访问后得知,`pages[].finalUrl` 即落定真址
- drift=suspected:主选择器漂移(兜底道顶上),连续 2 次该升选择器;版本在 probe.v
- probe_debug 时间线:挂起直读 research.json 定位,不猜

## 升级与维护

- 升级:`browse workspace update` 后重跑两步
- mdream 钉 2.0.1 exact(scripts/vendor/ 三件套,probe 字节等内嵌,workspace check 守卫)
