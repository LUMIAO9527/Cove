import { api, ContentBlock, SessionTranscript, TranscriptTurn } from "../api";
import { icon } from "../styles/icons";
import { toast } from "./confirm";
import { escapeHtml } from "./projects";

/**
 * Session history viewer: read-only transcript of one conversation.
 * Renders the full user/assistant turn sequence, with thinking and tool
 * calls rendered as collapsible sections. Read-only — continuing the
 * conversation still goes through the terminal (`openClaudeSession`).
 */
export async function renderSessionDetailView(
    container: HTMLElement,
    sid: string,
    projectEncoded: string,
    projectPath: string,
    sessionTitle: string,
    onBack: () => void
): Promise<void> {
    // Loading state.
    container.innerHTML = `
        <div class="scroll-area">
            <div class="nav-bar">
                <button class="back-btn" id="back-btn">${icon("back", 18)}</button>
                <span class="section-label">${escapeHtml(sessionTitle)}</span>
                <button class="btn btn-ghost section-action" id="resume-btn" title="在终端继续此会话">
                    ${icon("play", 14)} 继续会话
                </button>
            </div>
            <div class="transcript-loading">加载会话记录…</div>
        </div>`;

    const backBtn = document.getElementById("back-btn");
    backBtn?.addEventListener("click", onBack);

    let transcript: SessionTranscript;
    try {
        transcript = await api.getTranscript(sid, projectEncoded);
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
            await api.openClaudeSession(projectPath, sid);
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

    body.innerHTML = `
        <div class="nav-bar">
            <button class="back-btn" id="back-btn2">${icon("back", 18)}</button>
            <span class="section-label">${escapeHtml(transcript.title)}</span>
            <span class="model-tag" style="margin-left:auto;">${escapeHtml(transcript.model)}</span>
            <button class="btn btn-ghost section-action" id="resume-btn2" title="在终端继续此会话">
                ${icon("play", 14)} 继续
            </button>
        </div>
        <div class="transcript">
            ${turnsHtml}
        </div>`;

    document.getElementById("back-btn2")?.addEventListener("click", onBack);
    document.getElementById("resume-btn2")?.addEventListener("click", async () => {
        try {
            await api.openClaudeSession(projectPath, sid);
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
