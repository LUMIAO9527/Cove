import { api, Conversation, ToolName } from "../api";
import { icon } from "../styles/icons";
import { confirmDialog, toast, promptDialog } from "./confirm";
import { escapeHtml, formatSize, formatTime, showConvoInfo, fullDeleteConfirmOptions, animateRemoveCard } from "./projects";
import { open } from "@tauri-apps/plugin-dialog";

/**
 * Loose conversations view (scattered sessions not attached to a registered project).
 * Like Codex's "对话" tab. Card layout mirrors the inline session rows in
 * projects.ts (title + meta on the left, hover-revealed icon actions on the right).
 *
 * 右上角「新对话▾」按钮：点击主体静默开新会话（用已存的默认工作目录）；
 * 点右侧小箭头 ▾ 弹菜单（查看/更改默认目录）。首次点击时引导选择目录。
 *
 * onSelectSession 只传 4 个业务参数（sid/encoded/cwd/title）。
 * 返回时怎么恢复（重新渲染 + 定位）是 main.ts 的职责，由调用方在闭包里
 * 持有 onBack 并传给 showSessionDetail——本视图不需要知道返回逻辑。
 */
export async function renderLooseView(
    container: HTMLElement,
    tool: ToolName,
    onSelectSession: (sid: string, encoded: string, projPath: string, title: string) => void
): Promise<void> {
    const convos = await api.getLooseConversations(tool);

    if (convos.length === 0) {
        container.innerHTML = `
            <div class="scroll-area">
                <div class="section-label">
                    ${icon("message", 13)} 对话
                    ${renderNewChatButton()}
                </div>
                <div class="empty-state">
                    <div class="empty-icon">${icon("message", 26)}</div>
                    <div class="empty-title">没有散落对话</div>
                    <div class="hint">未归入项目的 Claude Code 会话会显示在这里</div>
                </div>
            </div>`;
        bindNewChatButton(container, tool);
        return;
    }

    const renderCard = (c: Conversation) => `
        <div class="card sub-session is-loose" data-sid="${escapeHtml(c.id)}">
            <button class="sub-info-btn" title="会话详情">${icon("info", 13)}</button>
            <div class="sub-main">
                <div class="sub-title loose-title" data-sid="${escapeHtml(c.id)}" data-encoded="${escapeHtml(c.project_encoded)}" data-cwd="${escapeHtml(c.cwd)}" title="${escapeHtml(c.title)}">${escapeHtml(c.title)}</div>
                <div class="meta">
                    <span class="model-tag">${escapeHtml(c.model)}</span>
                    <span class="sep">·</span>
                    <span>${c.message_count} 条</span>
                    <span class="sep">·</span>
                    <span>${formatSize(c.size_bytes)}</span>
                    <span class="sep">·</span>
                    <span>${formatTime(c.last_updated)}</span>
                </div>
                ${
                    c.first_user_preview
                        ? `<div class="preview">— ${escapeHtml(c.first_user_preview.slice(0, 50))}</div>`
                        : ""
                }
            </div>
            <div class="sub-actions">
                <button class="action-chip loose-action" data-action="resume" data-sid="${escapeHtml(c.id)}" data-cwd="${escapeHtml(c.cwd)}" title="继续会话">${icon("play", 13)}<span class="action-chip-label">继续</span></button>
                <button class="action-chip loose-action" data-action="rename" data-sid="${escapeHtml(c.id)}" data-encoded="${escapeHtml(c.project_encoded)}" title="重命名">${icon("edit", 13)}<span class="action-chip-label">重命名</span></button>
                <button class="action-chip loose-action" data-action="archive" data-sid="${escapeHtml(c.id)}" data-encoded="${escapeHtml(c.project_encoded)}" title="归档">${icon("archive", 13)}<span class="action-chip-label">归档</span></button>
                <button class="action-chip loose-action is-danger" data-action="delete" data-sid="${escapeHtml(c.id)}" data-encoded="${escapeHtml(c.project_encoded)}" title="删除">${icon("trash", 13)}<span class="action-chip-label">删除</span></button>
            </div>
        </div>`;

    container.innerHTML = `
        <div class="scroll-area">
            <div class="section-label">
                ${icon("message", 13)} 对话 · ${convos.length} 条
                ${renderNewChatButton()}
            </div>
            ${convos.map(renderCard).join("")}
        </div>`;

    bindNewChatButton(container, tool);

    // Click anywhere on the row (sub-main) => open the transcript viewer.
    // 不再只绑定标题文字——整个左侧内容区（标题+meta+预览）都可点进详情。
    container.querySelectorAll<HTMLElement>(".sub-main").forEach((main) => {
        main.addEventListener("click", (e) => {
            e.stopPropagation();
            const card = main.closest(".sub-session") as HTMLElement;
            const sid = card?.dataset.sid!;
            const titleEl = card?.querySelector<HTMLElement>(".sub-title");
            const encoded = titleEl?.dataset.encoded ?? "";
            const cwd = titleEl?.dataset.cwd ?? "";
            const sessionTitle = titleEl?.textContent?.trim() ?? sid;
            onSelectSession(sid, encoded, cwd, sessionTitle);
        });
    });

    // Info button => hover flyout with session metadata (no rename/remove).
    container.querySelectorAll<HTMLElement>(".sub-info-btn").forEach((btn) => {
        btn.addEventListener("click", (e) => e.stopPropagation());
        const card = btn.closest(".sub-session") as HTMLElement;
        const sid = card?.dataset.sid!;
        const convo = convos.find((c) => c.id === sid);
        if (convo) showConvoInfo(btn, convo);
    });

    container.querySelectorAll<HTMLElement>(".loose-action").forEach((btn) => {
        btn.addEventListener("click", async (e) => {
            e.stopPropagation();
            const sid = btn.dataset.sid!;
            const action = btn.dataset.action!;

            if (action === "resume") {
                const cwd = btn.dataset.cwd!;
                try {
                    await api.openSession(tool, cwd, sid);
                } catch (err) {
                    toast("启动失败：" + String(err));
                }
                return;
            }

            if (action === "rename") {
                const encoded = btn.dataset.encoded!;
                const card = btn.closest(".card") as HTMLElement;
                const titleEl = card?.querySelector<HTMLElement>(".sub-title");
                const currentTitle = titleEl?.textContent?.trim() ?? "";
                const name = await promptDialog({
                    title: "重命名会话",
                    body: "新名字将写入对话记录，<span class='mono'>claude /resume</span> 列表会显示这个名字。",
                    placeholder: "输入新会话名",
                    initialValue: currentTitle,
                    confirmText: "重命名",
                });
                if (!name || !name.trim()) return;
                try {
                    const newName = await api.renameSession(tool, sid, encoded, name);
                    // In-place update keeps scroll position and feels instant.
                    if (titleEl) titleEl.textContent = newName;
                    toast("已重命名为：" + newName);
                } catch (err) {
                    toast("重命名失败：" + String(err));
                }
                return;
            }

            const encoded = btn.dataset.encoded!;
            if (action === "delete") {
                const ok = await confirmDialog(fullDeleteConfirmOptions(sid));
                if (!ok) return;
                const card = btn.closest(".card") as HTMLElement;
                const success = await animateRemoveCard(card, () =>
                    api.deleteConvo(tool, sid, encoded)
                );
                if (success) toast("已删除会话");
            } else if (action === "archive") {
                const card = btn.closest(".card") as HTMLElement;
                const success = await animateRemoveCard(card, () =>
                    api.archiveConvo(tool, sid, encoded)
                );
                if (success) toast("已归档会话");
            }
        });
    });
}

