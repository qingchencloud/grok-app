# Chat — coding agent (ui-ux-pro-max)

Overrides MASTER. Product: **native desktop IDE chat** (Rust + egui).

## Intent
- Cursor / Linear density for **writing code**
- High body contrast (WCAG AA), monochrome + blue accent
- Tools = log chips; assistant = document stream; user = short command bubble

## Visual rules
| Element | Spec |
|---------|------|
| Stage bg | Light `#F7F7F8` · Dark `#0F1115` |
| Body text | Light `#09090B` · Dark `#F1F5F9` |
| Accent | `#2563EB` / dark `#60A5FA` |
| Assistant | Grok avatar 22 + muted “Grok” label + **full-width prose** (no card shell) |
| User | **Right-hug**: measure text width → left spacer → bubble → avatar. Never RTL-only. |
| User bubble | Soft blue fill, asymmetric radius (ne tight), max ~72% column |
| User actions | 「撤回」under bubble, same right edge |
| Tools | Muted mono one-liner + rail; status word only (no pill stack) |
| Thought | Collapsing muted text — no purple card |
| Plan | Simple bordered block |
| Tables | Custom grid |

## Anti-patterns (learned the hard way)
- `Layout::right_to_left` + default `vertical` Align::Min → short msgs float left (“行” bug)
- Turn hairline dividers + name labels + white cards = noisy mess
- Purple thought cards + green pill badges stacking
- Per-message heavy elevated panel for short “有需要再说”
- Emoji icons on Windows
