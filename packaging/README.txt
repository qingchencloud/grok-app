Grok Desktop
============

原生 Windows 桌面客户端，作为官方 Grok Build CLI 的图形前端。

系统要求
--------
- Windows 10 / 11 (x64)
- 已安装 Grok CLI，并完成登录（与 TUI 共用认证）

安装 Grok CLI（目标电脑上执行一次）
----------------------------------
  irm https://x.ai/cli/install.ps1 | iex
  grok login

安装本客户端
------------
方式 A — 安装到当前用户（推荐，无需管理员）
  1. 解压本压缩包
  2. 双击 Install.bat
     或: powershell -ExecutionPolicy Bypass -File .\Install.ps1
  3. 从开始菜单 / 桌面启动 “Grok Desktop”

方式 B — 绿色便携
  直接双击 GrokDesktop.exe 即可（无需安装）

卸载
----
  开始菜单 → Grok Desktop → 卸载
  或运行安装目录中的 Uninstall.ps1
  设置 → 应用 里也可看到 “Grok Desktop”（当前用户安装）

使用说明
--------
- 聊天、会话、工具调用均通过本机 `grok agent stdio`（ACP）
- 崩溃日志: %APPDATA%\GrokApp\crash.log
- 配置: %USERPROFILE%\.grok\config.toml

版本与构建
----------
见 VERSION.txt
