<div align="center">

# 🐬 Cove

### Uma ferramenta para a bandeja do sistema do Windows para gerenciar projetos e conversas do Claude Code e do Reasonix.

`system tray` · `flyout panel` · `no telemetry` · `local only`

<sub>Trate suas conversas do Claude Code como e-mail — limpas, arquivadas, nunca órfãs.</sub>

[![Windows](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-0078D4?logo=windows11&logoColor=white)](#install)
[![Tauri](https://img.shields.io/badge/Tauri-2.11-FFC131?logo=tauri&logoColor=black)](#architecture)
[![Rust](https://img.shields.io/badge/Rust-1.96+-CE422B?logo=rust&logoColor=white)](#build-from-source)
[![TypeScript](https://img.shields.io/badge/TypeScript-native-3178C6?logo=typescript&logoColor=white)](#architecture)
[![Release](https://img.shields.io/badge/release-v0.5.0-blue?logo=github&logoColor=white)](https://github.com/LUMIAO9527/Cove/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green?logo=opensourceinitiative&logoColor=white)](./LICENSE)
[![No Telemetry](https://img.shields.io/badge/telemetry-none-success)](#runtime-data)

**Outros idiomas:** &nbsp;[English](./README.md) · [简体中文](./README.zh-CN.md) · [日本語](./README.ja.md) · [Español](./README.es.md) · [Français](./README.fr.md) · [Deutsch](./README.de.md) · [Русский](./README.ru.md) · [한국어](./README.ko.md)

</div>

---

> O Cove vive na bandeja do sistema. Clique no ícone e um painel flutuante no estilo Win11 de 380×580 aparece; ele se oculta no momento em que você clica fora.
>
> Ele resolve um problema real: **quando você exclui uma conversa do Claude Code, excluir apenas a transcrição `.jsonl` deixa 7 artefatos relacionados para trás como "órfãos"** — tarefas, file-history, session-env, telemetria e mais. O Cove limpa todos os **8 locais juntos** e adiciona arquivamento suave (mover + restaurar) além de uma verificação global de órfãos.

---

## 📑 Sumário

- ✨ Recursos
- 📥 Instalação
- 🏗️ Arquitetura
- ⚙️ Dados de Execução
- 🛠️ Desenvolvimento
- 📄 Licença

---

## ✨ Recursos

- **Suporte multi-ferramenta** — Gerencie o **Claude Code** e o **Reasonix** lado a lado. Um seletor em cápsula na barra de título escolhe qual ferramenta cada página mostra; o status de instalação é detectado automaticamente.
- **Projetos e conversas** — Examina `~/.claude/projects/`, lista todas as conversas agrupadas por projeto e mostra o modelo de cada uma, contagem de mensagens, tamanho e o resumo da primeira pergunta.
- **Títulos inteligentes** — `custom-title` → `ai-title` → `summary` → primeira mensagem do usuário. Nunca mostra "Sem título".
- **Arquivamento suave** — Move uma conversa e todos os seus dados relacionados para uma área de arquivamento; totalmente restaurável ao local original.
- **Exclusão definitiva** — Remove permanentemente uma conversa junto com todos os 8 locais de dados relacionados.
- **Verificação global de órfãos** — Encontra cada órfão do tipo "transcrição sumiu, mas sobras permanecem" em todos os projetos; limpe um ou vários de uma vez.
- **Exibição do modelo** — Modelo padrão global na barra superior, além do modelo real em que cada conversa foi executada.
- **Inicializador de nova conversa** — Inicia o `claude` em um diretório de trabalho escolhido com um clique, com um diretório padrão memorizado.
- **Visualizador de histórico de sessão** — Navegação somente leitura da transcrição completa (fluxo de mensagens usuário/assistente; raciocínio/chamadas de ferramenta recolhíveis).
- **Estilo Win11** — Translucidez Mica, painel flutuante da bandeja, animações de entrada/saída dos cartões, tema escuro.

---

## 📥 Instalação

### Opção 1: Baixar (recomendado)

Baixe qualquer um destes em [Releases](../../releases):

| Arquivo | Descrição | Tamanho |
|---------|-----------|---------|
| `Cove.exe` | Arquivo único portátil — dê um duplo clique para executar | ~10 MB |
| `Cove_0.5.0_x64-setup.exe` | Instalador NSIS | ~2.2 MB |
| `Cove_0.5.0_x64_en-US.msi` | Instalador MSI | ~3.5 MB |

**Apenas Windows 10/11 x64.** Após instalar/executar, um ícone de bandeja aparece; clique nele para abrir o painel.

### Compilar a partir do código-fonte

Requer Windows 10/11 + Rust 1.96+ + Node.js 24+ + VS 2022 Build Tools (carga de trabalho C++).

```bash
git clone https://github.com/LUMIAO9527/Cove.git
cd Cove
npm install --include=dev
npm run tauri dev          # dev mode (hot reload)
npm run tauri build        # build release artifacts
```

---

## 🏗️ Arquitetura

**Tauri 2.11 + Rust + TypeScript nativo (sem React/Vue) + Vite.** Artefato < 11 MB, memória em tempo de execução ~34 MB.

### Arquitetura multi-ferramenta

Cada ferramenta possui um layout de dados e um esquema de sessão completamente diferentes, portanto a varredura/transcrição/inicialização é despachada por ferramenta:

| | Claude Code | Reasonix |
|---|---|---|
| Sessões | `~/.claude/projects/<codificado>/<SID>.jsonl` | `~/.reasonix/sessions/<nome>.jsonl` + `.meta.json` |
| ID | UUID | nome do arquivo (sem UUID) |
| Retomar | `claude --resume <SID>` | `reasonix code -r` (última do workspace) |
| Limpeza | varredura de órfãos completa de 8 locais | não aplicável (sidecars excluídos com a sessão) |

O enum `ToolKind` (`src-tauri/src/tools/`) encaminha cada operação ao adaptador correto.

### O núcleo: 8 locais de dados relacionados

Ao excluir uma conversa, remover apenas a transcrição `.jsonl` deixa os outros 7 como órfãos. O Cove trata todos eles:

| # | Dados | Caminho | Chave de junção |
|---|-------|---------|-----------------|
| ① | Transcrição | `projects\<encoded>\<SID>.jsonl` | nome do arquivo |
| ② | Subdiretório de mesmo nome | `projects\<encoded>\<SID>\` (subagentes/resultados) | nome do diretório |
| ③ | Tarefas pendentes | `tasks\<SID>\` | nome do diretório |
| ④ | Snapshots de edição | `file-history\<SID>\` | nome do diretório |
| ⑤ | Eventos de telemetria | `telemetry\1p_failed_events.<SID>.<X>.json` | prefixo do nome do arquivo |
| ⑥ | Ambiente de sessão | `session-env\<SID>\` | nome do diretório |
| ⑦ | Histórico de comandos | `history.jsonl` | campo `sessionId` embutido |
| ⑧ | Metadados de processo | `sessions\<PID>.json` | `sessionId` no arquivo |

### Estrutura do código

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

## ⚙️ Dados de Execução

| Finalidade | Caminho |
|------------|---------|
| Dados do Claude Code (o Cove lê e grava) | `~/.claude/` |
| Área de arquivamento do Cove | `~/.claude-managed/archive/` |

O Cove **não coleta telemetria**. Todos os dados permanecem locais.

---

## 🛠️ Desenvolvimento

```bash
cd src-tauri
cargo test                # run integration tests
```

**Problemas comuns**

- `cargo build` relata `link.exe not found` → instale o VS 2022 Build Tools (com a carga de trabalho C++).
- `npm install` instalou apenas alguns pacotes → use `npm install --include=dev`.
- Comandos do Rust não encontrados → o Rust é instalado em `~/.cargo/bin` por padrão e pode não estar no PATH.

---

## 📄 Licença

[MIT](./LICENSE) · ⭐ Dê uma estrela neste repositório se o Cove ajudar você.
