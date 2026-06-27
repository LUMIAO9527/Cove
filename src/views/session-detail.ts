import { api, ContentBlock, SessionTranscript, TranscriptTurn, ToolName } from "../api";
import { icon } from "../styles/icons";
import { toast } from "./confirm";
import { escapeHtml, bindHoverMenu, createAnchoredMenu } from "./projects";
import { save } from "@tauri-apps/plugin-dialog";

/**
 * Session history viewer: read-only transcript of one conversation.
 * Renders the full user/assistant turn sequence, with thinking and tool
 * calls rendered as collapsible sections. Read-only — continuing the
 * conversation still goes through the terminal (`openSession`).
 */
export async function renderSessionDetailView(
    container: HTMLElement,
    tool: ToolName,
    sid: string,
    projectEncoded: string,
    projectPath: string,
    sessionTitle: string,
    onBack: () => void,
    isArchived: boolean = false
): Promise<void> {
    // Loading state. nav-bar 结构和加载完态完全一致（内联 style + split-button），
    // 避免 loading→loaded 切换时标题行布局跳动。
    container.innerHTML = `
        <div class="scroll-area">
            <div class="nav-bar is-transcript" style="display:block">
                <div class="nav-row">
                    <button class="back-btn" id="back-btn">${icon("back", 13)}</button>
                    <span class="nav-title" style="padding:0" title="${escapeHtml(sessionTitle)}">${escapeHtml(sessionTitle)}</span>
                    <span class="new-chat-wrap" style="flex-shrink:0">
                        <button class="btn btn-ghost section-action new-chat-main" id="resume-btn" title="在终端继续此会话" disabled>
                            ${icon("play", 14)} 继续
                        </button>
                        <button class="btn btn-ghost section-action new-chat-caret" id="resume-caret" title="更多操作" disabled>▾</button>
                    </span>
                </div>
            </div>
            <div class="transcript-loading">加载会话记录…</div>
        </div>`;

    const backBtn = document.getElementById("back-btn");
    backBtn?.addEventListener("click", onBack);

    let transcript: SessionTranscript;
    try {
        transcript = await api.getTranscript(tool, sid, projectEncoded);
    } catch (err) {
        container.querySelector(".transcript-loading")!.innerHTML =
            `<div class="empty-state">
                <div class="empty-icon">${icon("warn", 26)}</div>
                <div class="empty-title">读取会话失败</div>
                <div class="hint">${escapeHtml(String(err))}</div>
             </div>`;
        return;
    }

    // Bind resume after we have the title (uses projectPath + sid).
    document.getElementById("resume-btn")?.addEventListener("click", async () => {
        try {
            await api.openSession(tool, projectPath, sid);
        } catch (err) {
            toast("启动失败：" + String(err));
        }
    });

    const body = container.querySelector<HTMLElement>(".scroll-area")!;
    const turnsHtml =
        transcript.turns.length === 0
            ? `<div class="empty-state">
                  <div class="empty-icon">${icon("inbox", 26)}</div>
                  <div class="empty-title">该会话没有消息记录</div>
                  <div class="hint">可能是一个刚创建、尚未对话的会话</div>
               </div>`
            : transcript.turns.map(renderTurn).join("");

    // Meta line under the title: model · last-dir · relative time.
    // Each segment is optional — only rendered when non-empty, joined by "·".
    const metaParts: string[] = [escapeHtml(transcript.model)];
    const dirName = lastDirOf(transcript.cwd);
    if (dirName) metaParts.push(`<span title="${escapeHtml(transcript.cwd)}">${escapeHtml(dirName)}</span>`);
    const relTime = relativeTime(transcript.last_updated);
    if (relTime) metaParts.push(escapeHtml(relTime));
    const metaHtml = `<div class="nav-meta">${metaParts.join(' <span class="nav-meta-sep">·</span> ')}</div>`;

    body.innerHTML = `
        <div class="nav-bar is-transcript" style="display:block">
            <div class="nav-row">
                <button class="back-btn" id="back-btn2">${icon("back", 13)}</button>
                <span class="nav-title" style="padding:0" title="${escapeHtml(transcript.title)}">${escapeHtml(transcript.title)}</span>
                ${
                    isArchived
                        ? `<span class="arch-badge">已归档</span>`
                        : `<span class="new-chat-wrap" style="flex-shrink:0">
                            <button class="btn btn-ghost section-action new-chat-main" id="resume-btn2" title="在终端继续此会话">
                                ${icon("play", 14)} 继续
                            </button>
                            <button class="btn btn-ghost section-action new-chat-caret" id="resume-caret" title="更多操作">▾</button>
                        </span>`
                }
            </div>
            ${metaHtml}
        </div>
        <div class="transcript">
            ${turnsHtml}
        </div>`;

    document.getElementById("back-btn2")?.addEventListener("click", onBack);
    document.getElementById("resume-btn2")?.addEventListener("click", async () => {
        try {
            await api.openSession(tool, projectPath, sid);
        } catch (err) {
            toast("启动失败：" + String(err));
        }
    });
    // ▾ 菜单：hover 触发（复制全部对话 / 导出为 .md / 在文件夹打开）。
    const resumeCaret = document.getElementById("resume-caret");
    if (resumeCaret) bindHoverMenu(resumeCaret, (anchor) => showSessionMenu(anchor, transcript, tool, sid, projectEncoded));

    // Wire collapsible thinking / tool sections.
    body.querySelectorAll<HTMLElement>(".collapsible-header").forEach((header) => {
        header.addEventListener("click", (e) => {
            e.stopPropagation();
            const section = header.closest(".collapsible") as HTMLElement;
            section?.classList.toggle("is-open");
        });
    });
}

