import { api, Project, Conversation, ModelState } from "../api";
import { icon } from "../styles/icons";
import { toast, promptDialog, confirmDialog } from "./confirm";
import { open } from "@tauri-apps/plugin-dialog";

export function formatSize(bytes: number): string {
    if (bytes < 1024) return bytes + " B";
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
    return (bytes / 1024 / 1024).toFixed(1) + " MB";
}

export function formatTime(unixMs: number): string {
    if (!unixMs) return "—";
    const diff = Date.now() - unixMs;
    const min = 60 * 1000,
        hour = 60 * min,
        day = 24 * hour;
    if (diff < min) return "刚刚";
    if (diff < hour) return Math.floor(diff / min) + " 分钟前";
    if (diff < day) return Math.floor(diff / hour) + " 小时前";
    return Math.floor(diff / day) + " 天前";
}

export function escapeHtml(s: string): string {
    return s
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
}

/**
 * 「全删会话」的确认弹窗选项工厂（loose/projects 内联复用）。
 *
 * 设计：项目详情页（conversations.ts）删会话走 selectDialog 让用户勾选删哪些
 * 关联项；散落对话页和项目内联展开删会话走全删（8 处关联数据 + history 行）。
 * 两套行为是已知差异——这里通过 confirm 文案**列出全部 8 类关联数据**让用户
 * 对"全删"的范围有明确预期，避免误以为只删了对话正文。（评审 P1 #5a）
 *
 * 项目详情页的 selectDialog 不用这套（它本来就能精细选择）。
 */
export function fullDeleteConfirmOptions(sid: string): {
    title: string;
    body: string;
    confirmText: string;
    variant: "danger";
    titleIcon: "trash";
} {
    return {
        title: "删除会话",
        body: `将永久删除会话 <span class="mono">${escapeHtml(sid.slice(0, 13))}…</span> 及其全部关联数据：<br>` +
              `<span class="mono" style="font-size:11px;color:var(--text-dim)">` +
              `对话正文 · 项目子目录 · tasks · file-history · telemetry · session-env · sessions · history 记录` +
              `</span><br><b>不可恢复</b>。`,
        confirmText: "删除",
        variant: "danger",
        titleIcon: "trash",
    };
}

/**
 * 滑出动画后移除卡片的统一辅助：先加 removing 类播 150ms 动画，再 await 业务
 * 调用，成功才 remove 卡片；失败则回滚 removing 类并 toast 报错。
 *
 * 修复评审 P1 #5b：旧代码先 `card.remove()` 再 await（甚至不 await），后端失败
 * 时前端已删卡，用户误以为成功。这里强制成功才删、失败回滚。
 *
 * `op` 必须是真正的后端调用（throw 表示失败）。
 */
export async function animateRemoveCard(
    card: HTMLElement | null,
    op: () => Promise<unknown>
): Promise<boolean> {
    if (!card) {
        // 没有 card DOM，直接跑 op 让它的错误能冒泡到调用方。
        try {
            await op();
            return true;
        } catch (err) {
            toast("操作失败：" + String(err));
            return false;
        }
    }
    card.classList.add("removing");
    await new Promise((r) => setTimeout(r, 150));
    try {
        await op();
        card.remove();
        return true;
    } catch (err) {
        card.classList.remove("removing");
        toast("操作失败：" + String(err));
        return false;
    }
}

/**
 * Projects main view.
 * - Card click     => launch a NEW Claude Code session in that dir.
 * - Card list-icon => open the project detail (historical sessions).
 * - Add button     => prompt for a folder path.
 *
 * onSelectSession 多带一个 restorePath：用于"返回时恢复项目卡片展开态"。
 * 从内联展开的会话点进详情再返回时，要回到展开的那条会话，而非项目列表顶部。
 */
