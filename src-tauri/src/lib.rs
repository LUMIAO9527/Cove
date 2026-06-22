// Cove — Claude Code project manager (system-tray popup form)
pub mod archive;
pub mod cleanup;
pub mod commands;
pub mod models;
pub mod paths;
pub mod projects_config;
pub mod related;
pub mod scan;
pub mod settings;
pub mod transcript;

use std::sync::Mutex;
use tauri::{
    Emitter, Manager, PhysicalPosition, Position,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// Popup window logical size (fixed). Height is constrained so that at 175%
/// DPI (this machine) the physical height (580*1.75=1015 + frame ≈ 1029)
/// fits within the work area height (1049 physical px at 175% DPI).
const POPUP_W: f64 = 380.0;
const POPUP_H: f64 = 580.0;

/// Popup visibility state machine.
///
/// Why a state machine: the webview emits spurious `Focused(false)` events
/// during initialization. We can't tell those apart from a real "user clicked
/// away" blur. The state machine resolves this:
///   HIDDEN  --show()-->  SHOWING  --Focused(true)-->  SHOWN  --Focused(false)--> HIDDEN
///
/// - Spurious Focused(false) during init all land while in SHOWING → ignored.
/// - Once the window genuinely gets focus (SHOWN), any focus loss hides it
///   IMMEDIATELY — no debounce, so "click outside to dismiss" feels native.
/// - Safety: if SHOWING never receives Focused(true) within ~3s (e.g. another
///   app grabbed focus), we still hide on the next Focused(false) — armed by
///   the timeout fallback in the event handler.
#[derive(Clone, Copy, Debug, PartialEq)]
enum PopupState {
    Hidden,
    Showing,
    Shown,
}
static POPUP_STATE: Mutex<PopupState> = Mutex::new(PopupState::Hidden);
static SHOW_TIMESTAMP: Mutex<u128> = Mutex::new(0);

/// While true, Focused(false) does NOT dismiss the popup. Set by the frontend
/// when it opens a system dialog (folder picker) — the OS transfers focus to
/// the native dialog, which would otherwise be read as "user clicked away" and
/// wrongly collapse the Cove popup.
static DIALOG_OPEN: Mutex<bool> = Mutex::new(false);

/// Internal setter used by the `set_dialog_open` command (defined in
/// commands.rs, where all other commands live).
pub(crate) fn set_dialog_open_internal(open: bool) {
    if let Ok(mut g) = DIALOG_OPEN.lock() {
        *g = open;
    }
    debug_log(&format!("set_dialog_open({})", open));
}

fn dialog_is_open() -> bool {
    DIALOG_OPEN.lock().map(|g| *g).unwrap_or(false)
}

fn now_ms() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Append a diagnostic line to <exe_dir>/cove-debug.log. Used to trace the
/// tray + show/hide flow in RELEASE builds (where stderr isn't visible).
#[cfg(not(debug_assertions))]
fn debug_log(msg: &str) {
    use std::io::Write;
    let dir = match std::env::current_exe() {
        Ok(p) => match p.parent() {
            Some(d) => d.to_path_buf(),
            None => return,
        },
        Err(_) => return,
    };
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(dir.join("cove-debug.log")) {
        let _ = writeln!(f, "{} {}", now_ms(), msg);
    }
}
#[cfg(debug_assertions)]
fn debug_log(_msg: &str) {}

/// Windows-only (release): ensure this exe's tray icon is "promoted" (shown in
/// the main taskbar tray, not hidden in overflow). Win11 stores per-exe state
/// in HKCU\Control Panel\NotifyIconSettings\<id>\IsPromoted. A freshly built
/// exe defaults to IsPromoted empty = hidden, so the user can't see the icon.
/// We self-promote by finding our entry and setting IsPromoted=1. Implemented
/// via the `reg` CLI (no third-party registry crate needed).
#[cfg(all(windows, not(debug_assertions)))]
fn promote_own_tray_icon() {
    use winreg::enums::*;
    use winreg::RegKey;
    let my_exe = match std::env::current_exe() {
        Ok(p) => p.to_string_lossy().to_lowercase(),
        Err(_) => return,
    };
    debug_log(&format!("promote: my_exe={}", my_exe));
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let parent = match hkcu.open_subkey_with_flags(
        "Control Panel\\NotifyIconSettings",
        KEY_READ | KEY_WRITE,
    ) {
        Ok(k) => k,
        Err(e) => {
            debug_log(&format!("promote: open parent failed: {e}"));
            return;
        }
    };
    let mut promoted = 0u32;
    let mut count = 0u32;
    // enumerate_keys returns an iterator yielding subkey names.
    for subkey_result in parent.enum_keys() {
        let name = match subkey_result {
            Ok(n) => n,
            Err(_) => continue,
        };
        count += 1;
        let sk = match parent.open_subkey_with_flags(&name, KEY_READ | KEY_WRITE) {
            Ok(k) => k,
            Err(_) => continue,
        };
        let exe: Option<String> = sk.get_value("ExecutablePath").ok();
        if let Some(exe) = exe {
            if exe.to_lowercase() == my_exe {
                let cur: Option<u32> = sk.get_value("IsPromoted").ok();
                if cur.unwrap_or(0) == 0 {
                    if sk.set_value("IsPromoted", &1u32).is_ok() {
                        promoted += 1;
                        debug_log(&format!("promote: set IsPromoted=1 for {}", name));
                    }
                }
            }
        }
    }
    debug_log(&format!("promote: scanned {} subkeys, promoted {}", count, promoted));
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn promote_own_tray_icon() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Single-instance: if a second copy is launched, surface the existing
        // window instead of starting a second process.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                show_popup(&w, tray_icon_center_x(app));
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_projects,
            commands::add_project,
            commands::remove_project,
            commands::rename_project,
            commands::get_project_detail,
            commands::get_loose_conversations,
            commands::get_model_info,
            commands::delete_convo,
            commands::archive_convo,
            commands::restore_convo,
            commands::get_archive_index,
            commands::get_archive_conversations,
            commands::purge_archived_convo,
            commands::clear_archive_legacy,
            commands::scan_orphan_data,
            commands::delete_orphan,
            commands::delete_all_orphans,
            commands::open_claude_session,
            commands::open_in_explorer,
            commands::hide_window,
            commands::get_model_state,
            commands::set_default_tier_cmd,
            commands::get_default_workspace,
            commands::set_default_workspace_cmd,
            commands::list_related_files,
            commands::delete_related_files,
            commands::rename_session,
            commands::get_session_transcript,
            commands::set_dialog_open,
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            tauri::WindowEvent::Focused(focused) => {
                if window.label() == "main" {
                    let state = POPUP_STATE.lock().map(|g| *g).unwrap_or(PopupState::Hidden);
                    debug_log(&format!("Focused({}) state={:?}", focused, state));
                    // CRITICAL: when Hidden (popup not shown), ignore ALL focus
                    // events. The webview emits Focused(true)/Focused(false)
                    // noise during window creation BEFORE show_popup ever runs —
                    // honoring those would wrongly arm/trigger a dismiss.
                    if state == PopupState::Hidden {
                        // no-op
                    } else if *focused {
                        // In Showing → graduate to Shown (arm immediate dismiss).
                        if state == PopupState::Showing {
                            if let Ok(mut s) = POPUP_STATE.lock() { *s = PopupState::Shown; }
                            debug_log("  Showing -> Shown (armed dismiss)");
                        }
                        // Abort any in-flight close animation.
                        let _ = window.app_handle().emit("cove-focus-regained", ());
                    } else {
                        // Focus lost.
                        // If the frontend opened a native dialog (folder picker),
                        // the OS steals focus to the dialog — don't treat that
                        // as "click outside", or the popup collapses mid-add.
                        if dialog_is_open() {
                            debug_log("  Focused(false) ignored: dialog open");
                            return;
                        }
                        match state {
                            PopupState::Shown => {
                                // Stable display → dismiss now (close anim first).
                                if let Ok(mut s) = POPUP_STATE.lock() { *s = PopupState::Hidden; }
                                debug_log("  Shown -> dismiss");
                                let _ = window.app_handle().emit("cove-request-close", ());
                            }
                            PopupState::Showing => {
                                // Init grace: the webview emits spurious
                                // Focused(false) events for up to ~1.5s after
                                // show() during initialization. Ignore them and
                                // wait for a genuine Focused(true) → Shown.
                                // (The 1.2s auto-graduate timer set in show_popup
                                // is the fallback if no focus event ever comes.)
                                // no-op
                            }
                            PopupState::Hidden => { /* handled above */ }
                        }
                    }
                }
            }
            _ => {}
        })
        .setup(|app| {
            // === v0.4.26 一次性迁移：清空旧归档区 ===
            // v0.4.25 及之前的 archive_conversation 有致命 bug（评审 P0 #1）：
            // 把 4 个同名 <sid> 目录 move 到同一扁平目标，后者销毁前者，
            // 导致归档过的会话数据残缺无法恢复。新结构（封装目录）从干净
            // 状态开始，旧的残缺归档直接清掉。靠 .archive-v2 marker 防重复。
            // 仅 release 跑（dev 不动用户真实数据）。
            //
            // 时序：先写 marker 再 clear_all（clear_all 会跳过 marker 文件，
            // 所以 marker 跨 clear_all 保留）。
            //   - marker 写成功 + clear_all 成功：迁移完成，下次启动 marker 已存在跳过。
            //   - marker 写成功 + clear_all 失败：marker 已在，下次启动**不重跑**
            //     clear_all——避免清掉用户第一次迁移后归档的新数据（这是关键安全保证）。
            //     残留旧归档用户可手动清。
            //   - marker 写失败（极罕见）：跳过本次 clear_all，下次启动重试。
            //
            // 注意：clear_all 必须跳过 .archive-v2（archive.rs 已实现），否则
            // marker 被清导致每次启动重跑迁移（第二轮评审 Qwen 发现的 bug）。
            #[cfg(not(debug_assertions))]
            {
                let archive_root = crate::paths::archive_dir();
                let marker = archive_root.join(".archive-v2");
                if !marker.exists() {
                    let _ = std::fs::create_dir_all(&archive_root);
                    // 先写 marker（防重复清空的安全闸），再清旧归档。
                    match std::fs::write(&marker, b"v0.4.27 capsule structure\n") {
                        Ok(()) => {
                            debug_log("migration: marker written, clearing legacy archive");
                            if let Err(e) = crate::archive::clear_all(&archive_root) {
                                // clear_all 失败不删 marker——下次启动 marker 已存在
                                // 不会重跑，避免清掉用户新归档数据。残留旧归档用户手动清。
                                debug_log(&format!("migration: clear_all failed (marker kept): {e}"));
                            } else {
                                debug_log("migration: legacy archive cleared");
                            }
                        }
                        Err(e) => {
                            debug_log(&format!("migration: marker write failed, skipping: {e}"));
                        }
                    }
                }
            }

            // Ensure the borderless window gets the Win11 flyout-style drop
            // shadow (DWM CS_DROPSHADOW). Without this, decorations:false
            // windows render flat with no depth.
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_shadow(true);
                // Apply Win11 Mica (system wallpaper blur, composited by DWM —
                // NOT a WebView2 transparency layer, so it avoids the historical
                // CSS backdrop-filter rendering pitfalls). Deep-dark variant to
                // match the Win11 calendar/notification flyout look. Requires
                // `transparent: true` in tauri.conf.json.
                use tauri::window::{Effect, EffectsBuilder};
                let _ = w.set_effects(
                    EffectsBuilder::new().effect(Effect::MicaDark).build(),
                );
                debug_log("setup: shadow + MicaDark applied");
            }

            // Right-click menu
            let show_i = MenuItem::with_id(app, "show", "显示 Cove", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出 Cove", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            // Tray icon — use the 128px high-res version (the old 32px tray.png
            // was too low-res/low-alpha to read clearly on Win11, and on high-DPI
            // displays the system needs a larger source to downscale from).
            let icon_bytes = include_bytes!("../icons/128x128.png");
            let icon = tauri::image::Image::from_bytes(icon_bytes)
                .map_err(|e| format!("failed to load tray icon: {e}"))?;

            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("Cove")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            show_popup(&w, tray_icon_center_x(app));
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        rect,
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        // Use the icon's REAL rect (from Shell_NotifyIconGetRect),
                        // not the cursor `position` — the cursor can be anywhere
                        // after a press-drag-release, but the icon stays put.
                        let app = tray.app_handle();
                        let scale = app.get_webview_window("main")
                            .and_then(|w| w.primary_monitor().ok().flatten())
                            .map(|m| m.scale_factor())
                            .unwrap_or(1.0);
                        let icon_center_x = rect_center_x(&rect, scale);
                        debug_log(&format!("tray click: icon rect {:?} scale={} center_x={}", rect, scale, icon_center_x));
                        if let Some(w) = app.get_webview_window("main") {
                            // 用 POPUP_STATE 而非 is_visible() 判断意图，避免 close
                            // 动画期间的抖动（评审 P2 #9）：
                            //   - state==Shown 且窗口可见 → 真正显示中，关闭
                            //   - state==Hidden 且窗口不可见 → 真正隐藏中，打开
                            //   - 其余（Showing/Closing 中、或 state 与可见性不一致）
                            //     → 正在过渡，忽略本次点击，避免"关→立即重开"抖动。
                            let state = POPUP_STATE.lock().map(|g| *g).unwrap_or(PopupState::Hidden);
                            let visible = w.is_visible().unwrap_or(false);
                            match (state, visible) {
                                (PopupState::Shown, true) => {
                                    debug_log("tray click: Shown+visible -> request close");
                                    if let Ok(mut s) = POPUP_STATE.lock() { *s = PopupState::Hidden; }
                                    let _ = app.emit("cove-request-close", ());
                                }
                                (PopupState::Hidden, false) => {
                                    show_popup(&w, Some(icon_center_x));
                                }
                                _ => {
                                    debug_log(&format!("tray click: ignored (state={:?} visible={})", state, visible));
                                }
                            }
                        }
                    }
                })
                .build(app)?;
            debug_log("tray icon built OK");
            // Self-promote our tray icon so it shows in the main taskbar tray
            // (Win11 hides new exes in overflow by default). Deferred ~1.5s
            // because Windows writes the NotifyIconSettings entry asynchronously
            // after the icon is registered — querying immediately finds nothing.
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(1500));
                promote_own_tray_icon();
            });

            // Auto-show the popup on EVERY launch (delayed ~600ms so the
            // webview finishes its init-time focus jitter, which lands in the
            // Hidden state and is ignored). Previous versions gated this on a
            // `.cove-welcome-seen` marker so only the first-ever launch showed;
            // that meant every subsequent launch looked "broken" (icon but no
            // popup). Windows tray flyouts (calendar, network, etc.) pop on
            // every open — Cove now matches that expectation.
            debug_log("auto-show scheduled on launch");
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(600));
                let inner = app_handle.clone();
                let _ = app_handle.run_on_main_thread(move || {
                    if let Some(w) = inner.get_webview_window("main") {
                        show_popup(&w, tray_icon_center_x(&inner));
                    }
                });
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Cove");
}

