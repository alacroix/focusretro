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

        let current_windows = state.accounts.lock().clone();
        let active_then_skipped = state.active_then_skipped_windows();
        let mut cache = state.taskbar_aumid_cache.lock();
        let mut handles = state.taskbar_icon_handles.lock();
        if state.is_taskbar_ungroup_enabled() {
            taskbar::apply_taskbar_identities(&current_windows, &mut cache, &mut handles);
            let ver = state.taskbar_order_version.load(Ordering::Relaxed);
            if ver != state.taskbar_order_version_applied.load(Ordering::Relaxed) {
                state
                    .taskbar_order_version_applied
                    .store(ver, Ordering::Relaxed);
                taskbar::reorder_taskbar_buttons(&active_then_skipped, &cache);
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
    if state.radial_open.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    let fg_id = crate::platform::get_foreground_window_id();
    if fg_id == 0 {
        return;
    }
    let last = state
        .last_foreground_id
        .load(std::sync::atomic::Ordering::Relaxed);
    if fg_id == last {
        return;
    }
    state
        .last_foreground_id
        .store(fg_id, std::sync::atomic::Ordering::Relaxed);
    let views = state.get_account_views();
    let Some(focused) = views.iter().find(|v| v.window_id == fg_id) else {
        return;
    };
    let current_idx = *state.current_index.lock();
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
                        t_focus_done_ms,
                    });
                    let _ = handle.emit("trace-added", ());
                    let h = handle.clone();
                    let name = character_name.to_string();
                    std::thread::spawn(move || {
                        let _ = h.emit("focus-changed", &name);
                    });
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

#[derive(Debug)]
pub(crate) enum RouteAction {
    Focus {
        name: String,
        auto_accept: bool,
        event_type: String,
    },
    StoreMessage(StoredMessage),
    Ignore,
}

pub(crate) fn route_event(event: &parser::GameEvent, state: &AppState) -> RouteAction {
    match event {
        parser::GameEvent::Turn(turn) => {
            if !state.is_autoswitch_enabled() {
                return RouteAction::Ignore;
            }
            if state.is_account_skipped(&turn.character_name) {
                return RouteAction::Ignore;
            }
            RouteAction::Focus {
                name: turn.character_name.clone(),
                auto_accept: false,
                event_type: "turn".into(),
            }
        }
        parser::GameEvent::GroupInvite(invite) => {
            if !state.is_group_invite_enabled() {
                return RouteAction::Ignore;
            }
            if !state.has_account(&invite.inviter_name) {
                return RouteAction::Ignore;
            }
            if state.is_account_skipped(&invite.receiver_name) {
                return RouteAction::Ignore;
            }
            RouteAction::Focus {
                name: invite.receiver_name.clone(),
                auto_accept: state.is_auto_accept_enabled(),
                event_type: "group_invite".into(),
            }
        }
        parser::GameEvent::Trade(trade) => {
            if !state.is_trade_enabled() {
                return RouteAction::Ignore;
            }
            if !state.has_account(&trade.requester_name) {
                return RouteAction::Ignore;
            }
            if state.is_account_skipped(&trade.receiver_name) {
                return RouteAction::Ignore;
            }
            RouteAction::Focus {
                name: trade.receiver_name.clone(),
                auto_accept: state.is_auto_accept_enabled(),
                event_type: "trade".into(),
            }
        }
        parser::GameEvent::PrivateMessage(pm) => {
            if !state.is_pm_enabled() {
                return RouteAction::Ignore;
            }
            RouteAction::StoreMessage(StoredMessage {
                receiver: pm.receiver_name.clone(),
                sender: pm.sender_name.clone(),
                message: pm.message.clone(),
                timestamp: now_epoch_secs(),
            })
        }
    }
}

