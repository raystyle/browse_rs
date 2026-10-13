# evo-explore

browse_rs 的浏览器研究插件(自托管市场,ADR-0014)。内含 **browse** skill。

## 安装(本地路径市场)

```bash
claude plugin marketplace add /mnt/wsl/repos/browse_rs   # 或 GitHub 形:raystyle/browse_rs
claude plugin install evo-explore@browse_rs
```

## browse skill 做什么

- **google-search 研究道**(双脚本对,workspace 仓):搜索判读 + 结果页 cdp 开标签并行访问取渲染内容 + AI Overview 流式采集;verdict/drift/挑战终态语义
- 引擎面:起停(headless/headful)、托管 Chromium 装版/pin/体检
- rune 编排与 CDP 直通、页内求值、油猴脚本管理
- 命令语法活权威 `browse --llms`;本插件给道路由与纪律

## 三要素纪律(核心编写逻辑)

事件驱动(观察器/回调即走)、原子错误(失败面单 JSON 证据面)、超时兜底(所有等待段有帽)。详见 SKILL.md 纪律红线与 references。

## 结构

- `.claude-plugin/plugin.json` / `.codex-plugin/plugin.json`:双端 manifest
- `skills/browse/`:skill 真文件视图(权威源 `docs/operations/skills/browse/`,`tools/skill-views.ps1 check` 守卫字节等)
