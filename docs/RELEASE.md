# Releases — one-click Windows installer

## What users download

| File | Use |
|------|-----|
| **`GrokDesktop-Setup-<ver>-windows-x64.exe`** | **Primary** — download → double-click → Install (no zip) |
| `GrokDesktop-<ver>-windows-x64.exe` | Portable single file (run without installing) |
| `GrokDesktop-<ver>-macos-*` | macOS single binary (no zip) |
| `*.sha256` | Checksums |

Open: **GitHub → [Releases](https://github.com/qingchencloud/grok-app/releases)**

## Install (Windows end user)

1. Download **`GrokDesktop-Setup-…-windows-x64.exe`**
2. Double-click → Next → Install  
   - Installs to `%LOCALAPPDATA%\Programs\Grok Desktop\` (no admin)
   - Start Menu shortcut created
3. One-time CLI:

```powershell
irm https://x.ai/cli/install.ps1 | iex
grok login
```

## Cut a release (maintainer)

### Tag

```bash
git tag v0.1.1
git push origin v0.1.1
```

### Or Actions UI

**Actions → Release → Run workflow** → version `0.1.1`

Wait until green → open **Releases** → Setup.exe is attached.

## Local Setup.exe (Windows machine with Inno Setup)

```powershell
# Install Inno Setup 6 once: https://jrsoftware.org/isinfo.php
.\packaging\build-setup.ps1 -Version 0.1.1
# → dist\GrokDesktop-Setup-0.1.1-windows-x64.exe
# → dist\GrokDesktop-0.1.1-windows-x64.exe  (portable)
```

## CI overview

| Workflow | Trigger | Output |
|----------|---------|--------|
| **CI** | push / PR | Tests (smoke artifacts, short retention) |
| **Release** | tag `v*` / manual | **Setup.exe + portable .exe** on Releases |
| **Pages** | `preview/**` | Landing page |

## Versioning

- Tag: `v0.1.1`  
- CI sets `Cargo.toml` version for that build  
- Changelog: [CHANGELOG.md](../CHANGELOG.md)