/// Show the popup. Position is anchored to the tray icon's REAL on-screen
/// position (`icon_center_x`): horizontally centered on the icon, rising
/// straight up from the taskbar with a gap. `icon_center_x` is None only if
/// we couldn't query the icon rect (rare), in which case we fall back to the
/// work-area bottom-right corner.
///
/// **Why query the icon rect every time**: the previous approach remembered
/// the last click's cursor position in a static (lost on every process
/// restart), and fell back to a hardcoded `workarea_right - 20` on cold start
/// — which was always wrong (the icon isn't at the work-area's right edge).
/// `TrayIcon::rect()` returns the icon's true coordinates via Win32
/// `Shell_NotifyIconGetRect`, so it works on every launch regardless of where
/// the user dragged the icon, and across different machines / DPI / displays.
fn show_popup(w: &tauri::WebviewWindow, icon_center_x: Option<i32>) {
    debug_log(&format!("show_popup: state_before visible={} icon_center_x={:?}", w.is_visible().unwrap_or(false), icon_center_x));
    // Force top-most every time we show — guarantees the popup is above all
    // other windows regardless of z-order churn (matches Win flyout behavior).
    let _ = w.set_always_on_top(true);
    let _ = w.unminimize();

    // === ALL coordinates below are PHYSICAL pixels ===
    //
    // Why physical, not logical: SPI_GETWORKAREA returns the work area in the
    // coordinate space the process actually runs in. This Tauri exe is
    // DPI-aware (the manifest declares per-monitor DPI awareness), so Win32
    // returns PHYSICAL pixels. The previous code wrongly assumed the process
    // was DPI-unaware and treated these physical values as "logical", then
    // handed them to set_position(LogicalPosition) — which Tauri multiplied by
    // scale_factor AGAIN (1.75×), sending the window to ~2190px on a 1836px
    // work area (off-screen / behind the taskbar). That was the root cause of
    // "popup bottom is cut off on the first click".
    //
    // Fix: do every step in physical px. Convert the logical POPUP_W/H to
    // physical via scale_factor, and use PhysicalPosition / PhysicalSize so
    // Tauri applies them verbatim with no extra scaling.

    // Get the scale factor from the PRIMARY MONITOR rather than from the window
    // itself. Rationale: `w.scale_factor()` returns 1.0 while the window is
    // hidden during the auto-show-on-launch path (the webview hasn't been
    // sited on a monitor yet), but `primary_monitor()` always reflects the real
    // DPI. Using a wrong scale here makes phys_w/phys_h way too small, which
    // in turn makes the Y clamp compute a top that's too low — the window then
    // renders at its REAL (larger) physical size and overflows past the
    // taskbar. This was the root cause of "popup position wrong on launch".
    let scale = w
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or_else(|| w.scale_factor().unwrap_or(1.0));
    let phys_w = (POPUP_W * scale).round() as i32;
    // The popup rises from the taskbar with a gap matching the Win11 calendar
    // flyout. 12px reads as the native spacing on this 175% DPI display
    // (≈21 physical); 8px was too tight per user feedback.
    const GAP_PX: i32 = 12;        // gap above the taskbar (physical px)
    const TOP_MARGIN_PX: i32 = 4;  // keep clear of the very top edge

    let wa = match work_area_for_monitor() {
        Some(wa) => {
            debug_log(&format!("show_popup: SPI workarea L={} T={} R={} B={}", wa.left, wa.top, wa.right, wa.bottom));
            wa
        }
        None => {
            debug_log("show_popup: SPI_GETWORKAREA failed, using hard fallback");
            WorkArea { left: 0, top: 0, right: 2880, bottom: 1836 }
        }
    };
    let wa_top = wa.top;
    let wa_bottom = wa.bottom;
    let work_h = (wa_bottom - wa_top - GAP_PX - TOP_MARGIN_PX).max(400);
    let phys_h = (POPUP_H * scale).round() as i32;
    let phys_h = phys_h.min(work_h);   // shrink if work area can't fit full height
    debug_log(&format!("show_popup: scale={} phys_w={} phys_h={} work_h={}", scale, phys_w, phys_h, work_h));

    // PhysicalSize fields are u32; our size math is i32 (clamps can go through
    // 0 for position), so cast only at the boundary.
    let phys_w_u32 = phys_w as u32;
    let phys_h_u32 = phys_h as u32;
    let cur_phys_h = w.outer_size().map(|s| s.height).unwrap_or(0);
    if cur_phys_h != phys_h_u32 {
        let _ = w.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: phys_w_u32,
            height: phys_h_u32,
        }));
        debug_log(&format!("show_popup: resized to {}x{} (phys)", phys_w, phys_h));
    }

    // X: horizontally centered on the tray icon; clamped into the work area.
    // Y: bottom edge sits GAP_PX above the taskbar (work-area bottom), rising
    //    straight up; clamped so the top never goes above wa.top + TOP_MARGIN.
    let (x, y) = match icon_center_x {
        Some(cx) => {
            let x = (cx - phys_w / 2)
                .max(wa.left + 4)
                .min(wa.right - phys_w - 4);
            let bottom = wa_bottom - GAP_PX;
            let y = (bottom - phys_h).max(wa_top + TOP_MARGIN_PX);
            (x, y)
        }
        None => {
            // Couldn't query the tray icon rect (very rare — only if the OS
            // refuses Shell_NotifyIconGetRect). Fall back to the work-area
            // bottom-right corner so the popup is at least fully on-screen.
            let x = (wa.right - phys_w - 12).max(wa.left + 4);
            let bottom = wa_bottom - GAP_PX;
            let y = (bottom - phys_h).max(wa_top + TOP_MARGIN_PX);
            (x, y)
        }
    };
    let _ = w.set_position(Position::Physical(PhysicalPosition::new(x, y)));
    debug_log(&format!("show_popup: set_position({},{}) (phys, pre-show)", x, y));

    let _ = w.show();
    // CRITICAL: re-apply position AFTER show. For a window created with
    // visible:false (the release config), the FIRST show() lets tao apply its
    // default window placement (centered), which OVERWRITES the set_position
    // we did before show. This is why the popup was misplaced on cold launch
    // but fine on tray-click (by then the window had been shown once already).
    // Re-setting position+size post-show guarantees our computed geometry wins.
    let _ = w.set_size(tauri::Size::Physical(tauri::PhysicalSize {
        width: phys_w_u32,
        height: phys_h_u32,
    }));
    let _ = w.set_position(Position::Physical(PhysicalPosition::new(x, y)));
    let _ = w.set_focus();
    // On cold launch, set_focus() alone doesn't bring the window to the
    // foreground (the system denies foreground privilege to a window shown via
    // a delayed callback). Without focus the state machine never sees
    // Focused(true) and gets stuck in `showing`, so clicking elsewhere doesn't
    // dismiss the popup. Force foreground focus via the AttachThreadInput trick.
    #[cfg(windows)]
    {
        if let Ok(hwnd) = w.hwnd() {
            force_foreground_focus(hwnd.0);
            debug_log("show_popup: force_foreground_focus applied");
        }
    }
    // Read back the ACTUAL position/size after show, to confirm our geometry
    // took effect (catches any case where the OS still overrides it).
    let actual_pos = w.outer_position().ok();
    let actual_size = w.outer_size().ok();
    debug_log(&format!(
        "show_popup: AFTER show actual_pos={:?} actual_size={:?}",
        actual_pos.map(|p| (p.x, p.y)),
        actual_size.map(|s| (s.width, s.height))
    ));
    if let Ok(mut s) = POPUP_STATE.lock() { *s = PopupState::Showing; }
    if let Ok(mut t) = SHOW_TIMESTAMP.lock() { *t = now_ms(); }
    let _ = w.app_handle().emit("cove-shown", ());
    debug_log("show_popup: done, state=Showing");

    // Fallback timer: if after 1.2s the popup is STILL in `Showing` (never got
    // Focused(true)), graduate it to `Shown` so it arms dismiss. This covers
    // cold launch where focus stealing failed — without it, the popup would be
    // a "zombie" (visible but clicks-away never close it). force_foreground_focus
    // usually makes this a no-op, but we keep the safety net.
    let app_handle = w.app_handle().clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let still_showing = POPUP_STATE.lock().map(|s| *s == PopupState::Showing).unwrap_or(false);
        if still_showing {
            let inner = app_handle.clone();
            let _ = app_handle.run_on_main_thread(move || {
                let still = POPUP_STATE.lock().map(|s| *s == PopupState::Showing).unwrap_or(false);
                if still {
                    if let Ok(mut s) = POPUP_STATE.lock() { *s = PopupState::Shown; }
                    if let Some(w) = inner.get_webview_window("main") {
                        let _ = w.app_handle().emit("cove-focus-regained", ());
                    }
                    debug_log("show_popup: 1.2s fallback -> Showing auto-graduated to Shown");
                }
            });
        }
    });
}

