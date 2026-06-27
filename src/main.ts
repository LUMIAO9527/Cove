import "./styles/theme.css";
import "./styles/animations.css";
import { icon } from "./styles/icons";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { renderProjectsView, renderHeader, escapeHtml } from "./views/projects";
import { renderConversationsView } from "./views/conversations";
import { renderLooseView } from "./views/loose";
import { renderArchiveView } from "./views/archive";
import { renderCleanupView } from "./views/cleanup";
import { renderSessionDetailView } from "./views/session-detail";
import { toast } from "./views/confirm";
import { Project, ToolName, DEFAULT_TOOL, api } from "./api";

const app = document.getElementById("app")!;

// 当前顶层视图的重新渲染函数。每次切换 tab / 进入列表视图时更新，
// 会话详情页（drill-in 子页面，只读历史查看）不更新它——避免重渲染打断阅读。
// cove-shown 事件触发时（用户每次重新打开 Cove 弹窗）会调用它一次，
// 保证"每次打开看到的就是最新的"，不需要用户手动刷新。
let currentView: (() => void) | null = null;

// 当前选中的 CLI 工具。顶部 titlebar 切换器修改它，切到哪个工具，
// 项目/散落对话/归档/清理四页就只显示该工具的数据。Claude 专属的归档/
// 清理页在非 Claude 工具下降级为说明页。
let currentTool: ToolName = DEFAULT_TOOL;

// ----- Flyout open/close animation (driven by Rust events) -----
// Rust emits "cove-shown" right after the window is shown → play open anim.
// Rust emits "cove-request-close" when focus is lost / tray toggled → play
// close anim, then call hide_window to actually hide the window.
// Rust emits "cove-focus-regained" if focus comes back during a close anim
// (user clicked back into the popup) → abort the close.

function playOpenAnim(): void {
    app.classList.remove("closing");
    app.classList.add("opening");
    app.addEventListener(
        "animationend",
        () => app.classList.remove("opening"),
        { once: true }
    );
}

async function playCloseAnimThenHide(): Promise<void> {
    app.classList.remove("opening");
    app.classList.add("closing");
    return new Promise((resolve) => {
        app.addEventListener(
            "animationend",
            async () => {
                app.classList.remove("closing");
                try { await invoke("hide_window"); } catch { /* window may be gone */ }
                resolve();
            },
            { once: true }
        );
    });
}

listen("cove-shown", () => {
    playOpenAnim();
    // 每次弹窗打开时重新拉一次数据——让用户每次打开都看到最新。
    // 会话详情（drill-in 子页面）不会注册到 currentView，因此打开时
    // 不会打断用户当前的阅读位置。
    currentView?.();
});
listen("cove-request-close", () => { void playCloseAnimThenHide(); });
listen("cove-focus-regained", () => {
    // Abort an in-flight close animation.
    app.classList.remove("closing");
});

// Cove 32x32 品牌图标（base64 内嵌，与打包的应用图标/tray 图标完全一致）。
// 不再用 CSS 画的蓝底白字 "C" 方块——那和真实应用图标不符。
const COVE_ICON =
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAACXBIWXMAAAsTAAALEwEAmpwYAAAAAXNSR0IArs4c6QAAAARnQU1BAACxjwv8YQUAAANjSURBVHgB7VZBbtpQEJ0xUdVFq5AbGAXSdBXIoklUKZATND1ByAnqngBzAsgJQk6Q9ASYLkKyCXTVqhDhniBEyiKtwNP5tgH728YGlu2TwPb3/Hnvz5+Zb4B/HQgrgpr5NF8mPxOPusNF5i8lwCZVFI3vTvhRlV6bgGjA2KqyGDPO18IC6OtuGYhq4Kw4zn0Vi3f6XAtYhLy1q/N/xT9IJv8briCVV5/3M2AdD+8+w6oCOOxlUPDcQ2zws46HnZZkxyKgwsRlD01kJBIJsJ0q2ITpfpOOxW41Zo7Gc2rTAYuOOCcM2U6BJEgpIuxpXnWDXZfjyAWYrM629ZkPrITaQQKICMzLaLcUQS5Bp1pwAJOEtWhDtkkUAUG+eds/3rzpnwfIRWIq+CB+XCE1aR6Tiai5SEFRnh8rINvulXI3/QESXSKQ6iNv5ku+qiDSqJU/8Tmw4NvsfaBnzBfA5DpvUpNcYgI0JRM1ZJo8Znru04kFZK97ml1OMxjK2DqTzK4CExG6EsOO52mYSMB2eyAayowcqd7fzx313m/5nNt7bNFHDn3XaUik4WH3i1+QMmtMiF0I6A0B7/k5h70s7vnauN/fOoUl4PaPgfs4xGJnQ7YJREDtDNITcmYfjmkcW/ORcPqHq4auwkzWAgPPo/wkLoR0Ze6/Nb3vRW6gQp8IYchCq/d7b0Idc3nWmbTsPpq8mGoiARyTIhu7QJ9z0QvESUhCIYn9w0u109kwC4Vpctmlabdg8hxK3LojGtncMkTFn7XcB3Zkm9Sf1x/8A3byesk1bt0XEIG5AohQ9Qmw6Jdsg2gfx2HgDxMoMfkZzEFAgJcEwfKd7b2DrQYPOgcMJyjf6/13uZakuusQdzLyUR2GQBmKKlj7PXqYkIxePmW8e+zYdOyOJo/7fHAy9w9yBsQgEAGzkBFODVcei3lVCdoUhmHkufbPcvamd7n2PB7YyZwA4TngLxkte/1DgxiIc4PQ/mI6JrSGI2t0AQkQ+T2Qve1VWIg+04SNMaWq5kHGlIhL7plRmtry59j9XnY1Aa6IGjP7V4+i7zunIpdlnoWpkke9v5dL3D1jv4jkSESCE5YU6zSqMy4tQGC7/V0dY8oOc2DFImERjNGLp7OoqlhZgBeiBJHW18U94ePjMqT/4cVfZNp1ptgvGtYAAAAASUVORK5CYII=";

