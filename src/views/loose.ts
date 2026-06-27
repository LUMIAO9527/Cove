import { api, Conversation, ToolName } from "../api";
import { icon } from "../styles/icons";
import { confirmDialog, toast } from "./confirm";
import { escapeHtml, formatSize, formatTime, showConvoInfo, fullDeleteConfirmOptions, animateRemoveCard, bindHoverMenu, renameSessionPrompt, createAnchoredMenu } from "./projects";
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

    // 批量清理入口：▾ 菜单点「按时间批量清理」后切换到批量模式。
    // 闭包持有 convos/container/tool/onSelectSession，批量模式函数需要这些。
    const onBatchClean = () => {
        renderBatchCleanView(container, tool, convos, onSelectSession, () => renderLooseView(container, tool, onSelectSession));
    };

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

    bindNewChatButton(container, tool, onBatchClean);

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

    // Info button => hover flyout with session metadata.
    // 传 onRename：复用本卡片已有的 loose-action rename 逻辑，让 info 浮层也能改名。
    container.querySelectorAll<HTMLElement>(".sub-info-btn").forEach((btn) => {
        btn.addEventListener("click", (e) => e.stopPropagation());
        const card = btn.closest(".sub-session") as HTMLElement;
        const sid = card?.dataset.sid!;
        const convo = convos.find((c) => c.id === sid);
        if (convo) {
            // 改名走统一入口 renameSessionPrompt。
            const onRename = () => renameSessionPrompt(
                tool, sid, convo.project_encoded,
                card?.querySelector<HTMLElement>(".sub-title")
            );
            showConvoInfo(btn, convo, onRename);
        }
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
                await renameSessionPrompt(
                    tool, sid, encoded,
                    card?.querySelector<HTMLElement>(".sub-title")
                );
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

/** 绑定新对话按钮（主体点击静默开 / 箭头 hover 弹菜单）。 */
function bindNewChatButton(scope: HTMLElement, tool: ToolName, onBatchClean?: () => void): void {
    const mainBtn = scope.querySelector<HTMLElement>("#new-chat-btn");
    const caretBtn = scope.querySelector<HTMLElement>("#new-chat-caret");
    if (mainBtn) mainBtn.addEventListener("click", () => void onNewChatClick(tool));
    if (caretBtn) bindHoverMenu(caretBtn, (anchor) => showWorkspaceMenu(anchor, onBatchClean));
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
 *  - 「打开默认目录」→ 在资源管理器打开。
 *  - 「按时间批量清理…」→ 切换到批量操作模式（onBatchClean 为空时不显示，如空列表）。
 */
async function showWorkspaceMenu(anchor: HTMLElement, onBatchClean?: () => void): Promise<HTMLElement> {
    // 先用工厂建空菜单（同步返回给 bindHoverMenu 绑 hover），内容异步填充。
    // 因 getDefaultWorkspace 是异步，无法一次性把内容/handlers 都传给 createAnchoredMenu，
    // 这里用空 html + 空 handlers 初始化，定位/外部关闭交给工厂，html 拉到再写入并补绑。
    const menu = createAnchoredMenu(anchor, "workspace-menu", "", {});

    const current = await api.getDefaultWorkspace();
    menu.innerHTML = `
        <div class="ws-current">
            <span class="ws-current-label">当前默认目录</span>
            <span class="ws-current-path mono">${escapeHtml(current || "未设置")}</span>
        </div>
        <button class="model-switcher-item" type="button" data-act="change">
            <span class="ms-tier">${icon("folder", 14)} 更改默认目录…</span>
        </button>
        <button class="model-switcher-item" type="button" data-act="open" ${current ? "" : "disabled"}>
            <span class="ms-tier">${icon("folder", 14)} 打开默认目录</span>
        </button>
        ${onBatchClean ? `<button class="model-switcher-item" type="button" data-act="batch">
            <span class="ms-tier">${icon("broom", 14)} 按时间批量清理…</span>
        </button>` : ""}`;

    // 异步填好内容后补绑 handlers（结构对齐 createAnchoredMenu 的 data-act 约定）。
    const bind = (act: string, fn: (e: MouseEvent) => void | Promise<void>): void => {
        menu.querySelector<HTMLElement>(`[data-act="${act}"]`)?.addEventListener("click", async (e) => {
            e.stopPropagation();
            menu.remove();
            await fn(e);
        });
    };
    bind("change", async () => {
        const picked = await pickWorkspaceFolder();
        if (!picked) return;
        try {
            await api.setDefaultWorkspace(picked);
            toast("默认工作目录已更新");
        } catch (err) { toast("更新失败：" + String(err)); }
    });
    bind("open", async () => {
        if (!current) return;
        try { await api.openInExplorer(current); }
        catch (err) { toast("打开失败：" + String(err)); }
    });
    bind("batch", () => { onBatchClean?.(); });

    return menu;
}

// ===========================================================================
// 散落对话页 · 按时间批量清理模式
//
// 从「新对话▾」菜单进入。列表按 last_updated 距今天数分 4 档渲染，每档一个
// 分组头（带全选复选框 + 计数），每条会话左侧带复选框。底部固定操作栏显示
// 已选数量 + 批量归档/批量删除。退出回到普通列表。
//
// 分组规则（按天）：
//   最近 3 天  → 0-3 天
//   一周前     → 4-7 天
//   一个月前   → 8-30 天
//   更早       → >30 天
// 空档（该时间段无会话）不显示。
// ===========================================================================

/** 时间分档定义：minDays/maxDays 是距今天数的闭区间，各档连续无重叠。
 *  标签必须与区间语义一致（之前用 ≤7 算"一周前"，导致 6 天前落到"一周前"、
 *  7 天前落到"一个月前"——标签和边界错位）。现改为连续区间：
 *    最近 3 天 → 0–3 天
 *    本周      → 4–7 天（都还没超过一周）
 *    本月      → 8–30 天
 *    更早      → >30 天 */
const TIME_BUCKETS: { key: string; label: string; minDays: number; maxDays: number }[] = [
    { key: "recent", label: "最近 3 天", minDays: 0, maxDays: 3 },
    { key: "week", label: "本周", minDays: 4, maxDays: 7 },
    { key: "month", label: "本月", minDays: 8, maxDays: 30 },
    { key: "older", label: "更早", minDays: 31, maxDays: Infinity },
];

/** 把会话按时间档分组，返回非空档（按档定义顺序）。 */
function groupByTime(convos: Conversation[]): { bucket: typeof TIME_BUCKETS[0]; items: Conversation[] }[] {
    const now = Date.now();
    const dayMs = 24 * 60 * 60 * 1000;
    const groups: Record<string, Conversation[]> = {};
    for (const c of convos) {
        const days = (now - c.last_updated) / dayMs;
        for (const b of TIME_BUCKETS) {
            if (days >= b.minDays && days <= b.maxDays) {
                (groups[b.key] = groups[b.key] || []).push(c);
                break;
            }
        }
    }
    return TIME_BUCKETS
        .filter((b) => groups[b.key]?.length)
        .map((b) => ({ bucket: b, items: groups[b.key] }));
}

/**
 * 渲染批量清理模式。
 * - onExit：退出回到普通列表（重新调 renderLooseView）。
 * 已选状态用闭包内的 Set<sid> 维护，每次勾选/全选后只更新计数文案，
 * 不重渲染整个列表（避免复选框焦点丢失 + 卡顿）。
 */
export function renderBatchCleanView(
    container: HTMLElement,
    tool: ToolName,
    convos: Conversation[],
    _onSelectSession: (sid: string, encoded: string, projPath: string, title: string) => void,
    onExit: () => void
): void {
    const groups = groupByTime(convos);
    // 已选 sid 集合。
    const selected = new Set<string>();

    // 先按组渲染整个列表骨架，再用事件委托处理勾选（避免给每个 checkbox 绑监听）。
    const groupsHtml = groups.map((g) => {
        const itemRows = g.items.map((c) => `
            <label class="batch-item" data-sid="${escapeHtml(c.id)}">
                <input type="checkbox" class="batch-check" data-sid="${escapeHtml(c.id)}" data-bucket="${g.bucket.key}" />
                <span class="batch-title" title="${escapeHtml(c.title)}">${escapeHtml(c.title)}</span>
                <span class="meta"><span class="sep">·</span>${formatTime(c.last_updated)}</span>
            </label>`).join("");
        return `
            <div class="batch-group" data-bucket="${g.bucket.key}">
                <div class="batch-group-head">
                    <label class="batch-group-select">
                        <input type="checkbox" class="batch-group-check" data-bucket="${g.bucket.key}" />
                        <span>${g.bucket.label}</span>
                    </label>
                    <span class="batch-group-count">${g.items.length} 条</span>
                </div>
                ${itemRows}
            </div>`;
    }).join("");

    container.innerHTML = `
        <div class="scroll-area has-batch">
            <div class="section-label">
                ${icon("message", 13)} 批量清理 · ${convos.length} 条
                <button class="btn btn-ghost section-action" id="batch-exit" title="退出批量模式">${icon("close", 14)} 退出</button>
            </div>
            ${groupsHtml}
            <div style="height:56px"></div>
        </div>
        <div class="batch-action-bar">
            <span class="batch-selected-count" id="batch-count">已选 0 条</span>
            <span class="batch-actions">
                <button class="btn btn-warn" id="batch-archive" disabled>${icon("archive", 13)} 批量归档</button>
                <button class="btn btn-danger" id="batch-delete" disabled>${icon("trash", 13)} 批量删除</button>
            </span>
        </div>`;

    const updateCount = (): void => {
        const n = selected.size;
        document.getElementById("batch-count")!.textContent = `已选 ${n} 条`;
        (document.getElementById("batch-archive") as HTMLButtonElement).disabled = n === 0;
        (document.getElementById("batch-delete") as HTMLButtonElement).disabled = n === 0;
        // 同步每个分组头的三态（全选/部分/空）。
        for (const g of groups) {
            const groupCheck = document.querySelector<HTMLInputElement>(
                `.batch-group-check[data-bucket="${g.bucket.key}"]`
            );
            if (!groupCheck) continue;
            const checkedInGroup = g.items.filter((c) => selected.has(c.id)).length;
            groupCheck.checked = checkedInGroup === g.items.length;
            groupCheck.indeterminate = checkedInGroup > 0 && checkedInGroup < g.items.length;
        }
    };

    // 单条复选框：勾选/取消更新 selected 集合。
    container.querySelectorAll<HTMLInputElement>(".batch-check").forEach((cb) => {
        cb.addEventListener("change", () => {
            const sid = cb.dataset.sid!;
            if (cb.checked) selected.add(sid);
            else selected.delete(sid);
            updateCount();
        });
    });

    // 分组全选复选框：勾选/取消整组。
    container.querySelectorAll<HTMLInputElement>(".batch-group-check").forEach((cb) => {
        cb.addEventListener("change", () => {
            const bucket = cb.dataset.bucket!;
            const g = groups.find((x) => x.bucket.key === bucket)!;
            for (const c of g.items) {
                const itemCb = container.querySelector<HTMLInputElement>(
                    `.batch-check[data-sid="${CSS.escape(c.id)}"]`
                );
                if (cb.checked) {
                    selected.add(c.id);
                    if (itemCb) itemCb.checked = true;
                } else {
                    selected.delete(c.id);
                    if (itemCb) itemCb.checked = false;
                }
            }
            updateCount();
        });
    });

    // 退出：回到普通列表。
    document.getElementById("batch-exit")!.addEventListener("click", onExit);

    // 批量归档：逐条调 archiveConvo，完成后重渲染。
    // 批量操作执行期间禁用按钮，防止重复触发（危险操作尤其需要）。
    let batchBusy = false;
    const setBatchBusy = (b: boolean): void => {
        batchBusy = b;
        (document.getElementById("batch-archive") as HTMLButtonElement).disabled = b || selected.size === 0;
        (document.getElementById("batch-delete") as HTMLButtonElement).disabled = b || selected.size === 0;
    };

    document.getElementById("batch-archive")!.addEventListener("click", async () => {
        if (batchBusy || selected.size === 0) return;
        setBatchBusy(true);
        const ok = await confirmDialog({
            title: "批量归档",
            body: `将归档已选 <span class="mono">${selected.size}</span> 条会话到归档区，可随时恢复。`,
            confirmText: "归档",
            variant: "accent",
            titleIcon: "archive",
        });
        if (!ok) { setBatchBusy(false); return; }
        let failed = 0;
        const sids = Array.from(selected);
        const countEl = document.getElementById("batch-count");
        for (let i = 0; i < sids.length; i++) {
            if (countEl) countEl.textContent = `归档中 ${i + 1}/${sids.length}…`;
            const sid = sids[i];
            const c = convos.find((x) => x.id === sid);
            if (!c) continue;
            try {
                await api.archiveConvo(tool, sid, c.project_encoded);
            } catch {
                failed += 1;
            }
        }
        toast(failed > 0 ? `已归档，${failed} 条失败` : `已归档 ${sids.length} 条`);
        onExit();
    });

    // 批量删除：全删确认（复用 fullDeleteConfirmOptions 文案，列明 8 类关联数据），
    // 逐条调 deleteConvo，完成后重渲染。
    document.getElementById("batch-delete")!.addEventListener("click", async () => {
        if (batchBusy || selected.size === 0) return;
        setBatchBusy(true);
        const sids = Array.from(selected);
        const ok = await confirmDialog({
            title: "批量删除",
            body: `将<strong>永久删除</strong>已选 <span class="mono">${sids.length}</span> 条会话及其全部关联数据（对话正文 · 子目录 · tasks · file-history · telemetry · session-env · sessions · history），<b>不可恢复</b>。`,
            confirmText: "删除",
            variant: "danger",
            titleIcon: "trash",
        });
        if (!ok) { setBatchBusy(false); return; }
        let failed = 0;
        const delCountEl = document.getElementById("batch-count");
        for (let i = 0; i < sids.length; i++) {
            if (delCountEl) delCountEl.textContent = `删除中 ${i + 1}/${sids.length}…`;
            const sid = sids[i];
            const c = convos.find((x) => x.id === sid);
            if (!c) continue;
            try {
                await api.deleteConvo(tool, sid, c.project_encoded);
            } catch {
                failed += 1;
            }
        }
        toast(failed > 0 ? `已删除，${failed} 条失败` : `已删除 ${sids.length} 条`);
        onExit();
    });

    updateCount();
}
