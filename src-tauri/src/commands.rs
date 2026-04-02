use crate::core::accounts;
use crate::platform::{self, PermissionStatus};
use crate::ready::BackendReady;
use crate::state::{AccountView, AppState, HotkeyBinding, StoredMessage, TraceEntry};
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{Emitter, Manager};

#[derive(Serialize, Clone)]
pub struct InitialState {
    pub accounts: Vec<AccountView>,
    pub permissions: PermissionStatus,
    pub language: String,
    pub hotkeys: Vec<HotkeyBinding>,
    pub show_debug: bool,
    pub pm_enabled: bool,
    pub theme: String,
    pub update_consent: Option<bool>,
    pub taskbar_ungroup: bool,
    pub icon_style: String,
}

#[tauri::command]
pub fn get_initial_state(state: tauri::State<'_, Arc<AppState>>) -> InitialState {
    let windows = accounts::detect_accounts();
    state.update_accounts(windows);

    #[cfg(target_os = "windows")]
    let taskbar_ungroup = state.is_taskbar_ungroup_enabled();
    #[cfg(not(target_os = "windows"))]
    let taskbar_ungroup = false;

    InitialState {
        accounts: state.get_account_views(),
        permissions: PermissionStatus {
            accessibility: platform::check_accessibility_permission(),
            screen_recording: platform::check_screen_recording_permission(),
            input_monitoring: platform::check_input_monitoring_permission(),
        },
        language: state.get_language(),
        hotkeys: state.get_hotkeys(),
        show_debug: state.is_show_debug(),
        pm_enabled: state.is_pm_enabled(),
        theme: state.get_theme(),
        update_consent: state.get_update_consent(),
        taskbar_ungroup,
        icon_style: state.get_icon_style(),
    }
}

#[tauri::command]
pub fn get_icon_style(state: tauri::State<'_, Arc<AppState>>) -> String {
    state.get_icon_style()
}

#[tauri::command]
pub fn set_icon_style(state: tauri::State<'_, Arc<AppState>>, style: String) {
    state.set_icon_style(style);
}

#[tauri::command]
pub async fn wait_for_ready(ready: tauri::State<'_, Arc<BackendReady>>) -> Result<(), ()> {
    // Reserve the waker slot BEFORE checking the flag to close the TOCTOU window:
    // if signal() fires between load() and notified().await, the notification
    // would be lost. With notified() called first, it captures the signal even
    // if it arrives between the load check and the await.
    let notified = ready.notify.notified();
    if ready.is_ready.load(Ordering::Acquire) {
        return Ok(());
    }
    notified.await;
    Ok(())
}

#[derive(Serialize, Clone)]
pub(crate) struct WheelPos {
    pub x: Option<f64>,
    pub y: Option<f64>,
}

/// Compute cursor position in CSS-pixel coordinates relative to the radial overlay window.
pub(crate) fn wheel_pos_payload(w: &tauri::WebviewWindow) -> WheelPos {
    if let (Ok(cursor), Ok(win_pos), Ok(scale)) =
        (w.cursor_position(), w.outer_position(), w.scale_factor())
    {
        WheelPos {
            x: Some((cursor.x - win_pos.x as f64) / scale),
            y: Some((cursor.y - win_pos.y as f64) / scale),
        }
    } else {
        WheelPos { x: None, y: None }
    }
}

#[tauri::command]
pub fn list_accounts(state: tauri::State<'_, Arc<AppState>>) -> Vec<AccountView> {
    state.get_account_views()
}

#[tauri::command]
pub fn refresh_accounts(state: tauri::State<'_, Arc<AppState>>) -> Vec<AccountView> {
    let windows = accounts::detect_accounts();
    state.update_accounts(windows);
    state.get_account_views()
}

#[tauri::command]
pub fn toggle_autoswitch(handle: tauri::AppHandle, state: tauri::State<'_, Arc<AppState>>) -> bool {
    let new_state = !state.is_autoswitch_enabled();
    state.set_autoswitch(new_state);
    let _ = handle.emit("autoswitch-changed", new_state);
    crate::update_tray_display(&handle, &state);
    new_state
}

#[tauri::command]
pub fn get_autoswitch_state(state: tauri::State<'_, Arc<AppState>>) -> bool {
    state.is_autoswitch_enabled()
}

#[tauri::command]
pub fn focus_account(
    name: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let wm = platform::create_window_manager();
    let windows = wm.list_dofus_windows();
    let win = windows
        .iter()
        .find(|w| w.character_name.eq_ignore_ascii_case(&name))
        .ok_or_else(|| format!("Window not found for '{}'", name))?;
    wm.focus_window(win).map_err(|e| e.to_string())?;
    state.set_current_by_name(&name);
    let _ = tauri::Emitter::emit(&app, "focus-changed", &name);
    Ok(())
}