export async function renderProjectsView(
    container: HTMLElement,
    onSelectProject: (p: Project) => void,
    onSelectSession: (sid: string, encoded: string, projPath: string, title: string, restorePath?: string) => void
): Promise<void> {
    const projects = await api.getProjects();

    container.innerHTML = `
        <div class="scroll-area">
            <div class="section-label">
                ${icon("folder", 13)} 项目 · ${projects.length}
                <button class="btn btn-ghost section-action" id="add-project-btn" title="添加项目文件夹">
                    ${icon("plus", 14)} 添加
                </button>
            </div>
            ${
                projects.length === 0
                    ? `<div class="empty-state">
                          <div class="empty-icon">${icon("folder", 26)}</div>
                          <div class="empty-title">还没有添加任何项目</div>
                          <div class="hint">点击右上角「添加」，选择项目文件夹</div>
                       </div>`
                    : projects
                          .map(
                              (p) => {
                const hasSessions = p.conversation_count > 0;
                return `
                <div class="card is-expandable" data-path="${escapeHtml(p.path)}" title="${escapeHtml(p.path)}">
                    <button class="card-action-icon project-info" data-path="${escapeHtml(p.path)}" title="项目详情">
                        ${icon("info", 14)}
                    </button>
                    <div class="title-row">
                        <div class="title">${escapeHtml(p.name)}</div>
                    </div>
                    <div class="meta">
                        <span>${p.conversation_count} 会话</span>
                        <span class="sep">·</span>
                        <span>${formatSize(p.total_size_bytes)}</span>
                        ${p.last_updated ? `<span class="sep">·</span><span>${formatTime(p.last_updated)}</span>` : ""}
                    </div>
                    <div class="card-hover-actions">
                        <button class="action-chip hover-action" data-action="new-session" data-path="${escapeHtml(p.path)}">${icon("plus", 13)}<span class="action-chip-label">新开会话</span></button>
                        ${hasSessions ? `<button class="action-chip hover-action toggle-expand" data-action="toggle-expand" data-path="${escapeHtml(p.path)}">${icon("chevron", 13)}<span class="action-chip-label">展开项目</span></button>` : ""}
                    </div>
                    <div class="expand-body" data-path="${escapeHtml(p.path)}"></div>
                </div>`;
                              }
                          )
                          .join("")
            }
        </div>`;

    // Add project via system folder picker.
    // IMPORTANT: the native folder picker steals focus from the Cove window.
    // We tell the backend "dialog is open" so the focus-loss doesn't collapse
    // the popup, then turn it off once the picker returns (success or cancel).
    document.getElementById("add-project-btn")!.addEventListener("click", async () => {
        await api.setDialogOpen(true);
        let selected: string | null = null;
        try {
            const picked = await open({
                directory: true,
                multiple: false,
                title: "选择项目文件夹",
            });
            selected = typeof picked === "string" ? picked : null;
        } finally {
            await api.setDialogOpen(false);
        }
        if (!selected) return;
        try {
            await api.addProject(selected);
            await renderProjectsView(container, onSelectProject, onSelectSession);
            toast("已添加项目");
        } catch (err) {
            toast("添加失败：" + String(err));
        }
    });

    // Card click => toggle inline expansion of recent sessions.
    // (Cards without sessions are not .is-expandable and do nothing on click.)
    // Card click => enter the project's standalone page (conversations view).
    // 内联展开不再是点卡片的行为——卡片点击直接进独立页，内联展开交给
    // hover 出来的「展开项目」按钮（见下方 .toggle-expand 处理）。
    container.querySelectorAll<HTMLElement>(".card.is-expandable").forEach((el) => {
        el.addEventListener("click", () => {
            const path = el.dataset.path!;
            const proj = projects.find((p) => p.path === path);
            if (proj) onSelectProject(proj);
        });
    });

    // Hover-action buttons (新开会话 / 展开项目).
    // 「展开项目」点它内联展开最近 5 条会话（不跳转）；再点收起。
    container.querySelectorAll<HTMLElement>(".hover-action").forEach((btn) => {
        btn.addEventListener("click", async (e) => {
            e.stopPropagation();
            const path = btn.dataset.path!;
            const action = btn.dataset.action!;
            if (action === "new-session") {
                try {
                    await api.openClaudeSession(path);
                } catch (err) {
                    toast("启动失败：" + String(err));
                }
                return;
            }
            if (action === "toggle-expand") {
                const card = btn.closest(".card") as HTMLElement;
                const body = card?.querySelector<HTMLElement>(".expand-body");
                const proj = projects.find((p) => p.path === path)!;
                if (!card || !body) return;
                if (card.classList.contains("is-expanded")) {
                    // 收起：清空 DOM，按钮文案切回「展开项目」。
                    card.classList.remove("is-expanded");
                    body.innerHTML = "";
                    setExpandLabel(btn, false);
                    return;
                }
                // 展开：拉取会话，渲染最近 5 条，按钮文案切「收起」。
                try {
                    const all = await api.getProjectDetail(path);
                    const recent = all.slice(0, 5);
                    renderInlineSessions(
                        body,
                        recent,
                        proj,
                        (sid, title) => onSelectSession(sid, proj.encoded_name, proj.path, title, proj.path)
                    );
                    card.classList.add("is-expanded");
                    setExpandLabel(btn, true);
                } catch (err) {
                    toast("加载会话失败：" + String(err));
                }
            }
        });
    });

    // Info icon => hover flyout with rename (click the name) + remove.
    // 直接 attach hover 绑定（不再包 click）：attachHoverFlyout 内部处理
    // mouseenter 显示、mouseleave 延迟关闭，浮层内的改名/移除按钮可点。
    // 关键：info 按钮的 click 必须 stopPropagation，否则点 info 会冒泡到
    // .card.is-expandable 的 click 监听，触发卡片展开/收起（用户报"点着没用"的根因之一）。
    container.querySelectorAll<HTMLElement>(".project-info").forEach((btn) => {
        btn.addEventListener("click", (e) => e.stopPropagation());
        const path = btn.dataset.path!;
        const proj = projects.find((p) => p.path === path)!;
        const card = btn.closest(".card") as HTMLElement;
        const titleEl = card?.querySelector<HTMLElement>(".title");
        showProjectInfo(
            btn,
            proj,
            // Rename: click the project name in the flyout to trigger this.
            async () => {
                const currentName = titleEl?.textContent?.trim() ?? proj.name;
                const name = await promptDialog({
                    title: "重命名项目",
                    body: "这是 Cove 里的显示别名，不影响磁盘目录或 Claude Code 的会话。",
                    placeholder: "输入项目别名",
                    initialValue: currentName,
                    confirmText: "重命名",
                });
                if (!name || !name.trim()) return;
                try {
                    const updated = await api.renameProject(path, name);
                    proj.name = updated.name;
                    if (titleEl) titleEl.textContent = updated.name;
                    toast("已重命名为：" + updated.name);
                } catch (err) {
                    toast("重命名失败：" + String(err));
                }
            },
            // Remove: confirm, then drop from the list (disk untouched).
            async () => {
                const ok = await confirmDialog({
                    title: "移除项目",
                    body: `只从 Cove 列表移除 <span class="mono">${escapeHtml(proj.name)}</span>，<b>不删除</b>磁盘上的任何数据或会话。`,
                    confirmText: "移除",
                    variant: "danger",
                    titleIcon: "trash",
                });
                if (!ok) return;
                try {
                    await api.removeProject(path);
                    await renderProjectsView(container, onSelectProject, onSelectSession);
                    toast("已移除项目");
                } catch (err) {
                    toast("移除失败：" + String(err));
                }
            }
        );
    });
}

