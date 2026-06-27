// Cove 轻量弹窗 —— 替换原生 confirm()/alert()，保持小窗视觉一致性
import { icon } from "../styles/icons";

export interface ConfirmOptions {
    title: string;
    body: string;
    /** 确认按钮文案，默认 "确认" */
    confirmText?: string;
    /** 取消按钮文案，默认 "取消" */
    cancelText?: string;
    /** 风格：danger 红色确认 / warn 橙色 / accent 蓝色（默认） */
    variant?: "danger" | "warn" | "accent";
    /** 标题图标（默认根据 variant 自动选） */
    titleIcon?: "warn" | "trash" | "terminal" | "restore" | "archive";
}

/**
 * 显示一个确认弹窗，返回 Promise<boolean>（true=确认，false=取消）。
 * 点击背景或按 Esc 视为取消。
 */
export function confirmDialog(opts: ConfirmOptions): Promise<boolean> {
    return new Promise((resolve) => {
        const variant = opts.variant ?? "danger";
        // 默认图标：用户未指定时统一用 warn（之前三元两分支都是 warn，是死代码）。
        const iconName = opts.titleIcon ?? "warn";
        const confirmText = opts.confirmText ?? "确认";
        const cancelText = opts.cancelText ?? "取消";

        const backdrop = document.createElement("div");
        backdrop.className = "modal-backdrop";
        backdrop.innerHTML = `
            <div class="modal" role="dialog" aria-modal="true">
                <div class="modal-title ${variant === "danger" ? "is-danger" : ""}">
                    ${icon(iconName, 16)} ${escapeText(opts.title)}
                </div>
                <div class="modal-body">${opts.body}</div>
                <div class="modal-actions">
                    <button class="btn" data-act="cancel">${escapeText(cancelText)}</button>
                    <button class="btn btn-${variant}" data-act="confirm">${escapeText(confirmText)}</button>
                </div>
            </div>`;

        let done = false;
        const cleanup = (result: boolean) => {
            if (done) return;
            done = true;
            // 对称退出动画：背景淡出后再移除，避免硬切。
            backdrop.classList.add("leaving");
            const finish = () => {
                backdrop.remove();
                document.removeEventListener("keydown", onKey);
                resolve(result);
            };
            backdrop.addEventListener("animationend", finish, { once: true });
            // 兜底：若 animationend 因故不触发，150ms 后强制收尾。
            setTimeout(finish, 200);
        };
        const onKey = (e: KeyboardEvent) => {
            if (e.key === "Escape") cleanup(false);
            if (e.key === "Enter") cleanup(true);
        };

        backdrop.addEventListener("click", (e) => {
            if (e.target === backdrop) cleanup(false);
        });
        backdrop.querySelector<HTMLButtonElement>(`[data-act="cancel"]`)!.addEventListener("click", () => cleanup(false));
        backdrop.querySelector<HTMLButtonElement>(`[data-act="confirm"]`)!.addEventListener("click", () => cleanup(true));
        document.addEventListener("keydown", onKey);

        document.body.appendChild(backdrop);
        // 聚焦确认按钮，方便回车确认
        requestAnimationFrame(() => {
            backdrop.querySelector<HTMLButtonElement>(`[data-act="confirm"]`)?.focus();
        });
    });
}

/** 显示一个短暂的提示（替代 alert），2.2 秒后淡出再移除（进出动画对称）。 */
export function toast(message: string): void {
    const el = document.createElement("div");
    el.className = "toast";
    el.textContent = message;
    document.body.appendChild(el);
    setTimeout(() => {
        el.classList.add("leaving");
        el.addEventListener("animationend", () => el.remove(), { once: true });
    }, 2200);
}

export interface PromptOptions {
    title: string;
    body?: string;
    placeholder?: string;
    /** 确认按钮文案，默认 "确认" */
    confirmText?: string;
    cancelText?: string;
    /** 初始值 */
    initialValue?: string;
}

