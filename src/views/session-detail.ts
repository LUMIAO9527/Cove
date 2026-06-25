import { api, ContentBlock, SessionTranscript, TranscriptTurn, ToolName } from "../api";
import { icon } from "../styles/icons";
import { toast } from "./confirm";
import { escapeHtml } from "./projects";

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
    onBack: () => void
): Promise<void> {
    // Loading state.
    container.innerHTML = `
        <div class="scroll-area">
            <div class="nav-bar is-transcript">
                <div class="nav-row">
                    <button class="back-btn" id="back-btn">${icon("back", 18)}</button>
                    <span class="nav-title" title="${escapeHtml(sessionTitle)}">${escapeHtml(sessionTitle)}</span>
                    <button class="btn btn-ghost section-action" id="resume-btn" title="在终端继续此会话">
                        ${icon("play", 14)} 继续会话
                    </button>
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
        <div class="nav-bar is-transcript">
            <div class="nav-row">
                <button class="back-btn" id="back-btn2">${icon("back", 18)}</button>
                <span class="nav-title" title="${escapeHtml(transcript.title)}">${escapeHtml(transcript.title)}</span>
                <button class="btn btn-ghost section-action" id="resume-btn2" title="在终端继续此会话">
                    ${icon("play", 14)} 继续
                </button>
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
