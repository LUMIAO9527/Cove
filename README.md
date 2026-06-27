<div align="center">

# 🐬 Cove

### A Windows system-tray tool to manage Claude Code & Reasonix projects and conversations.

`system tray` · `flyout panel` · `no telemetry` · `local only`

<sub>Treat your Claude Code conversations like email — clean, archived, never orphaned.</sub>

[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-0078D4?logo=windows11&logoColor=white)](#install)
[![Tauri](https://img.shields.io/badge/Tauri-2.11-FFC131?logo=tauri&logoColor=black)](#architecture)
[![Rust](https://img.shields.io/badge/Rust-1.96+-CE422B?logo=rust&logoColor=white)](#build-from-source)
[![TypeScript](https://img.shields.io/badge/TypeScript-native-3178C6?logo=typescript&logoColor=white)](#architecture)
[![Release](https://img.shields.io/badge/release-v0.6.0-blue?logo=github&logoColor=white)](https://github.com/LUMIAO9527/Cove/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?logo=opensourceinitiative&logoColor=white)](./LICENSE)
[![No Telemetry](https://img.shields.io/badge/telemetry-none-success)](#runtime-data)

**Read this in another language:** &nbsp;[简体中文](./README.zh-CN.md) · [日本語](./README.ja.md) · [Español](./README.es.md) · [Français](./README.fr.md) · [Deutsch](./README.de.md) · [Português (BR)](./README.pt-BR.md) · [Русский](./README.ru.md) · [한국어](./README.ko.md)

</div>

---

> Cove lives in the system tray. Click the icon and a 380×580 Win11-style flyout pops out; it hides the moment you click away.
>
> It solves a real pain point: **when you delete a Claude Code conversation, deleting the `.jsonl` transcript alone leaves 7 related artifacts behind as "orphans"** — tasks, file-history, session-env, telemetry, and more. Cove cleans all **8 locations together**, and adds soft-archive (move + restore) plus a global orphan scan.

---

## 📑 Table of Contents

- [✨ Features](#-features)
- [📥 Install](#-install)
- [🏗️ Architecture](#-architecture)
- [⚙️ Runtime Data](#-runtime-data)
- [🛠️ Development](#-development)
- [📄 License](#-license)

---

## ✨ Features

- **Multi-tool support** — Manage **Claude Code** and **Reasonix** side by side. A titlebar capsule switcher picks which tool each page shows; install status is probed automatically.
- **Projects & conversations** — Lists every conversation grouped by project, with model, message count, size, and first-question summary.
- **Drag-to-reorder projects** — Reorder project cards by dragging the handle on the left; the order persists across restarts.
- **Smart titles** — `custom-title` → `ai-title` → `summary` → first user message. Never shows "Untitled".
- **Soft archive** — Moves a conversation and all its related data to an archive area; fully restorable to the original spot.
- **True delete** — Permanently removes a conversation plus all 8 related data locations.
- **Global orphan scan** — Finds every "transcript gone but leftovers remain" orphan across all projects; clean one or many at once.
- **Batch cleanup by time** — Group scattered conversations by time (recent / this week / this month / older), select a whole group, and archive or delete in one action.
- **Model display** — Global default model in the top bar, plus the actual model each conversation ran on.
- **New-conversation launcher** — Launch `claude` in a chosen working directory in one click, with a remembered default directory.
- **Session history viewer** — Read-only browsing of the full transcript (user/assistant message stream; thinking/tool calls collapsible). Copy the whole conversation or export as Markdown.
- **Hover action menus** — Each page's title bar has a hover-triggered ▾ menu for quick actions (open data folder, change default workspace, etc.).
- **Win11 style** — Mica translucency, tray flyout, card enter/slide-out animations, dark theme.

---

## 📥 Install

### Option 1: Download (recommended)

Grab any of these from [Releases](../../releases):

| File | Description | Size |
|------|-------------|------|
| `Cove.exe` | Portable single file — double-click to run | ~10 MB |
| `Cove_0.6.0_x64-setup.exe` | NSIS installer | ~2.2 MB |
| `Cove_0.6.0_x64_en-US.msi` | MSI installer | ~3.5 MB |

**Windows 10/11 x64 only.** After install/run, a tray icon appears; click it to pop out the panel.

### Build from source

Requires Windows 10/11 + Rust 1.96+ + Node.js 24+ + VS 2022 Build Tools (C++ workload).

```bash
git clone https://github.com/LUMIAO9527/Cove.git
cd Cove
npm install --include=dev
npm run tauri dev          # dev mode (hot reload)
npm run tauri build        # build release artifacts
```

---

## 🏗️ Architecture

**Tauri 2.11 + Rust + native TypeScript (no React/Vue) + Vite.** Artifact < 11 MB, runtime memory ~34 MB.

### Multi-tool architecture

Each tool has a completely different data layout and session schema, so scan / transcript / launch dispatch per tool:

| | Claude Code | Reasonix |
|---|---|---|
| Sessions | `~/.claude/projects/<encoded>/<SID>.jsonl` | `~/.reasonix/sessions/<name>.jsonl` + `.meta.json` |
| ID | UUID | filename stem (no UUID) |
| Resume | `claude --resume <SID>` | `reasonix code -r` (workspace's latest) |
| Cleanup | full 8-location orphan scan | not applicable (sidecars deleted with session) |

A `ToolKind` enum (`src-tauri/src/tools/`) routes every operation to the right adapter.

### The core: 8 related data locations

When deleting a conversation, removing only the `.jsonl` transcript leaves the other 7 as orphans. Cove handles all of them:

| # | Data | Path | Join key |
|---|------|------|----------|
| ① | Transcript | `projects\<encoded>\<SID>.jsonl` | filename |
| ② | Same-name subdir | `projects\<encoded>\<SID>\` (subagents/results) | dir name |
| ③ | Todo tasks | `tasks\<SID>\` | dir name |
| ④ | Edit snapshots | `file-history\<SID>\` | dir name |
| ⑤ | Telemetry events | `telemetry\1p_failed_events.<SID>.<X>.json` | filename prefix |
| ⑥ | Session env | `session-env\<SID>\` | dir name |
| ⑦ | Command history | `history.jsonl` | inline `sessionId` field |
| ⑧ | Process metadata | `sessions\<PID>.json` | in-file `sessionId` |

### Code structure

```text
Cove/
├── src-tauri/src/          # Rust backend
│   ├── lib.rs              # tray / window / state machine / single-instance / Mica
│   ├── commands.rs         # Tauri command bridge
│   ├── tools/              # multi-tool dispatch (ToolKind + per-tool adapters)
│   │   ├── mod.rs          #   ToolKind enum + install probe + launch
│   │   ├── claude.rs       #   Claude Code adapter
│   │   └── reasonix.rs     #   Reasonix adapter
│   ├── scan.rs             # jsonl parsing, title fallback
│   ├── transcript.rs       # full session parsing (read-only viewer)
│   ├── related.rs          # locate the 8 related data locations
│   ├── cleanup.rs          # related delete, orphan scan
│   ├── archive.rs          # archive / restore / index
│   ├── paths.rs            # path encoding / decoding
│   ├── settings.rs         # settings.json read/write
│   ├── projects_config.rs  # project list read/write (per-tool)
│   └── models.rs           # data structures
├── src-tauri/tests/        # integration tests
├── src/                    # frontend (native TS)
│   ├── main.ts             # entry / routing / animation
│   ├── api.ts              # invoke wrappers
│   └── views/              # project / conversation / archive / cleanup / detail views
└── src/styles/             # theme + animations + icons
```

---

## ⚙️ Runtime Data

| Purpose | Path |
|---------|------|
| Claude Code data (Cove reads & writes) | `~/.claude/` |
| Cove archive area | `~/.claude-managed/archive/` |

Cove collects **no telemetry**. All data stays local.

---

## 🛠️ Development

```bash
cd src-tauri
cargo test                # run integration tests
```

**Common issues**

- `cargo build` reports `link.exe not found` → install VS 2022 Build Tools (with the C++ workload).
- `npm install` only installed a few packages → use `npm install --include=dev`.
- Rust commands not found → Rust installs to `~/.cargo/bin` by default and may not be on PATH.

---

## 📄 License

[MIT](./LICENSE) · ⭐ Star this repo if Cove helps you.
