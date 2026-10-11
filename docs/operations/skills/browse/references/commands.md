# browse 命令道速查(路由配套)

> 活权威:`browse --llms`(投影 docs/surface/,tests/surface_contract.rs 锁漂移)。本页只收高频道与组合形,不复制全字典。

## 引擎面

```bash
browse up [--headless] [--proxy http://…:5330] [--profile <dir>]   # 起(附着优先,缺则 spawn 隔离实例)
browse down                                                        # 关自起引擎(不碰用户浏览器)
browse status [--json]                                             # daemon/引擎态
browse chrome install <版本>          # 托管 Chromium 装版并 pin(镜像下载 sha256 锚校验)
browse chrome list / use / update / remove / doctor                 # 托管集管理
```

- 无显示会话有头意图自动补无头;linux 无头自动叠加固组(disable-gpu / ozone headless 等)
- 无头 UA 与有头同串(155 新无头),browse 不做 UA 改写

## 读与求值

```bash
browse fetch <url> [--markdown]      # 一次性只读:HTTP 直取,三条件自动升引擎腿
browse --js '<片段>'                  # 页内求值,跨调用变量持久;管径源码走管道
browse rune <script.rn> --vars k=v   # Rune 编排:cdp::js / dialect / raw / wait_event
```

- rune 对象字面量冒号形(`#{"k" : v}`);js 体 `(async (args) => {…})(argsJson)`,args 对象注入引号天然安全
- 多步流程、CDP 直通、跨标签编排走 rune(见 research-google.md 的 mech 实例)

## 油猴与技能对

```bash
browse userscript install <file> --restart   # 装(幂等,--restart 生效)
browse userscript list / enable / disable / remove / update / path
browse workspace status / install / update / list / site / page   # 技能仓管理
browse events [--since N] [--last 0]         # 探针回调事件环(含 cta 建议)
```

## 治理

```bash
browse issue new <标题> --acceptance <验收>   # 缺陷一键反馈(账本真源)
browse artifact publish / attest / list       # 产物登记
```
