use crate::platform;
use crate::state::{AppState, HotkeyBinding};
use log::{error, info};
use std::ffi::c_void;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

type CFMachPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CGEventRef = *mut c_void;

const K_CG_SESSION_EVENT_TAP: u32 = 1;
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;
const K_CG_EVENT_MOUSE_MOVED: u64 = 5;
const K_CG_EVENT_KEY_DOWN: u64 = 10;
const K_CG_EVENT_KEY_UP: u64 = 11;
const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

const FLAG_CMD: u64 = 0x00100000;
const FLAG_ALT: u64 = 0x00080000;
const FLAG_SHIFT: u64 = 0x00020000;
const FLAG_CTRL: u64 = 0x00040000;

// Radial geometry — must match RadialSelector.tsx constants
const OUTER_R: f64 = 124.0;
const INNER_R: f64 = 34.0;
// Radial overlay window size (logical px); center = WIN_SIZE / 2
const RADIAL_WIN_SIZE: f64 = 350.0;
const RADIAL_WIN_CX: f64 = RADIAL_WIN_SIZE / 2.0;

#[repr(C)]
#[derive(Copy, Clone)]
struct CGPoint {
    x: f64,
    y: f64,
}

type CGEventTapCallBack = extern "C" fn(
    proxy: *mut c_void,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: CFMachPortRef,
        order: i64,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: *const c_void);
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopRun();
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
}

extern "C" {
    static kCFRunLoopCommonModes: *const c_void;
}

struct HotkeyContext {
    state: Arc<AppState>,
    handle: AppHandle,
    last_hover_seg: std::sync::atomic::AtomicI32,
}

fn js_code_to_mac_keycode(code: &str) -> Option<u16> {
    match code {
        "KeyA" => Some(0x00),
        "KeyS" => Some(0x01),
        "KeyD" => Some(0x02),
        "KeyF" => Some(0x03),
        "KeyH" => Some(0x04),
        "KeyG" => Some(0x05),
        "KeyZ" => Some(0x06),
        "KeyX" => Some(0x07),
        "KeyC" => Some(0x08),
        "KeyV" => Some(0x09),
        "KeyB" => Some(0x0B),
        "KeyQ" => Some(0x0C),
        "KeyW" => Some(0x0D),
        "KeyE" => Some(0x0E),
        "KeyR" => Some(0x0F),
        "KeyY" => Some(0x10),
        "KeyT" => Some(0x11),
        "KeyU" => Some(0x20),
        "KeyI" => Some(0x22),
        "KeyO" => Some(0x1F),
        "KeyP" => Some(0x23),
        "KeyL" => Some(0x25),
        "KeyJ" => Some(0x26),
        "KeyK" => Some(0x28),
        "KeyN" => Some(0x2D),
        "KeyM" => Some(0x2E),
        "Digit1" => Some(0x12),
        "Digit2" => Some(0x13),
        "Digit3" => Some(0x14),
        "Digit4" => Some(0x15),
        "Digit5" => Some(0x17),
        "Digit6" => Some(0x16),
        "Digit7" => Some(0x1A),
        "Digit8" => Some(0x1C),
        "Digit9" => Some(0x19),
        "Digit0" => Some(0x1D),
        "Space" => Some(0x31),
        "Tab" => Some(0x30),
        "F1" => Some(0x7A),
        "F2" => Some(0x78),
        "F3" => Some(0x63),
        "F4" => Some(0x76),
        "F5" => Some(0x60),
        "F6" => Some(0x61),
        "F7" => Some(0x62),
        "F8" => Some(0x64),
        "F9" => Some(0x65),
        "F10" => Some(0x6D),
        "F11" => Some(0x67),
        "F12" => Some(0x6F),
        _ => None,
    }
}

fn matches_binding(keycode: u16, flags: u64, binding: &HotkeyBinding) -> bool {
    let expected = match js_code_to_mac_keycode(&binding.key) {
        Some(k) => k,
        None => return false,
    };
    if keycode != expected {
        return false;
    }
    let has_cmd = flags & FLAG_CMD != 0;
    let has_alt = flags & FLAG_ALT != 0;
    let has_shift = flags & FLAG_SHIFT != 0;
    let has_ctrl = flags & FLAG_CTRL != 0;
    has_cmd == binding.cmd && has_alt == binding.alt && has_shift == binding.shift && has_ctrl == binding.ctrl
}

