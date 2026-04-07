use crate::platform;
use crate::radial::{
    focus_selected_or_current, radial_outer_r, radial_segment_at, radial_win_size,
    resolve_selection, INNER_R,
};
use crate::state::{AppState, HotkeyBinding};
use log::{error, info};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetCursorPos, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP,
    WM_MOUSEMOVE, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

struct HotkeyContext {
    state: Arc<AppState>,
    handle: AppHandle,
    last_hover_seg: Arc<std::sync::atomic::AtomicI32>,
    scale: Arc<parking_lot::Mutex<f64>>,
}

thread_local! {
    static HOTKEY_CTX: std::cell::RefCell<Option<HotkeyContext>> =
        const { std::cell::RefCell::new(None) };
}

/// Scan codes for letter keys (Key*).
/// These are PS/2 set-1 scan codes — always physical-position-based,
/// independent of the active keyboard layout (AZERTY, QWERTZ, etc.).
fn js_code_to_scan(code: &str) -> Option<u32> {
    match code {
        "KeyQ" => Some(0x10),
        "KeyW" => Some(0x11),
        "KeyE" => Some(0x12),
        "KeyR" => Some(0x13),
        "KeyT" => Some(0x14),
        "KeyY" => Some(0x15),
        "KeyU" => Some(0x16),
        "KeyI" => Some(0x17),
        "KeyO" => Some(0x18),
        "KeyP" => Some(0x19),
        "KeyA" => Some(0x1E),
        "KeyS" => Some(0x1F),
        "KeyD" => Some(0x20),
        "KeyF" => Some(0x21),
        "KeyG" => Some(0x22),
        "KeyH" => Some(0x23),
        "KeyJ" => Some(0x24),
        "KeyK" => Some(0x25),
        "KeyL" => Some(0x26),
        "KeyZ" => Some(0x2C),
        "KeyX" => Some(0x2D),
        "KeyC" => Some(0x2E),
        "KeyV" => Some(0x2F),
        "KeyB" => Some(0x30),
        "KeyN" => Some(0x31),
        "KeyM" => Some(0x32),
        _ => None,
    }
}

fn js_code_to_vk(code: &str) -> Option<u16> {
    match code {
        // Letters A-Z are handled via scan codes (js_code_to_scan) — not here.
        // Digits 0–9 → 0x30–0x39
        "Digit0" => Some(0x30),
        "Digit1" => Some(0x31),
        "Digit2" => Some(0x32),
        "Digit3" => Some(0x33),
        "Digit4" => Some(0x34),
        "Digit5" => Some(0x35),
        "Digit6" => Some(0x36),
        "Digit7" => Some(0x37),
        "Digit8" => Some(0x38),
        "Digit9" => Some(0x39),
        // Function keys F1–F12 → 0x70–0x7B
        "F1" => Some(0x70),
        "F2" => Some(0x71),
        "F3" => Some(0x72),
        "F4" => Some(0x73),
        "F5" => Some(0x74),
        "F6" => Some(0x75),
        "F7" => Some(0x76),
        "F8" => Some(0x77),
        "F9" => Some(0x78),
        "F10" => Some(0x79),
        "F11" => Some(0x7A),
        "F12" => Some(0x7B),
        // Special keys
        "Space" => Some(0x20),
        "Tab" => Some(0x09),
        "ArrowLeft" => Some(0x25),
        "ArrowUp" => Some(0x26),
        "ArrowRight" => Some(0x27),
        "ArrowDown" => Some(0x28),
        // Numpad digits → VK_NUMPAD0–VK_NUMPAD9 (0x60–0x69)
        "Numpad0" => Some(0x60),
        "Numpad1" => Some(0x61),
        "Numpad2" => Some(0x62),
        "Numpad3" => Some(0x63),
        "Numpad4" => Some(0x64),
        "Numpad5" => Some(0x65),
        "Numpad6" => Some(0x66),
        "Numpad7" => Some(0x67),
        "Numpad8" => Some(0x68),
        "Numpad9" => Some(0x69),
        // Numpad operators
        "NumpadMultiply" => Some(0x6A),
        "NumpadAdd" => Some(0x6B),
        "NumpadSubtract" => Some(0x6D),
        "NumpadDecimal" => Some(0x6E),
        "NumpadDivide" => Some(0x6F),
        _ => None,
    }
}

