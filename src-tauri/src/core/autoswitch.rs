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
        std::thread::sleep(std::time::Duration::from_secs(5));
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
