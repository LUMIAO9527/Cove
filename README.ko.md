<div align="center">

# 🐬 Cove

### Windows 시스템 트레이 도구로 Claude Code 프로젝트와 대화를 관리합니다.

`system tray` · `flyout panel` · `no telemetry` · `local only`

<sub>Claude Code 대화를 이메일처럼 관리하세요 — 깔끔하고, 아카이브되며, 고아 데이터가 생기지 않습니다.</sub>

[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-0078D4?logo=windows11&logoColor=white)](#install)
[![Tauri](https://img.shields.io/badge/Tauri-2.11-FFC131?logo=tauri&logoColor=black)](#architecture)
[![Rust](https://img.shields.io/badge/Rust-1.96+-CE422B?logo=rust&logoColor=white)](#build-from-source)
[![TypeScript](https://img.shields.io/badge/TypeScript-native-3178C6?logo=typescript&logoColor=white)](#architecture)
[![Release](https://img.shields.io/badge/release-v0.4.28-blue?logo=github&logoColor=white)](https://github.com/LUMIAO9527/Cove/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?logo=opensourceinitiative&logoColor=white)](./LICENSE)
[![No Telemetry](https://img.shields.io/badge/telemetry-none-success)](#runtime-data)

**다른 언어:** &nbsp;[English](./README.md) · [简体中文](./README.zh-CN.md) · [日本語](./README.ja.md) · [Español](./README.es.md) · [Français](./README.fr.md) · [Deutsch](./README.de.md) · [Português (BR)](./README.pt-BR.md) · [Русский](./README.ru.md)

</div>

---

> Cove는 시스템 트레이에 상주합니다. 아이콘을 클릭하면 380×580 크기의 Win11 스타일 플라이아웃 패널이 나타나며, 다른 곳을 클릭하면 즉시 숨겨집니다.
>
> 실제 고통스러운 문제를 해결합니다: **Claude Code 대화를 삭제할 때 `.jsonl` 트랜스크립트만 지우면 7개의 관련 데이터가 "고아 데이터"로 남게 됩니다** — 작업, 파일 히스토리, 세션 환경, 텔레메트리 등입니다. Cove는 **8개 위치를 모두 함께** 정리하며, 소프트 아카이브(이동 + 복원)와 전역 고아 데이터 스캔 기능도 추가합니다.

---

## 📑 목차

- ✨ 기능
- 📥 설치
- 🏗️ 아키텍처
- ⚙️ 런타임 데이터
- 🛠️ 개발
- 📄 라이선스

---

## ✨ 기능

- **프로젝트 및 대화** — `~/.claude/projects/`를 스캔하여 모든 대화를 프로젝트별로 그룹화해 표시하며, 각각의 모델, 메시지 수, 크기, 첫 질문 요약을 보여줍니다.
- **스마트 제목** — `custom-title` → `ai-title` → `summary` → 첫 번째 사용자 메시지 순으로 적용됩니다. "Untitled"가 표시되지 않습니다.
- **소프트 아카이브** — 대화와 관련된 모든 데이터를 아카이브 영역으로 이동시키며, 원래 위치로 완전히 복원할 수 있습니다.
- **완전 삭제** — 대화와 8개의 관련 데이터 위치를 모두 영구적으로 제거합니다.
- **전역 고아 데이터 스캔** — 모든 프로젝트에서 "트랜스크립트는 사라졌지만 잔여물이 남은" 고아 데이터를 찾아, 하나 또는 여러 개를 한 번에 정리할 수 있습니다.
- **모델 표시** — 상단 바에 전역 기본 모델을 표시하고, 각 대화가 실제로 실행된 모델도 보여줍니다.
- **새 대화 실행기** — 선택한 작업 디렉터리에서 `claude`를 한 번의 클릭으로 실행하며, 기본 디렉터리를 기억합니다.
- **세션 히스토리 뷰어** — 전체 트랜스크립트를 읽기 전용으로 탐색합니다 (사용자/어시스턴트 메시지 스트림; 사고/도구 호출은 접을 수 있음).
- **Win11 스타일** — Mica 반투명 효과, 트레이 플라이아웃 패널, 카드 진입/슬라이드 아웃 애니메이션, 다크 테마.

---

## 📥 설치

### 옵션 1: 다운로드 (권장)

[Releases](../../releases)에서 아래 파일 중 하나를 다운로드하세요:

| 파일 | 설명 | 크기 |
|------|-------------|------|
| `Cove.exe` | 단일 실행 파일 — 더블클릭으로 실행 | ~10 MB |
| `Cove_0.4.28_x64-setup.exe` | NSIS 설치 프로그램 | ~2.2 MB |
| `Cove_0.4.28_x64_en-US.msi` | MSI 설치 프로그램 | ~3.5 MB |

**Windows 10/11 x64 전용입니다.** 설치/실행 후 트레이 아이콘이 나타나며, 클릭하면 패널이 나타납니다.

### 소스에서 빌드

Windows 10/11 + Rust 1.96+ + Node.js 24+ + VS 2022 Build Tools(C++ 워크로드)가 필요합니다.

```bash
git clone https://github.com/LUMIAO9527/Cove.git
cd Cove
npm install --include=dev
npm run tauri dev          # dev mode (hot reload)
npm run tauri build        # build release artifacts
```

---

## 🏗️ 아키텍처

**Tauri 2.11 + Rust + 네이티브 TypeScript(React/Vue 없음) + Vite.** 빌드 결과물 크기 < 11 MB, 런타임 메모리 ~34 MB.

### 핵심: 8개의 관련 데이터 위치

대화를 삭제할 때 `.jsonl` 트랜스크립트만 제거하면 나머지 7개가 고아 데이터로 남게 됩니다. Cove는 이를 모두 처리합니다:

| # | 데이터 | 경로 | 조인 키 |
|---|------|------|----------|
| ① | 트랜스크립트 | `projects\<encoded>\<SID>.jsonl` | 파일명 |
| ② | 동일 이름 하위 디렉터리 | `projects\<encoded>\<SID>\` (서브에이전트/결과) | 디렉터리명 |
| ③ | 할 일 작업 | `tasks\<SID>\` | 디렉터리명 |
| ④ | 편집 스냅샷 | `file-history\<SID>\` | 디렉터리명 |
| ⑤ | 텔레메트리 이벤트 | `telemetry\1p_failed_events.<SID>.<X>.json` | 파일명 접두사 |
| ⑥ | 세션 환경 | `session-env\<SID>\` | 디렉터리명 |
| ⑦ | 명령 히스토리 | `history.jsonl` | 인라인 `sessionId` 필드 |
| ⑧ | 프로세스 메타데이터 | `sessions\<PID>.json` | 파일 내 `sessionId` |

### 코드 구조

```text
Cove/
├── src-tauri/src/          # Rust backend
│   ├── lib.rs              # tray / window / state machine / single-instance / Mica
│   ├── commands.rs         # Tauri command bridge
│   ├── scan.rs             # jsonl parsing, title fallback
│   ├── transcript.rs       # full session parsing (read-only viewer)
│   ├── related.rs          # locate the 8 related data locations
│   ├── cleanup.rs          # related delete, orphan scan
│   ├── archive.rs          # archive / restore / index
│   ├── paths.rs            # path encoding / decoding
│   ├── settings.rs         # settings.json read/write
│   ├── projects_config.rs  # project list read/write
│   └── models.rs           # data structures
├── src-tauri/tests/        # integration tests
├── src/                    # frontend (native TS)
│   ├── main.ts             # entry / routing / animation
│   ├── api.ts              # invoke wrappers
│   └── views/              # project / conversation / archive / cleanup / detail views
└── src/styles/             # theme + animations + icons
```

---

## ⚙️ 런타임 데이터

| 용도 | 경로 |
|---------|------|
| Claude Code 데이터 (Cove가 읽고 씀) | `~/.claude/` |
| Cove 아카이브 영역 | `~/.claude-managed/archive/` |

Cove는 **텔레메트리를 수집하지 않습니다**. 모든 데이터는 로컬에 머무릅니다.

---

## 🛠️ 개발

```bash
cd src-tauri
cargo test                # run integration tests
```

**자주 발생하는 문제**

- `cargo build`가 `link.exe not found`를 보고하면 → VS 2022 Build Tools(C++ 워크로드 포함)를 설치하세요.
- `npm install`이 몇 개의 패키지만 설치했다면 → `npm install --include=dev`를 사용하세요.
- Rust 명령을 찾을 수 없다면 → Rust는 기본적으로 `~/.cargo/bin`에 설치되며 PATH에 없을 수 있습니다.

---

## 📄 라이선스

[MIT](./LICENSE) · ⭐ Cove가 도움이 되었다면 이 저장소에 Star를 눌러주세요.
