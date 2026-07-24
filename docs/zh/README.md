# Grok App / Grok Desktop（中文）

**非官方**桌面客户端：基于官方 [Grok Build CLI](https://github.com/xai-org/grok-build) 的 `grok agent stdio`（ACP 协议）。

> 与 xAI 无隶属关系。「Grok」为 xAI 商标。本项目只是 CLI 的图形壳。

英文主文档见仓库根目录 [README.md](../../README.md)。

## 功能

- Agent 连接、流式对话、思考/工具/计划
- 会话索引（与 CLI 隔离，可导入 CLI 会话）
- 模型 / 工作目录 / 权限 / 图片附件
- 设置映射 `~/.grok/config.toml`
- **界面语言：** 设置 → 外观 → 语言（English / 中文）

## 构建

```bash
git clone https://github.com/qingchencloud/grok-app.git
cd grok-app
cargo run --bin GrokDesktop
cargo test --test core_logic
```

Windows 开发热重载：`.\dev.ps1`  
发布打包：`.\packaging\build-release.ps1`

## 配置

`%APPDATA%\GrokApp\config.json` 中的 `ui_locale`：

- `en` — 英文（默认）
- `zh` — 中文

## 官网单页

仓库内 `preview/` 为中英双语落地页，可配合 GitHub Pages 或静态托管。

## 许可证

MIT — 见 [LICENSE](../../LICENSE)。