fn read_modifiers() -> (bool, bool, bool, bool) {
    unsafe {
        let shift = (GetKeyState(VK_SHIFT.0 as i32) as u16) & 0x8000 != 0;
        let ctrl = (GetKeyState(VK_CONTROL.0 as i32) as u16) & 0x8000 != 0;
        let alt = (GetKeyState(VK_MENU.0 as i32) as u16) & 0x8000 != 0;
        let cmd = ((GetKeyState(VK_LWIN.0 as i32) as u16) | (GetKeyState(VK_RWIN.0 as i32) as u16))
            & 0x8000
            != 0;
        (shift, ctrl, alt, cmd)
    }
}

/// Returns true if the event should be consumed (not forwarded to other apps).
fn fire_action(action: &str, c: &HotkeyContext) -> bool {
    if action == "radial" {
        use std::sync::atomic::Ordering;
        if c.state.radial_open.load(Ordering::Acquire) {
            return false; // guard against key-repeat, don't consume
        }
        c.state.radial_open.store(true, Ordering::Release);
        c.last_hover_seg.store(-1, Ordering::Relaxed);

        // Capture physical cursor position NOW on the hook thread, before
        // crossing to the main thread where the cursor may have moved.
        let mut pt = POINT { x: 0, y: 0 };
        unsafe {
            let _ = GetCursorPos(&mut pt);
        }
        let phys_x = pt.x as f64;
        let phys_y = pt.y as f64;
        let n_accounts = c.state.get_account_views().len();
        let win_size = radial_win_size(n_accounts);
        let win_cx = win_size / 2.0;

        let h = c.handle.clone();
        let state_ref = c.state.clone();
        let scale_arc = c.scale.clone();

        let _ = h.clone().run_on_main_thread(move || {
            if let Some(w) = h.get_webview_window("radial-overlay") {
                let scale = w.scale_factor().unwrap_or(1.0);
                // Cache scale so mouse_callback can convert physical→logical
                *scale_arc.lock() = scale;

                // Store logical cursor as radial center for segment math
                state_ref.set_radial_center(phys_x / scale, phys_y / scale);

                // Position window centered on cursor (physical pixel coords)
                let _ = w.set_size(tauri::LogicalSize::new(win_size, win_size));
                let win_x = (phys_x - win_cx * scale) as i32;
                let win_y = (phys_y - win_cx * scale) as i32;
                let _ = w.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                    x: win_x,
                    y: win_y,
                }));
                let _ = w.set_ignore_cursor_events(true);
                let _ = w.show();
                // NO set_focus() — keep focus on game window

                let theme = state_ref.get_theme();
                let _ = w.eval(format!(
                    "window.__radialShow&&window.__radialShow({:.2},{:.2},'{}')",
                    win_cx, win_cx, theme
                ));
            }
        });
        return false;
    }

    // Sync current index from the actual foreground window before cycling,
    // so next/prev always starts from wherever focus currently is.
    let (fg_id, fg_pid) = platform::get_foreground_info();
    if fg_id != 0 {
        c.state.sync_current_from_window_id(fg_id);
    }

    // Focus gate: if enabled, only fire when Dofus or FocusRetro is the active app.
    if c.state.is_hotkeys_focused_only() {
        let ids = c.state.get_account_window_ids();
        let is_dofus = fg_id != 0 && ids.contains(&fg_id);
        let is_app = fg_pid != 0 && fg_pid == std::process::id();
        if !is_dofus && !is_app {
            return false; // skip, don't consume
        }
    }

    let win = match action {
        "next" => c.state.cycle_next(),
        "prev" => c.state.cycle_prev(),
        "principal" => c.state.get_principal(),
        _ => return false,
    };
    if let Some(win) = win {
        let wm = platform::create_window_manager();
        let _ = wm.focus_window(&win);
        let handle = c.handle.clone();
        let name = win.character_name.clone();
        std::thread::spawn(move || {
            let _ = handle.emit("focus-changed", &name);
        });
    }
    c.state.is_hotkeys_consume()
}

