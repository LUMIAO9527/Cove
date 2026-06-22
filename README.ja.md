<div align="center">

# 🐬 Cove

### Claude Code のプロジェクトと会話を管理する Windows システムトレイツール。

`system tray` · `flyout panel` · `no telemetry` · `local only`

<sub>Claude Code の会話をメールのように扱いましょう — クリーンで、アーカイブされ、決して孤児データにならないように。</sub>

[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-0078D4?logo=windows11&logoColor=white)](#install)
[![Tauri](https://img.shields.io/badge/Tauri-2.11-FFC131?logo=tauri&logoColor=black)](#architecture)
[![Rust](https://img.shields.io/badge/Rust-1.96+-CE422B?logo=rust&logoColor=white)](#build-from-source)
[![TypeScript](https://img.shields.io/badge/TypeScript-native-3178C6?logo=typescript&logoColor=white)](#architecture)
[![Release](https://img.shields.io/badge/release-v0.4.28-blue?logo=github&logoColor=white)](https://github.com/LUMIAO9527/Cove/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?logo=opensourceinitiative&logoColor=white)](./LICENSE)
[![No Telemetry](https://img.shields.io/badge/telemetry-none-success)](#runtime-data)

**他の言語:** &nbsp;[English](./README.md) · [简体中文](./README.zh-CN.md) · [Español](./README.es.md) · [Français](./README.fr.md) · [Deutsch](./README.de.md) · [Português (BR)](./README.pt-BR.md) · [Русский](./README.ru.md) · [한국어](./README.ko.md)

</div>

---

> Cove はシステムトレイに常駐します。アイコンをクリックすると、380×580 の Win11 風フライアウトがポップアップし、別の場所をクリックするとすぐに隠れます。
>
> 実際のペインポイントを解決します: **Claude Code の会話を削除する際、`.jsonl` トランスクリプトだけを削除すると、7 つの関連アーティファクトが "孤児データ" として残ります** — タスク、ファイル履歴、セッション環境、テレメトリなどです。Cove は **8 箇所すべてをまとめて**クリーンアップし、ソフトアーカイブ（移動＋復元）とグローバルな孤児データスキャンを追加します。

---

## 📑 目次

- ✨ 機能
- 📥 インストール
- 🏗️ アーキテクチャ
- ⚙️ ランタイムデータ
- 🛠️ 開発
- 📄 ライセンス

---

## ✨ 機能

- **プロジェクトと会話** — `~/.claude/projects/` をスキャンし、プロジェクトごとにグループ化されたすべての会話を一覧表示し、それぞれのモデル、メッセージ数、サイズ、最初の質問のサマリーを表示します。
- **スマートタイトル** — `custom-title` → `ai-title` → `summary` → 最初のユーザーメッセージ。「Untitled」は表示しません。
- **ソフトアーカイブ** — 会話とその関連データをすべてアーカイブ領域に移動します。元の場所に完全に復元できます。
- **完全削除** — 会話と 8 つの関連データ箇所すべてを完全に削除します。
- **グローバル孤児データスキャン** — すべてのプロジェクトにわたり、「トランスクリプトは削除されたが残り物が残っている」孤児データをすべて検出します。1 つでも複数でも一度にクリーンアップできます。
- **モデル表示** — 上部バーにグローバルなデフォルトモデル、さらに各会話が実際に実行したモデルを表示します。
- **新規会話ランチャー** — 選択した作業ディレクトリで `claude` をワンクリックで起動します。デフォルトディレクトリは記憶されます。
- **セッション履歴ビューアー** — 完全なトランスクリプトを読み取り専用で閲覧できます（ユーザー/アシスタントのメッセージストリーム、思考/ツール呼び出しは折りたたみ可能）。
- **Win11 スタイル** — Mica 半透明効果、トレイフライアウト、カードの入場/スライドアウトアニメーション、ダークテーマ。

---

## 📥 インストール

### オプション 1: ダウンロード（推奨）

[Releases](../../releases) から以下のいずれかを入手できます:

| File | Description | Size |
|------|-------------|------|
| `Cove.exe` | ポータブル単体ファイル — ダブルクリックで実行 | ~10 MB |
| `Cove_0.4.28_x64-setup.exe` | NSIS インストーラー | ~2.2 MB |
| `Cove_0.4.28_x64_en-US.msi` | MSI インストーラー | ~3.5 MB |

**Windows 10/11 x64 専用です。** インストール/実行後、トレイアイコンが表示されます。クリックするとパネルがポップアップします。

### ソースからビルド

Windows 10/11 + Rust 1.96+ + Node.js 24+ + VS 2022 Build Tools（C++ ワークロード）が必要です。

```bash
git clone https://github.com/LUMIAO9527/Cove.git
cd Cove
npm install --include=dev
npm run tauri dev          # dev mode (hot reload)
npm run tauri build        # build release artifacts
```

---

## 🏗️ アーキテクチャ

**Tauri 2.11 + Rust + ネイティブ TypeScript（React/Vue なし）+ Vite。** 成果物は 11 MB 未満、実行時メモリは約 34 MB です。

### コア: 8 つの関連データ箇所

会話を削除する際、`.jsonl` トランスクリプトだけを削除すると、残りの 7 つが孤児データとして残ります。Cove はそれらすべてを処理します:

| # | データ | パス | 結合キー |
|---|------|------|----------|
| ① | トランスクリプト | `projects\<encoded>\<SID>.jsonl` | ファイル名 |
| ② | 同名サブディレクトリ | `projects\<encoded>\<SID>\` (サブエージェント/結果) | ディレクトリ名 |
| ③ | Todo タスク | `tasks\<SID>\` | ディレクトリ名 |
| ④ | 編集スナップショット | `file-history\<SID>\` | ディレクトリ名 |
| ⑤ | テレメトリイベント | `telemetry\1p_failed_events.<SID>.<X>.json` | ファイル名プレフィックス |
| ⑥ | セッション環境 | `session-env\<SID>\` | ディレクトリ名 |
| ⑦ | コマンド履歴 | `history.jsonl` | インライン `sessionId` フィールド |
| ⑧ | プロセスメタデータ | `sessions\<PID>.json` | ファイル内 `sessionId` |

### コード構成

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

## ⚙️ ランタイムデータ

| 用途 | パス |
|---------|------|
| Claude Code データ（Cove が読み書き） | `~/.claude/` |
| Cove アーカイブ領域 | `~/.claude-managed/archive/` |

Cove は**テレメトリを収集しません**。すべてのデータはローカルに留まります。

---

## 🛠️ 開発

```bash
cd src-tauri
cargo test                # run integration tests
```

**よくある問題**

- `cargo build` で `link.exe not found` と表示される → VS 2022 Build Tools（C++ ワークロード付き）をインストールしてください。
- `npm install` で少数のパッケージしかインストールされない → `npm install --include=dev` を使用してください。
- Rust コマンドが見つからない → Rust はデフォルトで `~/.cargo/bin` にインストールされ、PATH に含まれていない場合があります。

---

## 📄 ライセンス

[MIT](./LICENSE) · Cove がお役に立ちましたら、このリポジトリに ⭐ Star をお願いします。
