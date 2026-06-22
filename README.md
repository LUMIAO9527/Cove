# Cove

> 一个 Windows 系统托盘工具，用来管理 [Claude Code](https://claude.com/claude-code) 的项目与对话。

Cove 常驻系统托盘，点击图标弹出 380×580 的 Win11 风格 flyout 面板，失焦自动隐藏。

它解决一个真实痛点：**删除一条 Claude Code 对话时，除了 `.jsonl` 正文，还有 7 处关联数据会残留成"孤儿"**——tasks、file-history、session-env、telemetry 等。Cove 把这 8 处一并处理，并提供软归档（移走 + 可恢复）和全局孤儿扫描。

---

## ✨ 功能

- **项目 & 对话管理** — 扫描 `~/.claude/projects/`，按项目分组列出所有对话，展示每条对话的模型、消息数、大小、首问摘要
- **智能标题** — `custom-title` → `ai-title` → `summary` → 首条用户消息，绝不会出现"无标题"
- **软归档** — 把对话及其全部关联数据移到归档区，随时可恢复到原位
- **真删除** — 永久删除对话 + 8 处关联数据
- **全局孤儿扫描** — 扫描所有"正文已删但附属残留"的孤儿，单项或批量清理
- **模型展示** — 顶栏全局默认模型 + 每条对话实际跑过的模型
- **新对话启动器** — 在指定工作目录一键启动 `claude`，支持默认目录记忆
- **会话历史查看** — 只读浏览完整对话记录（user/assistant 消息流，思考过程/工具调用可折叠）
- **Win11 风格** — Mica 半透明、托盘 flyout、卡片入场/滑出动画、深色主题

---

## 📥 安装

### 方式一：下载（推荐）

从 [Releases](../../releases) 下载任一：

| 文件 | 说明 | 大小 |
|---|---|---|
| `Cove.exe` | 免安装单文件，双击即用 | ~10 MB |
| `Cove_0.4.28_x64-setup.exe` | NSIS 安装包 | ~2.2 MB |
| `Cove_0.4.28_x64_en-US.msi` | MSI 安装包 | ~3.5 MB |

仅支持 Windows 10/11 x64。安装/运行后托盘出现图标，点击即弹出面板。

### 方式二：从源码构建

需要 Windows 10/11 + Rust 1.96+ + Node.js 24+ + VS 2022 Build Tools（C++ 工作负载）。

```bash
git clone <this-repo>
cd Cove
npm install --include=dev
npm run tauri dev          # 开发模式（热重载）
npm run tauri build        # 打包 release 产物
```

---

## 🏗️ 架构

**Tauri 2.11 + Rust + 原生 TypeScript（无 React/Vue）+ Vite**。产物 < 11 MB，运行时内存 ~34 MB。

### 核心：8 处关联数据

删除一条对话时，如果只删 `.jsonl` 正文，其余 7 处会残留成孤儿。Cove 把它们一并处理：

| # | 数据 | 路径 | 关联键 |
|---|---|---|---|
| ① | 对话正文 | `projects\<编码>\<SID>.jsonl` | 文件名 |
| ② | 同名子目录 | `projects\<编码>\<SID>\` (subagents/results) | 目录名 |
| ③ | Todo 任务 | `tasks\<SID>\` | 目录名 |
| ④ | 编辑快照 | `file-history\<SID>\` | 目录名 |
| ⑤ | 遥测事件 | `telemetry\1p_failed_events.<SID>.<X>.json` | 文件名前缀 |
| ⑥ | 会话环境 | `session-env\<SID>\` | 目录名 |
| ⑦ | 命令历史 | `history.jsonl` | 行内 `sessionId` 字段 |
| ⑧ | 进程元数据 | `sessions\<PID>.json` | 文件内 `sessionId` |

### 代码结构

```
Cove/
├── src-tauri/src/          # Rust 后端
│   ├── lib.rs              # 托盘/窗口/状态机/单实例/Mica
│   ├── commands.rs         # Tauri command 桥接层
│   ├── scan.rs             # jsonl 解析、标题回退
│   ├── transcript.rs       # 会话全文解析（只读查看）
│   ├── related.rs          # 8 处关联数据定位
│   ├── cleanup.rs          # 关联删除、孤儿扫描
│   ├── archive.rs          # 归档/恢复/索引
│   ├── paths.rs            # 路径编码/解码
│   ├── settings.rs         # settings.json 读写
│   ├── projects_config.rs  # 项目列表读写
│   └── models.rs           # 数据结构
├── src-tauri/tests/        # 集成测试
├── src/                    # 前端（原生 TS）
│   ├── main.ts             # 入口/路由/动画
│   ├── api.ts              # invoke 封装
│   └── views/              # 项目/对话/归档/清理/会话详情等视图
└── src/styles/             # 主题 + 动画 + 图标
```

---

## ⚙️ 运行时数据

| 用途 | 路径 |
|---|---|
| Claude Code 数据（Cove 读写） | `~/.claude/` |
| Cove 归档区 | `~/.claude-managed/archive/` |

Cove 不收集任何遥测，所有数据都在本地。

---

## 🛠️ 开发

```bash
cd src-tauri
cargo test                # 跑集成测试
```

**常见问题**

- `cargo build` 报 `link.exe not found` → 安装 VS 2022 Build Tools（含 C++ 工作负载）
- `npm install` 只装了几个包 → 用 `npm install --include=dev`
- Rust 命令找不到 → Rust 默认装在 `~/.cargo/bin`，可能不在系统 PATH

---

## 📄 License

[MIT](LICENSE)