/// Called from a spawned thread to hide the radial wheel and focus the selected account.
fn close_radial(
    h: AppHandle,
    state_ref: Arc<AppState>,
    scale_arc: Arc<parking_lot::Mutex<f64>>,
    phys_x: f64,
    phys_y: f64,
) {
    let scale = *scale_arc.lock();
    let selected = resolve_selection(&state_ref, phys_x / scale, phys_y / scale);

    if let Some(w) = h.get_webview_window("radial-overlay") {
        let _ = w.eval("window.__radialHide&&window.__radialHide()");
        let _ = w.hide();
    }

    focus_selected_or_current(h, state_ref, selected);
}

fn matches_keyboard_binding(
    vk: u16,
    scan: u32,
    shift: bool,
    ctrl: bool,
    alt: bool,
    cmd: bool,
    binding: &HotkeyBinding,
) -> bool {
    // Letter keys (Key*): compare by physical scan code so the binding fires for
    // the same physical key regardless of the active layout (AZERTY, QWERTZ, …).
    // WH_KEYBOARD_LL vkCode for letters is layout-dependent, so it cannot be used.
    let key_matches = if binding.key.starts_with("Key") {
        js_code_to_scan(&binding.key) == Some(scan)
    } else {
        js_code_to_vk(&binding.key) == Some(vk)
    };
    key_matches
        && shift == binding.shift
        && ctrl == binding.ctrl
        && alt == binding.alt
        && cmd == binding.cmd
}

fn matches_mouse_binding(
    button: &str,
    shift: bool,
    ctrl: bool,
    alt: bool,
    cmd: bool,
    binding: &HotkeyBinding,
) -> bool {
    binding.key == button
        && shift == binding.shift
        && ctrl == binding.ctrl
        && alt == binding.alt
        && cmd == binding.cmd
}

unsafe extern "system" fn hotkey_callback(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if ncode < 0 {
        return CallNextHookEx(None, ncode, wparam, lparam);
    }

    let msg_id = wparam.0 as u32;

    // On keyup: if the radial is open and the released key matches the radial binding, hide it
    if msg_id == WM_KEYUP || msg_id == WM_SYSKEYUP {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = kb.vkCode as u16;
        let scan = kb.scanCode;
        HOTKEY_CTX.with(|ctx| {
            if let Some(ref c) = *ctx.borrow() {
                use std::sync::atomic::Ordering;
                if c.state.radial_open.load(Ordering::Acquire) {
                    let hotkeys = c.state.get_hotkeys();
                    for binding in &hotkeys {
                        if binding.action == "radial" && !binding.key.is_empty() {
                            let key_match = if binding.key.starts_with("Key") {
                                js_code_to_scan(&binding.key) == Some(scan)
                            } else {
                                js_code_to_vk(&binding.key) == Some(vk)
                            };
                            if key_match {
                                c.state.radial_open.store(false, Ordering::Release);

                                // Capture physical cursor at key-release moment
                                let mut pt = POINT { x: 0, y: 0 };
                                let _ = GetCursorPos(&mut pt);
                                let phys_x = pt.x as f64;
                                let phys_y = pt.y as f64;

                                let h = c.handle.clone();
                                let state_ref = c.state.clone();
                                let scale_arc = c.scale.clone();

                                std::thread::spawn(move || {
                                    close_radial(h, state_ref, scale_arc, phys_x, phys_y);
                                });
                            }
                        }
                    }
                }
            }
        });
        return CallNextHookEx(None, ncode, wparam, lparam);
    }

    // Only act on key-down events (also WM_SYSKEYDOWN for Alt+key combos)
    if msg_id != WM_KEYDOWN && msg_id != WM_SYSKEYDOWN {
        return CallNextHookEx(None, ncode, wparam, lparam);
    }

    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    let vk = kb.vkCode as u16;
    let scan = kb.scanCode;
    let (shift, ctrl, alt, cmd) = read_modifiers();

    let should_consume = HOTKEY_CTX.with(|ctx| -> bool {
        if let Some(ref c) = *ctx.borrow() {
            let hotkeys = c.state.get_hotkeys();
            for binding in &hotkeys {
                if matches_keyboard_binding(vk, scan, shift, ctrl, alt, cmd, binding)
                    && fire_action(&binding.action.clone(), c)
                {
                    return true;
                }
            }
        }
        false
    });

    if should_consume {
        LRESULT(1)
    } else {
        CallNextHookEx(None, ncode, wparam, lparam)
    }
}

