import { api, Project } from "../api";
import { icon } from "../styles/icons";
import { toast, selectDialog, promptDialog } from "./confirm";
import { escapeHtml, formatSize, formatTime, showConvoInfo, bindCopyable, animateRemoveCard } from "./projects";

/** Project detail: historical sessions for one project (precise, encode-based). */
export async function renderConversationsView(
    container: HTMLElement,
    project: Project,
    onBack: () => void,
    onSelectSession: (sid: string, encoded: string, projPath: string, title: string) => void
): Promise<void> {
    const convos = await api.getProjectDetail(project.path);

    container.innerHTML = `
        <div class="scroll-area">
            <div class="nav-bar is-project">
                <button class="back-btn" id="back-btn">${icon("back", 18)}</button>
                <span class="section-label">${escapeHtml(project.name)}</span>
                <button class="btn btn-ghost section-action" id="new-session-btn" title="在此项目开新会话">
                    ${icon("plus", 14)} 新开会话
                </button>
            </div>
            <div class="path-bar">
                <span class="path-chip copyable" data-copy-text="${escapeHtml(project.path)}" title="点击复制路径">${icon("folder", 13)}<span class="path-chip-text">${escapeHtml(project.path)}</span></span>
                <button class="btn btn-ghost path-open-btn" id="open-folder-btn" title="在文件资源管理器中打开">${icon("folder", 13)} 打开文件夹</button>
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

    // 路径 chip：点击复制完整路径（复用 projects.ts 的 bindCopyable）。
    bindCopyable(container);
    // 打开文件夹：在系统资源管理器中打开项目目录。
    document.getElementById("open-folder-btn")?.addEventListener("click", async () => {
        try {
            await api.openInExplorer(project.path);
        } catch (err) {
            toast("打开失败：" + String(err));
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
    container.querySelectorAll<HTMLElement>(".sub-info-btn").forEach((btn) => {
        btn.addEventListener("click", (e) => e.stopPropagation());
        const card = btn.closest(".sub-session") as HTMLElement;
        const sid = card?.dataset.sid!;
        const convo = convos.find((c) => c.id === sid);
        if (convo) showConvoInfo(btn, convo);
    });

    // New session in this project
    document.getElementById("new-session-btn")!.addEventListener("click", async () => {
        try {
            await api.openClaudeSession(project.path);
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
                    await api.openClaudeSession(project.path, sid);
                } catch (err) {
                    toast("启动失败：" + String(err));
                }
                return;
            }

            if (action === "rename") {
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
                    const newName = await api.renameSession(sid, encoded, name);
                    // Update the card title in-place; a refresh would also work
                    // but this keeps scroll position and feels instant.
                    if (titleEl) titleEl.textContent = newName;
                    toast("已重命名为：" + newName);
                } catch (err) {
                    toast("重命名失败：" + String(err));
                }
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
                    api.archiveConvo(sid, encoded)
                );
                if (success) toast("已归档会话");
            }
        });
    });
}
