use crate::core::{accounts, parser};
use crate::platform;
use crate::state::{AppState, StoredMessage, TraceEntry};
use log::{debug, error, info};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub fn setup(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let state = app.state::<Arc<AppState>>().inner().clone();

    refresh_accounts(&handle, &state);

    start_notification_listener(handle.clone(), state.clone());

    let poll_handle = handle.clone();
    let poll_state = state.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(3));
        refresh_accounts(&poll_handle, &poll_state);
    });

    info!("FocusRetro autoswitch initialized");
    Ok(())
}

fn refresh_accounts(handle: &AppHandle, state: &Arc<AppState>) {
    let windows = accounts::detect_accounts();
    state.update_accounts(windows);
    let views = state.get_account_views();
    let _ = handle.emit("accounts-updated", &views);
    crate::update_tray_display(handle, state);
    sync_focus_from_foreground(handle, state);
    #[cfg(target_os = "windows")]
    {
        use crate::platform::windows::taskbar;
        use std::sync::atomic::Ordering;

        let current_windows = state.accounts.lock().unwrap().clone();
        let mut cache = state.taskbar_aumid_cache.lock().unwrap();
        let mut handles = state.taskbar_icon_handles.lock().unwrap();
        if state.is_taskbar_ungroup_enabled() {
            taskbar::apply_taskbar_identities(&current_windows, &mut cache, &mut handles);
            let ver = state.taskbar_order_version.load(Ordering::Relaxed);
            if ver != state.taskbar_order_version_applied.load(Ordering::Relaxed) {
                state.taskbar_order_version_applied.store(ver, Ordering::Relaxed);
                taskbar::reorder_taskbar_buttons(&current_windows, &cache);
            }
        } else {
            taskbar::reset_taskbar_identities(&current_windows, &mut cache, &mut handles);
        }
    }
}

/// Updates current account from the actual foreground window and emits focus-changed if it changed.
/// Skips the update while the radial overlay is open, and caches the last known foreground ID
/// to avoid redundant lock acquisitions when the foreground hasn't changed.
fn sync_focus_from_foreground(handle: &AppHandle, state: &Arc<AppState>) {
    if state.radial_open.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let fg_id = crate::platform::get_foreground_window_id();
    if fg_id == 0 {
        return;
    }
    let last = state.last_foreground_id.load(std::sync::atomic::Ordering::Relaxed);
    if fg_id == last {
        return;
    }
    state.last_foreground_id.store(fg_id, std::sync::atomic::Ordering::Relaxed);
    let views = state.get_account_views();
    let Some(focused) = views.iter().find(|v| v.window_id == fg_id) else {
        return;
    };
    let current_idx = *state.current_index.lock().unwrap();
    let current_window_id = views.get(current_idx).map(|v| v.window_id);
    if Some(fg_id) == current_window_id {
        return;
    }
    state.sync_current_from_window_id(fg_id);
    let h = handle.clone();
    let name = focused.character_name.clone();
    std::thread::spawn(move || {
        let _ = h.emit("focus-changed", &name);
    });
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn focus_character_with_fallback(
    character_name: &str,
    auto_accept: bool,
    state: Arc<AppState>,
    handle: AppHandle,
    event_type: String,
    t_notification_ms: u64,
    t_parsed_ms: u64,
    t_focus_triggered_ms: u64,
) {
    #[cfg(target_os = "macos")]
    {
        // On macOS, a short delay before focus lets the notification/OS settle and avoids
        // focus flipping back (e.g. after trade/invite). Run focus in a thread with 50ms delay.
        let name = character_name.to_string();
        let state_mac = state.clone();
        let handle_mac = handle.clone();
        let event_type_mac = event_type.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            info!("[Autoswitch] Running fallback direct focus for {}", name);
            let wm_fallback = platform::create_window_manager();
            let windows = wm_fallback.list_dofus_windows();
            if let Some(win) = windows
                .iter()
                .find(|w| w.character_name.eq_ignore_ascii_case(&name))
            {
                match wm_fallback.focus_window(win) {
                    Err(e) => error!("[Autoswitch] Focus failed: {}", e),
                    Ok(()) => {
                        info!("[Autoswitch] Focused {}", name);
                        state_mac.set_current_by_name(&name);
                        let t_focus_done_ms = now_millis();
                        state_mac.add_trace(TraceEntry {
                            event_type: event_type_mac,
                            character_name: name.clone(),
                            t_notification_ms,
                            t_parsed_ms,
                            t_focus_triggered_ms,
                            t_focus_done_ms,
                        });
                        let _ = handle_mac.emit("trace-added", ());
                        let _ = handle_mac.emit("focus-changed", &name);
                    }
                }
            } else {
                info!("[Autoswitch] Window not found for {}", name);
            }
            if auto_accept {
                std::thread::sleep(std::time::Duration::from_millis(300));
                info!("[Autoswitch] Auto-accept: sending Enter for {}", name);
                if let Err(e) = wm_fallback.send_enter_key() {
                    error!("[Autoswitch] Auto-accept Enter failed: {}", e);
                }
            }
        });
        return;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let wm = platform::create_window_manager();
        let windows = wm.list_dofus_windows();
        if let Some(win) = windows
            .iter()
            .find(|w| w.character_name.eq_ignore_ascii_case(character_name))
        {
            match wm.focus_window(win) {
                Err(e) => error!("[Autoswitch] Focus failed: {}", e),
                Ok(()) => {
                    info!("[Autoswitch] Focused {}", character_name);
                    state.set_current_by_name(character_name);
                    let t_focus_done_ms = now_millis();
                    state.add_trace(TraceEntry {
                        event_type,
                        character_name: character_name.to_string(),
                        t_notification_ms,
                        t_parsed_ms,
                        t_focus_triggered_ms,
                        t_focus_done_ms,
                    });
                    let _ = handle.emit("trace-added", ());
                    let h = handle.clone();
                    let name = character_name.to_string();
                    std::thread::spawn(move || { let _ = h.emit("focus-changed", &name); });
                }
            }
        } else {
            info!("[Autoswitch] Window not found for {}", character_name);
        }

        if auto_accept {
            let name = character_name.to_string();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                info!("[Autoswitch] Auto-accept: sending Enter for {}", name);
                let wm_enter = platform::create_window_manager();
                if let Err(e) = wm_enter.send_enter_key() {
                    error!("[Autoswitch] Auto-accept Enter failed: {}", e);
                }
            });
        }
    }
}

