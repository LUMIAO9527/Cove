// Cove 图标系统 — 内联 SVG，Stroke 1.5，24×24 viewBox，currentColor 描边
// Linear/Raycast 风格：线条克制、几何统一。颜色继承父元素 color。

type IconName =
    | "folder"
    | "archive"
    | "broom"
    | "refresh"
    | "back"
    | "warn"
    | "check"
    | "terminal"
    | "play"
    | "trash"
    | "restore"
    | "inbox"
    | "message"
    | "close"
    | "edit"
    | "chevron"
    | "info"
    | "plus"
    | "copy"
    | "grip";

const PATHS: Record<IconName, string> = {
    // 文件夹（项目）
    folder:
        '<path d="M3 7a2 2 0 0 1 2-2h4l2 2.5h8a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/>',
    // 归档箱
    archive:
        '<rect x="3" y="4" width="18" height="4" rx="1"/><path d="M5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8"/><path d="M10 12h4"/>',
    // 扫帚（清理）
    broom:
        '<path d="M14 4l6 6"/><path d="M16 6l-9 9"/><path d="M7 15c-2 1-3 3-3 5h6c0-2-1-4-3-5z"/>',
    // 刷新
    refresh:
        '<path d="M4 12a8 8 0 0 1 13.5-5.8L20 8"/><path d="M20 4v4h-4"/><path d="M20 12a8 8 0 0 1-13.5 5.8L4 16"/><path d="M4 20v-4h4"/>',
    // 返回（左箭头）
    back: '<path d="M15 5l-7 7 7 7"/>',
    // 警告
    warn: '<path d="M12 3l9.5 16.5a1 1 0 0 1-.9 1.5H3.4a1 1 0 0 1-.9-1.5L12 3z"/><path d="M12 10v5"/><circle cx="12" cy="18" r="0.6" fill="currentColor" stroke="none"/>',
    // 勾选
    check: '<path d="M5 12l5 5 9-10"/>',
    // 终端
    terminal:
        '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 10l3 2-3 2"/><path d="M13 14h4"/>',
    // 播放（启动/继续）
    play: '<path d="M7 5l12 7-12 7V5z"/>',
    // 删除
    trash:
        '<path d="M4 7h16"/><path d="M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"/><path d="M6 7l1 13a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-13"/><path d="M10 11v6"/><path d="M14 11v6"/>',
    // 恢复（回收入箱）
    restore:
        '<path d="M3 7a2 2 0 0 1 2-2h4l2 2.5h8a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z"/><path d="M12 11v6"/><path d="M9 14l3-3 3 3"/>',
    // 收件箱（归档区入口）
    inbox:
        '<path d="M3 13l3-8a1 1 0 0 1 1-.7h10a1 1 0 0 1 1 .7L21 13"/><path d="M3 13v5a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-5"/><path d="M3 13h5l1.5 2.5h5L16 13h5"/>',
    // 对话消息（散落对话 Tab）
    message:
        '<path d="M21 11.5a8.5 8.5 0 0 1-12.3 7.6L3 21l1.9-5.7A8.5 8.5 0 1 1 21 11.5z"/>',
    // 关闭 ×
    close: '<path d="M6 6l12 12"/><path d="M18 6L6 18"/>',
    // 编辑/重命名（铅笔）
    edit:
        '<path d="M14.5 5.5l4 4"/><path d="M4 20l1-4L16 5a2 2 0 0 1 3 0v0a2 2 0 0 1 0 3L8 19l-4 1z"/>',
    // 下箭头（展开/收起开关）
    chevron: '<path d="M6 9l6 6 6-6"/>',
    // 信息（圆圈 i，项目详情浮层）
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5"/><circle cx="12" cy="8" r="0.6" fill="currentColor" stroke="none"/>',
    // 加号（新建会话）
    plus: '<path d="M12 5v14"/><path d="M5 12h14"/>',
    // 复制（剪贴板，未安装页"点击复制命令"按钮）
    copy: '<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2"/>',
    // 拖拽手柄（两列各三个圆点，经典 grip 标识，仅项目卡拖拽排序用）
    grip: '<circle cx="9" cy="6" r="1.3" fill="currentColor" stroke="none"/><circle cx="9" cy="12" r="1.3" fill="currentColor" stroke="none"/><circle cx="9" cy="18" r="1.3" fill="currentColor" stroke="none"/><circle cx="15" cy="6" r="1.3" fill="currentColor" stroke="none"/><circle cx="15" cy="12" r="1.3" fill="currentColor" stroke="none"/><circle cx="15" cy="18" r="1.3" fill="currentColor" stroke="none"/>',
};

/** 生成指定图标的 SVG 字符串（可直接插入 innerHTML） */
export function icon(name: IconName, size = 16): string {
    return `<svg class="icon" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${PATHS[name]}</svg>`;
}
