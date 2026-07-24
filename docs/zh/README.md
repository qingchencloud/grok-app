# Grok App / Grok Desktop（中文）

**非官方**桌面客户端：基于官方 [Grok Build CLI](https://github.com/xai-org/grok-build) 的 `grok agent stdio`（ACP）。

> 与 xAI 无隶属关系。「Grok」为 xAI 商标。

英文主文档：[README.md](../../README.md)

| | |
|--|--|
| 仓库 | [qingchencloud/grok-app](https://github.com/qingchencloud/grok-app)（当前私有） |
| **下载安装包** | **[Releases](https://github.com/qingchencloud/grok-app/releases)** |
| 发版说明 | [docs/RELEASE.md](../RELEASE.md) |
| 配置项 | [docs/CONFIGURATION.md](../CONFIGURATION.md) |
| 勿上传内容 | [docs/REPO_HYGIENE.md](../REPO_HYGIENE.md) |
| 界面语言 | 设置 → 外观 → Language（English / 中文） |

## 下载客户端（重要）

1. 打开仓库 **[Releases](https://github.com/qingchencloud/grok-app/releases)**  
2. 选择版本（如 `v0.1.0`）  
3. 下载：
   - Windows：`GrokDesktop-<版本>-windows-x64.zip`
   - macOS：`GrokDesktop-<版本>-macos-*.zip`
4. 解压后：
   - Windows：双击 `Launch.bat`（便携）或 `Install.bat`（安装到当前用户）
   - macOS：运行 `./GrokDesktop`（可能需在「隐私与安全性」中允许）
5. 本机仍需 Grok CLI 并登录：

```powershell
# Windows
irm https://x.ai/cli/install.ps1 | iex
grok login
```

```bash
# macOS
curl -fsSL https://x.ai/cli/install.sh | bash
grok login
```

私有仓库时，仅有权限的协作者可下载；公开后所有人可下。

## 如何打指定版本包（维护者）

```bash
# 推荐：打 tag 触发 CI 自动打包并上传到 Releases
git tag v0.1.0
git push origin v0.1.0
```

或在 GitHub：**Actions → Release → Run workflow**，填写版本号 `0.1.0`。

详见 [RELEASE.md](../RELEASE.md)。

## 功能

- Agent 连接、流式对话、思考/工具/计划  
- 会话索引（与 CLI 隔离，可导入 CLI 会话）  
- 模型 / 工作目录 / 权限 / 图片附件  
- 设置映射 `~/.grok/config.toml`  
- 中英文界面切换  

## 从源码构建

```bash
git clone https://github.com/qingchencloud/grok-app.git
cd grok-app
cargo run --bin GrokDesktop
cargo test --test core_logic
```

Windows 打包：`.\packaging\build-release.ps1`

## 配置与隐私

- 桌面配置：`%APPDATA%\GrokApp\config.json`（不进 Git）  
- 登录态：`~/.grok/auth.json`（不进 Git）  
- 默认模型等产品常量见 [CONFIGURATION.md](../CONFIGURATION.md)  
- **禁止上传**：`target/`、`dist/`、`vendor/`、密钥、本机会话目录 — 见 [REPO_HYGIENE.md](../REPO_HYGIENE.md)

## 许可证

MIT — 见 [LICENSE](../../LICENSE)。