fn start_notification_listener(handle: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        let is_first_start = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        loop {
            let listener = platform::create_notification_listener();
            let callback_handle = handle.clone();
            let callback_state = state.clone();
            let mode_state = state.clone();
            let mode_handle = handle.clone();
            let is_first = std::sync::Arc::clone(&is_first_start);

            let result = listener.start(
                Box::new(move |segments| {
                    let t_notification_ms = now_millis();
                    debug!("[Autoswitch] Notification segments: {:?}", segments);

                    let event = match parser::parse_game_event(&segments) {
                        Some(e) => e,
                        None => {
                            info!("[Autoswitch] No game event matched");
                            return false;
                        }
                    };

                    match route_event(&event, &callback_state) {
                        RouteAction::Focus {
                            name,
                            auto_accept,
                            event_type,
                        } => {
                            match &event {
                                parser::GameEvent::Turn(_) => {
                                    info!("[Autoswitch] Turn detected for: {}", name);
                                    let _ = callback_handle.emit("turn-switched", &name);
                                }
                                parser::GameEvent::GroupInvite(inv) => {
                                    info!(
                                        "[Autoswitch] Group invite: {} invited by {}",
                                        name, inv.inviter_name
                                    );
                                    let _ = callback_handle.emit("group-invite", &name);
                                }
                                parser::GameEvent::Trade(tr) => {
                                    info!(
                                        "[Autoswitch] Trade request: {} from {}",
                                        name, tr.requester_name
                                    );
                                    let _ = callback_handle.emit("trade-request", &name);
                                }
                                _ => {}
                            }
                            focus_character_with_fallback(
                                &name,
                                auto_accept,
                                callback_state.clone(),
                                callback_handle.clone(),
                                event_type,
                                t_notification_ms,
                            );
                            true
                        }
                        RouteAction::StoreMessage(stored) => {
                            info!(
                                "[Autoswitch] PM from {} to {}: {}",
                                stored.sender, stored.receiver, stored.message
                            );
                            callback_state.add_message(stored.clone());
                            let _ = callback_handle.emit("new-pm", &stored);
                            false
                        }
                        RouteAction::Ignore => {
                            info!("[Autoswitch] event ignored");
                            false
                        }
                    }
                }),
                Box::new(move |mode: String| {
                    mode_state.set_notif_mode(mode.clone());
                    mode_state.set_listener_healthy(true);
                    if !is_first.swap(false, std::sync::atomic::Ordering::Relaxed) {
                        let ts = now_millis();
                        mode_state.add_trace(TraceEntry {
                            event_type: "listener_reconnect".into(),
                            character_name: String::new(),
                            t_notification_ms: ts,
                            t_focus_done_ms: ts,
                        });
                        let _ = mode_handle.emit("trace-added", ());
                    }
                    let _ = mode_handle.emit("notif-mode-changed", mode);
                }),
            );

            match result {
                Err(e) => {
                    error!(
                        "[Autoswitch] Notification listener failed: {}, retrying in 2s",
                        e
                    );
                    state.set_listener_healthy(false);
                    state.increment_listener_restart_count();
                    let ts = now_millis();
                    state.add_trace(TraceEntry {
                        event_type: "notification_center_restart".into(),
                        character_name: e.to_string(),
                        t_notification_ms: ts,
                        t_focus_done_ms: ts,
                    });
                    let _ = handle.emit("trace-added", ());
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
                Ok(()) => break,
            }
        } // end loop
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::{
        GameEvent, GroupInviteNotification, PrivateMessage, TradeRequest, TurnNotification,
    };
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;

    fn make_state() -> AppState {
        AppState::from_prefs(crate::state::Preferences::default(), PathBuf::new())
    }

    fn make_window(name: &str, id: u64) -> crate::platform::GameWindow {
        crate::platform::GameWindow {
            character_name: name.into(),
            window_id: id,
            pid: 0,
            title: format!("{} - Dofus Retro v1.0", name),
        }
    }

    fn turn(name: &str) -> GameEvent {
        GameEvent::Turn(TurnNotification {
            character_name: name.into(),
        })
    }

    fn invite(receiver: &str, inviter: &str) -> GameEvent {
        GameEvent::GroupInvite(GroupInviteNotification {
            receiver_name: receiver.into(),
            inviter_name: inviter.into(),
        })
    }

    fn trade(receiver: &str, requester: &str) -> GameEvent {
        GameEvent::Trade(TradeRequest {
            receiver_name: receiver.into(),
            requester_name: requester.into(),
        })
    }

    fn pm(receiver: &str, sender: &str) -> GameEvent {
        GameEvent::PrivateMessage(PrivateMessage {
            receiver_name: receiver.into(),
            sender_name: sender.into(),
            message: "hello".into(),
        })
    }

    // --- Turn ---

    #[test]
    fn turn_autoswitch_on_returns_focus() {
        let state = make_state();
        state.update_accounts(vec![make_window("Craette", 1)]);
        match route_event(&turn("Craette"), &state) {
            RouteAction::Focus {
                name,
                auto_accept,
                event_type,
            } => {
                assert_eq!(name, "Craette");
                assert!(!auto_accept);
                assert_eq!(event_type, "turn");
            }
            other => panic!("expected Focus, got {:?}", other),
        }
    }

    #[test]
    fn turn_autoswitch_off_returns_ignore() {
        let state = make_state();
        state.autoswitch_enabled.store(false, Ordering::Relaxed);
        assert!(matches!(
            route_event(&turn("Craette"), &state),
            RouteAction::Ignore
        ));
    }

    // --- GroupInvite ---

    #[test]
    fn group_invite_known_inviter_returns_focus() {
        let state = make_state();
        state.update_accounts(vec![make_window("Alice", 1), make_window("Bob", 2)]);
        match route_event(&invite("Bob", "Alice"), &state) {
            RouteAction::Focus {
                name,
                auto_accept,
                event_type,
            } => {
                assert_eq!(name, "Bob");
                assert!(!auto_accept);
                assert_eq!(event_type, "group_invite");
            }
            other => panic!("expected Focus, got {:?}", other),
        }
    }

    #[test]
    fn group_invite_known_inviter_auto_accept_on() {
        let state = make_state();
        state.update_accounts(vec![make_window("Alice", 1), make_window("Bob", 2)]);
        state.auto_accept_enabled.store(true, Ordering::Relaxed);
        match route_event(&invite("Bob", "Alice"), &state) {
            RouteAction::Focus { auto_accept, .. } => assert!(auto_accept),
            other => panic!("expected Focus, got {:?}", other),
        }
    }

    #[test]
    fn group_invite_unknown_inviter_returns_ignore() {
        let state = make_state();
        state.update_accounts(vec![make_window("Bob", 1)]);
        assert!(matches!(
            route_event(&invite("Bob", "Stranger"), &state),
            RouteAction::Ignore
        ));
    }

    #[test]
    fn group_invite_toggle_off_returns_ignore() {
        let state = make_state();
        state.update_accounts(vec![make_window("Alice", 1), make_window("Bob", 2)]);
        state.group_invite_enabled.store(false, Ordering::Relaxed);
        assert!(matches!(
            route_event(&invite("Bob", "Alice"), &state),
            RouteAction::Ignore
        ));
    }

    // --- Trade ---

    #[test]
    fn trade_known_requester_returns_focus() {
        let state = make_state();
        state.update_accounts(vec![make_window("Alice", 1), make_window("Bob", 2)]);
        match route_event(&trade("Bob", "Alice"), &state) {
            RouteAction::Focus {
                name, event_type, ..
            } => {
                assert_eq!(name, "Bob");
                assert_eq!(event_type, "trade");
            }
            other => panic!("expected Focus, got {:?}", other),
        }
    }

    #[test]
    fn trade_unknown_requester_returns_ignore() {
        let state = make_state();
        state.update_accounts(vec![make_window("Bob", 1)]);
        assert!(matches!(
            route_event(&trade("Bob", "Stranger"), &state),
            RouteAction::Ignore
        ));
    }

    #[test]
    fn trade_toggle_off_returns_ignore() {
        let state = make_state();
        state.update_accounts(vec![make_window("Alice", 1), make_window("Bob", 2)]);
        state.trade_enabled.store(false, Ordering::Relaxed);
        assert!(matches!(
            route_event(&trade("Bob", "Alice"), &state),
            RouteAction::Ignore
        ));
    }

    #[test]
    fn trade_auto_accept_on() {
        let state = make_state();
        state.update_accounts(vec![make_window("Alice", 1), make_window("Bob", 2)]);
        state.auto_accept_enabled.store(true, Ordering::Relaxed);
        match route_event(&trade("Bob", "Alice"), &state) {
            RouteAction::Focus { auto_accept, .. } => assert!(auto_accept),
            other => panic!("expected Focus, got {:?}", other),
        }
    }

    // --- PrivateMessage ---

    #[test]
    fn pm_enabled_returns_store_message() {
        let state = make_state();
        assert!(matches!(
            route_event(&pm("Bob", "Alice"), &state),
            RouteAction::StoreMessage(_)
        ));
    }

    #[test]
    fn pm_disabled_returns_ignore() {
        let state = make_state();
        state.pm_enabled.store(false, Ordering::Relaxed);
        assert!(matches!(
            route_event(&pm("Bob", "Alice"), &state),
            RouteAction::Ignore
        ));
    }

    // --- Skipped accounts ---

    #[test]
    fn turn_skipped_account_returns_ignore() {
        let state = make_state();
        state.update_accounts(vec![make_window("Craette", 1)]);
        state.set_skipped("Craette", true);
        assert!(matches!(
            route_event(&turn("Craette"), &state),
            RouteAction::Ignore
        ));
    }

    #[test]
    fn group_invite_skipped_receiver_returns_ignore() {
        let state = make_state();
        state.update_accounts(vec![make_window("Craette", 1), make_window("Alice", 2)]);
        state.set_skipped("Craette", true);
        assert!(matches!(
            route_event(&invite("Craette", "Alice"), &state),
            RouteAction::Ignore
        ));
    }

    #[test]
    fn trade_skipped_receiver_returns_ignore() {
        let state = make_state();
        state.update_accounts(vec![make_window("Craette", 1), make_window("Alice", 2)]);
        state.set_skipped("Craette", true);
        assert!(matches!(
            route_event(&trade("Craette", "Alice"), &state),
            RouteAction::Ignore
        ));
    }
}
