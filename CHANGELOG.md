# Changelog

All notable changes to **Grok Desktop** (`qingchencloud/grok-app`) are documented here.

## [0.1.8] — 2026-08-07

### Added
- **Dynamic slash palette** merges host commands with agent `available_commands_update` (CLI tools such as `/usage`, `/goal`, workflows)
- **`x.ai/ask_user_question` modal** — structured single/multi-select answers, plan-mode Chat/Skip, dismiss
- **Usage chip** in the composer bar (context tokens + refresh via `/usage`)
- **Announcement banner** from `_x.ai/announcements/update` (dismissible per id)

### Notes
- Requires Grok Build CLI 1.0+ for full behavior

## [0.1.7] — 2026-08-07

### Added
- **Auto** permission mode (`session/set_mode` id `auto`) in the Shift+Tab cycle and mode picker — aligned with Grok CLI 1.0
- Slash commands: `/auto`, `/workflow` (plus existing `/goal`, `/plan`)
- ACP `authenticate` with `cached_token` after `initialize` (graceful if it fails)
- Live model catalog from `_x.ai/models/update` and `session/new` models payload
- `session/new` `_meta.yoloMode` / `_meta.autoMode` per CLI 1.0 ACP docs

### Changed
- Agent version now reads CLI 1.0 `_meta.agentVersion` when top-level `agentInfo` is absent
- Mode cycle is **Normal → Auto → Plan → Always-Approve**
- Switching modes also writes `[ui].permission_mode` in `~/.grok/config.toml` when present

### Notes
- Compatible with **Grok Build CLI 1.0.0**; core ACP chat path remains `grok agent stdio`
- Install with **`GrokDesktop-Setup-0.1.7-windows-x64.exe`** when published

## [0.1.6] — 2026-07-29

### Added
- In-app **Share Grok Desktop** dialog from the sidebar and Settings → About
- Localized share copy with project homepage and latest Windows download links

### Fixed
- Logging in after the desktop agent has started now restarts the agent so it loads the new Grok CLI credentials
- Runtime `Authentication required` responses now trigger a safe reconnect or return the client to the login guide instead of leaving chat unusable
- The Windows executable now embeds the Grok application icon, so installed taskbar and shortcut entries no longer fall back to the generic window icon

### Notes
- Install with **`GrokDesktop-Setup-0.1.6-windows-x64.exe`** to upgrade the existing per-user installation

## [0.1.5] — 2026-07-29

### Fixed
- Windows upgrades now stop the existing Grok Desktop process tree before replacing files
- The installer no longer gets stuck when the running app turns a close request into **Hide to tray**
- The active `grok agent` child process is also stopped during upgrade, preventing an orphaned tray/session process

### Notes
- Install with **`GrokDesktop-Setup-0.1.5-windows-x64.exe`**; it upgrades the existing per-user installation in place

## [0.1.4] — 2026-07-29

### Added
- Codex-style new-chat workspace: choose a project, compose the first request, then create the real CLI session
- Grok CLI mode selector in the composer: **Normal → Plan → Always-Approve**, including `Shift+Tab`
- Grok Imagine image generation through the official xAI Images API with session-only API-key handling
- System-language detection with an explicit manual language override
- First-run checks for Grok CLI installation and CLI login state

### Changed
- Reworked the chat page, composer, dialogs, message/tool rendering, light/dark themes, and responsive layout
- Moved model, reasoning effort, workspace, attachments, and execution mode into a clearer composer workflow
- Updated product wording to describe Grok Desktop as a visual client for Grok CLI, without requiring terminal interaction after setup

### Fixed
- Existing-session prompts are hard-bound to the selected session and no longer silently create or run in another conversation
- Historical replay and stale-session events can no longer prematurely mark a live turn complete
- A real Stop control remains visible while the ACP prompt is active and sends `session/cancel`
- Reconnects and app exit now clean up superseded `grok agent` child processes
- Tray **Quit** exits the application instead of leaving an uncloseable tray process
- Image-generation and other dialogs can be closed via title close, Cancel, backdrop click, or `Esc`

### Notes
- Install with **`GrokDesktop-Setup-0.1.4-windows-x64.exe`** for Desktop + Start Menu shortcuts

## [0.1.3] — 2026-07-25

### Added
- **Skills** manager in Settings: list/search/filter, enable/disable (`[skills].disabled`), open file/folder
- **System tray**: show/hide/quit; optional close-to-tray (X hides instead of quit)
- **Desktop notifications** when an agent turn finishes (optional: only when window is in background)

### Fixed
- Composer input no longer grows without bound; capped height with internal scroll (attachments strip capped too)

### Notes
- Install with **`GrokDesktop-Setup-0.1.3-windows-x64.exe`** for Desktop + Start Menu shortcuts

## [0.1.2] — 2026-07-25

### Fixed
- **Windows Setup**: real per-user install to `%LOCALAPPDATA%\Programs\Grok Desktop\`, always creates Desktop + Start Menu shortcuts and an Add/Remove Programs entry
- **Console flash**: opening Settings / probing CLI / opening links no longer flashes a black terminal window (`CREATE_NO_WINDOW` + `ShellExecuteW`)

### Notes
- Download **`GrokDesktop-Setup-*-windows-x64.exe`** for install (not the portable single-file `.exe`)

## [0.1.1] — 2026-07-25

### Added
- In-app update checks via GitHub Releases (`qingchencloud/grok-app`)
- Formal changelog modal: download / later; corner badge stays after “Later”
- Bottom-left update reminder when a newer build is available
- Settings → About: check updates, open releases, startup check toggle
- OS title bar shows app version (`Grok  vX.Y.Z`)

### Notes
- Primary Windows asset: `GrokDesktop-Setup-<ver>-windows-x64.exe`
- Demo override for local UI preview: `GROK_DEMO_UPDATE=1` (not used in release builds)

## [0.1.0] — 2026-07-24

### Added
- Native desktop shell over `grok agent stdio` (ACP)
- Streaming chat, thoughts, tools, plans
- App-owned session index + CLI import / archive
- Settings: model, effort, cwd, profile, language (**en** / **zh**)
- Image paste / attach / drag-drop
- Windows packaging scripts (`packaging/`)
- CI (Linux test, Windows + macOS release builds)
- Release workflow: versioned zips on GitHub Releases
- Landing page (`preview/`) + bilingual docs

### Notes
- Unofficial client; auth and tools remain with the official Grok CLI.