// ===========================================================================
// 「新对话」按钮（含小箭头 ▾）
//
// 设计见 docs/new-chat-feature-design.md：
//  - 点击按钮主体 → 读默认目录。有值静默开新会话；无值（首次）弹说明浮层
//    引导选择目录，选完写入并开新会话。
//  - 点击右侧 ▾ → 弹菜单：只读显示当前默认目录 + 「更改默认目录…」入口。
//  - 打开系统文件夹选择器前后必须 setDialogOpen(true/false)，否则选择器
//    抢焦点会让 Cove 弹窗被失焦收回（Bug C 机制，见 HANDOFF.md）。
// ===========================================================================

/** section-label 右侧的「新对话▾」按钮（主体 + 小箭头分区，点击分开）。 */
function renderNewChatButton(): string {
    return `
        <span class="new-chat-wrap" id="new-chat-wrap">
            <button class="btn btn-ghost section-action new-chat-main" id="new-chat-btn" title="在此目录开一个新 Claude Code 会话">
                ${icon("plus", 14)} 新对话
            </button>
            <button class="btn btn-ghost section-action new-chat-caret" id="new-chat-caret" title="默认工作目录">
                ▾
            </button>
        </span>`;
}

/** 绑定新对话按钮的两种点击（主体静默开 / 箭头弹菜单）。 */
function bindNewChatButton(scope: HTMLElement, tool: ToolName): void {
    const mainBtn = scope.querySelector<HTMLElement>("#new-chat-btn");
    const caretBtn = scope.querySelector<HTMLElement>("#new-chat-caret");
    if (mainBtn) mainBtn.addEventListener("click", () => void onNewChatClick(tool));
    if (caretBtn) caretBtn.addEventListener("click", (e) => {
        e.stopPropagation();
        showWorkspaceMenu(caretBtn);
    });
}

