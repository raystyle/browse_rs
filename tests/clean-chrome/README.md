# clean-chrome 内核冒烟+回归测试套件

对 clean-chrome 自编 Chromium 内核(非厂牌 Chrome)的出包冒烟与行为回归。面向 REQ-021(Release dcheck 关闭)与 REQ-022(后台节流四层源码级去除)两特性窗,及既有契约面(9222 默认口/auto-allow/pipe 三态/address 绑定/webdriver=false/userscripts 页/静默化/零外联/console 预览)。

## 载体

PEP 723 Python 3.9+、stdlib only、`uv run` 直跑(与 clean-chrome `tools/e2e` 同形;经 AI-LAB 口径裁定 2026-10-15:测试件非运维脚本,不受 pwsh 验收面纪律辖)。无 pytest、无第三方依赖:门要跟着二进制走,不是跟着测试框架走。

## 命令

```bash
# P0 出包冒烟 —— 每个 Release 二进制必跑
uv run tests/clean-chrome/smoke_gate.py --chrome <dir-or-binary> [--headful] [--leg REGEX]

# P0.5 userscripts 存储面 extreme gate(移植自 clean-chrome tools/e2e/extreme_gate.py,12 腿)
uv run tests/clean-chrome/storage_gate.py --chrome <dir-or-binary> [--iterations N]

# P1/P2 后台节流回归 —— 有头桌面机(真最小化,CDP+xdotool+osascript 三级降级,降无可降则 SKIP 不假绿)
uv run tests/clean-chrome/throttle_gate.py --chrome <dir-or-binary> [--headful] [--leg REGEX]
uv run tests/clean-chrome/throttle_gate.py --chrome ... --leg reverse      # 只跑 A/B 反证两腿

# P3 正交记录 + P4 检测面记录 —— 只记账不判失败
uv run tests/clean-chrome/orthogonal_gate.py --chrome <dir-or-binary> [--headful]
```

`--chrome` 接平台二进制、mac `.app` 束或构建/部署目录(自动解析)。退出码 0=全绿、1=有红、2=用法错;`--leg` 正则过滤支持失败腿单跑。判据全部机读:stdout 人话行 + `<work>/receipt.json`(JSON 回执:每腿 name/ok/note/data)。

## 判据与阈值

- L1 回归:≥3 个连续 gap ≥1000ms(单次尖峰=调度噪声不算)
- L3 回归:任一 gap ≥50000ms
- dcheck 扫:主二进制 `DCHECK failed` 计数 ≤1(v8 CSA 格式串字面量豁免)
- 静默面:三态启动(裸/`--enable-automation`/`--no-sandbox`)同窗定尺寸内容视口全等(infobar 挤压内容 → 高度差暴露)
- 零外联:冷启 netlog DNS+URL 双通道扫 Google 域族,零命中(全程域名透明台账入回执)
- A/B 反证:带反向恢复旗标时节流必须回归(reverse-blink 期望 ~1s 对齐、reverse-feature 期望 5 分钟线后分钟级钳制);探测健康反而判红(A/B VOID)

## 预期行为注记(不判回归)

- 折叠标签组冻结、critical 内存压丢弃:S012 定档正交路径,只记录(kimi 评审 F1)
- Runtime.enable 侧信道:隐蔽是产品选择,只记账
- L2 遮挡反向恢复无运行时旗标(源码短路),A/B 需对照库存 Chrome

## 布局

- `lib/minicdp.py` — stdlib CDP client(vendored from clean-chrome tools/e2e,零改)
- `lib/common.py` — boot/kill、平台解析、Origin 夹具、Gate 收腿器与 JSON 回执
- `smoke_gate.py` — P0:16 腿(9222/dcheck/握手/bind/pipe×3/webdriver/userscripts/paused×2/silent×3/egress/console/pipe-priority)
- `storage_gate.py` — P0.5:12 腿存储面
- `throttle_gate.py` — P1/P2:主证 6.5min/深度 5min 线/遮挡/优先级/反向×2
- `orthogonal_gate.py` — P3/P4 记录面

## 已知机器面

- wsl(WSLg)mutter 不响应最小化 → 节流门降级 SKIP(设计如此);mac 上 CDP `Browser.setWindowBounds` 最小化生效
- linux 桌面 Xwayland 需 `XAUTHORITY=/run/user/1000/.mutter-Xwaylandauth.*` + `DISPLAY=:0` 传给门
- pipe 腿的 POSIX fd3/4 契约要求管端先抬到 fd≥10 再 dup2(os.pipe 自返 3/4 会自噬,已修)
