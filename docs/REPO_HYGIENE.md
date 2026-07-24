# What must not be uploaded

## Never commit

| Path / pattern | Why |
|----------------|-----|
| `target/` | Build output (huge) |
| `dist/` | Local release zips |
| `vendor/`, `reference/` | Third-party clones, not our product |
| `*.exe`, `*.pdb` | Binaries (except packaging scripts) |
| `.env`, `*.pem`, `*.key`, `auth.json` | Secrets |
| `~/.grok/` contents | Live auth & sessions — **local only** |
| `%APPDATA%/GrokApp/` | User config, attachments — **local only** |
| WeChat / screenshot dumps | Unrelated |
| `scripts/_*.py` | One-off agent scratch |

Covered by root `.gitignore`.

## Safe to commit

| Path | Why |
|------|-----|
| `src/`, `tests/`, `examples/` | Source |
| `assets/` | Icons / logo |
| `packaging/` | Install scripts |
| `preview/`, `docs/`, `design-system/` | Docs / site |
| `Cargo.toml`, `Cargo.lock` | Reproducible builds |
| `.github/workflows/` | CI / Release |

## Before every push

```bash
git status
git check-ignore -v path/you/worry/about
# Ensure no secrets:
#   auth.json, .env, real API keys, personal absolute paths
```

## Hardcoded defaults (OK)

Defaults like model `grok-4.5`, app folder `GrokApp`, and CLI install URL are **product defaults**, not your secrets.  
User overrides live only on disk under AppData / `~/.grok` — see [CONFIGURATION.md](CONFIGURATION.md).