/** 显示一个带单行输入框的弹窗，返回用户输入的字符串（取消则返回 null）。 */
export function promptDialog(opts: PromptOptions): Promise<string | null> {
    return new Promise((resolve) => {
        const confirmText = opts.confirmText ?? "确认";
        const cancelText = opts.cancelText ?? "取消";
        const placeholder = opts.placeholder ?? "";
        const initial = opts.initialValue ?? "";

        const backdrop = document.createElement("div");
        backdrop.className = "modal-backdrop";
        backdrop.innerHTML = `
            <div class="modal" role="dialog" aria-modal="true">
                <div class="modal-title">${escapeText(opts.title)}</div>
                ${opts.body ? `<div class="modal-body">${opts.body}</div>` : ""}
                <input class="modal-input" type="text" placeholder="${escapeText(placeholder)}" value="${escapeText(initial)}" />
                <div class="modal-actions">
                    <button class="btn" data-act="cancel">${escapeText(cancelText)}</button>
                    <button class="btn btn-accent" data-act="confirm">${escapeText(confirmText)}</button>
                </div>
            </div>`;

        let done = false;
        const input = backdrop.querySelector<HTMLInputElement>(".modal-input")!;
        const cleanup = (result: string | null) => {
            if (done) return;
            done = true;
            backdrop.classList.add("leaving");
            const finish = () => {
                backdrop.remove();
                document.removeEventListener("keydown", onKey);
                resolve(result);
            };
            backdrop.addEventListener("animationend", finish, { once: true });
            setTimeout(finish, 200);
        };
        const onKey = (e: KeyboardEvent) => {
            if (e.key === "Escape") cleanup(null);
            if (e.key === "Enter") cleanup(input.value);
        };

        backdrop.addEventListener("click", (e) => {
            if (e.target === backdrop) cleanup(null);
        });
        backdrop.querySelector<HTMLButtonElement>(`[data-act="cancel"]`)!.addEventListener("click", () => cleanup(null));
        backdrop.querySelector<HTMLButtonElement>(`[data-act="confirm"]`)!.addEventListener("click", () => cleanup(input.value));
        input.addEventListener("keydown", (e) => {
            if (e.key === "Enter") e.preventDefault();
        });
        document.addEventListener("keydown", onKey);

        document.body.appendChild(backdrop);
        requestAnimationFrame(() => input.focus());
    });
}

function escapeText(s: string): string {
    return s
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#39;");
}

export interface SelectItem {
    label: string;
    path: string;
    sizeBytes: number;
    /** If false, the checkbox is disabled (e.g. shared file can't be wholly deleted). */
    infoOnly?: boolean;
}

/**
 * 显示一个带复选框列表的选择弹窗，返回被勾选项的 path 数组。
 * 默认全选；用户可取消勾选某些项。返回 null 表示整体取消。
 */
export function selectDialog(opts: {
    title: string;
    body?: string;
    items: SelectItem[];
    confirmText?: string;
    variant?: "danger" | "warn" | "accent";
    titleIcon?: "warn" | "trash";
    formatSize?: (b: number) => string;
}): Promise<string[] | null> {
    return new Promise((resolve) => {
        const variant = opts.variant ?? "danger";
        const iconName = opts.titleIcon ?? "trash";
        const fmt = opts.formatSize ?? ((b: number) => (b < 1024 ? b + " B" : (b / 1024).toFixed(1) + " KB"));
        const itemsHtml = opts.items
            .map((it, i) => {
                const checked = it.infoOnly ? "" : "checked";
                const disabled = it.infoOnly ? "disabled" : "";
                return `
                <label class="select-item">
                    <input type="checkbox" data-i="${i}" ${checked} ${disabled} />
                    <span class="select-item-label">${escapeText(it.label)}</span>
                    <span class="select-item-size">${escapeText(fmt(it.sizeBytes))}</span>
                </label>`;
            })
            .join("");

        const backdrop = document.createElement("div");
        backdrop.className = "modal-backdrop";
        backdrop.innerHTML = `
            <div class="modal select-modal" role="dialog" aria-modal="true">
                <div class="modal-title ${variant === "danger" ? "is-danger" : ""}">
                    ${icon(iconName, 16)} ${escapeText(opts.title)}
                </div>
                ${opts.body ? `<div class="modal-body">${opts.body}</div>` : ""}
                <div class="select-list">${itemsHtml}</div>
                <div class="modal-actions">
                    <button class="btn" data-act="cancel">取消</button>
                    <button class="btn btn-${variant}" data-act="confirm">${escapeText(opts.confirmText ?? "删除")}</button>
                </div>
            </div>`;

        let done = false;
        const cleanup = (result: string[] | null) => {
            if (done) return;
            done = true;
            backdrop.classList.add("leaving");
            const finish = () => {
                backdrop.remove();
                document.removeEventListener("keydown", onKey);
                resolve(result);
            };
            backdrop.addEventListener("animationend", finish, { once: true });
            setTimeout(finish, 200);
        };
        const collect = (): string[] => {
            const paths: string[] = [];
            backdrop.querySelectorAll<HTMLInputElement>("input[type=checkbox]").forEach((cb) => {
                if (cb.checked && !cb.disabled) {
                    const i = parseInt(cb.dataset.i || "0", 10);
                    paths.push(opts.items[i].path);
                }
            });
            return paths;
        };
        const onKey = (e: KeyboardEvent) => {
            if (e.key === "Escape") cleanup(null);
            if (e.key === "Enter") cleanup(collect());
        };
        backdrop.addEventListener("click", (e) => {
            if (e.target === backdrop) cleanup(null);
        });
        backdrop.querySelector<HTMLButtonElement>(`[data-act="cancel"]`)!.addEventListener("click", () => cleanup(null));
        backdrop.querySelector<HTMLButtonElement>(`[data-act="confirm"]`)!.addEventListener("click", () => cleanup(collect()));
        document.addEventListener("keydown", onKey);
        document.body.appendChild(backdrop);
    });
}