function renderTurn(turn: TranscriptTurn): string {
    const isUser = turn.role === "user";
    const roleLabel = isUser ? "你" : "AI";
    const roleClass = isUser ? "turn-user" : "turn-assistant";
    const blocks = turn.blocks.map((b) => renderBlock(b)).join("");
    return `
        <div class="turn ${roleClass}">
            <div class="turn-role">${roleLabel}</div>
            <div class="turn-body">${blocks}</div>
        </div>`;
}

function renderBlock(block: ContentBlock): string {
    switch (block.kind) {
        case "Text":
            return `<div class="block-text">${escapeHtml(block.text)}</div>`;

        case "Thinking":
            return `
                <div class="collapsible block-thinking">
                    <div class="collapsible-header">${icon("chevron", 12)} 思考过程</div>
                    <div class="collapsible-body"><pre>${escapeHtml(block.thinking)}</pre></div>
                </div>`;

        case "ToolUse": {
            const inputStr = formatToolInput(block.input);
            return `
                <div class="collapsible block-tool-use">
                    <div class="collapsible-header">${icon("chevron", 12)} 工具调用：${escapeHtml(block.name)}</div>
                    <div class="collapsible-body"><pre>${escapeHtml(inputStr)}</pre></div>
                </div>`;
        }

        case "ToolResult":
            return `
                <div class="collapsible block-tool-result">
                    <div class="collapsible-header">${icon("chevron", 12)} 工具结果</div>
                    <div class="collapsible-body"><pre>${escapeHtml(block.text)}</pre></div>
                </div>`;

        default:
            return "";
    }
}

/** Pretty-print a tool input object, truncated to keep the DOM light. */
function formatToolInput(input: unknown): string {
    let str: string;
    try {
        str = typeof input === "string" ? input : JSON.stringify(input, null, 2);
    } catch {
        return String(input);
    }
    const MAX = 2000;
    if (str.length > MAX) {
        return str.slice(0, MAX) + `\n…（已截断，共 ${str.length} 字符）`;
    }
    return str;
}

/** Last path segment of a working directory. Handles both / and \, strips a
 *  trailing separator. Returns "" for empty input. Used for the compact meta
 *  line; the full path is shown via the element's title attribute. */
function lastDirOf(cwd: string): string {
    if (!cwd) return "";
    const trimmed = cwd.replace(/[\\/]+$/, "");
    if (!trimmed) return "";
    const segs = trimmed.split(/[\\/]+/);
    return segs[segs.length - 1] || "";
}

/** Format an ISO timestamp as a short Chinese relative time.
 *  Returns "" for empty/unparseable input. */
function relativeTime(iso: string): string {
    if (!iso) return "";
    const then = Date.parse(iso);
    if (Number.isNaN(then)) return "";
    const diffMs = Date.now() - then;
    // Future timestamps (clock skew) → treat as "just now".
    if (diffMs < 0) return "刚刚";
    const min = Math.floor(diffMs / 60000);
    if (min < 1) return "刚刚";
    if (min < 60) return `${min} 分钟前`;
    const hr = Math.floor(min / 60);
    if (hr < 24) return `${hr} 小时前`;
    const day = Math.floor(hr / 24);
    if (day === 1) return "昨天";
    if (day < 7) return `${day} 天前`;
    const wk = Math.floor(day / 7);
    if (wk < 5) return `${wk} 周前`;
    // Beyond ~5 weeks, a calendar date is clearer than "X 月前".
    const d = new Date(then);
    return `${d.getMonth() + 1}-${d.getDate()}`;
}

// ===========================================================================
// 会话详情页 ▾ 菜单（继续会话 split-button 的小箭头）
//
// 复用 model-switcher flyout 范式（与散落对话页 showWorkspaceMenu 同构）。
// 三项功能：
//  - 复制全部对话：纯前端拼纯文本（user/assistant 轮流），navigator.clipboard。
//  - 导出为 .md：拼 markdown，save 对话框选路径，后端 save_text_file 写盘。
//  - 在文件夹打开：后端 open_session_location 定位 jsonl 父目录。
// ===========================================================================

