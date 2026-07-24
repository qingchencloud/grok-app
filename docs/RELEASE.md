# Releases — versioned client downloads

## What you get

Each release attaches **zip** assets:

| Asset | Contents |
|-------|----------|
| `GrokDesktop-<ver>-windows-x64.zip` | `GrokDesktop.exe`, `Launch.bat`, `Install.bat`, install scripts, `VERSION.txt` |
| `GrokDesktop-<ver>-macos-<arch>.zip` | `GrokDesktop` binary, `VERSION.txt`, license |
| `*.sha256` | SHA-256 checksums |

Open: **GitHub → Releases** (private repo: collaborators only until public).

## Cut a release (recommended)

### Option A — tag from your machine

```bash
# 1) bump Cargo.toml version to match (optional but tidy)
# version = "0.1.1"

git add -A
git commit -m "chore: release v0.1.1"
git tag v0.1.1
git push origin main
git push origin v0.1.1
```

Tag pattern: **`vMAJOR.MINOR.PATCH`** (e.g. `v0.1.0`).  
CI workflow **Release** builds Windows + macOS and uploads the zips.

### Option B — GitHub Actions UI

1. **Actions → Release → Run workflow**  
2. Enter version: `0.1.1` (no `v`)  
3. Optionally **draft**  
4. Wait for green → open **Releases** → download zip  

## Install from a release

### Windows

1. Download `GrokDesktop-*-windows-x64.zip`  
2. Unzip  
3. Portable: double-click **Launch.bat**  
   Or install for current user: **Install.bat**  
4. Install CLI + login if needed:

```powershell
irm https://x.ai/cli/install.ps1 | iex
grok login
```

### macOS

1. Download `GrokDesktop-*-macos-*.zip`  
2. Unzip and run `./GrokDesktop`  
3. If Gatekeeper blocks: System Settings → Privacy & Security → Open Anyway  
4. CLI:

```bash
curl -fsSL https://x.ai/cli/install.sh | bash
grok login
```

## Local package (without CI)

```powershell
cargo build --release
.\packaging\build-release.ps1
# → dist\GrokDesktop-<ver>-windows-x64-portable.zip
```

## CI overview

| Workflow | When | Output |
|----------|------|--------|
| **CI** | push / PR to `main` | Test + build artifacts (90-day Actions artifacts) |
| **Release** | tag `v*` or manual | **Permanent** GitHub Release downloads |
| **Pages** | `preview/**` changes | Landing page (enable Pages in settings) |

## Versioning policy

- GitHub tag: `v0.1.0`  
- `Cargo.toml` `version` should match (CI overwrites for the release build)  
- Changelog: [CHANGELOG.md](../CHANGELOG.md)