#[tauri::command]
pub fn focus_next_account(state: tauri::State<'_, Arc<AppState>>) -> Option<String> {
    let win = state.cycle_next()?;
    let wm = platform::create_window_manager();
    wm.focus_window(&win).ok()?;
    Some(win.character_name)
}

#[tauri::command]
pub fn focus_prev_account(state: tauri::State<'_, Arc<AppState>>) -> Option<String> {
    let win = state.cycle_prev()?;
    let wm = platform::create_window_manager();
    wm.focus_window(&win).ok()?;
    Some(win.character_name)
}

#[tauri::command]
pub fn toggle_group_invite(state: tauri::State<'_, Arc<AppState>>) -> bool {
    let new_state = !state.is_group_invite_enabled();
    state.set_group_invite(new_state);
    new_state
}

#[tauri::command]
pub fn get_group_invite_state(state: tauri::State<'_, Arc<AppState>>) -> bool {
    state.is_group_invite_enabled()
}

#[tauri::command]
pub fn toggle_trade(state: tauri::State<'_, Arc<AppState>>) -> bool {
    let new_state = !state.is_trade_enabled();
    state.set_trade(new_state);
    new_state
}

#[tauri::command]
pub fn get_trade_state(state: tauri::State<'_, Arc<AppState>>) -> bool {
    state.is_trade_enabled()
}

#[tauri::command]
pub fn toggle_workshop_invite(state: tauri::State<'_, Arc<AppState>>) -> bool {
    let new_state = !state.is_workshop_enabled();
    state.set_workshop(new_state);
    new_state
}

