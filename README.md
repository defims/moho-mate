<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="logo-dark.svg">
    <img src="logo.svg" width="320" alt="Moho Mate logo">
  </picture>
</p>

<h1 align="center">Moho Mate</h1>

English | [中文](README_CN.md)

**Your AI mate for Moho.** While you animate, Moho Mate has your back — it inspects projects, runs Lua, batch-renders, and installs community scripts, so you can stay in the flow. macOS, Apple Silicon & Intel.

## What it does

- 💬 **An AI mate that knows Moho** — chat it through project inspection, Lua one-offs, and repetitive setup tasks
- 🎞 **Render & encode** — get your animation out as video, just by asking
- 📦 **[mohoscripts.com](https://mohoscripts.com)** — community script packages, just ask and they're installed

## Requirements

- Moho Pro 14.4+
- macOS 12+ recommended — tested on Monterey (Intel); Apple Silicon & Intel builds provided, other versions untested

## Install (closed beta)

1. Download the DMG for your Mac from [Releases](https://github.com/defims/moho-mate/releases) — `arm64` = Apple Silicon, `x64` = Intel
2. Open the DMG and drag **moho-mate** into Applications
3. First launch: **right-click → Open** (unsigned during beta; plain double-click gets blocked by Gatekeeper)
4. Moho Mate lives in the **menu bar** (🎬) — there is no Dock icon

## Activation

Invite-only during the beta. Ask the maintainer for a beta code, open **Settings**, paste it, **Activate**. One code covers 2 devices — ask for a rebind when you change machines.

## Feedback & community

- 🐛 Bugs → [open an issue](https://github.com/defims/moho-mate/issues). Always include: the version number (Settings → click to copy), the log bundle (Settings → Diagnostics → Package Logs), and steps to reproduce
- 💡 Ideas & questions → [GitHub Discussions](https://github.com/defims/moho-mate/discussions)
- 💬 Questions & community → [join our Discord](https://discord.gg/WRAg5Wv6F)

> **Privacy note:** the beta includes automatic diagnostic log upload (contents limited to diagnostics) so issues can be fixed faster — tell us if you want yours excluded.

## License & acknowledgements

Third-party components:

- **pi_agent_rust** — © 2026 Jeffrey Emanuel (Dicklesworthstone). **MIT (with OpenAI/Anthropic Rider)** — standard MIT grant plus an additional clause denying rights to OpenAI, L.L.C.; Anthropic, PBC; and their affiliates. moho-mate uses the [defims/picrab](https://github.com/defims/picrab) fork via path dependency.
- **asupersync** — same author, same license family (MIT with OpenAI/Anthropic Rider). Transitive dependency of pi_agent_rust.
- **pi-web** — © 2026 agegr (Federico Jaramillo Martinez), MIT. moho-mate uses [defims/picrab-web](https://github.com/defims/picrab-web) fork; frontend synced to `frontend/src/` via `cargo xtask sync-pi-web`. See `frontend/pi-web.LICENSE`.
