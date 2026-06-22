import "./styles/theme.css";
import "./styles/animations.css";
import { icon } from "./styles/icons";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { renderProjectsView, renderHeader } from "./views/projects";
import { renderConversationsView } from "./views/conversations";
import { renderLooseView } from "./views/loose";
import { renderArchiveView } from "./views/archive";
import { renderCleanupView } from "./views/cleanup";
import { renderSessionDetailView } from "./views/session-detail";
import { Project } from "./api";

const app = document.getElementById("app")!;

// 当前顶层视图的重新渲染函数。每次切换 tab / 进入列表视图时更新，
// 会话详情页（drill-in 子页面，只读历史查看）不更新它——避免重渲染打断阅读。
// cove-shown 事件触发时（用户每次重新打开 Cove 弹窗）会调用它一次，
// 保证"每次打开看到的就是最新的"，不需要用户手动刷新。
let currentView: (() => void) | null = null;

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

// Navigation highlight
function setActiveNav(id: string): void {
    footer.querySelectorAll<HTMLElement>(".btn").forEach((b) => b.classList.remove("is-active"));
    document.getElementById(id)?.classList.add("is-active");
}

// View: projects (root)
async function showProjects(): Promise<void> {
    setActiveNav("nav-projects");
    currentView = () => { void showProjects(); };
    await renderHeader(document.getElementById("titlebar-model")!);
    await renderProjectsView(
        content,
        async (proj: Project) => {
            // 项目详情会话列表入口：返回时回到该项目详情页，定位到该会话。
            // 具名回调避免 onBack 里重新渲染时的递归类型问题。
            const enterConvo = (sid: string, encoded: string, projPath: string, title: string) => {
                showSessionDetail(sid, encoded, projPath, title, async () => {
                    await renderConversationsView(content, proj, () => showProjects(), enterConvo);
                    scrollToSession(sid);
                });
            };
            await renderConversationsView(content, proj, () => showProjects(), enterConvo);
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
    onBack: () => void
): void {
    void renderSessionDetailView(
        content,
        sid,
        projEncoded,
        projPath,
        sessionTitle,
        onBack
    );
}

// View: loose conversations
function showConversations(): void {
    setActiveNav("nav-conversations");
    currentView = () => { showConversations(); };
    void renderHeader(document.getElementById("titlebar-model")!);
    // 散落对话入口：返回时回到散落对话页，并定位到刚才那条会话。
    // 用具名函数避免 onBack 里重新渲染时的递归类型问题。
    const enterSession = (sid: string, encoded: string, projPath: string, title: string) => {
        showSessionDetail(sid, encoded, projPath, title, async () => {
            await renderLooseView(content, enterSession);
            scrollToSession(sid);
        });
    };
    renderLooseView(content, enterSession);
}

// View: archive
function showArchive(): void {
    setActiveNav("nav-archive");
    currentView = () => { showArchive(); };
    void renderHeader(document.getElementById("titlebar-model")!);
    renderArchiveView(content);
}

// View: cleanup
function showCleanup(): void {
    setActiveNav("nav-cleanup");
    currentView = () => { showCleanup(); };
    void renderHeader(document.getElementById("titlebar-model")!);
    renderCleanupView(content);
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

showProjects();
