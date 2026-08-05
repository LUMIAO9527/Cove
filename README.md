<div align="center">

# 🐬 Cove

**A local Windows tray app for managing Claude Code and Reasonix projects and conversations.**

[Download the portable app](https://github.com/LUMIAO9527/Cove/releases/latest/download/Cove.exe) · [Latest release](https://github.com/LUMIAO9527/Cove/releases/latest)

[![Windows](https://img.shields.io/badge/Windows-10%2F11%20x64-0078D4?logo=windows11&logoColor=white)](https://github.com/LUMIAO9527/Cove/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](./LICENSE)
[![No telemetry](https://img.shields.io/badge/telemetry-none-success)](#privacy-and-data)

[简体中文](./README.zh-CN.md)

</div>

## What Cove does

Cove keeps projects, conversations, archives, and cleanup tools in a compact system-tray panel. It reads the local data created by supported coding agents and does not require a cloud service.

- Browse Claude Code and Reasonix projects and conversations.
- Read, copy, or export complete conversation transcripts.
- Reorder projects and launch a new conversation from a chosen folder.
- Soft-archive conversations and restore them later.
- Permanently delete a conversation together with its related local data.
- Find Claude Code leftovers whose main transcript has already been removed.
- Batch archive or delete conversations by time group.

## Screenshots

| Preview | What it shows |
| :---: | --- |
| <img src="./assets/screenshots/01-projects.png" alt="Cove project list" width="260"> | **Projects** — Add local workspaces, open their data folders, and reorder them from the tray panel. |
| <img src="./assets/screenshots/02-conversations.png" alt="Cove conversation list" width="260"> | **Conversations** — Browse a project's sessions with title, model, message count, size, and age. |
| <img src="./assets/screenshots/03-cleanup.png" alt="Cove cleanup scan" width="260"> | **Cleanup** — Review Claude Code leftovers and Cove's safety classification before removing them. |
| <img src="./assets/screenshots/04-archive.png" alt="Cove archive" width="260"> | **Archive** — Review archived conversations and restore them to their original location. |
| <img src="./assets/screenshots/05-models.png" alt="Cove model configuration" width="260"> | **Model configuration** — Inspect and switch the configured model for each Claude Code model tier. |
| <img src="./assets/screenshots/06-session-detail.png" alt="Cove session detail" width="260"> | **Session detail** — Read the full transcript, expand or collapse thinking sections, and continue a session. |
| <img src="./assets/screenshots/07-reasonix.png" alt="Cove Reasonix detection" width="260"> | **Reasonix** — Detect whether Reasonix is installed and show installation guidance when it is unavailable. |

## Install

Download [`Cove.exe`](https://github.com/LUMIAO9527/Cove/releases/latest/download/Cove.exe) and run it directly. Cove currently supports Windows 10/11 x64.

The app lives in the system tray. Click its icon to open the panel; clicking elsewhere hides it.

## Privacy and data

Cove has no telemetry and sends no project or conversation data to a Cove server. All operations happen on the local machine.

Archive and delete actions can affect the original data used by Claude Code or Reasonix. Use archive when you may need to restore a conversation later; permanent deletion cannot be undone by Cove.

## Build from source

Prerequisites: Node.js, Rust, and the Windows C++ build tools required by Tauri.

```powershell
git clone https://github.com/LUMIAO9527/Cove.git
cd Cove
npm install --include=dev
npm run tauri dev
```

Create a release build with:

```powershell
npm run tauri build
```

The frontend is native TypeScript with Vite; the desktop backend is Rust and Tauri. Tests are under `src-tauri/tests/` and can be run with `cargo test` from `src-tauri/`.

## Repository layout

```text
src/              Frontend views, styles, and Tauri API wrappers
src-tauri/src/    Rust backend and per-tool adapters
src-tauri/tests/  Backend integration tests
```

## License

[MIT](./LICENSE)
