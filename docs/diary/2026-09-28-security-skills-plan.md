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

## 收口批：三篇补 A 面与旗舰实弹（同日再续）

对照源清单逐项盘点再补三篇（49d0d99）：upload-attack-surface、
error-based-probing、insertion-points。至此用户清单全部条目有落点
（A 面 skill、B 层候选或 C 层留档），悬空清零。

旗舰配方实弹（httpbin 靶，引擎跨宿主 spawn 隔离实例）：态往返三段
全通（before 1、clear 后 0、import 后 1）；零侵入探针全通（omit 空
cookies 对 include 带 selftest 的差分成立）。

坑记再两笔：公共实例 /get 不回 cookies 字段（探针改打 /cookies）；
方言对象键名 with 前缀被解析器拒（改名即过）。部署注记：跨宿主
daemon 下 goto 的引擎侧点名不吃 wsl 侧登记（fetch 是 CLI 侧点名故
命中），各端要 goto 点名须本端 add 登记。[实证: browse down 幂等
退场；S001 收口节与本文随批过 PEVO]

## R4 实弹：用户登录加双 lab 双解（同日三续）

环境两役先行：WSLg 通道带伤（weston shared_memory I/O error 致
copy mode，窗口注册任务栏打不开，msrdc 重启无效），`wsl --shutdown`
除根（use_gfxredir 归 1）后有头窗正常，用户本人在窗内登录 Academy，
切无头（engine-profile 持久登录态）。portswigger 裸连被 GFW 间歇
RST，herdr 派单 ccoe 工位（w2E:p1）探明网关家猫双代理口
（192.168.88.2:5330/5340），引擎挂 --proxy-server 后稳定；断言侧
独立复核双口 CONNECT 200，且核出其配方 `<-loopback>` 语义反写
（该 token 是移除回环隐式旁路，与其注释意图相反；默认即旁路，不挂
更稳），已按飞轮吸收回告工位。

双 lab 双解：水平 IDOR（wiener 基线后一发 id 换 carlos 取 API key，
dialogAccept 喂 prompt 交卷）；垂直越权（裸 /admin 403，允许路径加
X-Original-URL 头取面板删 carlos）。[实证: 两题 congrats true；
改进七条回写 skill（判读副作用权威、前端改写头形、四坑、labs 无头
实战节）；S001 R4 节随批]

坑记三笔：prompt 对话框开着时 pageEval 全堵（cdp timeout），先
dialogStatus 再 dialogAccept；「ACCESS THE LAB」是 target=_blank
锚，无头无手势弹不出页，改 a.target="_self" 再 click（点击片段被
导航打断属预期）；launch URL GET 直达 400 必须题页点出。