/// Compute the account segment index under the cursor, or None if not on the wheel.
fn radial_segment_at(cursor_x: f64, cursor_y: f64, center_x: f64, center_y: f64, n: usize) -> Option<usize> {
    if n == 0 { return None; }
    let dx = cursor_x - center_x;
    let dy = cursor_y - center_y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < INNER_R || dist > OUTER_R { return None; }
    let mut angle = dy.atan2(dx) + std::f64::consts::PI / 2.0;
    if angle < 0.0 { angle += 2.0 * std::f64::consts::PI; }
    if angle >= 2.0 * std::f64::consts::PI { angle -= 2.0 * std::f64::consts::PI; }
    Some((angle / (2.0 * std::f64::consts::PI) * n as f64).floor() as usize % n)
}

extern "C" fn hotkey_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    if event.is_null() || user_info.is_null() {
        return event;
    }
    let ctx = unsafe { &*(user_info as *const HotkeyContext) };

    // Mouse moved: update hover segment while radial is open
    if event_type == K_CG_EVENT_MOUSE_MOVED as u32 {
        use std::sync::atomic::Ordering;
        if ctx.state.radial_open.load(Ordering::Relaxed) {
            if let Some(keydown) = ctx.state.get_radial_center() {
                let cursor = unsafe { CGEventGetLocation(event) };
                let accounts = ctx.state.get_account_views();
                let n = accounts.len();
                let rel_x = RADIAL_WIN_CX + (cursor.x - keydown.0);
                let rel_y = RADIAL_WIN_CX + (cursor.y - keydown.1);
                let seg = radial_segment_at(rel_x, rel_y, RADIAL_WIN_CX, RADIAL_WIN_CX, n)
                    .map(|s| s as i32)
                    .unwrap_or(-1);
                let prev = ctx.last_hover_seg.swap(seg, Ordering::Relaxed);
                if seg != prev {
                    let h = ctx.handle.clone();
                    let _ = h.clone().run_on_main_thread(move || {
                        if let Some(w) = h.get_webview_window("radial-overlay") {
                            let _ = w.eval(&format!("window.__radialHover&&window.__radialHover({})", seg));
                        }
                    });
                }
            }
        }
        return event;
    }

    // Keyup: if radial is open and key matches, compute selection and hide
    if event_type == K_CG_EVENT_KEY_UP as u32 {
        use std::sync::atomic::Ordering;
        if ctx.state.radial_open.load(Ordering::Relaxed) {
            let keycode = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u16;
            let hotkeys = ctx.state.get_hotkeys();
            for binding in &hotkeys {
                if binding.action == "radial" && !binding.key.is_empty() {
                    if let Some(expected) = js_code_to_mac_keycode(&binding.key) {
                        if keycode == expected {
                                        ctx.state.radial_open.store(false, Ordering::Relaxed);
                            // Get cursor position before entering main thread (event may be freed after)
                            let cursor_now = unsafe { CGEventGetLocation(event) };
                            let h = ctx.handle.clone();
                            let state_ref = ctx.state.clone();
                            let _ = h.clone().run_on_main_thread(move || {
                                use tauri_nspanel::ManagerExt as NSPanelManagerExt;

                                // Compute hovered account: delta from keydown cursor position
                                let selected_name = state_ref.get_radial_center().and_then(|keydown| {
                                    let accounts = state_ref.get_account_views();
                                    let n = accounts.len();
                                    if n == 0 { return None; }
                                    let rel_x = RADIAL_WIN_CX + (cursor_now.x - keydown.0);
                                    let rel_y = RADIAL_WIN_CX + (cursor_now.y - keydown.1);
                                    let seg = radial_segment_at(rel_x, rel_y, RADIAL_WIN_CX, RADIAL_WIN_CX, n)?;
                                    Some(accounts[seg].character_name.clone())
                                });

                                // Hide panel
                                if let Some(w) = h.get_webview_window("radial-overlay") {
                                    let _ = w.eval("window.__radialHide&&window.__radialHide()");
                                    if let Ok(panel) = h.get_webview_panel("radial-overlay") {
                                        panel.order_out(None);
                                    } else {
                                        let _ = w.hide();
                                    }
                                }

                                // Focus selected account or restore current window
                                let h2 = h.clone();
                                let state2 = state_ref.clone();
                                std::thread::spawn(move || {
                                    let wm = crate::platform::create_window_manager();
                                    if let Some(name) = selected_name {
                                        let windows = wm.list_dofus_windows();
                                        if let Some(win) = windows.iter().find(|w| {
                                            w.character_name.eq_ignore_ascii_case(&name)
                                        }) {
                                            let _ = wm.focus_window(win);
                                            state2.set_current_by_name(&name);
                                            let _ = h2.emit("focus-changed", &name);
                                        }
                                    } else if let Some(win) = state2.get_current_window() {
                                        let _ = wm.focus_window(&win);
                                    }
                                });
                            });
                        }
                    }
                }
            }
        }
        return event;
    }

    let keycode = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u16;
    let flags = unsafe { CGEventGetFlags(event) };
    let hotkeys = ctx.state.get_hotkeys();

    for binding in &hotkeys {
        if matches_binding(keycode, flags, binding) {
            // Radial: check guard first (no expensive calls before this)
            if binding.action == "radial" {
                use std::sync::atomic::Ordering;
                if ctx.state.radial_open.load(Ordering::Relaxed) {
                    break; // key-repeat guard
                }
                ctx.state.radial_open.store(true, Ordering::Relaxed);
                ctx.last_hover_seg.store(-1, Ordering::Relaxed);
                // CGEventGetLocation returns screen logical coordinates (points) — works on all monitors
                let cursor = unsafe { CGEventGetLocation(event) };
                let h = ctx.handle.clone();
                let state_ref = ctx.state.clone();
                let _ = h.clone().run_on_main_thread(move || {
                    use tauri_nspanel::ManagerExt as NSPanelManagerExt;
                    if let Some(w) = h.get_webview_window("radial-overlay") {
                        // Position small window centered on cursor, then show — no cross-display spanning
                        let win_x = cursor.x - RADIAL_WIN_CX;
                        let win_y = cursor.y - RADIAL_WIN_CX;
                        let _ = w.set_size(tauri::LogicalSize::new(RADIAL_WIN_SIZE, RADIAL_WIN_SIZE));
                        let _ = w.set_position(tauri::LogicalPosition::new(win_x, win_y));
                        if let Ok(panel) = h.get_webview_panel("radial-overlay") {
                            panel.order_front_regardless();
                        } else {
                            let _ = w.show();
                        }
                        // Store keydown cursor (screen logical) for segment detection on keyup
                        state_ref.set_radial_center(cursor.x, cursor.y);
                        let theme = state_ref.get_theme();
                        let _ = w.eval(&format!("window.__radialShow({:.2},{:.2},'{}')", RADIAL_WIN_CX, RADIAL_WIN_CX, theme));
                    }
                });
                break;
            }

            // Sync foreground window for cycle/principal actions (not radial — window server call is slow)
            let fg_id = platform::get_foreground_window_id();
            if fg_id != 0 {
                ctx.state.sync_current_from_window_id(fg_id);
            }

            let win = match binding.action.as_str() {
                "next" => ctx.state.cycle_next(),
                "prev" => ctx.state.cycle_prev(),
                "principal" => ctx.state.get_principal(),
                _ => continue,
            };

            if let Some(win) = win {
                let wm = platform::create_window_manager();
                let _ = wm.focus_window(&win);
                let handle = ctx.handle.clone();
                let name = win.character_name.clone();
                std::thread::spawn(move || {
                    let _ = handle.emit("focus-changed", &name);
                });
            }
            break;
        }
    }

    event
}

pub fn start_hotkey_listener(handle: AppHandle, state: Arc<AppState>) {
    let ctx = Box::new(HotkeyContext {
        state,
        handle,
        last_hover_seg: std::sync::atomic::AtomicI32::new(-1),
    });
    let ctx_addr = Box::into_raw(ctx) as usize;

    std::thread::spawn(move || unsafe {
        let user_info = ctx_addr as *mut c_void;
        let events_mask: u64 = (1 << K_CG_EVENT_MOUSE_MOVED) | (1 << K_CG_EVENT_KEY_DOWN) | (1 << K_CG_EVENT_KEY_UP);

        let tap = CGEventTapCreate(
            K_CG_SESSION_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            K_CG_EVENT_TAP_OPTION_LISTEN_ONLY,
            events_mask,
            hotkey_callback,
            user_info,
        );

        if tap.is_null() {
            error!("[Hotkeys] Failed to create CGEventTap — check Accessibility permission");
            return;
        }

        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);

        info!("[Hotkeys] CGEventTap started — listening for global hotkeys");
        CFRunLoopRun();
    });
}