// Custom title bar (draggable)
const titlebar = document.createElement("div");
titlebar.className = "titlebar";
titlebar.innerHTML = `
    <img class="titlebar-brand" src="${COVE_ICON}" alt="Cove" draggable="false">
    <span class="titlebar-title" id="titlebar-title">Cove</span>
    <span class="titlebar-model" id="titlebar-model"></span>
    <span class="tool-capsule" id="tool-capsule"></span>
`;

// Content area
const content = document.createElement("div");
content.className = "content";

// Bottom navigation — icon-only tabs (compact, Raycast-style)
const footer = document.createElement("div");
footer.className = "footer";
footer.innerHTML = `
    <button class="btn btn-ghost" id="nav-projects" title="项目">${icon("folder", 17)}</button>
    <button class="btn btn-ghost" id="nav-conversations" title="散落对话">${icon("message", 17)}</button>
    <button class="btn btn-ghost" id="nav-archive" title="归档区">${icon("inbox", 17)}</button>
    <button class="btn btn-ghost" id="nav-cleanup" title="磁盘清理">${icon("broom", 17)}</button>
`;

app.appendChild(titlebar);
app.appendChild(content);
app.appendChild(footer);

// ----- Tool switcher (titlebar): dual-icon capsule -----
// 一个外胶囊里包两个小胶囊(Claude 图标 / Reasonix 图标)，点哪个亮哪个。
// 未安装的工具灰掉不可点；当前选中的高亮。高度与右侧模型胶囊对齐。
// 安装状态在启动时查后端 get_installed_tools（probe CLI 是否在 PATH）。
const TOOL_LABELS: Record<ToolName, string> = {
    claude: "Claude Code",
    reasonix: "Reasonix",
};
let installedTools: Record<string, boolean> = { claude: true, reasonix: false };

/** 当前选中的工具（尊重用户选择，即使未安装——view 层负责提示未安装）。 */
function effectiveTool(): ToolName {
    return currentTool;
}

/** 当前工具是否已安装（view 据此决定显示数据还是"未安装"提示）。 */
function toolInstalled(): boolean {
    return !!installedTools[currentTool];
}

/** 渲染双胶囊。任何工具都可点选高亮（即使未安装——用户可切过去看，
 *  功能区会提示"未安装"）。当前选中的 active 高亮。 */
function renderToolCapsule(): void {
    const el = document.getElementById("tool-capsule");
    if (!el) return;
    const tools: ToolName[] = ["claude", "reasonix"];
    el.innerHTML = tools
        .map((t) => {
            const installed = installedTools[t];
            const active = currentTool === t;
            const cls = ["tool-pill", active ? "is-active" : "", installed ? "" : "not-installed"]
                .filter(Boolean)
                .join(" ");
            const title = installed
                ? TOOL_LABELS[t]
                : `${TOOL_LABELS[t]}（未安装）`;
            const glyph = t === "claude" ? claudeGlyph() : reasonixGlyph();
            return `<button class="${cls}" data-tool="${t}" title="${title}">${glyph}</button>`;
        })
        .join("");
    el.querySelectorAll<HTMLElement>(".tool-pill").forEach((btn) => {
        btn.addEventListener("click", (e) => {
            e.stopPropagation();
            const t = btn.dataset.tool as ToolName;
            if (t === currentTool) return;
            currentTool = t;
            renderToolCapsule();
            currentView ? currentView() : showProjects();
        });
    });
}

