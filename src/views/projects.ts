import { api, Project, Conversation, ModelState, ModelInfo, ToolName } from "../api";
import { icon } from "../styles/icons";
import { toast, promptDialog, confirmDialog } from "./confirm";
import { open } from "@tauri-apps/plugin-dialog";

// 拖拽排序的 document 级监听器引用（重渲染前移除上一组，防内存泄漏）。
let dragMoveHandler: ((e: MouseEvent) => void) | null = null;
let dragUpHandler: ((e: MouseEvent) => void) | null = null;

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
 * 重命名会话统一入口。6 处会话卡片（项目内联展开 / 项目详情 / 散落对话 /
 * 归档 / info 浮层等）原本各自重复一份 promptDialog + try/catch + toast；
 * 抽这里一行调用即可。
 *
 * - titleEl：传入则改名成功后就地更新 DOM 文字（无需重渲染整页）。
 * - 返回 boolean 表示是否真的改了名（被取消或失败也算 false，调用方可据此判断）。
 */
export async function renameSessionPrompt(
    tool: ToolName,
    sid: string,
    encoded: string,
    titleEl?: HTMLElement | null
): Promise<boolean> {
    const currentTitle = titleEl?.textContent?.trim() ?? "";
    const name = await promptDialog({
        title: "重命名会话",
        body: "新名字将写入对话记录，<span class='mono'>claude /resume</span> 列表会显示这个名字。",
        placeholder: "输入新会话名",
        initialValue: currentTitle,
        confirmText: "重命名",
    });
    if (!name || !name.trim()) return false;
    try {
        const newName = await api.renameSession(tool, sid, encoded, name);
        if (titleEl) titleEl.textContent = newName;
        toast("已重命名为：" + newName);
        return true;
    } catch (err) {
        toast("重命名失败：" + String(err));
        return false;
    }
}

/**
 * 锚定 ▾ 菜单工厂。6 处页面 split-button 的 ▾ 菜单原本各自重复：
 *   - new div + className（含同类清理）
 *   - innerHTML 填项
 *   - appendChild + 按 anchor.getBoundingClientRect 定位
 *   - 点外部 mousedown 关闭
 * 抽到这里统一，调用方只写 className 后缀 + 项 HTML + 项的点击行为字典。
 *
 * @param anchor  触发该菜单的元素（▾ 按钮），菜单定位以它的右下角为锚点
 * @param subClass  菜单的修饰类（如 "cleanup-menu"），用于 CSS 微调和"清同类残留"
 * @param html  菜单内的 HTML（应当只含 `.model-switcher-item` 项）
 * @param handlers  按 data-act 值索引的点击回调，回调内不需要 menu.remove，工厂统一处理
 */
