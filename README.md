# Clawd on Desk — Tauri Port

A Tauri (Rust + WebKit) port of [Clawd on Desk](https://github.com/rullerzhou-afk/clawd-on-desk) — a desktop pet that reacts to your Claude Code / Codex / Cursor / opencode / Copilot / Gemini / Kiro sessions in real-time.

> **Status:** Early WIP. macOS-only target (Windows/Linux out of scope).
>
> Original Electron version by [@rullerzhou-afk](https://github.com/rullerzhou-afk) — this port preserves all renderer code, themes, assets, and hook scripts, and re-implements the Electron main process in Rust.

## Why port?

The Electron version works great but runs ~650MB RAM for a desktop pet (7 Chromium processes). Target for this port: **~30–60MB** using macOS native WebKit.

| | Electron (original) | Tauri (this port, estimated) |
|---|---|---|
| DMG size | ~150MB | ~10MB |
| Memory (idle) | ~650MB | ~50MB |
| Startup | 1–2s | instant |
| FFI for AppKit/NSWindow | private APIs via JS | native Rust via `cocoa`/`objc` |

## What's reused vs rewritten

**Reused as-is** (webview side, identical to original):
- `src/` — renderer JS (eye tracking, animation switching, theme loader, i18n)
- `themes/` — theme system (Clawd crab, Calico cat, template)
- `assets/` — SVG / GIF / APNG / sounds
- `hooks/` — Node hook scripts (invoked by Claude Code etc., external to the app)
- `docs/` — guides

**Rewriting in Rust** (`src-tauri/`):
- Main process (window creation, tray, click-through, always-on-top)
- HTTP server on `127.0.0.1:23333` (replaces `src/server.js`)
- State machine (replaces `src/state.js`)
- macOS `NSWindow` / `LSUIElement` / `setActivationPolicy` calls
- Permission bubble window management

## Build

```bash
npm install
npm run tauri dev       # dev mode
npm run tauri build     # release DMG
```

Requires Rust (via rustup) and Node.js.

## Credits

- Original **Clawd on Desk** by [@rullerzhou-afk](https://github.com/rullerzhou-afk) ([MIT license](https://github.com/rullerzhou-afk/clawd-on-desk))
- Clawd pixel art referenced from [clawd-tank](https://github.com/marciogranzotto/clawd-tank) by [@marciogranzotto](https://github.com/marciogranzotto)
- **Clawd** character © [Anthropic](https://www.anthropic.com). Unofficial fan project.
- **Calico cat** artwork by 鹿鹿 ([@rullerzhou-afk](https://github.com/rullerzhou-afk)). All rights reserved.

## License

Source code: [MIT](LICENSE).

Artwork in `assets/` and `themes/` is **not** covered by MIT — see the original repo's [assets/LICENSE](https://github.com/rullerzhou-afk/clawd-on-desk/blob/main/assets/LICENSE).