#[tauri::command]
pub fn get_workshop_invite_state(state: tauri::State<'_, Arc<AppState>>) -> bool {
    state.is_workshop_enabled()
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn toggle_taskbar_ungroup(state: tauri::State<'_, Arc<AppState>>) -> bool {
    let new_state = !state.is_taskbar_ungroup_enabled();
    state.set_taskbar_ungroup(new_state);
    new_state
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn get_taskbar_ungroup_state(state: tauri::State<'_, Arc<AppState>>) -> bool {
    state.is_taskbar_ungroup_enabled()
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn toggle_taskbar_ungroup(_state: tauri::State<'_, Arc<AppState>>) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn get_taskbar_ungroup_state(_state: tauri::State<'_, Arc<AppState>>) -> bool {
    false
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub fn apply_window_icon(state: tauri::State<'_, Arc<AppState>>, window_id: u64, rgba: Vec<u8>) {
    use crate::platform::windows::taskbar;
    let mut handles = state.taskbar_icon_handles.lock();
    taskbar::set_window_icon(window_id as isize, &rgba, &mut handles);
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn apply_window_icon(_state: tauri::State<'_, Arc<AppState>>, _window_id: u64, _rgba: Vec<u8>) {
}

#[tauri::command]
pub fn set_tray_icon(handle: tauri::AppHandle, rgba: Vec<u8>) {
    if rgba.len() != 32 * 32 * 4 {
        return;
    }
    if let Some(tray) = handle.tray_by_id("main") {
        let icon = tauri::image::Image::new_owned(rgba, 32, 32);
        let _ = tray.set_icon(Some(icon));
    }
}

#[tauri::command]
pub fn toggle_pm(state: tauri::State<'_, Arc<AppState>>) -> bool {
    let new_state = !state.is_pm_enabled();
    state.set_pm(new_state);
    new_state
}

#[tauri::command]
pub fn get_pm_state(state: tauri::State<'_, Arc<AppState>>) -> bool {
    state.is_pm_enabled()
}

#[tauri::command]
pub fn get_messages(state: tauri::State<'_, Arc<AppState>>) -> Vec<StoredMessage> {
    state.get_messages()
}

#[tauri::command]
pub fn clear_messages(state: tauri::State<'_, Arc<AppState>>) {
    state.clear_messages();
}

#[tauri::command]
pub fn toggle_auto_accept(state: tauri::State<'_, Arc<AppState>>) -> bool {
    let new_state = !state.is_auto_accept_enabled();
    state.set_auto_accept(new_state);
    new_state
}

#[tauri::command]
pub fn get_auto_accept_state(state: tauri::State<'_, Arc<AppState>>) -> bool {
    state.is_auto_accept_enabled()
}

#[tauri::command]
pub fn reorder_account(
    name: String,
    new_position: usize,
    state: tauri::State<'_, Arc<AppState>>,
) -> Vec<AccountView> {
    state.reorder_account(&name, new_position);

    #[cfg(target_os = "windows")]
    if state.is_taskbar_ungroup_enabled() {
        use crate::platform::windows::taskbar;
        use std::sync::atomic::Ordering;
        let windows = state.active_then_skipped_windows();
        let cache = state.taskbar_aumid_cache.lock();
        taskbar::reorder_taskbar_buttons(&windows, &cache);
        // Mark as applied so the next poll doesn't reorder a second time
        let ver = state.taskbar_order_version.load(Ordering::Relaxed);
        state
            .taskbar_order_version_applied
            .store(ver, Ordering::Relaxed);
    }

    state.get_account_views()
}

#[tauri::command]
pub fn set_principal(name: String, state: tauri::State<'_, Arc<AppState>>) -> Vec<AccountView> {
    state.set_principal(&name);
    state.get_account_views()
}

#[tauri::command]
pub fn set_account_skipped(
    name: String,
    skipped: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Vec<AccountView> {
    state.set_skipped(&name, skipped);

    #[cfg(target_os = "windows")]
    if state.is_taskbar_ungroup_enabled() {
        use crate::platform::windows::taskbar;
        use std::sync::atomic::Ordering;
        let windows = state.active_then_skipped_windows();
        let cache = state.taskbar_aumid_cache.lock();
        taskbar::reorder_taskbar_buttons(&windows, &cache);
        let ver = state.taskbar_order_version.load(Ordering::Relaxed);
        state
            .taskbar_order_version_applied
            .store(ver, Ordering::Relaxed);
    }

    state.get_account_views()
}

#[tauri::command]
pub fn update_account_profile(
    name: String,
    color: Option<String>,
    icon_path: Option<String>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Vec<AccountView> {
    state.update_profile(&name, color, icon_path);
    state.get_account_views()
}

#[tauri::command]
pub fn get_profiles(state: tauri::State<'_, Arc<AppState>>) -> Vec<AccountView> {
    state.get_account_views()
}

#[tauri::command]
pub fn focus_principal(state: tauri::State<'_, Arc<AppState>>) -> Option<String> {
    let win = state.get_principal()?;
    let wm = platform::create_window_manager();
    wm.focus_window(&win).ok()?;
    Some(win.character_name)
}

#[tauri::command]
pub fn check_permissions() -> PermissionStatus {
    PermissionStatus {
        accessibility: platform::check_accessibility_permission(),
        screen_recording: platform::check_screen_recording_permission(),
        input_monitoring: platform::check_input_monitoring_permission(),
    }
}

#[tauri::command]
pub fn request_screen_recording(app: tauri::AppHandle) {
    let _ = app.run_on_main_thread(|| {
        platform::request_screen_recording_permission();
    });
}

#[tauri::command]
pub fn request_input_monitoring(app: tauri::AppHandle) {
    let _ = app.run_on_main_thread(|| {
        platform::request_input_monitoring_permission();
    });
}

#[tauri::command]
pub fn open_settings(section: String) {
    let url = match section.as_str() {
        "accessibility" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
        "screen_recording" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
        "input_monitoring" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
        }
        _ => return,
    };
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(target_os = "macos"))]
    let _ = url;
}

#[tauri::command]
pub fn get_hotkeys(state: tauri::State<'_, Arc<AppState>>) -> Vec<HotkeyBinding> {
    state.get_hotkeys()
}

#[tauri::command]
pub fn set_hotkey(
    action: String,
    key: String,
    cmd: bool,
    alt: bool,
    shift: bool,
    ctrl: bool,
    state: tauri::State<'_, Arc<AppState>>,
) -> Vec<HotkeyBinding> {
    state.set_hotkey(&action, key, cmd, alt, shift, ctrl);
    state.get_hotkeys()
}

#[tauri::command]
pub fn reset_hotkeys(state: tauri::State<'_, Arc<AppState>>) -> Vec<HotkeyBinding> {
    state.reset_hotkeys();
    state.get_hotkeys()
}

#[tauri::command]
pub fn get_language(state: tauri::State<'_, Arc<AppState>>) -> String {
    state.get_language()
}

#[tauri::command]
pub fn set_language(lang: String, state: tauri::State<'_, Arc<AppState>>) {
    state.set_language(lang);
}

#[tauri::command]
pub fn get_traces(state: tauri::State<'_, Arc<AppState>>) -> Vec<TraceEntry> {
    state.get_traces()
}

#[tauri::command]
pub fn clear_traces(state: tauri::State<'_, Arc<AppState>>) {
    state.clear_traces();
}

#[derive(Serialize)]
pub struct ListenerHealthSnapshot {
    pub healthy: bool,
    pub restart_count: u32,
    pub mode: String,
}

#[tauri::command]
pub fn get_listener_health(state: tauri::State<'_, Arc<AppState>>) -> ListenerHealthSnapshot {
    let (healthy, restart_count, mode) = state.get_listener_health_snapshot();
    ListenerHealthSnapshot {
        healthy,
        restart_count,
        mode,
    }
}

#[tauri::command]
pub fn toggle_show_debug(state: tauri::State<'_, Arc<AppState>>) -> bool {
    let new_state = !state.is_show_debug();
    state.set_show_debug(new_state);
    new_state
}

#[tauri::command]
pub fn get_show_debug(state: tauri::State<'_, Arc<AppState>>) -> bool {
    state.is_show_debug()
}

#[tauri::command]
pub fn get_theme(state: tauri::State<'_, Arc<AppState>>) -> String {
    state.get_theme()
}

#[tauri::command]
pub fn set_theme(theme: String, state: tauri::State<'_, Arc<AppState>>) {
    state.set_theme(theme);
}

#[tauri::command]
pub fn show_radial(app: tauri::AppHandle, state: tauri::State<'_, Arc<AppState>>) {
    use std::sync::atomic::Ordering;
    state.radial_open.store(true, Ordering::Release);
    let h = app.clone();
    std::thread::spawn(move || {
        if let Some(w) = h.get_webview_window("radial-overlay") {
            let _ = w.show();
            let _ = w.set_focus();
            let pos = wheel_pos_payload(&w);
            let _ = w.emit("show-radial", pos);
        }
    });
}

#[tauri::command]
pub fn hide_radial(app: tauri::AppHandle, state: tauri::State<'_, Arc<AppState>>) {
    use std::sync::atomic::Ordering;
    state.radial_open.store(false, Ordering::Release);
    if let Some(w) = app.get_webview_window("radial-overlay") {
        let _ = w.hide();
    }
    // Return focus to the current Dofus window so the game gets focus back,
    // not the FocusRetro main window.
    if let Some(win) = state.get_current_window() {
        let wm = platform::create_window_manager();
        let _ = wm.focus_window(&win);
    }
}

#[tauri::command]
pub fn get_available_layouts() -> Vec<String> {
    vec![
        "maximize", "split-h", "split-v", "grid-2x2", "grid-3x2", "grid-4x2",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

#[tauri::command]
pub fn get_update_consent(state: tauri::State<'_, Arc<AppState>>) -> Option<bool> {
    state.get_update_consent()
}

#[tauri::command]
pub fn set_update_consent(state: tauri::State<'_, Arc<AppState>>, consent: bool) {
    state.set_update_consent(consent);
}

#[tauri::command]
pub fn get_close_to_tray(state: tauri::State<'_, Arc<AppState>>) -> bool {
    state.is_close_to_tray()
}

#[tauri::command]
pub fn set_close_to_tray(value: bool, state: tauri::State<'_, Arc<AppState>>) {
    state.set_close_to_tray(value);
}

#[tauri::command]
pub fn set_close_behavior_prompted(value: bool, state: tauri::State<'_, Arc<AppState>>) {
    state.set_close_behavior_prompted(value);
}

#[tauri::command]
pub fn apply_close(window: tauri::WebviewWindow, state: tauri::State<'_, Arc<AppState>>) {
    if state.is_close_to_tray() {
        let _ = window.hide();
    } else {
        state.save_sync();
        window.app_handle().exit(0);
    }
}

#[tauri::command]
pub fn apply_layout(
    layout: String,
    state: tauri::State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let wm = platform::create_window_manager();
    let live = wm.list_dofus_windows();
    let profiles = state.profiles.lock();
    let ordered_names: Vec<String> = state
        .accounts
        .lock()
        .iter()
        .filter(|w| {
            !profiles
                .iter()
                .find(|p| p.character_name.eq_ignore_ascii_case(&w.character_name))
                .is_some_and(|p| p.is_skipped)
        })
        .map(|w| w.character_name.clone())
        .collect();
    drop(profiles);
    let windows: Vec<_> = ordered_names
        .iter()
        .filter_map(|name| {
            live.iter()
                .find(|w| w.character_name.eq_ignore_ascii_case(name))
        })
        .cloned()
        .collect();
    wm.arrange_windows(&windows, &layout)
        .map_err(|e| e.to_string())?;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_focus();
    }
    Ok(())
}
