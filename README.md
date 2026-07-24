# Grok App / Grok Desktop

**Unofficial** native desktop client for the official [Grok Build CLI](https://github.com/xai-org/grok-build) (`grok agent stdio` · ACP).

> Not affiliated with xAI. “Grok” is a trademark of xAI. This project is a community GUI shell over the published CLI.

```
┌─────────────────┐     ACP JSON-RPC      ┌──────────────────────┐
│  Grok Desktop   │ ───────────────────►  │  grok agent stdio    │
│  (this app)     │ ◄───────────────────  │  (official CLI)      │
└─────────────────┘   session/update      └──────────────────────┘
```

- **Repo:** private working tree under `qingchencloud/grok-app` (public later)
- **UI languages:** English (default) · 中文 — Settings → Appearance → Language  
- **中文文档:** [docs/zh/README.md](docs/zh/README.md)  
- **Landing page:** [preview/](preview/) (static bilingual site)

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
3. Rust toolchain for development

## Build & run

```bash
git clone https://github.com/qingchencloud/grok-app.git
cd grok-app

# Dev (watch + restart on Windows)
# .\dev.ps1

cargo run --bin GrokDesktop
cargo test --test core_logic
cargo build --release
# → target/release/GrokDesktop.exe
```

Packaging (Windows):

```powershell
.\packaging\build-release.ps1
```

## Configuration

App config: `%APPDATA%\GrokApp\config.json` (or platform equivalent)

| Key | Meaning |
|-----|---------|
| `ui_locale` | `en` (default) or `zh` |
| `dark_mode` | Theme |
| `model` / `effort` | Agent defaults |
| `user_display_name` / `user_avatar_path` | Chat identity |

## CI

GitHub Actions (`.github/workflows/ci.yml`):

- `cargo fmt --check`
- `cargo test`
- `cargo build --release` (Windows job)

## Topics / keywords

`grok` · `grok-app` · `grok-desktop` · `GrokDesktop` · `xai` · `xAI` · `grok-build` · `ACP` · `agent-client-protocol` · `rust` · `egui` · `eframe` · `desktop` · `ai-agent` · `coding-agent` · `stdio` · `cli-gui` · `qingchencloud`

## License

MIT — see [LICENSE](LICENSE).
