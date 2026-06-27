import { api, OrphanEntry } from "../api";
import { icon } from "../styles/icons";
import { confirmDialog, toast } from "./confirm";
import { escapeHtml, formatSize, bindHoverMenu, createAnchoredMenu } from "./projects";

/**
 * 每种孤儿数据的中文说明与安全等级，帮用户判断该不该删。
 *
 * 重要前提：**清理页里列出的每一项都是"孤儿"**——对应的会话正文（jsonl）
 * 已经不在了（被删除 / 归档时已随会话移走 / 会话从未完整建立）。所以这里的
 * `safe` 判断的不是"删了会不会影响某个还活着的会话"（那些会话根本不在这页），
 * 而是"**这项数据本身还有没有价值**"——会话既然已不在，它的附属数据通常也就
 * 没人再用了；但个别类型（如 project-subdir）可能装着用户当时保存的文件，
 * 删了不可逆，所以仍标"有影响"，让用户自己斟酌。
 *
 * 常见成因（供文案参考）：旧版 Cove / 手动删 jsonl / Claude Code 自身清理
 * 残留——这些目录按会话 ID 命名，会话没了，目录就成了孤儿。
 */
const KIND_INFO: Record<string, { label: string; desc: string; safe: boolean }> = {
    tasks: {
        label: "任务记录",
        desc: "已删除会话的后台任务残留（如命令执行记录）。所属会话已不在，删除安全，无任何功能受影响。",
        safe: true,
    },
    "file-history": {
        // 重新判定为安全（v0.4.29）：这里存的是会话内的文件修改快照，
        // 唯一用途是给"还活着的会话"提供文件回滚；既然会话已殁，这些快照
        // 再无消费者，删除安全。（旧版笼统标"有影响"是按"会话还活着"假设
        // 写的，对孤儿项是误导——会让用户以为删了会破坏对话。）
        label: "文件历史快照",
        desc: "会话内的文件修改快照，原用于文件回滚。所属会话已不在，这些快照再无用途，删除安全。",
        safe: true,
    },
    telemetry: {
        label: "遥测日志",
        desc: "使用统计与调试事件日志。删除完全安全，不影响任何功能。",
        safe: true,
    },
    "project-subdir": {
        // 保持"有影响"：可能装着用户会话里保存的附件 / 草稿等真实文件，
        // 即便会话已不在，这些文件本身可能还有价值，删了不可逆。
        label: "会话附属文件",
        desc: "会话的专属子目录（附件、草稿等）。所属会话已不在，但里面可能保存着仍有用的文件，删除前建议先在文件夹里看一眼。",
        safe: false,
    },
    "session-env": {
        label: "会话环境",
        desc: "已删除会话的环境变量记录。所属会话已不在，删除安全，不影响任何对话。",
        safe: true,
    },
};

function kindLabel(kind: string): string {
    return KIND_INFO[kind]?.label || kind;
}
function kindDesc(kind: string): string {
    return KIND_INFO[kind]?.desc || "未识别的残留数据，建议保留以防万一。";
}
function kindSafe(kind: string): boolean {
    return KIND_INFO[kind]?.safe ?? false;
}

/**
 * 磁盘清理视图（底部 tab 之一，和项目/散落对话/归档平级，不再是"进去再返回"的子页面）。
 *
 * 改动（v0.4.16）：去掉左上角的返回箭头（底部已有 4-tab 导航，箭头冗余）；
 * section-label 右侧加「重新扫描」按钮，重新扫描残留数据（清理页数据不会自动刷新）。
 */
