# ADR 索引

| 编号 | 决定 | 状态 |
|---|---|---|
| [ADR-0001](ADR-0001-daemon-persistent.md) | 常驻 daemon + CLI 自动拉起 | accepted |
| [ADR-0002](ADR-0002-dialect-faithful-port.md) | 方言忠实移植，不做子命令操作面 | accepted |
| [ADR-0003](ADR-0003-attach-first-spawn-fallback.md) | 附着优先缺则自起；spawn 带 --no-sandbox | accepted |
| [ADR-0004](ADR-0004-auto-connect-attach-first-page.md) | 自动连接 + attach 首个 page target | accepted |
| [ADR-0005](ADR-0005-pipe-channel.md) | spawn 管道通道（S005，Windows 先行） | accepted |
| [ADR-0006](ADR-0006-multi-instance-name.md) | BROWSE_NAME 多实例命名空间 | accepted |
| [ADR-0007](ADR-0007-embedded-chromium-manager.md) | 内嵌 Chromium 版本管理器托管 clean-chrome | accepted |
| [ADR-0008](ADR-0008-workspace-skill-layers.md) | goto 回执技能触发层与 workspace 单仓外置知识 | accepted |
| [ADR-0009](ADR-0009-template-engine-minijinja.md) | JS 命令小程序模板引擎选 minijinja | accepted |
| [ADR-0010](ADR-0010-rune-orchestration-face.md) | Rune 编排面：browse rune 与自注册 cdp 模块（自有栈） | accepted |
| [ADR-0011](ADR-0011-rune-raw-universal-channel.md) | Rune 面通用 CDP 命令通道（cdp::raw）与糖皮回收 | accepted |
| [ADR-0012](ADR-0012-unified-fallback-timeout.md) | 统一兜底超时（--timeout 旗标、60 秒缺省、卡点回执同形） | accepted |

规则：一条 ADR 一个决定；被替代时 frontmatter 改 `status: superseded` 并加
`superseded_by: ADR-00xx`（指向在册编号），原文冻结；Context 写给 18 个月后
的陌生人；Consequences 必须有坏的一面。模板见 [0000-template.md](0000-template.md)。