unsafe extern "system" fn mouse_callback(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if ncode < 0 {
        return CallNextHookEx(None, ncode, wparam, lparam);
    }

    let msg = wparam.0 as u32;

    // Hover tracking: update radial highlight while wheel is open
    if msg == WM_MOUSEMOVE {
        HOTKEY_CTX.with(|ctx| {
            if let Some(ref c) = *ctx.borrow() {
                use std::sync::atomic::Ordering;
                if c.state.radial_open.load(Ordering::Acquire) {
                    if let Some(keydown) = c.state.get_radial_center() {
                        let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
                        let scale = *c.scale.lock();
                        let lx = ms.pt.x as f64 / scale;
                        let ly = ms.pt.y as f64 / scale;
                        let accounts = c.state.get_account_views();
                        let n = accounts.len();
                        let win_cx = radial_win_size(n) / 2.0;
                        let rel_x = win_cx + (lx - keydown.0);
                        let rel_y = win_cx + (ly - keydown.1);
                        let seg = radial_segment_at(
                            rel_x,
                            rel_y,
                            win_cx,
                            win_cx,
                            n,
                            radial_outer_r(n),
                            INNER_R,
                        )
                        .map(|s| s as i32)
                        .unwrap_or(-1);
                        let prev = c.last_hover_seg.swap(seg, Ordering::Relaxed);
                        if seg != prev {
                            let h = c.handle.clone();
                            let _ = h.clone().run_on_main_thread(move || {
                                if let Some(w) = h.get_webview_window("radial-overlay") {
                                    let _ = w.eval(format!(
                                        "window.__radialHover&&window.__radialHover({})",
                                        seg
                                    ));
                                }
                            });
                        }
                    }
                }
            }
        });
        return CallNextHookEx(None, ncode, wparam, lparam);
    }

    // Mouse button release: if radial is open and the button matches the radial binding, hide it
    if msg == WM_XBUTTONUP {
        let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let xbutton = (ms.mouseData >> 16) as u16;
        let button = match xbutton {
            1 => "Mouse4",
            2 => "Mouse5",
            _ => return CallNextHookEx(None, ncode, wparam, lparam),
        };
        HOTKEY_CTX.with(|ctx| {
            if let Some(ref c) = *ctx.borrow() {
                use std::sync::atomic::Ordering;
                if c.state.radial_open.load(Ordering::Acquire) {
                    let hotkeys = c.state.get_hotkeys();
                    for binding in &hotkeys {
                        if binding.action == "radial" && binding.key == button {
                            c.state.radial_open.store(false, Ordering::Release);
                            let mut pt = POINT { x: 0, y: 0 };
                            let _ = GetCursorPos(&mut pt);
                            let phys_x = pt.x as f64;
                            let phys_y = pt.y as f64;
                            let h = c.handle.clone();
                            let state_ref = c.state.clone();
                            let scale_arc = c.scale.clone();
                            std::thread::spawn(move || {
                                close_radial(h, state_ref, scale_arc, phys_x, phys_y);
                            });
                        }
                    }
                }
            }
        });
        return CallNextHookEx(None, ncode, wparam, lparam);
    }

    if msg != WM_XBUTTONDOWN {
        return CallNextHookEx(None, ncode, wparam, lparam);
    }

    let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
    let xbutton = (ms.mouseData >> 16) as u16;
    let button = match xbutton {
        1 => "Mouse4",
        2 => "Mouse5",
        _ => return CallNextHookEx(None, ncode, wparam, lparam),
    };

    let (shift, ctrl, alt, cmd) = read_modifiers();

    let should_consume = HOTKEY_CTX.with(|ctx| -> bool {
        if let Some(ref c) = *ctx.borrow() {
            let hotkeys = c.state.get_hotkeys();
            for binding in &hotkeys {
                if matches_mouse_binding(button, shift, ctrl, alt, cmd, binding)
                    && fire_action(&binding.action.clone(), c)
                {
                    return true;
                }
            }
        }
        false
    });

    if should_consume {
        LRESULT(1)
    } else {
        CallNextHookEx(None, ncode, wparam, lparam)
    }
}