/** 切换「展开项目」按钮的文案：展开时显示「收起」+ 上箭头，收起时显示「展开项目」+ 下箭头。
 *  按钮内部结构是 图标 + .action-chip-label，文案在 label 里，图标是第一个 svg。
 *  展开/收起时同步翻转 chevron 方向（用 CSS rotate 更省事，但直接换图标更直白）。 */
function setExpandLabel(btn: HTMLElement, expanded: boolean): void {
    const label = btn.querySelector<HTMLElement>(".action-chip-label");
    if (label) label.textContent = expanded ? "收起" : "展开项目";
    btn.title = expanded ? "收起内联会话" : "展开最近会话";
}

/** Render up to N session rows inside an expanded project card body. */
function renderInlineSessions(
    body: HTMLElement,
    convos: Conversation[],
    proj: Project,
    onSelectSession: (sid: string, title: string) => void
): void {
    if (convos.length === 0) {
        body.innerHTML = `<div class="sub-empty">暂无会话</div>`;
        return;
    }
    body.innerHTML = convos
        .map(
            (c) => `
            <div class="sub-session" data-sid="${escapeHtml(c.id)}">
                <button class="sub-info-btn" title="会话详情">${icon("info", 13)}</button>
                <div class="sub-main">
                    <div class="sub-title" title="${escapeHtml(c.title)}">${escapeHtml(c.title)}</div>
                    <div class="meta">
                        <span>${c.message_count} 条</span>
                        <span class="sep">·</span>
                        <span>${formatTime(c.last_updated)}</span>
                    </div>
                </div>
                <div class="sub-actions">
                    <button class="action-chip inline-action" data-action="resume" data-sid="${escapeHtml(c.id)}" title="继续会话">${icon("play", 13)}<span class="action-chip-label">继续</span></button>
                    <button class="action-chip inline-action" data-action="rename" data-sid="${escapeHtml(c.id)}" title="重命名">${icon("edit", 13)}<span class="action-chip-label">重命名</span></button>
                    <button class="action-chip inline-action" data-action="archive" data-sid="${escapeHtml(c.id)}" title="归档">${icon("archive", 13)}<span class="action-chip-label">归档</span></button>
                    <button class="action-chip inline-action is-danger" data-action="delete" data-sid="${escapeHtml(c.id)}" title="删除">${icon("trash", 13)}<span class="action-chip-label">删除</span></button>
                </div>
            </div>`
        )
        .join("");

    // Click a session row => open the read-only transcript viewer.
    body.querySelectorAll<HTMLElement>(".sub-main").forEach((row) => {
        row.addEventListener("click", (e) => {
            e.stopPropagation();
            const card = row.closest(".sub-session") as HTMLElement;
            const sid = card?.dataset.sid!;
            const titleEl = card?.querySelector<HTMLElement>(".sub-title");
            const title = titleEl?.textContent?.trim() ?? sid;
            // 透传给顶层的 onSelectSession（restorePath 由 renderProjectsView
            // 调 renderInlineSessions 时在闭包里补上，这里只传 sid+title）。
            onSelectSession(sid, title);
        });
    });

    // Info button => hover flyout with session metadata.
    body.querySelectorAll<HTMLElement>(".sub-info-btn").forEach((btn) => {
        btn.addEventListener("click", (e) => e.stopPropagation());
        const card = btn.closest(".sub-session") as HTMLElement;
        const sid = card?.dataset.sid!;
        const convo = convos.find((c) => c.id === sid);
        if (convo) showConvoInfo(btn, convo);
    });

    body.querySelectorAll<HTMLElement>(".inline-action").forEach((btn) => {
        btn.addEventListener("click", async (e) => {
            e.stopPropagation();
            const sid = btn.dataset.sid!;
            const action = btn.dataset.action!;
            const encoded = proj.encoded_name;
            const card = btn.closest(".sub-session") as HTMLElement;
            const titleEl = card?.querySelector<HTMLElement>(".sub-title");

            if (action === "resume") {
                try {
                    await api.openClaudeSession(proj.path, sid);
                } catch (err) {
                    toast("启动失败：" + String(err));
                }
                return;
            }

            if (action === "rename") {
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
                    if (titleEl) titleEl.textContent = newName;
                    toast("已重命名为：" + newName);
                } catch (err) {
                    toast("重命名失败：" + String(err));
                }
                return;
            }

            // delete / archive: 统一走 animateRemoveCard（成功才删卡，失败回滚）。
            if (action === "delete") {
                const ok = await confirmDialog(fullDeleteConfirmOptions(sid));
                if (!ok) return;
            }
            const op = action === "delete"
                ? () => api.deleteConvo(sid, encoded)
                : () => api.archiveConvo(sid, encoded);
            const success = await animateRemoveCard(card, op);
            if (success) toast(action === "delete" ? "已删除会话" : "已归档会话");
        });
    });
}