/** Claude Code 官方图标（simple-icons "claude"，星芒造型）。
 *  品牌色 Claude orange #cc785c；未选中时用 currentColor 跟随文字色。 */
function claudeGlyph(): string {
    return `<svg class="tool-glyph tool-glyph-claude" viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
        <path d="m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z"/>
    </svg>`;
}

/** Reasonix 官方品牌 mark（docs/logo.svg 的同心菱形，静态无动画版）。
 *  品牌渐变 cyan→blue→violet；未选中时用 currentColor 跟随文字色。 */
function reasonixGlyph(): string {
    return `<svg class="tool-glyph tool-glyph-reasonix" viewBox="0 0 92 92" width="14" height="14" aria-hidden="true">
        <path class="rx-diamond-outer" d="M 46 6 L 86 46 L 46 86 L 6 46 Z" fill="none" stroke-width="6" stroke-linejoin="round"/>
        <path class="rx-diamond-inner" d="M 46 24 L 68 46 L 46 68 L 24 46 Z" fill="none" stroke-width="4" stroke-linejoin="round"/>
    </svg>`;
}

async function loadInstalledTools(): Promise<void> {
    try {
        installedTools = await api.getInstalledTools();
    } catch {
        installedTools = { claude: true, reasonix: false };
    }
    // 不强制回退：用户可以切到未安装的工具（功能区会提示"未安装"）。
    // 只在 currentTool 完全无效时兜底。
    renderToolCapsule();
}

// Navigation highlight
function setActiveNav(id: string): void {
    footer.querySelectorAll<HTMLElement>(".btn").forEach((b) => b.classList.remove("is-active"));
    document.getElementById(id)?.classList.add("is-active");
}

// View: projects (root)
async function showProjects(): Promise<void> {
    setActiveNav("nav-projects");
    currentView = () => { void showProjects(); };
    renderToolCapsule();
    const tool = effectiveTool();
    await renderHeader(document.getElementById("titlebar-model")!, tool);
    if (!toolInstalled()) { renderNotInstalledPage(content, tool); return; }
    await renderProjectsView(
        content,
        tool,
        async (proj: Project) => {
            // 项目详情会话列表入口：返回时回到该项目详情页，定位到该会话。
            // 具名回调避免 onBack 里重新渲染时的递归类型问题。
            const enterConvo = (sid: string, encoded: string, projPath: string, title: string) => {
                showSessionDetail(sid, encoded, projPath, title, async () => {
                    await renderConversationsView(content, tool, proj, () => showProjects(), enterConvo);
                    scrollToSession(sid);
                });
            };
            await renderConversationsView(content, tool, proj, () => showProjects(), enterConvo);
        },
        // 项目卡片内联展开入口：返回时回到项目列表，并恢复该项目的展开态，
        // 精确定位到刚才那条会话（main.ts 里做定位，因为 showProjects 是全量重渲染）。
        (sid, projEncoded, projPath, sessionTitle, restorePath) => {
            showSessionDetail(sid, projEncoded, projPath, sessionTitle, async () => {
                await showProjects();
                // 重渲染完后，重新展开那个项目卡片，再定位到该会话行。
                if (restorePath) restoreInlineExpansion(restorePath, sid);
            });
        }
    );
}

// View: session transcript (history viewer). Drill-in from a session card.
// `projPath` is the real cwd (for resume); `projEncoded` is the jsonl subdir.
// `onBack` 由各入口传入：返回时应恢复来源页（项目列表/散落对话/项目详情），
// 而不是写死回项目列表——否则从散落对话或项目详情点进来再返回，
// 会丢掉来源页和导航高亮，回不到刚才看的那条会话。
function showSessionDetail(
    sid: string,
    projEncoded: string,
    projPath: string,
    sessionTitle: string,
    onBack: () => void,
    isArchived: boolean = false
): void {
    void renderSessionDetailView(
        content,
        effectiveTool(),
        sid,
        projEncoded,
        projPath,
        sessionTitle,
        onBack,
        isArchived
    );
}