fn start_notification_listener(handle: AppHandle, state: Arc<AppState>) {
    let listener = platform::create_notification_listener();
    let callback_handle = handle.clone();
    let callback_state = state.clone();

    std::thread::spawn(move || {
        let mode_state = callback_state.clone();
        let mode_handle = callback_handle.clone();
        let result = listener.start(Box::new(move |segments| {
            let t_notification_ms = now_millis();
            debug!("[Autoswitch] Notification segments: {:?}", segments);

            let event = match parser::parse_game_event(&segments) {
                Some(e) => e,
                None => {
                    info!("[Autoswitch] No game event matched");
                    return false;
                }
            };

            let t_parsed_ms = now_millis();

            match event {
                parser::GameEvent::Turn(turn) => {
                    if !callback_state.is_autoswitch_enabled() {
                        info!("[Autoswitch] autoswitch disabled, ignoring turn");
                        return false;
                    }
                    info!("[Autoswitch] Turn detected for: {}", turn.character_name);
                    let _ = callback_handle.emit("turn-switched", &turn.character_name);
                    let t_focus_triggered_ms = now_millis();
                    focus_character_with_fallback(
                        &turn.character_name,
                        false,
                        callback_state.clone(),
                        callback_handle.clone(),
                        "turn".into(),
                        t_notification_ms,
                        t_parsed_ms,
                        t_focus_triggered_ms,
                    );
                    true
                }
                parser::GameEvent::GroupInvite(invite) => {
                    if !callback_state.is_group_invite_enabled() {
                        info!("[Autoswitch] group invite disabled, ignoring");
                        return false;
                    }
                    if !callback_state.has_account(&invite.inviter_name) {
                        info!(
                            "[Autoswitch] group invite from unknown '{}', ignoring",
                            invite.inviter_name
                        );
                        return false;
                    }
                    info!(
                        "[Autoswitch] Group invite: {} invited by {}",
                        invite.receiver_name, invite.inviter_name
                    );
                    let _ = callback_handle.emit("group-invite", &invite.receiver_name);
                    let accept = callback_state.is_auto_accept_enabled();
                    let t_focus_triggered_ms = now_millis();
                    focus_character_with_fallback(
                        &invite.receiver_name,
                        accept,
                        callback_state.clone(),
                        callback_handle.clone(),
                        "group_invite".into(),
                        t_notification_ms,
                        t_parsed_ms,
                        t_focus_triggered_ms,
                    );
                    true
                }
                parser::GameEvent::Trade(trade) => {
                    if !callback_state.is_trade_enabled() {
                        info!("[Autoswitch] trade disabled, ignoring");
                        return false;
                    }
                    if !callback_state.has_account(&trade.requester_name) {
                        info!(
                            "[Autoswitch] trade from unknown '{}', ignoring",
                            trade.requester_name
                        );
                        return false;
                    }
                    info!(
                        "[Autoswitch] Trade request: {} from {}",
                        trade.receiver_name, trade.requester_name
                    );
                    let _ = callback_handle.emit("trade-request", &trade.receiver_name);
                    let accept = callback_state.is_auto_accept_enabled();
                    let t_focus_triggered_ms = now_millis();
                    focus_character_with_fallback(
                        &trade.receiver_name,
                        accept,
                        callback_state.clone(),
                        callback_handle.clone(),
                        "trade".into(),
                        t_notification_ms,
                        t_parsed_ms,
                        t_focus_triggered_ms,
                    );
                    true
                }
                parser::GameEvent::PrivateMessage(pm) => {
                    if !callback_state.is_pm_enabled() {
                        info!("[Autoswitch] PM disabled, ignoring");
                        return false;
                    }
                    info!(
                        "[Autoswitch] PM from {} to {}: {}",
                        pm.sender_name, pm.receiver_name, pm.message
                    );
                    let stored = StoredMessage {
                        receiver: pm.receiver_name.clone(),
                        sender: pm.sender_name.clone(),
                        message: pm.message.clone(),
                        timestamp: now_epoch_secs(),
                    };
                    callback_state.add_message(stored.clone());
                    let _ = callback_handle.emit("new-pm", &stored);
                    false
                }
            }
        }), Box::new(move |mode| {
            mode_state.set_notif_mode(mode.clone());
            let _ = mode_handle.emit("notif-mode-changed", mode);
        }));

        if let Err(e) = result {
            error!("[Autoswitch] Notification listener failed: {}", e);
        }
    });
}