/** 给带 .copyable 的元素绑定点击复制：路径/目录/ID 等有价值的长文本。
 *  navigator.clipboard 在 WebView2 里可用（tauri.conf 已 withGlobalTauri），
 *  失败时回退到 textarea 兜底。复制成功 toast 提示。
 *  导出供 conversations.ts 的路径区复用（同一套复制交互）。 */
export function bindCopyable(scope: HTMLElement): void {
    scope.querySelectorAll<HTMLElement>(".copyable").forEach((el) => {
        el.title = "点击复制";
        el.addEventListener("click", async (e) => {
            e.stopPropagation();
            const text = (el.dataset.copyText || el.textContent) ?? "";
            try {
                await navigator.clipboard.writeText(text);
                toast("已复制");
            } catch {
                // 兜底：临时 textarea + execCommand
                const ta = document.createElement("textarea");
                ta.value = text;
                ta.style.position = "fixed";
                ta.style.opacity = "0";
                document.body.appendChild(ta);
                ta.select();
                try { document.execCommand("copy"); toast("已复制"); }
                catch { toast("复制失败"); }
                ta.remove();
            }
        });
    });
}

/** Hover 触发的浮层绑定（项目卡 + 会话行 info 统一用这套）。
 *  - 鼠标进入 anchor → 显示浮层
 *  - 鼠标在 anchor / 浮层之间移动 → 浮层保持（短延迟关闭 + 进入任一则取消）
 *  - 鼠标同时离开两者 → 60ms 后关闭（响应快，又不至于划过间隙就消失）
 *
 *  关键：按钮和浮层之间有 4px gap，鼠标穿过 gap 时会短暂同时离开两者。
 *  60ms 延迟正是为了容差这段穿越——比 200ms/120ms 都跟手。
 *  历程：200ms 太慢→缩到 120ms→仍反馈偏慢→再缩到 60ms。 */
