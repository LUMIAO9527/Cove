<div align="center">

# 🐬 Cove

### Una herramienta de bandeja del sistema de Windows para gestionar proyectos y conversaciones de Claude Code y Reasonix.

`system tray` · `flyout panel` · `no telemetry` · `local only`

<sub>Trata tus conversaciones de Claude Code como el correo electrónico: limpias, archivadas, nunca huérfanas.</sub>

[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-0078D4?logo=windows11&logoColor=white)](#install)
[![Tauri](https://img.shields.io/badge/Tauri-2.11-FFC131?logo=tauri&logoColor=black)](#architecture)
[![Rust](https://img.shields.io/badge/Rust-1.96+-CE422B?logo=rust&logoColor=white)](#build-from-source)
[![TypeScript](https://img.shields.io/badge/TypeScript-native-3178C6?logo=typescript&logoColor=white)](#architecture)
[![Release](https://img.shields.io/badge/release-v0.6.0-blue?logo=github&logoColor=white)](https://github.com/LUMIAO9527/Cove/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?logo=opensourceinitiative&logoColor=white)](./LICENSE)
[![No Telemetry](https://img.shields.io/badge/telemetry-none-success)](#runtime-data)

**Otros idiomas:** &nbsp;[English](./README.md) · [简体中文](./README.zh-CN.md) · [日本語](./README.ja.md) · [Français](./README.fr.md) · [Deutsch](./README.de.md) · [Português (BR)](./README.pt-BR.md) · [Русский](./README.ru.md) · [한국어](./README.ko.md)

</div>

---

> Cove vive en la bandeja del sistema. Haz clic en el icono y aparece un panel flotante estilo Win11 de 380×580; se oculta en cuanto haces clic en cualquier otro lugar.
>
> Resuelve un problema real: **cuando eliminas una conversación de Claude Code, borrar únicamente la transcripción `.jsonl` deja 7 artefactos relacionados como "huérfanos"** — tareas, file-history, session-env, telemetría y más. Cove limpia las **8 ubicaciones a la vez**, y añade archivado suave (mover + restaurar) además de un escaneo global de huérfanos.

---

## 📑 Tabla de contenidos

- ✨ Características
- 📥 Instalación
- 🏗️ Arquitectura
- ⚙️ Datos en tiempo de ejecución
- 🛠️ Desarrollo
- 📄 Licencia

---

## ✨ Características

- **Soporte multi-herramienta** — Gestiona **Claude Code** y **Reasonix** en paralelo. Un selector tipo cápsula en la barra de título elige la herramienta que muestra cada página; el estado de instalación se detecta automáticamente.
- **Proyectos y conversaciones** — Escanea `~/.claude/projects/`, lista cada conversación agrupada por proyecto y muestra el modelo, el conteo de mensajes, el tamaño y el resumen de la primera pregunta de cada una.
- **Títulos inteligentes** — `custom-title` → `ai-title` → `summary` → primer mensaje del usuario. Nunca muestra "Sin título".
- **Archivado suave** — Mueve una conversación y todos sus datos relacionados a un área de archivado; totalmente restaurable a la ubicación original.
- **Eliminación real** — Elimina permanentemente una conversación más las 8 ubicaciones de datos relacionadas.
- **Escaneo global de huérfanos** — Encuentra cada huérfano del tipo "transcripción desaparecida pero quedan restos" en todos los proyectos; limpia uno o varios a la vez.
- **Visualización del modelo** — Modelo global por defecto en la barra superior, más el modelo real en el que se ejecutó cada conversación.
- **Lanzador de nueva conversación** — Inicia `claude` en un directorio de trabajo elegido con un solo clic, con un directorio por defecto recordado.
- **Visor del historial de sesiones** — Navegación de solo lectura de la transcripción completa (flujo de mensajes usuario/asistente; pensamiento/llamadas de herramienta plegables).
- **Estilo Win11** — Translucidez Mica, panel flotante de la bandeja, animaciones de entrada/deslizamiento de tarjetas, tema oscuro.

---

## 📥 Instalación

### Opción 1: Descargar (recomendado)

Descarga cualquiera de estos desde [Releases](../../releases):

| Archivo | Descripción | Tamaño |
|------|-------------|------|
| `Cove.exe` | Archivo único portátil — doble clic para ejecutar | ~10 MB |
| `Cove_0.6.0_x64-setup.exe` | Instalador NSIS | ~2.2 MB |
| `Cove_0.6.0_x64_en-US.msi` | Instalador MSI | ~3.5 MB |

**Solo Windows 10/11 x64.** Tras instalar/ejecutar, aparece un icono en la bandeja; haz clic en él para desplegar el panel.

### Compilar desde el código fuente

Requiere Windows 10/11 + Rust 1.96+ + Node.js 24+ + VS 2022 Build Tools (carga de trabajo de C++).

```bash
git clone https://github.com/LUMIAO9527/Cove.git
cd Cove
npm install --include=dev
npm run tauri dev          # dev mode (hot reload)
npm run tauri build        # build release artifacts
```

---

## 🏗️ Arquitectura

**Tauri 2.11 + Rust + TypeScript nativo (sin React/Vue) + Vite.** Artefacto < 11 MB, memoria en tiempo de ejecución ~34 MB.

### Arquitectura multi-herramienta

Cada herramienta tiene un diseño de datos y un esquema de sesión completamente distintos, por lo que el escaneo, la transcripción y el lanzamiento se distribuyen por herramienta:

| | Claude Code | Reasonix |
|---|---|---|
| Sesiones | `~/.claude/projects/<encoded>/<SID>.jsonl` | `~/.reasonix/sessions/<name>.jsonl` + `.meta.json` |
| ID | UUID | nombre de archivo (sin UUID) |
| Reanudar | `claude --resume <SID>` | `reasonix code -r` (la última del espacio de trabajo) |
| Limpieza | escaneo de huérfanos completo en 8 ubicaciones | no aplicable (los sidecars se eliminan con la sesión) |

Un enumerado `ToolKind` (`src-tauri/src/tools/`) enruta cada operación al adaptador correcto.

### El núcleo: 8 ubicaciones de datos relacionadas

Al eliminar una conversación, quitar únicamente la transcripción `.jsonl` deja los otros 7 como huérfanos. Cove gestiona todos ellos:

| # | Dato | Ruta | Clave de unión |
|---|------|------|----------|
| ① | Transcripción | `projects\<encoded>\<SID>.jsonl` | nombre de archivo |
| ② | Subdirectorio del mismo nombre | `projects\<encoded>\<SID>\` (subagents/results) | nombre de directorio |
| ③ | Tareas pendientes | `tasks\<SID>\` | nombre de directorio |
| ④ | Instantáneas de edición | `file-history\<SID>\` | nombre de directorio |
| ⑤ | Eventos de telemetría | `telemetry\1p_failed_events.<SID>.<X>.json` | prefijo de nombre de archivo |
| ⑥ | Entorno de sesión | `session-env\<SID>\` | nombre de directorio |
| ⑦ | Historial de comandos | `history.jsonl` | campo `sessionId` en línea |
| ⑧ | Metadatos del proceso | `sessions\<PID>.json` | `sessionId` en el archivo |

### Estructura del código

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

## ⚙️ Datos en tiempo de ejecución

| Propósito | Ruta |
|---------|------|
| Datos de Claude Code (Cove lee y escribe) | `~/.claude/` |
| Área de archivado de Cove | `~/.claude-managed/archive/` |

Cove **no recopila telemetría**. Todos los datos se quedan en local.

---

## 🛠️ Desarrollo

```bash
cd src-tauri
cargo test                # run integration tests
```

**Problemas comunes**

- `cargo build` muestra `link.exe not found` → instala VS 2022 Build Tools (con la carga de trabajo de C++).
- `npm install` solo instaló unos pocos paquetes → usa `npm install --include=dev`.
- No se encuentran los comandos de Rust → Rust se instala por defecto en `~/.cargo/bin` y puede que no esté en el PATH.

---

## 📄 Licencia

[MIT](./LICENSE) · ⭐ Dale una estrella a este repositorio si Cove te ayuda.
