<div align="center">

# 🐬 Cove

### Un outil pour la zone de notification Windows permettant de gérer les projets et conversations Claude Code & Reasonix.

`system tray` · `flyout panel` · `no telemetry` · `local only`

<sub>Traitez vos conversations Claude Code comme des e-mails — propres, archivées, jamais orphelines.</sub>

[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-0078D4?logo=windows11&logoColor=white)](#install)
[![Tauri](https://img.shields.io/badge/Tauri-2.11-FFC131?logo=tauri&logoColor=black)](#architecture)
[![Rust](https://img.shields.io/badge/Rust-1.96+-CE422B?logo=rust&logoColor=white)](#build-from-source)
[![TypeScript](https://img.shields.io/badge/TypeScript-native-3178C6?logo=typescript&logoColor=white)](#architecture)
[![Release](https://img.shields.io/badge/release-v0.6.0-blue?logo=github&logoColor=white)](https://github.com/LUMIAO9527/Cove/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?logo=opensourceinitiative&logoColor=white)](./LICENSE)
[![No Telemetry](https://img.shields.io/badge/telemetry-none-success)](#runtime-data)

**Autres langues :** &nbsp;[English](./README.md) · [简体中文](./README.zh-CN.md) · [日本語](./README.ja.md) · [Español](./README.es.md) · [Deutsch](./README.de.md) · [Português (BR)](./README.pt-BR.md) · [Русский](./README.ru.md) · [한국어](./README.ko.md)

</div>

---

> Cove réside dans la zone de notification. Cliquez sur l'icône et un panneau volant de style Win11 de 380×580 apparaît ; il disparaît dès que vous cliquez ailleurs.
>
> Il résout un véritable problème : **lorsque vous supprimez une conversation Claude Code, supprimer uniquement la transcription `.jsonl` laisse 7 artefacts liés derrière comme « orphelins »** — tâches, historique de fichiers, session-env, télémétrie, et plus encore. Cove nettoie les **8 emplacements ensemble**, et ajoute l'archivage doux (déplacer + restaurer) ainsi qu'un scan global des orphelins.

---

## 📑 Table des matières

- ✨ Fonctionnalités
- 📥 Installation
- 🏗️ Architecture
- ⚙️ Données d'exécution
- 🛠️ Développement
- 📄 Licence

---

## ✨ Fonctionnalités

- **Prise en charge multi-outils** — Gérez **Claude Code** et **Reasonix** côte à côte. Un sélecteur à capsule dans la barre supérieure détermine l'outil affiché sur chaque page ; l'état d'installation est détecté automatiquement.
- **Projets et conversations** — Analyse `~/.claude/projects/`, liste chaque conversation regroupée par projet, et affiche le modèle de chacune, le nombre de messages, la taille et le résumé de la première question.
- **Titres intelligents** — `custom-title` → `ai-title` → `summary` → premier message utilisateur. N'affiche jamais « Sans titre ».
- **Archivage doux** — Déplace une conversation et toutes ses données associées vers une zone d'archivage ; entièrement restaurable à l'emplacement d'origine.
- **Suppression définitive** — Retire définitivement une conversation ainsi que les 8 emplacements de données associés.
- **Scan global des orphelins** — Trouve chaque orphelin « transcription disparue mais restes présents » parmi tous les projets ; nettoyez-en un ou plusieurs à la fois.
- **Affichage du modèle** — Modèle par défaut global dans la barre supérieure, plus le modèle réel sur lequel chaque conversation s'est exécutée.
- **Lanceur de nouvelle conversation** — Lancez `claude` dans un répertoire de travail choisi en un clic, avec un répertoire par défaut mémorisé.
- **Visionneuse d'historique de session** — Navigation en lecture seule de la transcription complète (flux de messages utilisateur/assistant ; réflexion/appels d'outils repliables).
- **Style Win11** — Translucidité Mica, panneau volant de la zone de notification, animations d'entrée/glissement des cartes, thème sombre.

---

## 📥 Installation

### Option 1 : Téléchargement (recommandé)

Récupérez l'un de ces fichiers depuis [Releases](../../releases) :

| Fichier | Description | Taille |
|------|-------------|------|
| `Cove.exe` | Fichier unique portable — double-cliquez pour lancer | ~10 MB |
| `Cove_0.6.0_x64-setup.exe` | Installateur NSIS | ~2.2 MB |
| `Cove_0.6.0_x64_en-US.msi` | Installateur MSI | ~3.5 MB |

**Windows 10/11 x64 uniquement.** Après l'installation/le lancement, une icône apparaît dans la zone de notification ; cliquez dessus pour faire apparaître le panneau.

### Compiler depuis les sources

Nécessite Windows 10/11 + Rust 1.96+ + Node.js 24+ + VS 2022 Build Tools (charge de travail C++).

```bash
git clone https://github.com/LUMIAO9527/Cove.git
cd Cove
npm install --include=dev
npm run tauri dev          # dev mode (hot reload)
npm run tauri build        # build release artifacts
```

---

## 🏗️ Architecture

**Tauri 2.11 + Rust + TypeScript natif (sans React/Vue) + Vite.** Artefact < 11 MB, mémoire à l'exécution ~34 MB.

### Architecture multi-outils

Chaque outil a une disposition de données et un schéma de session complètement différents, l'analyse / la transcription / le lancement sont donc répartis par outil :

| | Claude Code | Reasonix |
|---|---|---|
| Sessions | `~/.claude/projects/<encoded>/<SID>.jsonl` | `~/.reasonix/sessions/<name>.jsonl` + `.meta.json` |
| ID | UUID | nom du fichier (sans UUID) |
| Reprise | `claude --resume <SID>` | `reasonix code -r` (dernière session du workspace) |
| Nettoyage | scan complet des orphelins sur 8 emplacements | non applicable (sidecars supprimés avec la session) |

Une énumération `ToolKind` (`src-tauri/src/tools/`) achemine chaque opération vers le bon adaptateur.

### Le cœur : 8 emplacements de données liés

Lors de la suppression d'une conversation, retirer uniquement la transcription `.jsonl` laisse les 7 autres en orphelins. Cove gère l'ensemble :

| # | Donnée | Chemin | Clé de jointure |
|---|------|------|----------|
| ① | Transcription | `projects\<encoded>\<SID>.jsonl` | nom de fichier |
| ② | Sous-répertoire de même nom | `projects\<encoded>\<SID>\` (sous-agents/résultats) | nom de répertoire |
| ③ | Tâches à faire | `tasks\<SID>\` | nom de répertoire |
| ④ | Instantanés d'édition | `file-history\<SID>\` | nom de répertoire |
| ⑤ | Événements de télémétrie | `telemetry\1p_failed_events.<SID>.<X>.json` | préfixe de nom de fichier |
| ⑥ | Env de session | `session-env\<SID>\` | nom de répertoire |
| ⑦ | Historique des commandes | `history.jsonl` | champ `sessionId` intégré |
| ⑧ | Métadonnées de processus | `sessions\<PID>.json` | `sessionId` dans le fichier |

### Structure du code

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

## ⚙️ Données d'exécution

| Objectif | Chemin |
|---------|------|
| Données Claude Code (Cove lit et écrit) | `~/.claude/` |
| Zone d'archivage de Cove | `~/.claude-managed/archive/` |

Cove ne collecte **aucune télémétrie**. Toutes les données restent locales.

---

## 🛠️ Développement

```bash
cd src-tauri
cargo test                # run integration tests
```

**Problèmes courants**

- `cargo build` signale `link.exe not found` → installez VS 2022 Build Tools (avec la charge de travail C++).
- `npm install` n'a installé que quelques paquets → utilisez `npm install --include=dev`.
- Commandes Rust introuvables → Rust s'installe dans `~/.cargo/bin` par défaut et peut ne pas être sur le PATH.

---

## 📄 Licence

[MIT](./LICENSE) · ⭐ Mettez une étoile à ce dépôt si Cove vous est utile.