/** 把 transcript 拼成纯文本（复制用）和 markdown（导出用）。 */
function transcriptToText(t: SessionTranscript, asMarkdown: boolean): string {
    const parts: string[] = [];
    if (asMarkdown) {
        parts.push(`# ${t.title}`);
        parts.push("");
        if (t.model) parts.push(`> 模型：${t.model}`);
        if (t.cwd) parts.push(`> 目录：${t.cwd}`);
        if (t.last_updated) parts.push(`> 时间：${t.last_updated}`);
        parts.push("");
        parts.push("---");
        parts.push("");
    }
    for (const turn of t.turns) {
        const isUser = turn.role === "user";
        const label = isUser ? "你" : "AI";
        if (asMarkdown) {
            parts.push(`## ${label}`);
            parts.push("");
        } else {
            parts.push(`【${label}】`);
        }
        for (const b of turn.blocks) {
            const text = blockToText(b, asMarkdown);
            if (text) {
                parts.push(text);
                parts.push("");
            }
        }
    }
    return parts.join("\n").replace(/\n{3,}/g, "\n\n").trim();
}

/** 单个 content block 转文本：Text 直出，思考/工具调用折叠成简短标记。 */
function blockToText(b: ContentBlock, asMarkdown: boolean): string {
    switch (b.kind) {
        case "Text":
            return b.text;
        case "Thinking":
            return asMarkdown
                ? `<details><summary>思考过程</summary>\n\n${b.thinking}\n\n</details>`
                : `[思考过程] ${b.thinking}`;
        case "ToolUse": {
            const inputStr = formatToolInput(b.input);
            return asMarkdown
                ? `<details><summary>工具调用：${b.name}</summary>\n\n\`\`\`\n${inputStr}\n\`\`\`\n\n</details>`
                : `[工具调用：${b.name}] ${inputStr}`;
        }
        case "ToolResult":
            return asMarkdown
                ? `<details><summary>工具结果</summary>\n\n\`\`\`\n${b.text}\n\`\`\`\n\n</details>`
                : `[工具结果] ${b.text}`;
        default:
            return "";
    }
}

/** 点 ▾ 弹出的菜单。复用 model-switcher flyout，点击外部关闭。 */
function showSessionMenu(
    anchor: HTMLElement,
    transcript: SessionTranscript,
    tool: ToolName,
    sid: string,
    projectEncoded: string
): HTMLElement {
    return createAnchoredMenu(anchor, "session-menu", `
        <button class="model-switcher-item" type="button" data-act="copy-first">
            <span class="ms-tier">${icon("message", 14)} 复制首条提问</span>
        </button>
        <button class="model-switcher-item" type="button" data-act="copy">
            <span class="ms-tier">${icon("copy", 14)} 复制全部对话</span>
        </button>
        <button class="model-switcher-item" type="button" data-act="export">
            <span class="ms-tier">${icon("folder", 14)} 导出为 .md…</span>
        </button>
        <button class="model-switcher-item" type="button" data-act="open">
            <span class="ms-tier">${icon("folder", 14)} 在文件夹打开</span>
        </button>`, {
        "copy-first": async () => {
            const firstUser = transcript.turns.find((t) => t.role === "user");
            const firstText = firstUser?.blocks.find((b) => b.kind === "Text");
            if (!firstText || !firstText.text.trim()) {
                toast("该会话没有用户提问");
                return;
            }
            try {
                await navigator.clipboard.writeText(firstText.text);
                toast("已复制首条提问");
            } catch { toast("复制失败"); }
        },
        "copy": async () => {
            try {
                await navigator.clipboard.writeText(transcriptToText(transcript, false));
                toast("已复制全部对话");
            } catch { toast("复制失败"); }
        },
        "export": async () => {
            await api.setDialogOpen(true);
            let savePath: string | null = null;
            try {
                const safeName = (transcript.title || sid).replace(/[\\/:*?"<>|]/g, "_").slice(0, 60);
                const picked = await save({
                    defaultPath: `${safeName}.md`,
                    filters: [{ name: "Markdown", extensions: ["md"] }],
                    title: "导出会话为 Markdown",
                });
                savePath = typeof picked === "string" ? picked : null;
            } finally {
                await api.setDialogOpen(false);
            }
            if (!savePath) return;
            try {
                await api.saveTextFile(savePath, transcriptToText(transcript, true));
                toast("已导出到：" + savePath);
            } catch (err) { toast("导出失败：" + String(err)); }
        },
        "open": async () => {
            try { await api.openSessionLocation(tool, sid, projectEncoded); }
            catch (err) { toast("打开失败：" + String(err)); }
        },
    });
}
