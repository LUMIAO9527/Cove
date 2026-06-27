import { api, Project, Conversation, ToolName } from "../api";
import { icon } from "../styles/icons";
import { toast, selectDialog } from "./confirm";
import { escapeHtml, formatSize, formatTime, showConvoInfo, animateRemoveCard, bindHoverMenu, renameSessionPrompt, createAnchoredMenu } from "./projects";

/** Project detail: historical sessions for one project (precise, encode-based). */
export async function renderConversationsView(
    container: HTMLElement,
    tool: ToolName,
    project: Project,
    onBack: () => void,
    onSelectSession: (sid: string, encoded: string, projPath: string, title: string) => void
): Promise<void> {
    const convos = await api.getProjectDetail(tool, project.path);

    container.innerHTML = `
        <div class="scroll-area">
            <div class="nav-bar is-project" style="display:block">
                <div class="nav-row">
                    <button class="back-btn" id="back-btn">${icon("back", 13)}</button>
                    <span class="section-label" style="flex:1;min-width:0">${escapeHtml(project.name)}</span>
                    <span class="new-chat-wrap" style="flex-shrink:0">
                        <button class="btn btn-ghost section-action new-chat-main" id="new-session-btn" title="在此项目开新会话">
                            ${icon("plus", 14)} 新开会话
                        </button>
                        <button class="btn btn-ghost section-action new-chat-caret" id="conv-caret" title="更多操作">▾</button>
                    </span>
                </div>
                <div class="nav-meta" id="proj-path" title="点击复制路径">${escapeHtml(project.path)}</div>
            </div>
            ${
                convos.length === 0
                    ? `<div class="empty-state">
                          <div class="empty-icon">${icon("inbox", 26)}</div>
                          <div class="empty-title">暂无历史会话</div>
                          <div class="hint">点击上方「新开会话」开始</div>
                       </div>`
                    : convos
                          .map(
                              (c) => `
                <div class="card sub-session" data-sid="${escapeHtml(c.id)}">
                    <button class="sub-info-btn" title="会话详情">${icon("info", 13)}</button>
                    <div class="sub-main">
                        <div class="sub-title convo-title" data-sid="${escapeHtml(c.id)}" title="${escapeHtml(c.title)}">${escapeHtml(c.title)}</div>
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
                        <button class="action-chip convo-action" data-action="resume" data-sid="${escapeHtml(c.id)}" title="继续会话">${icon("play", 13)}<span class="action-chip-label">继续</span></button>
                        <button class="action-chip convo-action" data-action="rename" data-sid="${escapeHtml(c.id)}" title="重命名">${icon("edit", 13)}<span class="action-chip-label">重命名</span></button>
                        <button class="action-chip convo-action" data-action="archive" data-sid="${escapeHtml(c.id)}" title="归档">${icon("archive", 13)}<span class="action-chip-label">归档</span></button>
                        <button class="action-chip convo-action is-danger" data-action="delete" data-sid="${escapeHtml(c.id)}" title="删除">${icon("trash", 13)}<span class="action-chip-label">删除</span></button>
                    </div>
                </div>`
                          )
                          .join("")
            }
        </div>`;

    document.getElementById("back-btn")!.addEventListener("click", onBack);

    // ▾ hover 菜单：打开文件夹 / 批量清理。
    const convCaret = document.getElementById("conv-caret");
    if (convCaret) bindHoverMenu(convCaret, (anchor) => showConvMenu(anchor, project, convos, container, tool, onBack, onSelectSession));

    // 路径小灰字：点击复制项目路径（像 info 浮层那样）。
    document.getElementById("proj-path")?.addEventListener("click", async () => {
        try {
            await navigator.clipboard.writeText(project.path);
            toast("已复制项目路径");
        } catch {
            toast("复制失败");
        }
    });

    // Click a session title => open the read-only transcript viewer.
    // Click anywhere on the row => open the transcript viewer.
    container.querySelectorAll<HTMLElement>(".sub-main").forEach((main) => {
        main.addEventListener("click", (e) => {
            e.stopPropagation();
            const card = main.closest(".sub-session") as HTMLElement;
            const sid = card?.dataset.sid!;
            const titleEl = card?.querySelector<HTMLElement>(".sub-title");
            const sessionTitle = titleEl?.textContent?.trim() ?? sid;
            onSelectSession(sid, project.encoded_name, project.path, sessionTitle);
        });
    });

    // Info button => hover flyout with session metadata.
    // 传 onRename：复用本卡片已有的 convo-action rename 逻辑，让 info 浮层也能改名。
    container.querySelectorAll<HTMLElement>(".sub-info-btn").forEach((btn) => {
        btn.addEventListener("click", (e) => e.stopPropagation());
        const card = btn.closest(".sub-session") as HTMLElement;
        const sid = card?.dataset.sid!;
        const convo = convos.find((c) => c.id === sid);
        if (convo) {
            const onRename = () => renameSessionPrompt(
                tool, sid, project.encoded_name,
                card?.querySelector<HTMLElement>(".sub-title")
            );
            showConvoInfo(btn, convo, onRename);
        }
    });

    // New session in this project
    document.getElementById("new-session-btn")!.addEventListener("click", async () => {
        try {
            await api.openSession(tool, project.path);
        } catch (err) {
            toast("启动失败：" + String(err));
        }
    });

    container.querySelectorAll<HTMLElement>(".convo-action").forEach((btn) => {
        btn.addEventListener("click", async (e) => {
            e.stopPropagation();
            const sid = btn.dataset.sid!;
            const action = btn.dataset.action!;
            const encoded = project.encoded_name;

            if (action === "resume") {
                try {
                    await api.openSession(tool, project.path, sid);
                } catch (err) {
                    toast("启动失败：" + String(err));
                }
                return;
            }

            if (action === "rename") {
                const card = btn.closest(".card") as HTMLElement;
                await renameSessionPrompt(
                    tool, sid, encoded,
                    card?.querySelector<HTMLElement>(".sub-title")
                );
                return;
            }

            if (action === "delete") {
                // Fetch the conversation's related data items and let the user
                // pick which to delete (default: all).
                let items;
                try {
                    items = await api.listRelatedFiles(sid, encoded);
                } catch {
                    toast("读取关联数据失败，无法删除");
                    return;
                }
                if (items.length === 0) {
                    toast("未找到任何关联数据");
                    return;
                }
                const totalBytes = items.reduce((s, it) => s + it.size_bytes, 0);
                const selectedPaths = await selectDialog({
                    title: "删除会话",
                    body: `会话 <span class="mono">${escapeHtml(sid.slice(0, 13))}…</span> 共 ${items.length} 项关联数据（${formatSize(totalBytes)}），请选择要删除的：`,
                    items: items.map((it) => ({
                        label: it.label,
                        path: it.path,
                        sizeBytes: it.size_bytes,
                        infoOnly: it.kind === "history",
                    })),
                    confirmText: "删除所选",
                    variant: "danger",
                    titleIcon: "trash",
                    formatSize,
                });
                if (!selectedPaths || selectedPaths.length === 0) return;
                const card = btn.closest(".card") as HTMLElement;
                card?.classList.add("removing");
                await new Promise((r) => setTimeout(r, 150));
                // If the user deleted the jsonl itself, the conversation is gone
                // from Claude Code's view — remove the card. Otherwise just
                // refresh (related data removed but conversation still exists).
                const deletedJsonl = items.some(
                    (it) => it.kind === "jsonl" && selectedPaths.includes(it.path)
                );
                try {
                    await api.deleteRelatedFiles(sid, selectedPaths);
                    if (deletedJsonl) {
                        card?.remove();
                    } else {
                        card?.classList.remove("removing");
                        toast(`已删除 ${selectedPaths.length} 项关联数据`);
                    }
                } catch (e) {
                    card?.classList.remove("removing");
                    toast("删除失败: " + (e as Error).message);
                }
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
// 项目详情页「新开会话 ▾」hover 菜单
// 打开文件夹 / 复制项目路径 / 按时间批量清理（复用 loose.ts 的 renderBatchCleanView）
// ===========================================================================

function showConvMenu(
    anchor: HTMLElement,
    project: Project,
    convos: Conversation[],
    container: HTMLElement,
    tool: ToolName,
    onBack: () => void,
    onSelectSession: (sid: string, encoded: string, projPath: string, title: string) => void
): HTMLElement {
    const hasConvos = convos.length > 0;
    return createAnchoredMenu(anchor, "conv-menu", `
        <button class="model-switcher-item" type="button" data-act="open-folder">
            <span class="ms-tier">${icon("folder", 14)} 打开文件夹</span>
        </button>
        <button class="model-switcher-item" type="button" data-act="batch" ${hasConvos ? "" : "disabled"}>
            <span class="ms-tier">${icon("broom", 14)} 按时间批量清理…</span>
        </button>`, {
        "open-folder": async () => {
            try { await api.openInExplorer(project.path); }
            catch (err) { toast("打开失败：" + String(err)); }
        },
        "batch": () => {
            if (!hasConvos) return;
            import("./loose").then(({ renderBatchCleanView }) => {
                renderBatchCleanView(
                    container,
                    tool,
                    convos,
                    onSelectSession,
                    () => renderConversationsView(container, tool, project, onBack, onSelectSession)
                );
            });
        },
    });
}