export async function renderCleanupView(container: HTMLElement): Promise<void> {
    container.innerHTML = `
        <div class="scroll-area">
            <div class="section-label">
                ${icon("broom", 13)} 磁盘清理
                <span class="new-chat-wrap" id="cleanup-wrap">
                    <button class="btn btn-ghost section-action new-chat-main" id="rescan-btn" title="重新扫描残留数据">${icon("refresh", 14)} 重新扫描</button>
                    <button class="btn btn-ghost section-action new-chat-caret" id="cleanup-caret" title="更多操作">▾</button>
                </span>
            </div>
            <div id="scan-result" style="padding:0 var(--sp-1);">
                <div class="loading">正在扫描残留数据…</div>
            </div>
        </div>`;

    // 重新扫描：直接重渲染（重新走 loading → 扫描流程）。
    document.getElementById("rescan-btn")!.addEventListener("click", () => {
        renderCleanupView(container);
    });

    // 「重新扫描 ▾」小箭头菜单：hover 触发。
    const cleanupCaret = document.getElementById("cleanup-caret");
    if (cleanupCaret) bindHoverMenu(cleanupCaret, showCleanupMenu);

    const orphans = await api.scanOrphans();
    const resultDiv = document.getElementById("scan-result")!;

    if (orphans.length === 0) {
        resultDiv.innerHTML = `
            <div class="empty-state">
                <div class="empty-icon">${icon("check", 26)}</div>
                <div class="empty-title">很干净！</div>
                <div class="hint">未发现残留数据</div>
            </div>`;
        return;
    }

    const totalBytes = orphans.reduce((s, o) => s + o.size_bytes, 0);
    const safeCount = orphans.filter((o) => kindSafe(o.kind)).length;
    const riskyCount = orphans.length - safeCount;

    resultDiv.innerHTML = `
        <div class="summary-bar">
            <div>
                <div class="summary-title">发现 <span class="num">${orphans.length}</span> 项残留 · ${formatSize(totalBytes)}</div>
                <div class="summary-sub">
                    ${safeCount > 0 ? `<span style="color:var(--success)">✓ ${safeCount} 项可安全删除</span>` : ""}
                    ${safeCount > 0 && riskyCount > 0 ? " · " : ""}
                    ${riskyCount > 0 ? `<span style="color:var(--warn)">⚠ ${riskyCount} 项删除有影响</span>` : ""}
                </div>
            </div>
            <button class="btn btn-warn" id="clean-safe" title="仅清理可安全删除的">清理安全项</button>
        </div>
        ${orphansHtml(orphans)}`;

    // 清理所有"安全"项
    const safeOrphans = orphans.filter((o) => kindSafe(o.kind));
    document.getElementById("clean-safe")!.addEventListener("click", async () => {
        if (safeCount === 0) {
            toast("没有可安全删除的项");
            return;
        }
        const ok = await confirmDialog({
            title: "清理安全项",
            body: `将删除 <span class="mono">${safeCount}</span> 项可安全删除的残留数据（${safeOrphans.map((o) => kindLabel(o.kind)).filter((v, i, a) => a.indexOf(v) === i).join("、")}），不影响任何功能。`,
            confirmText: "清理",
            variant: "warn",
            titleIcon: "warn",
        });
        if (!ok) return;
        let n = 0;
        for (const o of safeOrphans) {
            if (await api.deleteOrphan(o.location)) n++;
        }
        toast(`已清理 ${n} 项`);
        renderCleanupView(container);
    });

    // 单项删除：滑出动画后删，成功才真正移除卡片 + 重扫整个视图刷新摘要
    // （updateSummary 的旧实现 remaining>0 时是空函数，导致摘要数字陈旧——
    // 评审 P1 #6 修复。重扫会丢滚动位置，但单删场景可接受）。
    resultDiv.querySelectorAll<HTMLElement>(".del-orphan").forEach((btn) => {
        btn.addEventListener("click", async () => {
            const loc = btn.dataset.loc!;
            const card = btn.closest(".orphan-card") as HTMLElement;
            card?.classList.add("removing");
            await new Promise((r) => setTimeout(r, 150));
            try {
                await api.deleteOrphan(loc);
            } catch (err) {
                card?.classList.remove("removing");
                toast("删除失败：" + String(err));
                return;
            }
            // 重扫整个视图：刷新摘要计数 + 移除已删卡片。
            renderCleanupView(container);
        });
    });
}

function orphansHtml(orphans: OrphanEntry[]): string {
    return orphans
        .map((o) => {
            const safe = kindSafe(o.kind);
            return `
        <div class="card orphan-card" data-kind="${escapeHtml(o.kind)}">
            <div class="orphan-head">
                <span class="orphan-tag ${safe ? "is-safe" : "is-risky"}">
                    ${icon(safe ? "check" : "warn", 12)} ${escapeHtml(kindLabel(o.kind))}
                </span>
                <span class="orphan-size">${formatSize(o.size_bytes)}</span>
            </div>
            <div class="orphan-desc">${escapeHtml(kindDesc(o.kind))}</div>
            ${o.belongs_to ? `<div class="meta"><span class="model-tag">${escapeHtml(o.belongs_to)}</span></div>` : ""}
            <div class="meta path-tag">${escapeHtml(o.location)}</div>
            <div class="actions">
                <button class="btn ${safe ? "btn-warn" : "btn-danger"} del-orphan" data-loc="${escapeHtml(o.location)}" data-kind="${escapeHtml(o.kind)}">
                    ${icon("trash", 13)} 删除${safe ? "" : "（有影响）"}
                </button>
            </div>
        </div>`;
        })
        .join("");
}

// ===========================================================================
// 清理页「重新扫描 ▾」小箭头菜单（split-button 的 ▾）
// 当前只一项：打开 ~/.claude 数据目录（方便用户手动查看残留文件）。
// ===========================================================================

/** 清理页「▾」菜单（hover 触发）。 */
function showCleanupMenu(anchor: HTMLElement): HTMLElement {
    return createAnchoredMenu(anchor, "cleanup-menu", `
        <button class="model-switcher-item" type="button" data-act="open-dir">
            <span class="ms-tier">${icon("folder", 14)} 打开数据目录…</span>
        </button>`, {
        "open-dir": async () => {
            try { await api.openAppDataDir("claude"); }
            catch (err) { toast("打开失败：" + String(err)); }
        },
    });
}
