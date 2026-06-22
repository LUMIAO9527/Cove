import { api, Conversation } from "../api";
import { icon } from "../styles/icons";
import { confirmDialog, toast } from "./confirm";
import { escapeHtml, formatSize, formatTime, showConvoInfo, animateRemoveCard } from "./projects";

/**
 * 归档区视图（底部 tab 之一，和项目/散落对话/清理平级）。
 *
 * 布局完全复刻散落对话页（loose.ts）的 .sub-session 卡片：
 *  - 标题（真实对话标题，custom>ai>summary>lastPrompt>lastUser>sid，绝不裸 sid）
 *  - meta 行（模型 · 消息数 · 大小 · 归档时间）
 *  - 右侧 .sub-info-btn 信息按钮 → hover 出 showConvoInfo 浮层（和会话页一致）
 *  - hover 卡片时下方撑出操作按钮（恢复 / 永久删除）
 *
 * 数据来源：get_archive_conversations 后端扫描归档目录下所有 jsonl，
 * 用 parse_single_jsonl 解析出完整 Conversation（不是贫瘠的 index.json）。
 *
 * 归档区的操作只有「恢复」和「永久删除」——已归档的会话要恢复回去才能
 * 继续/重命名，不在这两个操作之外做别的。
 */
export async function renderArchiveView(container: HTMLElement): Promise<void> {
    const convos = await api.getArchiveConversations();

    container.innerHTML = `
        <div class="scroll-area">
            <div class="section-label">
                ${icon("inbox", 13)} 归档区 · ${convos.length} 条
                ${
                    convos.length > 0
                        ? `<button class="btn btn-ghost section-action" id="purge-all-btn" title="永久删除所有归档会话">${icon("trash", 14)} 清空归档</button>`
                        : ""
                }
            </div>
            ${
                convos.length === 0
                    ? `<div class="empty-state">
                          <div class="empty-icon">${icon("inbox", 26)}</div>
                          <div class="empty-title">暂无归档对话</div>
                          <div class="hint">归档的会话会保存在这里</div>
                       </div>`
                    : convos.map(renderCard).join("")
            }
        </div>`;

    // 信息按钮 → hover 浮层（标题/模型/消息数/大小/归档时间/cwd/完整 ID）。
    container.querySelectorAll<HTMLElement>(".sub-info-btn").forEach((btn) => {
        btn.addEventListener("click", (e) => e.stopPropagation());
        const card = btn.closest(".sub-session") as HTMLElement;
        const sid = card?.dataset.sid!;
        const convo = convos.find((c) => c.id === sid);
        if (convo) showConvoInfo(btn, convo);
    });

    // 操作按钮：恢复 / 永久删除（hover 卡片时才撑出显示）。
    container.querySelectorAll<HTMLElement>(".arch-action").forEach((btn) => {
        btn.addEventListener("click", async (e) => {
            e.stopPropagation();
            const sid = btn.dataset.sid!;
            const encoded = btn.dataset.encoded!;
            const action = btn.dataset.action!;
            const card = btn.closest(".sub-session") as HTMLElement;

            if (action === "restore") {
                const success = await animateRemoveCard(card, () =>
                    api.restoreConvo(sid, encoded)
                );
                if (success) {
                    toast("已恢复到原项目");
                    // 全部恢复完后重渲染（刷新计数 + 可能清空）。
                    if (container.querySelectorAll(".sub-session").length === 0) {
                        renderArchiveView(container);
                    }
                }
                return;
            }

            if (action === "purge") {
                const ok = await confirmDialog({
                    title: "永久删除",
                    body: `将<strong>永久删除</strong>归档会话<br><span class="mono">${escapeHtml(sid.slice(0, 13))}…</span><br>不可恢复。`,
                    confirmText: "永久删除",
                    variant: "danger",
                    titleIcon: "trash",
                });
                if (!ok) return;
                const success = await animateRemoveCard(card, () =>
                    api.purgeArchivedConvo(sid, encoded)
                );
                if (success && container.querySelectorAll(".sub-session").length === 0) {
                    renderArchiveView(container);
                }
            }
        });
    });

    // 清空归档：确认后循环永久删除全部，重渲染。
    const purgeAllBtn = document.getElementById("purge-all-btn");
    if (purgeAllBtn) {
        purgeAllBtn.addEventListener("click", async () => {
            const ok = await confirmDialog({
                title: "清空归档区",
                body: `将<strong>永久删除</strong>全部 ${convos.length} 条归档会话，<b>不可恢复</b>。<br>确定清空吗？`,
                confirmText: "清空归档",
                variant: "danger",
                titleIcon: "trash",
            });
            if (!ok) return;
            let failed = 0;
            for (const c of convos) {
                const r = await api.purgeArchivedConvo(c.id, c.project_encoded);
                if (!r) failed += 1;
            }
            if (failed > 0) {
                toast(`已清空，${failed} 条删除失败`);
            }
            renderArchiveView(container);
        });
    }
}

/** 归档会话行卡片——布局复刻 loose.ts，操作按钮换成 恢复/永久删除。 */
function renderCard(c: Conversation): string {
    return `
        <div class="card sub-session is-archive" data-sid="${escapeHtml(c.id)}">
            <button class="sub-info-btn" title="会话详情">${icon("info", 13)}</button>
            <div class="sub-main">
                <div class="sub-title" data-encoded="${escapeHtml(c.project_encoded)}" data-cwd="${escapeHtml(c.cwd)}" title="${escapeHtml(c.title)}">${escapeHtml(c.title)}</div>
                <div class="meta">
                    <span class="model-tag">${escapeHtml(c.model)}</span>
                    <span class="sep">·</span>
                    <span>${c.message_count} 条</span>
                    <span class="sep">·</span>
                    <span>${formatSize(c.size_bytes)}</span>
                    <span class="sep">·</span>
                    <span>归档于 ${formatTime(c.last_updated)}</span>
                </div>
                ${
                    c.first_user_preview
                        ? `<div class="preview">— ${escapeHtml(c.first_user_preview.slice(0, 50))}</div>`
                        : ""
                }
            </div>
            <div class="sub-actions">
                <button class="action-chip arch-action" data-action="restore" data-sid="${escapeHtml(c.id)}" data-encoded="${escapeHtml(c.project_encoded)}" title="恢复到原项目">${icon("restore", 13)}<span class="action-chip-label">恢复</span></button>
                <button class="action-chip arch-action is-danger" data-action="purge" data-sid="${escapeHtml(c.id)}" data-encoded="${escapeHtml(c.project_encoded)}" title="永久删除">${icon("trash", 13)}<span class="action-chip-label">永久删除</span></button>
            </div>
        </div>`;
}