// View: loose conversations
function showConversations(): void {
    setActiveNav("nav-conversations");
    currentView = () => { showConversations(); };
    renderToolCapsule();
    const tool = effectiveTool();
    void renderHeader(document.getElementById("titlebar-model")!, tool);
    if (!toolInstalled()) { renderNotInstalledPage(content, tool); return; }
    // 散落对话入口：返回时回到散落对话页，并定位到刚才那条会话。
    // 用具名函数避免 onBack 里重新渲染时的递归类型问题。
    const enterSession = (sid: string, encoded: string, projPath: string, title: string) => {
        showSessionDetail(sid, encoded, projPath, title, async () => {
            await renderLooseView(content, tool, enterSession);
            scrollToSession(sid);
        });
    };
    renderLooseView(content, tool, enterSession);
}

// View: archive (per-tool: Claude 8-关联数据 / Reasonix sidecar 归档)
function showArchive(): void {
    setActiveNav("nav-archive");
    currentView = () => { showArchive(); };
    renderToolCapsule();
    const tool = effectiveTool();
    void renderHeader(document.getElementById("titlebar-model")!, tool);
    if (!toolInstalled()) { renderNotInstalledPage(content, tool); return; }
    // 归档会话也能点进查看对话记录（返回时回到归档页）。
    const enterSession = (sid: string, encoded: string, projPath: string, title: string) => {
        showSessionDetail(sid, encoded, projPath, title, async () => {
            renderArchiveView(content, tool, enterSession);
        }, true);
    };
    renderArchiveView(content, tool, enterSession);
}

// View: cleanup (Claude-only; 磁盘清理依赖 Claude 的 ~/.claude 关联数据结构，
// reasonix 没有等价的孤儿数据概念——这是真实数据模型差异，不是功能缺失)
function showCleanup(): void {
    setActiveNav("nav-cleanup");
    currentView = () => { showCleanup(); };
    renderToolCapsule();
    const tool = effectiveTool();
    void renderHeader(document.getElementById("titlebar-model")!, tool);
    if (!toolInstalled()) { renderNotInstalledPage(content, tool); return; }
    if (tool === "claude") {
        renderCleanupView(content);
    } else {
        renderFeatureUnsupportedPage(content, "磁盘清理", tool);
    }
}

/** 工具未安装时的统一提示页。切换到未装工具的任一功能区都显示这个，
 *  告诉用户怎么装。工具本身可选中（胶囊高亮），只是没有数据可管。
 *
 *  安装命令做成两按钮：主按钮是命令行（点击复制到剪贴板），旁边一个小按钮
 *  打开空白终端（定位到主目录）。用户复制命令 → 点开终端 → 粘贴回车即装。
 *  两个按钮完全解耦：不预填不自动执行，让用户在按回车前看清命令。 */
function renderNotInstalledPage(container: HTMLElement, tool: ToolName): void {
    const installCmd = tool === "claude"
        ? "npm i -g @anthropic-ai/claude-code"
        : "npm i -g reasonix";
    container.innerHTML = `
        <div class="empty-state">
            <div class="empty-icon">${icon("info", 28)}</div>
            <div class="empty-title">${TOOL_LABELS[tool]} 未安装</div>
            <div class="empty-hint">安装后即可在 Cove 中管理它的项目与会话。</div>
            <div class="install-row">
                <button class="install-cmd" title="点击复制安装命令">
                    <span class="install-cmd-text mono">${escapeHtml(installCmd)}</span>
                    ${icon("copy", 13)}
                </button>
                <button class="install-open" id="install-open-terminal" title="打开终端">
                    ${icon("terminal", 14)}
                </button>
            </div>
        </div>
    `;
    // 主按钮：点击复制安装命令到剪贴板（复用 projects.ts 的 bindCopyable 机制，
    // 但这里内联实现避免跨模块依赖；复制后 toast 提示）。
    const cmdBtn = container.querySelector<HTMLElement>(".install-cmd");
    cmdBtn?.addEventListener("click", async (e) => {
        e.stopPropagation();
        try {
            await navigator.clipboard.writeText(installCmd);
            toast("已复制安装命令");
        } catch {
            toast("复制失败，请手动选择命令");
        }
    });
    // 小按钮：打开空白终端（定位主目录），用户粘贴刚复制的命令回车执行。
    container.querySelector<HTMLElement>("#install-open-terminal")
        ?.addEventListener("click", async (e) => {
            e.stopPropagation();
            try {
                await api.openInstallTerminal();
            } catch (err) {
                toast("打开终端失败：" + String(err));
            }
        });
}