/// Work area rectangle (excludes the taskbar), in physical pixels.
#[derive(Clone, Copy)]
struct WorkArea {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(windows)]
fn work_area_for_monitor() -> Option<WorkArea> {
    use windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETWORKAREA;
    use windows_sys::Win32::Foundation::RECT;

    let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    // SAFETY: SystemParametersInfoW with SPI_GETWORKAREA writes a RECT into our
    // local buffer. pvParam must point to a RECT for this action, which it does.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rect as *mut _ as *mut _, 0)
    };
    if ok != 0 {
        Some(WorkArea {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        })
    } else {
        None
    }
}

#[cfg(not(windows))]
fn work_area_for_monitor() -> Option<WorkArea> {
    None
}

/// Forcefully bring `hwnd` to the foreground and give it focus.
///
/// Why this exists: on cold launch the popup window is shown via a delayed
/// `run_on_main_thread` callback. By then the system's "foreground window"
/// privilege window has often closed, so `set_focus()` silently fails — the
/// window appears on screen but never receives Focused(true). The popup state
/// machine then gets stuck in `Showing` (it only arms dismiss after a genuine
/// Focused(true)), so clicking elsewhere never dismisses it.
///
/// The robust fix is the standard `AttachThreadInput` trick: temporarily share
/// input state with whatever window currently HAS focus, which grants us
/// SetForegroundWindow permission. Combined with SetFocus this reliably grabs
/// focus even on cold launch. This is the documented workaround for the
/// SetForegroundWindow restriction.
/// Forcefully bring `hwnd` to the foreground and give it focus.
///
/// Why this exists: on cold launch the popup window is shown via a delayed
/// `run_on_main_thread` callback. By then the system's "foreground window"
/// privilege has often closed, so `set_focus()` silently fails — the window
/// appears on screen but never receives Focused(true). The popup state machine
/// then gets stuck in `showing` (it only arms dismiss after a genuine
/// Focused(true)), so clicking elsewhere never dismisses it.
///
/// The robust fix is the standard `AttachThreadInput` trick: temporarily share
/// input state with whatever window currently HAS focus, which grants us
/// SetForegroundWindow permission. This is the documented workaround for the
/// SetForegroundWindow restriction on cold-launched windows.
#[cfg(windows)]
fn force_foreground_focus(hwnd_raw: *mut std::ffi::c_void) {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };

    let fg = unsafe { GetForegroundWindow() };
    let our_thread = unsafe { GetCurrentThreadId() };
    // Attach our thread's input queue to the foreground window's thread. This
    // makes us part of the "foreground" input context, which unlocks
    // SetForegroundWindow (otherwise it's silently blocked on cold launch).
    // Reverted right after we grab focus. (windows-sys BOOL is an i32 alias.)
    let attached = if !fg.is_null() {
        let mut pid = 0u32;
        let fg_thread = unsafe { GetWindowThreadProcessId(fg, &mut pid) };
        if fg_thread != 0 && fg_thread != our_thread {
            unsafe { AttachThreadInput(our_thread, fg_thread, 1) != 0 }
        } else {
            false
        }
    } else {
        false
    };

    unsafe {
        let _ = BringWindowToTop(hwnd_raw);
        let _ = SetForegroundWindow(hwnd_raw);
    }

    if attached && !fg.is_null() {
        let mut pid = 0u32;
        let fg_thread = unsafe { GetWindowThreadProcessId(fg, &mut pid) };
        if fg_thread != 0 && fg_thread != our_thread {
            unsafe { AttachThreadInput(our_thread, fg_thread, 0) };
        }
    }
}