pub fn start_hotkey_listener(handle: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        HOTKEY_CTX.with(|ctx| {
            *ctx.borrow_mut() = Some(HotkeyContext {
                state: state.clone(),
                handle: handle.clone(),
                last_hover_seg: Arc::new(std::sync::atomic::AtomicI32::new(-1)),
                scale: Arc::new(parking_lot::Mutex::new(1.0)),
            });
        });

        let kb_hook =
            match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hotkey_callback), None, 0) } {
                Ok(h) => h,
                Err(e) => {
                    error!("[Hotkeys] WH_KEYBOARD_LL failed: {:?}", e);
                    return;
                }
            };

        let mouse_hook =
            match unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_callback), None, 0) } {
                Ok(h) => h,
                Err(e) => {
                    error!("[Hotkeys] WH_MOUSE_LL failed: {:?}", e);
                    unsafe {
                        let _ = UnhookWindowsHookEx(kb_hook);
                    }
                    return;
                }
            };

        info!("[Hotkeys] WH_KEYBOARD_LL + WH_MOUSE_LL hooks installed");

        unsafe {
            let mut msg = MSG::default();
            loop {
                let result = GetMessageW(&mut msg, None, 0, 0);
                if result.0 <= 0 {
                    break;
                }
            }
            let _ = UnhookWindowsHookEx(kb_hook);
            let _ = UnhookWindowsHookEx(mouse_hook);
        }

        info!("[Hotkeys] Hooks uninstalled");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::HotkeyBinding;

    fn binding(key: &str, cmd: bool, alt: bool, shift: bool, ctrl: bool) -> HotkeyBinding {
        HotkeyBinding {
            action: "test".into(),
            key: key.into(),
            cmd,
            alt,
            shift,
            ctrl,
        }
    }

    // Windows virtual key codes
    const VK_F1: u16 = 0x70;
    const VK_F2: u16 = 0x71;
    const VK_F3: u16 = 0x72;

    #[test]
    fn exact_match_no_modifiers() {
        assert!(matches_keyboard_binding(
            VK_F1,
            0,
            false,
            false,
            false,
            false,
            &binding("F1", false, false, false, false)
        ));
    }

    #[test]
    fn wrong_key_returns_false() {
        assert!(!matches_keyboard_binding(
            VK_F2,
            0,
            false,
            false,
            false,
            false,
            &binding("F1", false, false, false, false)
        ));
    }

    #[test]
    fn unknown_key_string_returns_false() {
        assert!(!matches_keyboard_binding(
            VK_F1,
            0,
            false,
            false,
            false,
            false,
            &binding("Banana", false, false, false, false)
        ));
    }

    #[test]
    fn modifier_mismatch_shift_returns_false() {
        assert!(!matches_keyboard_binding(
            VK_F1,
            0,
            true,
            false,
            false,
            false,
            &binding("F1", false, false, false, false)
        ));
    }

    #[test]
    fn modifier_match_shift_returns_true() {
        assert!(matches_keyboard_binding(
            VK_F1,
            0,
            true,
            false,
            false,
            false,
            &binding("F1", false, false, true, false)
        ));
    }

    #[test]
    fn modifier_match_ctrl_returns_true() {
        assert!(matches_keyboard_binding(
            VK_F2,
            0,
            false,
            true,
            false,
            false,
            &binding("F2", false, false, false, true)
        ));
    }

    #[test]
    fn modifier_match_alt_returns_true() {
        assert!(matches_keyboard_binding(
            VK_F3,
            0,
            false,
            false,
            true,
            false,
            &binding("F3", false, true, false, false)
        ));
    }

    #[test]
    fn modifier_mismatch_ctrl_returns_false() {
        assert!(!matches_keyboard_binding(
            VK_F1,
            0,
            false,
            true,
            false,
            false,
            &binding("F1", false, false, false, false)
        ));
    }

    #[test]
    fn modifier_mismatch_alt_returns_false() {
        assert!(!matches_keyboard_binding(
            VK_F1,
            0,
            false,
            false,
            true,
            false,
            &binding("F1", false, false, false, false)
        ));
    }

    #[test]
    fn multiple_modifiers_must_all_match() {
        assert!(matches_keyboard_binding(
            VK_F1,
            0,
            true,
            true,
            false,
            false,
            &binding("F1", false, false, true, true)
        ));
        assert!(!matches_keyboard_binding(
            VK_F1,
            0,
            true,
            false,
            false,
            false,
            &binding("F1", false, false, true, true)
        ));
    }

    // --- Numpad key mappings ---
    const VK_NUMPAD0: u16 = 0x60;
    const VK_NUMPAD_ADD: u16 = 0x6B;
    const VK_NUMPAD_DECIMAL: u16 = 0x6E;

    #[test]
    fn numpad0_maps_correctly() {
        assert_eq!(js_code_to_vk("Numpad0"), Some(VK_NUMPAD0));
    }

    #[test]
    fn numpad_add_maps_correctly() {
        assert_eq!(js_code_to_vk("NumpadAdd"), Some(VK_NUMPAD_ADD));
    }

    #[test]
    fn numpad_decimal_maps_correctly() {
        assert_eq!(js_code_to_vk("NumpadDecimal"), Some(VK_NUMPAD_DECIMAL));
    }

    #[test]
    fn numpad0_binding_matches() {
        assert!(matches_keyboard_binding(
            VK_NUMPAD0,
            0,
            false,
            false,
            false,
            false,
            &binding("Numpad0", false, false, false, false)
        ));
    }

    // --- Letter keys use scan codes (layout-independent) ---
    // AZERTY simulation: "KeyZ" is the physical QWERTY-Z position (scan 0x2C),
    // which is the "W" key on AZERTY. VK_W=0x57 is what Windows sends for AZERTY-W.
    const SCAN_KEY_Z: u32 = 0x2C; // physical QWERTY-Z / AZERTY-W position
    const SCAN_KEY_W: u32 = 0x11; // physical QWERTY-W / AZERTY-Z position
    const VK_W: u16 = 0x57;
    const VK_Z: u16 = 0x5A;

    #[test]
    fn letter_key_matches_by_scan_code_not_vk() {
        // Binding recorded as "KeyZ" (user pressed AZERTY-W, event.code="KeyZ").
        // Hook fires with scan=0x2C (physical QWERTY-Z) and vkCode=VK_W (0x57, AZERTY layout).
        assert!(matches_keyboard_binding(
            VK_W,
            SCAN_KEY_Z,
            false,
            false,
            false,
            false,
            &binding("KeyZ", false, false, false, false)
        ));
    }

    #[test]
    fn letter_key_wrong_scan_no_match() {
        // AZERTY-Z has scan 0x11 (QWERTY-W position) — must NOT match "KeyZ" binding.
        assert!(!matches_keyboard_binding(
            VK_Z,
            SCAN_KEY_W,
            false,
            false,
            false,
            false,
            &binding("KeyZ", false, false, false, false)
        ));
    }

    // --- matches_mouse_binding ---

    #[test]
    fn mouse_exact_match() {
        assert!(matches_mouse_binding(
            "Mouse4",
            false,
            false,
            false,
            false,
            &binding("Mouse4", false, false, false, false)
        ));
    }

    #[test]
    fn mouse_wrong_button_returns_false() {
        assert!(!matches_mouse_binding(
            "Mouse5",
            false,
            false,
            false,
            false,
            &binding("Mouse4", false, false, false, false)
        ));
    }

    #[test]
    fn mouse_modifier_mismatch_returns_false() {
        assert!(!matches_mouse_binding(
            "Mouse4",
            true,
            false,
            false,
            false,
            &binding("Mouse4", false, false, false, false)
        ));
    }
}
