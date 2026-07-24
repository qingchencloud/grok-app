# Configuration reference

## App config (this desktop client)

**Path (Windows):** `%APPDATA%\GrokApp\config.json`  
**Path (macOS/Linux):** `$XDG_CONFIG_HOME/GrokApp/config.json` or `~/.config/GrokApp/config.json`

| Key | Default | Meaning |
|-----|---------|---------|
| `grok_path` | `""` (auto) | Path to `grok` binary; empty → `~/.grok/bin` + `PATH` |
| `cwd` | process cwd | Default working directory for sessions |
| `model` | `grok-4.5` | Default model id (override in UI) |
| `effort` | `medium` | `low` / `medium` / `high` → CLI reasoning effort |
| `always_approve` | `true` | Pass `--always-approve` to agent |
| `dark_mode` | `true` | Theme |
| `ui_locale` | `en` | `en` or `zh` |
| `font_scale` | `1.0` | UI font scale 0.85–1.35 |
| `auto_connect` | `true` | Connect agent on launch |
| `smooth_stream` | `true` | Smooth token drip |
| `show_thoughts` | `true` | Show thinking blocks |
| `expand_tools` | `false` | Expand tool rows by default |
| `enter_to_send` | `true` | Enter sends message |
| `user_display_name` | `""` | Chat display name (empty → Me / 我) |
| `user_avatar_path` | `""` | Local image for avatar |
| `extra_agent_args` | `[]` | Extra args before `stdio` |
| `window_width` / `window_height` | 1440 / 920 | Last window size |

These are **defaults**, not secrets. Change them in **Settings** or by editing the JSON after quitting the app.

## CLI config (shared with Grok TUI)

**Path:** `~/.grok/config.toml` (or `$GROK_HOME/config.toml`)

Written carefully from **Settings → Advanced**. Do not put API keys in the desktop repo.

## Auth (never commit)

| Item | Location |
|------|----------|
| CLI login | `~/.grok/auth.json` or `XAI_API_KEY` env |
| Desktop secrets | **None** — auth is CLI-owned |

## Hardcoded product constants (source)

| Constant | Where | Why |
|----------|--------|-----|
| App dir name `GrokApp` | `config.rs` | OS config folder name |
| Default model list | `config.rs` `MODELS` | Picker suggestions (editable) |
| CLI install URL | `install.rs` `INSTALL_URL` | Official `https://x.ai/cli/...` |
| Binary name `GrokDesktop` | `Cargo.toml` | Ship name |

No machine-specific paths (e.g. `D:\…`) or personal tokens belong in the repository.