#[cfg(not(windows))]
fn force_foreground_focus(_hwnd_raw: *mut std::ffi::c_void) {}

/// Query the tray icon's REAL on-screen rect and return its horizontal center
/// (physical px). Used by first-run / menu-show / single-instance to place the
/// popup directly above the icon — no hardcoded offsets, no stale memory.
///
/// Uses Tauri's `TrayIcon::rect()`, which on Windows calls
/// `Shell_NotifyIconGetRect` (the official, Win11-compatible API). Returns None
/// if the OS refuses to give us the rect (very rare).
///
/// NOTE: `TrayIcon::rect()` marshals to the main thread internally, so this
/// must be called from the main thread (it is — all our callers run on it:
/// the run_on_main_thread closure, single-instance callback, menu event).
fn tray_icon_center_x(app: &tauri::AppHandle) -> Option<i32> {
    let tray = app.tray_by_id("main-tray")?;
    let rect = tray.rect().ok().flatten()?;
    let scale = app.get_webview_window("main")
        .and_then(|w| w.primary_monitor().ok().flatten())
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let cx = rect_center_x(&rect, scale);
    debug_log(&format!("tray rect query: {:?} scale={} -> center_x={}", rect, scale, cx));
    Some(cx)
}

/// Extract the horizontal center (physical px) of a `tauri::Rect`. The rect's
/// position/size are `Position`/`Size` enums (Physical or Logical variant), so
/// we normalize via `to_physical(scale)`. Tray-icon rects from
/// `Shell_NotifyIconGetRect` are already physical, but we go through
/// `to_physical` for robustness across Tauri versions / DPI states.
fn rect_center_x(rect: &tauri::Rect, scale: f64) -> i32 {
    let pos = rect.position.to_physical::<i32>(scale);
    let size = rect.size.to_physical::<i32>(scale);
    pos.x + size.width / 2
}
