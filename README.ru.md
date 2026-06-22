<div align="center">

# 🐬 Cove

### Инструмент для Windows в системном трее для управления проектами и беседами Claude Code.

`system tray` · `flyout panel` · `no telemetry` · `local only`

<sub>Относитесь к своим беседам Claude Code как к электронной почте — чистыми, архивированными, никогда не оставленными сиротами.</sub>

[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-0078D4?logo=windows11&logoColor=white)](#install)
[![Tauri](https://img.shields.io/badge/Tauri-2.11-FFC131?logo=tauri&logoColor=black)](#architecture)
[![Rust](https://img.shields.io/badge/Rust-1.96+-CE422B?logo=rust&logoColor=white)](#build-from-source)
[![TypeScript](https://img.shields.io/badge/TypeScript-native-3178C6?logo=typescript&logoColor=white)](#architecture)
[![Release](https://img.shields.io/badge/release-v0.4.28-blue?logo=github&logoColor=white)](https://github.com/LUMIAO9527/Cove/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?logo=opensourceinitiative&logoColor=white)](./LICENSE)
[![No Telemetry](https://img.shields.io/badge/telemetry-none-success)](#runtime-data)

**Другие языки:** &nbsp;[English](./README.md) · [简体中文](./README.zh-CN.md) · [日本語](./README.ja.md) · [Español](./README.es.md) · [Français](./README.fr.md) · [Deutsch](./README.de.md) · [Português (BR)](./README.pt-BR.md) · [한국어](./README.ko.md)

</div>

---

> Cove живёт в системном трее. Щёлкните по значку, и появляется всплывающая панель в стиле Win11 размером 380×580; она скрывается, как только вы щёлкаете в другое место.
>
> Это решает реальную проблему: **при удалении беседы Claude Code удаление только транскрипта `.jsonl` оставляет 7 связанных артефактов в виде «данных-сирот»** — задачи, историю файлов, окружение сессии, телеметрию и многое другое. Cove очищает все **8 мест одновременно**, а также добавляет мягкое архивирование (перемещение + восстановление) и глобальный поиск данных-сирот.

---

## 📑 Содержание

- ✨ Возможности
- 📥 Установка
- 🏗️ Архитектура
- ⚙️ Данные времени выполнения
- 🛠️ Разработка
- 📄 Лицензия

---

## ✨ Возможности

- **Проекты и беседы** — Сканирует `~/.claude/projects/`, перечисляет все беседы, сгруппированные по проектам, и отображает модель, количество сообщений, размер и краткое содержание первого вопроса для каждой из них.
- **Умные заголовки** — `custom-title` → `ai-title` → `summary` → первое сообщение пользователя. Никогда не показывает «Untitled».
- **Мягкое архивирование** — Перемещает беседу и все связанные с ней данные в область архива; полностью восстанавливается в исходное место.
- **Полное удаление** — Навсегда удаляет беседу вместе со всеми 8 связанными местами хранения данных.
- **Глобальный поиск данных-сирот** — Находит каждую ситуацию «транскрипт удалён, но остатки остались» (данные-сироты) во всех проектах; можно очистить один или сразу несколько.
- **Отображение моделей** — Глобальная модель по умолчанию на верхней панели, а также фактическая модель, на которой выполнялась каждая беседа.
- **Запуск новой беседы** — Запускает `claude` в выбранном рабочем каталоге одним щелчком, с запоминаемым каталогом по умолчанию.
- **Просмотр истории сессий** — Просмотр полного транскрипта только для чтения (поток сообщений пользователя/ассистента; размышления/вызовы инструментов сворачиваются).
- **Стиль Win11** — Полупрозрачность Mica, всплывающая панель в трее, анимации появления/выезжания карточек, тёмная тема.

---

## 📥 Установка

### Вариант 1: Загрузка (рекомендуется)

Возьмите любой из них со страницы [Releases](../../releases):

| Файл | Описание | Размер |
|------|-------------|------|
| `Cove.exe` | Портативный один файл — двойной щелчок для запуска | ~10 МБ |
| `Cove_0.4.28_x64-setup.exe` | Установщик NSIS | ~2.2 МБ |
| `Cove_0.4.28_x64_en-US.msi` | Установщик MSI | ~3.5 МБ |

**Только Windows 10/11 x64.** После установки/запуска появляется значок в трее; щёлкните по нему, чтобы открыть панель.

### Сборка из исходного кода

Требуется Windows 10/11 + Rust 1.96+ + Node.js 24+ + средства сборки VS 2022 (рабочая нагрузка C++).

```bash
git clone https://github.com/LUMIAO9527/Cove.git
cd Cove
npm install --include=dev
npm run tauri dev          # dev mode (hot reload)
npm run tauri build        # build release artifacts
```

---

## 🏗️ Архитектура

**Tauri 2.11 + Rust + нативный TypeScript (без React/Vue) + Vite.** Артефакт < 11 МБ, потребление памяти во время выполнения ~34 МБ.

### Суть: 8 связанных мест хранения данных

При удалении беседы удаление только транскрипта `.jsonl` оставляет остальные 7 в виде данных-сирот. Cove обрабатывает их все:

| № | Данные | Путь | Ключ связи |
|---|------|------|----------|
| ① | Транскрипт | `projects\<encoded>\<SID>.jsonl` | имя файла |
| ② | Подкаталог с тем же именем | `projects\<encoded>\<SID>\` (субагенты/результаты) | имя каталога |
| ③ | Задачи Todo | `tasks\<SID>\` | имя каталога |
| ④ | Снимки изменений | `file-history\<SID>\` | имя каталога |
| ⑤ | События телеметрии | `telemetry\1p_failed_events.<SID>.<X>.json` | префикс имени файла |
| ⑥ | Окружение сессии | `session-env\<SID>\` | имя каталога |
| ⑦ | История команд | `history.jsonl` | поле `sessionId` внутри |
| ⑧ | Метаданные процесса | `sessions\<PID>.json` | `sessionId` внутри файла |

### Структура кода

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

## ⚙️ Данные времени выполнения

| Назначение | Путь |
|---------|------|
| Данные Claude Code (Cove читает и записывает) | `~/.claude/` |
| Область архива Cove | `~/.claude-managed/archive/` |

Cove **не собирает телеметрию**. Все данные остаются локальными.

---

## 🛠️ Разработка

```bash
cd src-tauri
cargo test                # run integration tests
```

**Частые проблемы**

- `cargo build` выдаёт `link.exe not found` → установите средства сборки VS 2022 (с рабочей нагрузкой C++).
- `npm install` установил лишь несколько пакетов → используйте `npm install --include=dev`.
- Команды Rust не найдены → Rust устанавливается в `~/.cargo/bin` по умолчанию и может отсутствовать в PATH.

---

## 📄 Лицензия

[MIT](./LICENSE) · ⭐ Поставьте звезду этому репозиторию, если Cove помогает вам.
