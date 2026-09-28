# 2026-09-28 README 双仓同步与 S001 安全 skill 研究立项

## README 双仓同步（workspace 自定义仓亮面）

browse_rs README 亮面 #63：特性清单加 workspace 条、usage 块 add/remove
双例、块后多根序语义段（序首整胜、env 钉死单根最高）。browse_workspace
仓 README 同步四处（缺省根定位、自定义仓同构契约、部署例、自管仓口径）。

坑记：workspace 部署克隆落后远端 5 提交，且进来的 c1c5dd8 恰好重写过
README（信噪比收口删迁移账目段）。stash 后 ff 拉平再弹回，冲突解法是
保远端干净底只叠新句，被删的 browser-harness 账目不复活。md 禁字挡板
拦一次破折号，改括号收口过。[实证: 提交 0afce4e（browse_rs）与
cf68d98（browse_workspace）双推，工作区双净]

## S001 立项：网络安全 skill 吸收研究计划

用户令（2026-09-28）：开网络安全 skill 研究计划，看 Burp/PortSwigger
生态如何吸收转换为 browse skill，官方入口四类在册（BApp 组织、
BChecks、bambdas、浏览器侧、机制型 BApp、中文圈）。

落 docs/research/S001-security-skills-absorption.md，核心产出三层分型：
A 层知识直转（6 张 skill 表）、B 层原语缺口（改写重放、流量录制、差分
助手，REQ 候选）、C 层结构性不收（代理面、mass fuzz、畸形字节、OOB
基建，各留理由）。R1 至 R4 四批计划各带验收判据；载体裁定安全 skill
走独立自定义仓（#63 多根通道首个自用案例）。

实证锚：BChecks 主动被动两形样例逐字抓自主分支入档；bambdas 五挂点
README 核验。机制型 BApp 回路表暂标 [记忆]，R1 批次开源码复核清零。
[实证: md 禁字挡板过；PEVO 门禁随批跑]

## R2 同日落成：browse-security-skills 仓首批 12 文件

用户加令（2026-09-28）：继续直到全部吸收为 skill，业务逻辑与越权类
重点；新建 browse-security-skills 仓。落 github.com/raystyle/
browse-security-skills（公开仓，root 40c6ac4）：8 篇 intent 加 2 篇
domain 靶站（httpbin 自验靶、portswigger labs）加双 README。

两处 S001 原判修正（实操发现）：page-skills 的 slug 是冻结白名单
（#51 F3 十值），安全配方无挂点，DOM XSS 落 intent 反查；方言已有
`requests()`/`requestDetail`/`responseBody`，B 层「流量录制」缺口
收窄为「批量导出」（raw/HAR 一键落盘）。剥授权三法全用今日原语：
页内 fetch `credentials:"omit"` 零侵入单发、`exportStorageState` 态
往返、`BROWSE_NAME` 双实例。

坑记两笔：hst secretguard 挡板拦 AWS 示例 key 字面量（教学串也不入
库，改文字指代）；JS 正则不吃 `(?i)` 内联旗标，改 `new RegExp` 带旗标
形。[实证: 验收四条全过：add 登记 list 并集双根；fetch 双靶站点名
命中；site 全文读通；rg 意图反查命中；默认仓零改动]