export function createAnchoredMenu(
    anchor: HTMLElement,
    subClass: string,
    html: string,
    handlers: Record<string, (e: MouseEvent) => void | Promise<void>>
): HTMLElement {
    // 清掉同类残留（防止 hover 反复触发叠出多份）。
    document.querySelectorAll("." + subClass).forEach((e) => e.remove());

    const menu = document.createElement("div");
    menu.className = "model-switcher " + subClass;
    menu.innerHTML = html;
    document.body.appendChild(menu);

    const r = anchor.getBoundingClientRect();
    menu.style.right = (window.innerWidth - r.right) + "px";
    menu.style.top = (r.bottom + 4) + "px";

    for (const [act, fn] of Object.entries(handlers)) {
        menu.querySelector<HTMLElement>(`[data-act="${act}"]`)?.addEventListener("click", async (e) => {
            e.stopPropagation();
            menu.remove();
            await fn(e);
        });
    }

    // 点外部关闭——和原本各页一致的延迟绑定，避免触发本次 click 立即关闭。
    setTimeout(() => {
        const closer = (ev: MouseEvent) => {
            if (!menu.contains(ev.target as Node)) {
                menu.remove();
                document.removeEventListener("mousedown", closer);
            }
        };
        document.addEventListener("mousedown", closer);
    }, 0);

    return menu;
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
    tool: ToolName,
    onSelectProject: (p: Project) => void,
    onSelectSession: (sid: string, encoded: string, projPath: string, title: string, restorePath?: string) => void
): Promise<void> {
    const projects = await api.getProjects(tool);

    container.innerHTML = `
        <div class="scroll-area">
            <div class="section-label">
                ${icon("folder", 13)} 项目 · ${projects.length}
                <span class="new-chat-wrap" id="add-wrap">
                    <button class="btn btn-ghost section-action new-chat-main" id="add-project-btn" title="添加项目文件夹">
                        ${icon("plus", 14)} 添加
                    </button>
                    <button class="btn btn-ghost section-action new-chat-caret" id="add-caret" title="更多操作">▾</button>
                </span>
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
                <div class="card is-expandable is-reorderable" data-path="${escapeHtml(p.path)}" title="${escapeHtml(p.path)}">
                    <div class="card-drag-handle" data-path="${escapeHtml(p.path)}" title="拖动以调整顺序" aria-label="拖动调整顺序">
                        ${icon("grip", 14)}
                    </div>
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
                    <div class="card-path mono" title="${escapeHtml(p.path)}">${escapeHtml(p.path)}</div>
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
            await api.addProject(tool, selected);
            await renderProjectsView(container, tool, onSelectProject, onSelectSession);
            toast("已添加项目");
        } catch (err) {
            toast("添加失败：" + String(err));
        }
    });

    // 「添加 ▾」小箭头菜单：hover 触发（用户反馈要求 hover 而非 click）。
    const addCaret = document.getElementById("add-caret");
    if (addCaret) bindHoverMenu(addCaret, showProjectsMenu);

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
                    await api.openSession(tool, path);
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
                    const all = await api.getProjectDetail(tool, path);
                    const recent = all.slice(0, 5);
                    renderInlineSessions(
                        body,
                        recent,
                        proj,
                        tool,
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

    // —— 拖拽排序（问题 1，方案 B：hover 手柄）——
    // ⚠️ 不用 HTML5 draggable——实测在 Tauri WebView2 里 dragstart 能触发但后续
    // dragover/drop 不可靠（用户拖不动）。改用 mousedown/mousemove/mouseup 自己
    // 实现拖拽（纯 JS，不依赖浏览器 drag 手势，最可靠）。
    //
    // 流程：
    //  - 手柄 mousedown → 记录起点 + 源卡片，但不立即拖（等 mousemove 超阈值）
    //  - document mousemove → 超过 5px 判定进入拖拽态，实时检测鼠标在哪张卡的
    //    上/下半，把源卡片挪到对应位置（insertBefore）
    //  - document mouseup → 结束拖拽；若全程没超阈值（只是点了一下手柄），什么都不做
    //    （也不进详情页，因为 mousedown 已 stopPropagation）
    {
        const cards = Array.from(container.querySelectorAll<HTMLElement>(".card.is-reorderable"));
        let dragSrc: HTMLElement | null = null;
        let dragging = false;
        let startX = 0, startY = 0;

        cards.forEach((card) => {
            const handle = card.querySelector<HTMLElement>(".card-drag-handle");
            if (!handle) return;
            // click 阻止冒泡（不触发卡片"进独立页"）。mousedown 也 stopPropagation
            // 但不 preventDefault（鼠标交互需要）。
            handle.addEventListener("click", (e) => e.stopPropagation());
            handle.addEventListener("mousedown", (e) => {
                e.stopPropagation();
                if (e.button !== 0) return; // 只响应左键
                dragSrc = card;
                startX = e.clientX;
                startY = e.clientY;
                dragging = false;
            });
        });

        // mousemove 在 document 上监听（鼠标移出手柄也能跟踪）。
        const onMove = (e: MouseEvent) => {
            if (!dragSrc) return;
            // 未进入拖拽态：先判断是否超过阈值。
            if (!dragging) {
                const dx = e.clientX - startX;
                const dy = e.clientY - startY;
                if (dx * dx + dy * dy < 25) return; // 5px 阈值
                dragging = true;
                dragSrc.classList.add("is-dragging");
                document.body.style.cursor = "grabbing";
            }
            // 已拖拽：找鼠标当前在哪张卡片上，按上/下半决定插入位置。
            const targetCard = cards.find((c) => {
                if (c === dragSrc) return false;
                const r = c.getBoundingClientRect();
                return e.clientY >= r.top && e.clientY <= r.bottom;
            });
            // 清掉所有落点标记。
            cards.forEach((c) => c.classList.remove("is-drag-over-top", "is-drag-over-bottom"));
            if (targetCard) {
                const r = targetCard.getBoundingClientRect();
                const after = (e.clientY - r.top) > r.height / 2;
                targetCard.classList.add(after ? "is-drag-over-bottom" : "is-drag-over-top");
            }
            e.preventDefault(); // 防止选中文本
        };

        const onUp = async (e: MouseEvent) => {
            if (!dragSrc) return;
            document.body.style.cursor = "";
            if (dragging) {
                // 找最终落点卡片，把源卡片挪过去。
                const targetCard = cards.find((c) => {
                    if (c === dragSrc) return false;
                    const r = c.getBoundingClientRect();
                    return e.clientY >= r.top && e.clientY <= r.bottom;
                });
                if (targetCard) {
                    const r = targetCard.getBoundingClientRect();
                    const after = (e.clientY - r.top) > r.height / 2;
                    const parent = targetCard.parentNode!;
                    if (after) {
                        if (targetCard.nextSibling) parent.insertBefore(dragSrc, targetCard.nextSibling);
                        else parent.appendChild(dragSrc);
                    } else {
                        parent.insertBefore(dragSrc, targetCard);
                    }
                    // 算新顺序持久化；失败回滚。
                    const ordered = Array.from(container.querySelectorAll<HTMLElement>(".card.is-reorderable"))
                        .map((c) => c.dataset.path!)
                        .filter(Boolean);
                    try {
                        await api.reorderProjects(tool, ordered);
                        toast("已调整顺序");
                    } catch (err) {
                        toast("保存顺序失败：" + String(err));
                        await renderProjectsView(container, tool, onSelectProject, onSelectSession);
                    }
                }
                dragSrc.classList.remove("is-dragging");
                cards.forEach((c) => c.classList.remove("is-drag-over-top", "is-drag-over-bottom"));
            }
            dragging = false;
            dragSrc = null;
        };

        // 重渲染前移除上一组 document 监听（renderProjectsView 每次重渲染都会
        // 新增一对，旧闭包持有已移除的 DOM 子树导致内存泄漏 + mousemove 累积开销）。
        if (dragMoveHandler) document.removeEventListener("mousemove", dragMoveHandler);
        if (dragUpHandler) document.removeEventListener("mouseup", dragUpHandler);
        dragMoveHandler = onMove;
        dragUpHandler = onUp;
        document.addEventListener("mousemove", onMove);
        document.addEventListener("mouseup", onUp);
    }

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
                    const updated = await api.renameProject(tool, path, name);
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
                    await api.removeProject(tool, path);
                    await renderProjectsView(container, tool, onSelectProject, onSelectSession);
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
    tool: ToolName,
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
    // 传 onRename：复用本项目卡已有的"重命名"逻辑（hover 操作里的 rename 动作），
    // 让会话卡 info 浮层也能改名（和项目卡浮层改名一致）。
    body.querySelectorAll<HTMLElement>(".sub-info-btn").forEach((btn) => {
        btn.addEventListener("click", (e) => e.stopPropagation());
        const card = btn.closest(".sub-session") as HTMLElement;
        const sid = card?.dataset.sid!;
        const convo = convos.find((c) => c.id === sid);
        if (convo) {
            // 改名走统一入口 renameSessionPrompt，6 处一致。
            const onRename = () => renameSessionPrompt(
                tool, sid, proj.encoded_name,
                card?.querySelector<HTMLElement>(".sub-title")
            );
            showConvoInfo(btn, convo, onRename);
        }
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
                    await api.openSession(tool, proj.path, sid);
                } catch (err) {
                    toast("启动失败：" + String(err));
                }
                return;
            }

            if (action === "rename") {
                await renameSessionPrompt(tool, sid, encoded, titleEl);
                return;
            }

            // delete / archive: 统一走 animateRemoveCard（成功才删卡，失败回滚）。
            if (action === "delete") {
                const ok = await confirmDialog(fullDeleteConfirmOptions(sid));
                if (!ok) return;
            }
            const op = action === "delete"
                ? () => api.deleteConvo(tool, sid, encoded)
                : () => api.archiveConvo(tool, sid, encoded);
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
    buildFlyout: () => HTMLElement | null
): void {
    // 防重复绑定：modelEl 这类持久元素每次切工具/切 tab 都会重跑 renderHeader，
    // 没有 idempotent 守卫的话每次都叠一组 mouseenter/mouseleave 监听，累积成
    // 内存泄漏 + 多余事件。卡片元素随重渲染重建不触发此问题，但守卫对两者都安全。
    if (anchor.dataset.hoverFlyoutBound === "1") return;
    anchor.dataset.hoverFlyoutBound = "1";

    let flyout: HTMLElement | null = null;
    let closeTimer: number | null = null;

    // 找 anchor 所在的卡片（项目卡 .card 或会话行 .sub-session），用于浮层打开时
    // 维持卡片的 hover 高亮（鼠标移到浮层时卡片 :hover 消失，但视觉上应保持高亮直到浮层关闭）。
    const cardOf = (): HTMLElement | null => anchor.closest(".card, .sub-session");

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
            cardOf()?.classList.remove("is-flyout-open");
        }, 60);
    };
    const open = (): void => {
        cancelClose();
        if (flyout) return;
        // buildFlyout 返回 null 表示本次不弹浮层（如模型胶囊切到非 Claude 工具时，
        // 旧 hover 监听仍在但不应再弹菜单）。返回 null 直接 return，不 append 任何东西。
        const built = buildFlyout();
        if (!built) return;
        document.querySelectorAll(".project-info-flyout").forEach((e) => e.remove());
        flyout = built;
        document.body.appendChild(flyout);
        const r = anchor.getBoundingClientRect();
        flyout.style.right = window.innerWidth - r.right + "px";
        flyout.style.top = r.bottom + 4 + "px";
        // 浮层打开时给卡片加 class 维持 hover 高亮（鼠标移到浮层时卡片 :hover 消失）。
        cardOf()?.classList.add("is-flyout-open");
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

/** 会话信息浮层：复用项目信息浮层的视觉。
 *  - 不传 onRename（默认）：纯只读——标题不可点、无编辑图标（归档卡用此模式）。
 *  - 传 onRename：标题可点改名，和项目卡浮层完全同一套交互（点标题 → 关浮层 → 调 onRename）。
 *    散落对话 / 项目详情 / 项目内联展开三处的会话卡已有"重命名"逻辑，这里只是
 *    复用同一逻辑多提供一个入口（用户反馈：会话卡 info 浮层也希望能改名）。
 *  显示标题/模型/消息数/大小/最后更新/cwd/完整会话 ID。
 *  hover 触发（attachHoverFlyout），三处共用。 */
export function showConvoInfo(anchor: HTMLElement, convo: Conversation, onRename?: () => void): void {
    attachHoverFlyout(anchor, () => {
        const flyout = document.createElement("div");
        // 会话卡浮层带 .is-convo（标题改名行为差异）。
        // 有 onRename → 额外加 .is-renamable：标题可点改名。
        // 尺寸/列宽/行距统一走 --flyout-* CSS 变量，无需内联覆盖。
        flyout.className = onRename
            ? "project-info-flyout is-convo is-renamable"
            : "project-info-flyout is-convo";
        flyout.innerHTML = `
            <div class="pif-name" title="${onRename ? "点击重命名" : ""}">${escapeHtml(convo.title)} ${onRename ? `<span class="pif-name-edit">${icon("edit", 12)}</span>` : ""}</div>
            <div class="pif-row"><span class="pif-label">模型</span><span class="pif-value mono">${escapeHtml(convo.model || "—")}</span></div>
            <div class="pif-row"><span class="pif-label">消息</span><span class="pif-value">${convo.message_count} 条</span></div>
            <div class="pif-row"><span class="pif-label">大小</span><span class="pif-value">${formatSize(convo.size_bytes)}</span></div>
            <div class="pif-row"><span class="pif-label">更新</span><span class="pif-value">${formatTime(convo.last_updated)}</span></div>
            ${convo.cwd ? `<div class="pif-row"><span class="pif-label">目录</span><span class="pif-value mono copyable">${escapeHtml(convo.cwd)}</span></div>` : ""}
            <div class="pif-row"><span class="pif-label">ID</span><span class="pif-value mono copyable">${escapeHtml(convo.id)}</span></div>`;
        bindCopyable(flyout);
        // 有 onRename 才绑点击改名（与项目卡 showProjectInfo 完全同一逻辑）。
        if (onRename) {
            flyout.querySelector<HTMLElement>(".pif-name")?.addEventListener("click", (e) => {
                e.stopPropagation();
                flyout.remove();
                onRename();
            });
        }
        return flyout;
    });
}

/** Tier label for the switcher row: capitalized tier alias ("opus"→"Opus").
 *  No hardcoded table — works for any discovered tier (fable→Fable, etc.). */
function tierLabel(tier: string): string {
    const t = tier.trim();
    if (!t) return "";
    return t.charAt(0).toUpperCase() + t.slice(1);
}

/** Find a slot by tier key; return its clean display name (prefers
 *  model_name, falls back to model). "" when the tier isn't configured. */
function tierModel(info: ModelInfo, tier: string): string {
    const slot = info.tiers.find((s) => s.tier === tier);
    if (!slot) return "";
    return slot.model_name || slot.model;
}

let modelStateCache: ModelState | null = null;

export async function renderHeader(modelEl: HTMLElement, tool: ToolName): Promise<void> {
    // Reasonix 的模型配置在它自己的 config.toml，Cove 不负责读写也不提供
    // 切换。非 Claude 工具下模型胶囊显示静态"未配置"（不挂 hover 切换器），
    // 而不是清空——避免顶栏留一块空白让人以为没渲染出来。
    if (tool !== "claude") {
        modelEl.classList.remove("clickable");
        modelEl.title = "此工具的模型由其自身配置管理，Cove 不提供切换";
        renderModelLabel(modelEl, "未配置");
        return;
    }
    modelEl.classList.add("clickable");
    modelEl.title = "默认模型（悬停切换）";
    // hover 触发模型菜单。attachHoverFlyout 靠 dataset 标记防重复绑定——
    // modelEl 是 titlebar 里的持久元素，每次切 tab/工具都重跑 renderHeader，
    // 没守卫会叠一堆 mouseenter/mouseleave 监听。复用第一次的监听器是安全的：
    // 其闭包读模块级 modelStateCache，refreshModelLabel 每次更新它，hover 时
    // 取的是最新模型状态。
    // buildFlyout 内部额外检查 clickable：切到非 Claude 工具后 modelEl 上旧
    // hover 监听仍在（守卫只挡新绑定不移除旧的），此时 hover 不应弹模型菜单，
    // 返回 null 让 attachHoverFlyout 的 open() 跳过 append。
    attachHoverFlyout(modelEl, () => {
        if (!modelEl.classList.contains("clickable")) return null;
        return buildModelSwitcher(modelEl);
    });
    await refreshModelLabel(modelEl);
}

/** 把 label 写进模型胶囊。空/空白 label 统一降级为"未配置"，保证胶囊永不空白
 *  （用户反馈：没配模型时不要留空）。
 *
 *  绿点（model-dot）放在 .titlebar-model 直接子级，与 marquee 并列——这样只有
 *  marquee 内部文字超长轮播时绿点不跟着平移（用户反馈：绿点跟着模型名动）。
 *  未配置（label 空/降级为"未配置"）时挂 .is-unset，绿点用警告色而非成功色。 */
function renderModelLabel(modelEl: HTMLElement, label: string): void {
    const text = label && label.trim() ? label : "未配置";
    const isUnset = !label || !label.trim();
    modelEl.classList.toggle("is-unset", isUnset);
    modelEl.innerHTML = `<span class="model-dot"></span><span class="model-marquee"><span class="model-marquee-inner">${escapeHtml(text)}</span></span>`;
    requestAnimationFrame(() => {
        const inner = modelEl.querySelector<HTMLElement>(".model-marquee-inner");
        const wrap = modelEl.querySelector<HTMLElement>(".model-marquee");
        if (!inner || !wrap) return;
        if (inner.scrollWidth - wrap.clientWidth > 4) {
            inner.classList.add("is-overflow");
        } else {
            inner.classList.remove("is-overflow");
        }
    });
}

async function refreshModelLabel(modelEl: HTMLElement): Promise<void> {
    let label = "未配置";
    try {
        const state = await api.getModelState();
        modelStateCache = state;
        // When `model` is a direct id (cc-switch sets e.g. "DeepSeek-V4-Pro"),
        // `tier` is empty — show that raw id directly. Otherwise resolve the
        // tier slot's clean name. 空字符串统一降级为"未配置"。
        if (state.tier) {
            label = tierModel(state.info, state.tier) || "未配置";
        } else if (state.model) {
            label = state.model;
        }
    } catch {
        /* keep "未配置" */
    }
    renderModelLabel(modelEl, label);
}

/**
 * 构建模型切换菜单（hover 浮层）。返回菜单元素，由 attachHoverFlyout 负责
 * append / 定位 / 关闭。
 *
 * 行排版完全同构：每行都是 [ms-tier 标签 | ms-name 模型名 | ms-slot 右槽]。
 * 右槽固定宽度（ms-slot），选中档位放 ✓，未选中的放空槽——这样有无 ✓ 的行
 * 模型名对齐位置一致，不会因为 ✓ 撑位导致 Default 行错位（用户反馈的根因）。
 *
 * 档位数量动态：遍历后端发现的 tiers（opus/sonnet/fable/haiku/...），来几个
 * 显示几个，不再硬编码三档。
 */
function buildModelSwitcher(modelEl: HTMLElement): HTMLElement {
    // 清掉可能残留的旧菜单（attachHoverFlyout 只清 .project-info-flyout）。
    document.querySelectorAll(".model-switcher").forEach((e) => e.remove());

    const menu = document.createElement("div");
    menu.className = "model-switcher";
    if (!modelStateCache) return menu;
    const state = modelStateCache;
    const current = state.tier;

    // 第一行：Default 行——只在 ccswitch 直连模型时显示。
    //   - 档位模式（model 是 opus/sonnet/fable/haiku）：Default = 选中档位，是
    //     同一件事，显示会重复，故不显示 Default 行。
    //   - 直连模式（ccswitch 把顶层 model 写成具体模型名如 "DeepSeek-V4-Pro"）：
    //     所有档位都不命中，Default 行单独显示这个直连模型，告诉用户当前实际
    //     生效的是它（而非某个档位）。这才是 Default 真正有价值的场景。
    if (!current && state.model) {
        const info = document.createElement("div");
        info.className = "model-switcher-item is-static";
        info.innerHTML =
            `<span class="ms-tier">Default</span>` +
            `<span class="ms-name">${escapeHtml(state.model)}</span>` +
            `<span class="ms-slot"></span>`;
        menu.appendChild(info);
    }

    // 所有已发现档位（由后端按 sonnet→opus→fable→haiku→alpha 排序给出）。
    // 选中的右槽放 ✓，其余放空槽。
    for (const slot of state.info.tiers) {
        const tier = slot.tier;
        const name = slot.model_name || slot.model || "未配置";
        const isCur = tier === current;
        const item = document.createElement("button");
        item.className = "model-switcher-item" + (isCur ? " is-current" : "");
        item.innerHTML =
            `<span class="ms-tier">${escapeHtml(tierLabel(tier))}</span>` +
            `<span class="ms-name">${escapeHtml(name)}</span>` +
            `<span class="ms-slot">${isCur ? icon("check") : ""}</span>`;
        item.onclick = async (ev) => {
            ev.stopPropagation();
            if (isCur) { menu.remove(); return; }
            try {
                await api.setDefaultTier(tier);
                // 选档位会把顶层 "model" 改写成该别名，故新激活模型就是这档；
                // 同步更新 model + tier 让缓存反映真实状态。
                modelStateCache = { model: tier, tier, info: state.info };
                await refreshModelLabel(modelEl);
                toast(`默认模型已切换为 ${tierLabel(tier)}`);
            } catch (e) {
                toast("切换失败: " + (e as Error).message);
            }
            menu.remove();
        };
        menu.appendChild(item);
    }
    return menu;
}

// ===========================================================================
// 通用 hover 菜单绑定（四页 ▾ split-button 共用）
// 把原本 click 触发的菜单改成 hover 触发：鼠标进入 caret 显示菜单，
// 鼠标离开 caret/菜单延迟关闭（80ms 容差，避免划过间隙误关）。
// showMenu(anchor) 负责创建并定位菜单元素（返回菜单 HTMLElement），
// 它内部原有的"点击外部关闭"逻辑仍保留（点菜单项后关闭）。
// ===========================================================================

/** 把 caret 的菜单从 click 改成 hover 触发。四页 ▾ 共用。
 *  showMenu 可同步或异步；同步时返回菜单元素由 bindHoverMenu 管理关闭，
 *  异步时返回 Promise（菜单自管关闭，bindHoverMenu 只负责 mouseenter 触发）。
 *  菜单项点击后由 showXxxMenu 自己 menu.remove()；bindHoverMenu 在下次 mouseenter
 *  时检测到菜单已不在 DOM，重新打开（修复 opened 标志不复位导致菜单"卡住打不开"）。 */
export function bindHoverMenu(
    caret: HTMLElement,
    showMenu: (anchor: HTMLElement) => HTMLElement | Promise<HTMLElement | void>
): void {
    let menu: HTMLElement | null = null;
    let closeTimer: number | null = null;

    const cancelClose = (): void => {
        if (closeTimer !== null) { clearTimeout(closeTimer); closeTimer = null; }
    };
    const scheduleClose = (): void => {
        cancelClose();
        closeTimer = window.setTimeout(() => {
            menu?.remove();
            menu = null;
        }, 80);
    };
    const attachMenuHover = (m: HTMLElement): void => {
        menu = m;
        m.addEventListener("mouseenter", cancelClose);
        m.addEventListener("mouseleave", scheduleClose);
    };

    caret.addEventListener("mouseenter", () => {
        cancelClose();
        // 菜单已被移除（如菜单项点击后自移除），重置 menu 引用允许重新打开。
        if (menu && !document.body.contains(menu)) menu = null;
        if (menu) return;  // 已打开，不重复
        const result = showMenu(caret);
        if (result instanceof HTMLElement) {
            attachMenuHover(result);
        } else if (result && typeof (result as Promise<HTMLElement>).then === "function") {
            // async showMenu：等它 resolve 拿到菜单元素再绑 hover。
            (result as Promise<HTMLElement | void>).then((m) => {
                if (m instanceof HTMLElement) attachMenuHover(m);
            });
        }
    });
    caret.addEventListener("mouseleave", scheduleClose);
}

// ===========================================================================
// 项目页「添加 ▾」小箭头菜单（split-button 的 ▾）
// ===========================================================================

/** 项目页「添加 ▾」菜单（hover 触发）。 */
function showProjectsMenu(anchor: HTMLElement): HTMLElement {
    return createAnchoredMenu(anchor, "projects-menu", `
        <button class="model-switcher-item" type="button" data-act="open-projects">
            <span class="ms-tier">${icon("folder", 14)} 打开项目数据目录</span>
        </button>
        <button class="model-switcher-item" type="button" data-act="open-claude">
            <span class="ms-tier">${icon("folder", 14)} 打开 Claude 配置目录</span>
        </button>`, {
        "open-projects": async () => {
            try { await api.openAppDataDir("projects"); }
            catch (err) { toast("打开失败：" + String(err)); }
        },
        "open-claude": async () => {
            try { await api.openAppDataDir("claude"); }
            catch (err) { toast("打开失败：" + String(err)); }
        },
    });
}