export function attachHoverFlyout(
    anchor: HTMLElement,
    buildFlyout: () => HTMLElement
): void {
    let flyout: HTMLElement | null = null;
    let closeTimer: number | null = null;

    const cancelClose = (): void => {
        if (closeTimer !== null) {
            clearTimeout(closeTimer);
            closeTimer = null;
        }
    };
    const scheduleClose = (): void => {
        cancelClose();
        closeTimer = window.setTimeout(() => {
            flyout?.remove();
            flyout = null;
        }, 60);
    };
    const open = (): void => {
        cancelClose();
        if (flyout) return;
        document.querySelectorAll(".project-info-flyout").forEach((e) => e.remove());
        flyout = buildFlyout();
        document.body.appendChild(flyout);
        const r = anchor.getBoundingClientRect();
        flyout.style.right = window.innerWidth - r.right + "px";
        flyout.style.top = r.bottom + 4 + "px";
        flyout.addEventListener("mouseenter", cancelClose);
        flyout.addEventListener("mouseleave", scheduleClose);
    };

    anchor.addEventListener("mouseenter", open);
    anchor.addEventListener("mouseleave", scheduleClose);
}

/** Anchored flyout showing project details. The project name is clickable to
 *  rename it; a remove button sits at the bottom.
 *  改为 hover 触发（attachHoverFlyout），浮层 hover 期间不消失，
 *  所以内部的改名标题、移除按钮照常可点。 */
function showProjectInfo(
    anchor: HTMLElement,
    proj: Project,
    onRename: () => void,
    onRemove: () => void
): void {
    attachHoverFlyout(anchor, () => {
        const addedStr = proj.added_at ? formatTime(proj.added_at) : "—";
        const flyout = document.createElement("div");
        flyout.className = "project-info-flyout";
        flyout.innerHTML = `
            <div class="pif-name" title="点击重命名">${escapeHtml(proj.name)} <span class="pif-name-edit">${icon("edit", 12)}</span></div>
            <div class="pif-row"><span class="pif-label">路径</span><span class="pif-value mono copyable">${escapeHtml(proj.path)}</span></div>
            <div class="pif-row"><span class="pif-label">会话</span><span class="pif-value">${proj.conversation_count} 条</span></div>
            <div class="pif-row"><span class="pif-label">大小</span><span class="pif-value">${formatSize(proj.total_size_bytes)}</span></div>
            <div class="pif-row"><span class="pif-label">添加于</span><span class="pif-value">${addedStr}</span></div>
            <button class="btn btn-danger pif-remove" style="margin-top:var(--sp-2);width:100%;height:28px;font-size:12px;">${icon("trash", 12)} 从列表移除</button>`;
        bindCopyable(flyout);
        // Click the project name => close flyout then run the rename flow.
        flyout.querySelector<HTMLElement>(".pif-name")?.addEventListener("click", (e) => {
            e.stopPropagation();
            flyout.remove();
            onRename();
        });
        // Remove button: close flyout then run the remove flow.
        flyout.querySelector<HTMLElement>(".pif-remove")?.addEventListener("click", (e) => {
            e.stopPropagation();
            flyout.remove();
            onRemove();
        });
        return flyout;
    });
}