/** 工具已安装，但该功能依赖另一种工具的数据结构（如磁盘清理依赖 Claude 的
 *  ~/.claude 关联数据，reasonix 没有等价概念）。这是真实数据模型差异。 */
function renderFeatureUnsupportedPage(container: HTMLElement, featureName: string, tool: ToolName): void {
    container.innerHTML = `
        <div class="empty-state">
            <div class="empty-icon">${icon("info", 28)}</div>
            <div class="empty-title">${featureName}仅支持 Claude Code</div>
            <div class="empty-hint">${featureName}依赖 Claude Code 的数据目录结构。${TOOL_LABELS[tool]} 的项目、会话、归档均已支持。</div>
        </div>
    `;
}

document.getElementById("nav-projects")!.addEventListener("click", showProjects);
document.getElementById("nav-conversations")!.addEventListener("click", showConversations);
document.getElementById("nav-archive")!.addEventListener("click", showArchive);
document.getElementById("nav-cleanup")!.addEventListener("click", showCleanup);

// —— 返回会话列表后的定位辅助 ——

/** 在当前渲染的列表里，按 data-sid 找到会话行并滚动到视区中央。
 *  散落对话 / 项目详情两处复用：它们返回时都是整页重渲染，渲染完调此函数。 */
function scrollToSession(sid: string): void {
    if (!sid) return;
    const row = document.querySelector<HTMLElement>(`.sub-session[data-sid="${CSS.escape(sid)}"]`);
    if (row) {
        // behavior:smooth 在 animations.css 里已设给 .scroll-area；center 让目标行居中。
        row.scrollIntoView({ block: "center", behavior: "smooth" });
        // 轻闪高亮一下，告诉用户"就是这条"（沿用已用的 hover 高亮色，无需新增 CSS）。
        row.classList.add("back-highlight");
        setTimeout(() => row.classList.remove("back-highlight"), 1200);
    }
}

/** 项目卡片内联展开入口返回后用：重新展开某个项目卡片，
 *  再定位到该会话行。模拟用户的展开点击行为，复用其数据拉取逻辑。
 *  v0.4.22 起内联展开的触发点从"点卡片"移到了卡片内的「展开项目」按钮
 *  （.toggle-expand），所以这里派发该按钮的 click 而非卡片 click。 */
async function restoreInlineExpansion(projectPath: string, sid: string): Promise<void> {
    const card = document.querySelector<HTMLElement>(
        `.card.is-expandable[data-path="${CSS.escape(projectPath)}"]`
    );
    if (!card) return;
    // 点「展开项目」按钮展开（若已展开则跳过）。
    if (!card.classList.contains("is-expanded")) {
        const expandBtn = card.querySelector<HTMLElement>(".toggle-expand");
        expandBtn?.click();
        // 等 expand-body 的 max-height 过渡 + 数据拉取完成（异步取会话）。
        // 给一个稍长的等待，保证 DOM 里的会话行已渲染。
        await new Promise((r) => setTimeout(r, 320));
    }
    scrollToSession(sid);
}

void loadInstalledTools().then(() => showProjects());

// ===========================================================================
// Sticky 标题视觉：滚到非顶端时给 .section-label / .nav-bar 加 .is-stuck，
// CSS 里 .is-stuck 触发毛玻璃 + 圆角；常态下背景透明，让最顶端不出现底色横条。
// 实现：MutationObserver 监听 #app 子树，每次出现新的 .scroll-area 就给它
// 装一个 scroll 监听器；scrollTop>0 → 给区内所有 sticky 元素加 .is-stuck。
// view 切换会整块替换 #app 内容，旧监听器随 DOM 移除自动失效，无需手动清理。
// ===========================================================================
function attachStickyWatcher(scroll: HTMLElement): void {
    if (scroll.dataset.stickyBound === "1") return;
    scroll.dataset.stickyBound = "1";
    const update = (): void => {
        const stuck = scroll.scrollTop > 0;
        scroll.querySelectorAll<HTMLElement>(".section-label, .nav-bar").forEach((el) => {
            el.classList.toggle("is-stuck", stuck);
        });
    };
    scroll.addEventListener("scroll", update, { passive: true });
    // 初始也跑一次（重渲染后 scrollTop 可能不是 0，如返回时恢复滚动位置）。
    requestAnimationFrame(update);
}

new MutationObserver(() => {
    document.querySelectorAll<HTMLElement>(".scroll-area").forEach(attachStickyWatcher);
}).observe(app, { childList: true, subtree: true });
