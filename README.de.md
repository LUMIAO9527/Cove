<div align="center">

# 🐬 Cove

### Ein Tool für den Windows-Infobereich zur Verwaltung von Claude-Code- und Reasonix-Projekten und -Konversationen.

`Infobereich` · `Flyout-Panel` · `keine Telemetrie` · `ausschließlich lokal`

<sub>Behandeln Sie Ihre Claude-Code-Konversationen wie E-Mails — aufgeräumt, archiviert, niemals verwaist.</sub>

[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-0078D4?logo=windows11&logoColor=white)](#install)
[![Tauri](https://img.shields.io/badge/Tauri-2.11-FFC131?logo=tauri&logoColor=black)](#architecture)
[![Rust](https://img.shields.io/badge/Rust-1.96+-CE422B?logo=rust&logoColor=white)](#build-from-source)
[![TypeScript](https://img.shields.io/badge/TypeScript-native-3178C6?logo=typescript&logoColor=white)](#architecture)
[![Release](https://img.shields.io/badge/release-v0.5.0-blue?logo=github&logoColor=white)](https://github.com/LUMIAO9527/Cove/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?logo=opensourceinitiative&logoColor=white)](./LICENSE)
[![No Telemetry](https://img.shields.io/badge/telemetry-none-success)](#runtime-data)

**Weitere Sprachen:** &nbsp;[English](./README.md) · [简体中文](./README.zh-CN.md) · [日本語](./README.ja.md) · [Español](./README.es.md) · [Français](./README.fr.md) · [Português (BR)](./README.pt-BR.md) · [Русский](./README.ru.md) · [한국어](./README.ko.md)

</div>

---

> Cove lebt im Infobereich. Ein Klick auf das Symbol öffnet ein Flyout-Panel im Win11-Stil (380×580), das sofort verschwindet, sobald Sie woanders klicken.
>
> Es löst ein echtes Problem: **Wenn Sie eine Claude-Code-Konversation löschen, hinterlässt das reine Löschen des `.jsonl`-Transkripts 7 zugehörige Artefakte als „verwaiste Daten“** — Aufgaben, Dateihistorie, Session-Env, Telemetrie und mehr. Cove bereinigt alle **8 Speicherorte gemeinsam** und bietet zusätzlich eine weiche Archivierung (verschieben + wiederherstellen) sowie einen globalen Scan nach verwaisten Daten.

---

## 📑 Inhaltsverzeichnis

- ✨ Funktionen
- 📥 Installation
- 🏗️ Architektur
- ⚙️ Laufzeitdaten
- 🛠️ Entwicklung
- 📄 Lizenz

---

## ✨ Funktionen

- **Multi-Tool-Unterstützung** — Verwalten Sie **Claude Code** und **Reasonix** nebeneinander. Ein Kapsel-Umschalter in der oberen Leiste wählt das pro Seite angezeigte Tool; der Installationsstatus wird automatisch erkannt.
- **Projekte & Konversationen** — Durchsucht `~/.claude/projects/`, listet jede Konversation nach Projekt gruppiert auf und zeigt für jede das Modell, die Nachrichtenanzahl, die Größe und die Zusammenfassung der ersten Frage an.
- **Intelligente Titel** — `custom-title` → `ai-title` → `summary` → erste Benutzernachricht. Zeigt niemals „Untitled".
- **Weiche Archivierung** — Verschiebt eine Konversation samt allen zugehörigen Daten in einen Archivbereich; vollständig am ursprünglichen Ort wiederherstellbar.
- **Echtes Löschen** — Entfernt eine Konversation sowie alle 8 zugehörigen Datenorte dauerhaft.
- **Globaler Scan nach verwaisten Daten** — Findet jede „Transkript weg, aber Reste vorhanden"-Verwaiste über alle Projekte hinweg; einzeln oder mehrere auf einmal bereinigen.
- **Modellanzeige** — Globales Standardmodell in der oberen Leiste, zusätzlich das tatsächlich ausgeführte Modell je Konversation.
- **Starter für neue Konversationen** — Startet `claude` in einem gewählten Arbeitsverzeichnis mit einem Klick, mit einem gemerkten Standardverzeichnis.
- **Sitzungsverlaufs-Betrachter** — Schreibgeschütztes Durchblättern des vollständigen Transkripts (Nachrichtenstrom Benutzer/Assistent; Thinking-/Tool-Aufrufe einklappbar).
- **Win11-Stil** — Mica-Transluzenz, Tray-Flyout, Karten-Ein-/Ausgleit-Animationen, dunkles Design.

---

## 📥 Installation

### Option 1: Herunterladen (empfohlen)

Laden Sie eine beliebige Datei aus den [Releases](../../releases) herunter:

| Datei | Beschreibung | Größe |
|------|-------------|------|
| `Cove.exe` | Portable Einzeldatei — Doppelklick zum Ausführen | ~10 MB |
| `Cove_0.5.0_x64-setup.exe` | NSIS-Installer | ~2.2 MB |
| `Cove_0.5.0_x64_en-US.msi` | MSI-Installer | ~3.5 MB |

**Nur Windows 10/11 x64.** Nach der Installation/Ausführung erscheint ein Tray-Symbol; klicken Sie darauf, um das Panel aufklappen zu lassen.

### Aus dem Quellcode bauen

Erfordert Windows 10/11 + Rust 1.96+ + Node.js 24+ + VS 2022 Build Tools (C++-Workload).

```bash
git clone https://github.com/LUMIAO9527/Cove.git
cd Cove
npm install --include=dev
npm run tauri dev          # Dev-Modus (Hot Reload)
npm run tauri build        # Release-Artefakte bauen
```

---

## 🏗️ Architektur

**Tauri 2.11 + Rust + natives TypeScript (ohne React/Vue) + Vite.** Artefakt < 11 MB, Speicherverbrauch zur Laufzeit ~34 MB.

### Multi-Tool-Architektur

Jedes Tool hat ein völlig anderes Datenlayout und Session-Schema, daher erfolgen Scan / Transkript / Start pro Tool:

| | Claude Code | Reasonix |
|---|---|---|
| Sessions | `~/.claude/projects/<encoded>/<SID>.jsonl` | `~/.reasonix/sessions/<name>.jsonl` + `.meta.json` |
| ID | UUID | Dateiname (ohne UUID) |
| Fortsetzen | `claude --resume <SID>` | `reasonix code -r` (neueste Session des Workspace) |
| Bereinigung | vollständiger 8-Speicherort-Orphan-Scan | nicht zutreffend (Sidecars werden mit der Session gelöscht) |

Eine `ToolKind`-Enumeration (`src-tauri/src/tools/`) leitet jede Operation an den richtigen Adapter weiter.

### Der Kern: 8 zugehörige Datenorte

Beim Löschen einer Konversation hinterlässt das Entfernen nur des `.jsonl`-Transkripts die anderen 7 als verwaiste Daten. Cove behandelt alle:

| # | Daten | Pfad | Join-Schlüssel |
|---|------|------|----------|
| ① | Transkript | `projects\<encoded>\<SID>.jsonl` | Dateiname |
| ② | Gleichnamiges Unterverzeichnis | `projects\<encoded>\<SID>\` (Subagents/Ergebnisse) | Verzeichnisname |
| ③ | Todo-Aufgaben | `tasks\<SID>\` | Verzeichnisname |
| ④ | Bearbeitungs-Snapshots | `file-history\<SID>\` | Verzeichnisname |
| ⑤ | Telemetrie-Ereignisse | `telemetry\1p_failed_events.<SID>.<X>.json` | Dateinamen-Präfix |
| ⑥ | Session-Env | `session-env\<SID>\` | Verzeichnisname |
| ⑦ | Befehlsverlauf | `history.jsonl` | Inline-Feld `sessionId` |
| ⑧ | Prozess-Metadaten | `sessions\<PID>.json` | In-Datei `sessionId` |

### Code-Struktur

```text
Cove/
├── src-tauri/src/          # Rust-Backend
│   ├── lib.rs              # Tray / Fenster / Zustandsmaschine / Single-Instance / Mica
│   ├── commands.rs         # Tauri-Command-Bridge
│   ├── tools/              # Multi-Tool-Verteilung (ToolKind + Adapter pro Tool)
│   │   ├── mod.rs          #   ToolKind-Enum + Installationsprüfung + Start
│   │   ├── claude.rs       #   Claude-Code-Adapter
│   │   └── reasonix.rs     #   Reasonix-Adapter
│   ├── scan.rs             # jsonl-Parsing, Titel-Fallback
│   ├── transcript.rs       # vollständiges Session-Parsing (schreibgeschützter Viewer)
│   ├── related.rs          # die 8 zugehörigen Datenorte lokalisieren
│   ├── cleanup.rs          # zugehöriges Löschen, Scan verwaister Daten
│   ├── archive.rs          # archivieren / wiederherstellen / Index
│   ├── paths.rs            # Pfad-Codierung / -Decodierung
│   ├── settings.rs         # settings.json lesen/schreiben
│   ├── projects_config.rs  # Projektliste lesen/schreiben (pro Tool)
│   └── models.rs           # Datenstrukturen
├── src-tauri/tests/        # Integrationstests
├── src/                    # Frontend (natives TS)
│   ├── main.ts             # Einstieg / Routing / Animation
│   ├── api.ts              # Invoke-Wrapper
│   └── views/              # Projekt- / Konversations- / Archiv- / Bereinigungs- / Detail-Ansichten
└── src/styles/             # Theme + Animationen + Icons
```

---

## ⚙️ Laufzeitdaten

| Zweck | Pfad |
|---------|------|
| Claude-Code-Daten (Cove liest & schreibt) | `~/.claude/` |
| Cove-Archivbereich | `~/.claude-managed/archive/` |

Cove erfasst **keine Telemetrie**. Alle Daten bleiben lokal.

---

## 🛠️ Entwicklung

```bash
cd src-tauri
cargo test                # Integrationstests ausführen
```

**Häufige Probleme**

- `cargo build` meldet `link.exe not found` → VS 2022 Build Tools installieren (mit der C++-Workload).
- `npm install` hat nur wenige Pakete installiert → `npm install --include=dev` verwenden.
- Rust-Befehle nicht gefunden → Rust wird standardmäßig nach `~/.cargo/bin` installiert und ist möglicherweise nicht im PATH.

---

## 📄 Lizenz

[MIT](./LICENSE) · ⭐ Vergeben Sie diesem Repo einen Stern, wenn Cove Ihnen hilft.