/** 会话信息浮层（只读）：复用项目信息浮层的视觉，但标题不可点、无操作按钮。
 *  显示标题/模型/消息数/大小/最后更新/cwd/完整会话 ID。
 *  hover 触发（attachHoverFlyout），散落对话、项目详情、内联展开三处共用。 */
export function showConvoInfo(anchor: HTMLElement, convo: Conversation): void {
    attachHoverFlyout(anchor, () => {
        const flyout = document.createElement("div");
        flyout.className = "project-info-flyout is-convo";
        flyout.innerHTML = `
            <div class="pif-name">${escapeHtml(convo.title)}</div>
            <div class="pif-row"><span class="pif-label">模型</span><span class="pif-value mono">${escapeHtml(convo.model || "—")}</span></div>
            <div class="pif-row"><span class="pif-label">消息</span><span class="pif-value">${convo.message_count} 条</span></div>
            <div class="pif-row"><span class="pif-label">大小</span><span class="pif-value">${formatSize(convo.size_bytes)}</span></div>
            <div class="pif-row"><span class="pif-label">更新</span><span class="pif-value">${formatTime(convo.last_updated)}</span></div>
            ${convo.cwd ? `<div class="pif-row"><span class="pif-label">目录</span><span class="pif-value mono copyable">${escapeHtml(convo.cwd)}</span></div>` : ""}
            <div class="pif-row"><span class="pif-label">ID</span><span class="pif-value mono copyable">${escapeHtml(convo.id)}</span></div>`;
        bindCopyable(flyout);
        return flyout;
    });
}

const TIER_LABEL: Record<string, string> = {
    opus: "Opus",
    sonnet: "Sonnet",
    haiku: "Haiku",
};

/** Map a tier key to its configured model name from ModelInfo. */
function tierModel(info: ModelState["info"], tier: string): string {
    if (tier === "opus") return info.opus_model;
    if (tier === "haiku") return info.haiku_model;
    return info.sonnet_model;
}

let modelStateCache: ModelState | null = null;

export async function renderHeader(modelEl: HTMLElement): Promise<void> {
    modelEl.classList.add("clickable");
    modelEl.title = "点击切换默认模型";
    modelEl.onclick = () => showModelSwitcher(modelEl);
    await refreshModelLabel(modelEl);
}

async function refreshModelLabel(modelEl: HTMLElement): Promise<void> {
    let label = "未配置";
    try {
        const state = await api.getModelState();
        modelStateCache = state;
        label = tierModel(state.info, state.tier) || "未配置";
    } catch {
        /* keep default */
    }
    modelEl.innerHTML = `<span class="model-dot-inner"></span>${escapeHtml(label)}`;
}

/** Pop up a model-tier switcher anchored under the model label. */
async function showModelSwitcher(modelEl: HTMLElement): Promise<void> {
    // Remove any existing menu.
    document.querySelectorAll(".model-switcher").forEach((e) => e.remove());
    if (!modelStateCache) return;
    const state = modelStateCache;

    const current = state.tier;
    const menu = document.createElement("div");
    menu.className = "model-switcher";
    for (const tier of ["opus", "sonnet", "haiku"]) {
        const name = tierModel(state.info, tier);
        const isCur = tier === current;
        const item = document.createElement("button");
        item.className = "model-switcher-item" + (isCur ? " is-current" : "");
        item.innerHTML =
            `<span class="ms-tier">${TIER_LABEL[tier] || tier}</span>` +
            `<span class="ms-name">${escapeHtml(name || "未配置")}</span>` +
            (isCur ? `<span class="ms-check">${icon("check")}</span>` : "");
        item.onclick = async (ev) => {
            ev.stopPropagation();
            if (isCur) { menu.remove(); return; }
            try {
                await api.setDefaultTier(tier);
                modelStateCache = { tier, info: state.info };
                await refreshModelLabel(modelEl);
                toast(`默认模型已切换为 ${TIER_LABEL[tier]}`);
            } catch (e) {
                toast("切换失败: " + (e as Error).message);
            }
            menu.remove();
        };
        menu.appendChild(item);
    }
    // Anchor: below the label, right-aligned to its right edge.
    document.body.appendChild(menu);
    const r = modelEl.getBoundingClientRect();
    menu.style.right = (window.innerWidth - r.right) + "px";
    menu.style.top = (r.bottom + 4) + "px";
    // Click outside closes.
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
