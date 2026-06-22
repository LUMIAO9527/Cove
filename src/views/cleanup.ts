import { api, OrphanEntry } from "../api";
import { icon } from "../styles/icons";
import { confirmDialog, toast } from "./confirm";
import { escapeHtml, formatSize } from "./projects";

/** 每种孤儿数据的中文说明与安全等级，帮用户判断该不该删。 */
const KIND_INFO: Record<string, { label: string; desc: string; safe: boolean }> = {
    tasks: {
        label: "任务记录",
        desc: "Claude Code 运行过的后台任务记录（如命令执行）。删除安全，仅影响历史任务列表。",
        safe: true,
    },
    "file-history": {
        label: "文件历史快照",
        desc: "文件修改的备份快照（用于撤销修改）。删除后会丢失文件回滚能力，但对话本身不受影响。",
        safe: false,
    },
    telemetry: {
        label: "遥测日志",
        desc: "使用统计与调试事件日志。删除完全安全，不影响任何功能。",
        safe: true,
    },
    "project-subdir": {
        label: "会话附属文件",
        desc: "某会话的专属子目录（附件、草稿等）。删除会丢失该会话的附属文件。",
        safe: false,
    },
    "session-env": {
        label: "会话环境",
        desc: "会话的环境变量记录。删除安全，不影响历史对话。",
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
                <button class="btn btn-ghost section-action" id="rescan-btn" title="重新扫描残留数据">${icon("refresh", 14)} 重新扫描</button>
            </div>
            <div id="scan-result" style="padding:0 var(--sp-1);">
                <div class="loading">正在扫描残留数据…</div>
            </div>
        </div>`;

    // 重新扫描：直接重渲染（重新走 loading → 扫描流程）。
    document.getElementById("rescan-btn")!.addEventListener("click", () => {
        renderCleanupView(container);
    });

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

/** （已废弃）删除单项后更新摘要。v0.4.26 起单项删除直接 renderCleanupView
 *  重扫整个视图，不再需要局部更新——摘要和卡片状态天然一致。保留空函数
 *  占位只是为了避免历史调用点报错（实际已无调用点）。 */
// 注：原函数体已删除；若以后需要局部更新可在此重写。