/**
 * 点「新对话」主体。三态：
 *  1. 有默认目录 → 静默开新会话（api.openClaudeSession(path)）。
 *  2. 无默认目录（首次）→ 弹说明浮层，确认后弹文件夹选择器。
 */
async function onNewChatClick(tool: ToolName): Promise<void> {
    let workspace = await api.getDefaultWorkspace();
    if (workspace && workspace.trim()) {
        // 已配置：静默开新会话。
        try {
            await api.openSession(tool, workspace);
        } catch (err) {
            toast("启动失败：" + String(err));
        }
        return;
    }
    // 首次引导：先说明，再选目录。
    const ok = await confirmDialog({
        title: "设置默认工作目录",
        body: "新对话默认从指定目录开始。<br>请选择一个工作目录，之后可随时更改。",
        confirmText: "选择目录…",
        variant: "accent",
        titleIcon: "terminal",
    });
    if (!ok) return;
    const picked = await pickWorkspaceFolder();
    if (!picked) return;
    try {
        await api.setDefaultWorkspace(picked);
        await api.openSession(tool, picked);
        toast("默认目录已设置，正在打开新会话");
    } catch (err) {
        toast("设置失败：" + String(err));
    }
}

/**
 * 打开系统文件夹选择器，返回选中的目录（取消返回 null）。
 * 包一层 setDialogOpen(true/false)，防止选择器抢焦点触发 Cove 失焦收回。
 */
async function pickWorkspaceFolder(): Promise<string | null> {
    await api.setDialogOpen(true);
    try {
        const picked = await open({
            directory: true,
            multiple: false,
            title: "选择默认工作目录",
        });
        return typeof picked === "string" ? picked : null;
    } finally {
        await api.setDialogOpen(false);
    }
}

/**
 * 点小箭头 ▾ 弹出的菜单（复用 model-switcher 的 flyout 范式）：
 *  - 只读显示当前默认目录（灰字，让用户知道现状）
 *  - 「更改默认目录…」→ 直接弹文件夹选择器（跳过说明浮层），选完写入 + toast。
 */
async function showWorkspaceMenu(anchor: HTMLElement): Promise<void> {
    // 清掉已有的菜单。
    document.querySelectorAll(".workspace-menu").forEach((e) => e.remove());

    const current = await api.getDefaultWorkspace();

    const menu = document.createElement("div");
    menu.className = "model-switcher workspace-menu";
    menu.innerHTML = `
        <div class="ws-current">
            <span class="ws-current-label">当前默认目录</span>
            <span class="ws-current-path mono">${escapeHtml(current || "未设置")}</span>
        </div>
        <button class="model-switcher-item ws-change" type="button">
            <span class="ms-tier">${icon("folder", 14)} 更改默认目录…</span>
        </button>`;
    document.body.appendChild(menu);

    // 锚到小箭头下方，右对齐。
    const r = anchor.getBoundingClientRect();
    menu.style.right = (window.innerWidth - r.right) + "px";
    menu.style.top = (r.bottom + 4) + "px";

    // 「更改默认目录」→ 文件夹选择器 → 写入 + toast。
    menu.querySelector<HTMLElement>(".ws-change")?.addEventListener("click", async (e) => {
        e.stopPropagation();
        menu.remove();
        const picked = await pickWorkspaceFolder();
        if (!picked) return;
        try {
            await api.setDefaultWorkspace(picked);
            toast("默认工作目录已更新");
        } catch (err) {
            toast("更新失败：" + String(err));
        }
    });

    // 点外部关闭。
    setTimeout(() => {
        const closer = (ev: MouseEvent) => {
            if (!menu.contains(ev.target as Node)) {
                menu.remove();
                document.removeEventListener("mousedown", closer);
            }
        };
        document.addEventListener("mousedown", closer);
    }, 0);
}
