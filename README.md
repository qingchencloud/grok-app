# Grok App / Grok Desktop

**Unofficial** native desktop client for the official [Grok Build CLI](https://github.com/xai-org/grok-build) (`grok agent stdio` · ACP).

> Not affiliated with xAI. “Grok” is a trademark of xAI. This project is a community GUI shell over the published CLI.

```
┌─────────────────┐     ACP JSON-RPC      ┌──────────────────────┐
│  Grok Desktop   │ ───────────────────►  │  grok agent stdio    │
│  (this app)     │ ◄───────────────────  │  (official CLI)      │
└─────────────────┘   session/update      └──────────────────────┘
```

| | |
|--|--|
| **Repo** | [`qingchencloud/grok-app`](https://github.com/qingchencloud/grok-app) (private until ready) |
| **UI language** | English (default) · 中文 — *Settings → Appearance → Language* |
| **中文文档** | [docs/zh/README.md](docs/zh/README.md) |
| **Download clients** | **[Releases](https://github.com/qingchencloud/grok-app/releases)** ← versioned zips |
| **How to cut a release** | [docs/RELEASE.md](docs/RELEASE.md) |
| **Config reference** | [docs/CONFIGURATION.md](docs/CONFIGURATION.md) |
| **What not to upload** | [docs/REPO_HYGIENE.md](docs/REPO_HYGIENE.md) |
| **Landing page** | [preview/](preview/) |

## Download (end users)

1. Open **[Releases](https://github.com/qingchencloud/grok-app/releases)**  
2. Pick a version (e.g. `v0.1.0`)  
3. Download:
   - **Windows:** `GrokDesktop-<ver>-windows-x64.zip`  
   - **macOS:** `GrokDesktop-<ver>-macos-*.zip`  
4. Unzip → run `Launch.bat` (Windows) or `./GrokDesktop` (macOS)  
5. Install & login CLI if needed:

```powershell
# Windows
irm https://x.ai/cli/install.ps1 | iex
grok login
```

```bash
# macOS / Linux
curl -fsSL https://x.ai/cli/install.sh | bash
grok login
```

> Private repo: only people with access can download until the repo is public.

## Features

| Area | Notes |
|------|--------|
| Agent | Manages `grok agent stdio` |
| Chat | Streaming replies, thoughts, tools, plans |
| Sessions | App-owned index + import from CLI `~/.grok/sessions` |
| Model / cwd | Top bar + settings |
| Permissions | Always-approve or interactive |
| Images | Paste / attach / drag-drop as ACP image blocks |
| CLI install | One-click install from settings (Windows) |
| Config map | Settings write `~/.grok/config.toml` carefully |

## Requirements

1. [Grok Build CLI](https://x.ai/cli)  
2. `grok login` once  
3. Rust toolchain **only if building from source**

## Build from source

```bash
git clone https://github.com/qingchencloud/grok-app.git
cd grok-app
cargo run --bin GrokDesktop
cargo test --test core_logic
cargo build --release
# → target/release/GrokDesktop(.exe)
```

Windows packaging:

```powershell
.\packaging\build-release.ps1
# → dist\GrokDesktop-<ver>-windows-x64-*.zip
```

## CI & releases

| Workflow | Trigger | Output |
|----------|---------|--------|
| **CI** | push / PR | Tests + build check |
| **Release** | tag `v*.*.*` or manual | **GitHub Release** with downloadable zips |
| **Pages** | `preview/**` | Static landing (enable Pages in settings) |

**Publish a version:**

```bash
git tag v0.1.0
git push origin v0.1.0
# or: Actions → Release → Run workflow → version 0.1.0
```

See [docs/RELEASE.md](docs/RELEASE.md).

## Configuration

User settings live **outside the repo** (`%APPDATA%\GrokApp\config.json`).  
Auth lives in **`~/.grok/auth.json`** (CLI). Never commit secrets.  

Defaults (model list, install URL, etc.) are documented in [docs/CONFIGURATION.md](docs/CONFIGURATION.md).

## Topics / keywords

`grok` · `grok-app` · `grok-desktop` · `GrokDesktop` · `xai` · `ACP` · `agent-client-protocol` · `rust` · `egui` · `eframe` · `desktop` · `ai-agent` · `coding-agent` · `cli` · `stdio` · `qingchencloud`

## License

MIT — see [LICENSE](LICENSE).
