# Changelog

All notable changes to **Grok Desktop** (`qingchencloud/grok-app`) are documented here.

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
