# Cove

> 一个 Windows 系统托盘弹窗工具，像管理邮件一样管理 [Claude Code](https://claude.com/claude-code) 的对话与项目：展示模型状态、软归档/恢复、真删除、以及跨 8 处关联数据的协同清理与全局孤儿扫描。

Cove 常驻系统托盘，点击图标弹出 380×580 的 Win11 风格 flyout 面板，失焦自动隐藏。它解决一个真实痛点：**删除一条 Claude Code 对话时，除了 `.jsonl` 正文，还有 7 处关联数据会残留成"孤儿"**（tasks、file-history、session-env、telemetry……）。Cove 把这 8 处一并处理，并提供软归档（移走 + 可恢复）和全局孤儿扫描。

当前版本 **v0.4.28**（三轮挑刺式代码评审后收敛，三模型一致判定可发布）。

---

## ✨ 功能

- **项目 & 对话管理**：扫描 `~/.claude/projects/`，按项目分组列出所有对话，展示每条对话的模型、消息数、大小、首问/末答摘要
- **三级标题回退**：`custom-title` → `ai-title` → `summary` → 首条用户消息 → SID 前缀，绝不会出现"无标题"
- **软归档（类 Gmail）**：把对话及其全部关联数据移到归档区，随时可恢复到原位
- **真删除**：永久删除对话 + 8 处关联数据，不可恢复
- **全局孤儿扫描**：扫描所有"正文已删但附属数据残留"的孤儿，单项或批量清理
- **模型展示**：顶栏全局默认模型（读 `settings.json`）+ 每条对话实际跑过的模型
- **新对话启动器**：在指定工作目录一键启动 `claude`，支持默认目录记忆
- **Win11 风格**：Mica 半透明、托盘 flyout、卡片入场/滑出动画、深色主题

---

## 📦 安装

### 方式一：下载安装包（推荐普通用户）

从 [Releases](../../releases) 下载任一：
- `Cove_0.4.28_x64-setup.exe` — NSIS 安装包（~2.2 MB）
- `Cove_0.4.28_x64_en-US.msi` — MSI 安装包（~3.5 MB）
- `Cove.exe` — 免安装单文件（~10 MB，直接双击运行）

安装后从开始菜单启动，托盘出现图标，点击即弹出面板。

### 方式二：从源码构建

需要 Windows 10/11 + Rust 1.96+ + Node.js 24+ + VS 2022 Build Tools（C++ 工作负载）。

```bash
git clone <this-repo>
cd Cove
npm install --include=dev          # 必须带 --include=dev
npm run tauri dev                  # 开发模式（热重载）
npm run tauri build                # 打包 release 产物
```

产物在 `src-tauri/target/release/bundle/`。

---

## 🏗️ 架构

技术栈：**Tauri 2.11 + Rust + 原生 TypeScript（无 React/Vue）+ Vite**。产物 < 11 MB，运行时内存 ~34 MB。

### 核心：8 处关联数据

删除一条 Claude Code 对话时，如果只删 `.jsonl` 正文，其余 7 处会残留成孤儿。Cove 把它们一并处理：

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

### 归档结构（v0.4.26+ 封装目录）

归档区采用每个 SID 一个封装目录的结构，彻底避免早期版本"同名目录互相覆盖销毁数据"的 bug：

```
~/.claude-managed/archive/
├── index.json
└── <encoded>/
    └── <sid>/                      ← 每个会话一个封装目录
        ├── transcript.jsonl
        ├── project_subdir/
        ├── tasks/
        ├── file-history/
        ├── session-env/
        ├── telemetry/
        └── session-meta/
```

恢复时按子名精确反路由到原始位置。

### 代码结构

```
Cove/
├── src-tauri/src/          # Rust 后端
│   ├── lib.rs              # 托盘/窗口/状态机/单实例/Mica + 首启迁移
│   ├── commands.rs         # 29 个 #[tauri::command] 桥接层
│   ├── scan.rs             # jsonl 解析、标题三级回退
│   ├── transcript.rs       # 会话全文解析（只读查看）
│   ├── related.rs          # 8 处关联数据定位（工具核心）
│   ├── cleanup.rs          # 关联删除、孤儿扫描、history 行级过滤
│   ├── archive.rs          # 封装目录归档/恢复/索引 + atomic_write
│   ├── paths.rs            # 路径编码/解码、verbatim 前缀剥离
│   ├── settings.rs         # settings.json + cove-settings.json
│   ├── projects_config.rs  # 项目列表读写 + 迁移
│   └── models.rs           # 数据结构
├── src-tauri/tests/        # 41 个集成测试（全过）
├── src/                    # 前端（原生 TS）
│   ├── main.ts             # 入口/路由/动画/定位
│   ├── api.ts              # invoke 封装
│   └── views/              # projects/conversations/loose/archive/cleanup/session-detail/confirm
└── src/styles/             # theme.css + animations.css + icons.ts
```

---

## 🧪 测试与质量

```bash
cd src-tauri
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
cargo test        # 41 项全过
```

测试用 `tempfile` 动态生成临时 `.claude` 目录，完全自包含、可重复、无副作用。

### 三轮挑刺式代码评审

v0.4.25 → v0.4.28 经过了三轮三模型（DeepSeek-V4-Pro + GLM-5.2 + Qwen3.7-Max）并行 adversarial review，共发现并修复：
- 归档时同名目录互相覆盖销毁数据（P0，最严重）
- restore 路由错配、配置非原子写、HTML 引号注入、乐观删除无回滚
- CSP 加固、shell 命令注入防御、托盘抖动、迁移幂等性

第三轮三模型一致结论：**已收敛，无发布阻塞项**。

---

## ⚙️ 运行时数据位置

| 用途 | 路径 |
|---|---|
| Claude Code 数据（Cove 读写） | `~/.claude/` |
| Cove 归档区 | `~/.claude-managed/archive/` |
| Cove 调试日志 | `~/.claude-managed/cove-debug.log` |
| 首启迁移标记 | `~/.claude-managed/archive/.archive-v2` |

Cove **不**收集任何遥测，所有数据都在本地。

---

## 🔒 安全设计要点

- **路径边界校验**：所有删除命令校验目标路径必须在 `~/.claude/` 之下（canonicalize + startswith），防 IPC 滥用删任意文件
- **shell 注入防御**：启动 `claude --resume <sid>` 前校验 sid 是合法 UUID 格式
- **HTML 注入防御**：所有用户可控内容（标题/路径/输入）经完整转义（含 `"` `'`）后渲染；CSP 限定 `default-src 'self'`
- **原子写入**：配置文件（项目列表/设置/归档索引/history）用 `atomic_write`（写 .tmp 再 rename），崩溃不截断
- **归档数据保全**：归档/恢复部分失败时保留 capsule 和索引，让用户能重试或手动取回，不静默销毁数据

---

## 🛠️ 开发

### 常见问题

**`cargo build` 报 `link.exe not found`？** 安装 VS 2022 Build Tools（含 C++ 工作负载）。

**`npm install` 只装了几个包？** 本机 npm 可能配了 `omit=dev`，用 `npm install --include=dev`。

**Rust 命令找不到？** Rust 默认装在 `~/.cargo/bin`，可能不在系统 PATH。每次新开终端 `set PATH=%USERPROFILE%\.cargo\bin;%PATH%`。

### 版本发布流程

1. 改版本号三处一致：`package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json`
2. 停掉运行中的 Cove（否则 exe 被锁）
3. `npm run build` + `npm run tauri build`
4. 产物复制到 `dist-release/`（Cove.exe 固定名 + 带版本号的 setup/msi），清旧版本
5. （内部）更新开发交接文档记录版本变更

---

## 📄 License

[MIT](LICENSE)。
