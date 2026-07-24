# Changelog

All notable changes to **Grok Desktop** (`qingchencloud/grok-app`) are documented here.

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
