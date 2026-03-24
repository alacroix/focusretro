mod commands;
mod core;
mod platform;
mod radial;
mod ready;
mod state;

use ready::BackendReady;
use state::{AppState, TraySnapshot};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

pub fn run() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("focusretro_lib=info"),
    )
    .init();

    // BackendReady is managed at builder level so it's available immediately —
    // before WebView2 starts. The frontend awaits wait_for_ready() before calling
    // any other command, eliminating the setup() race on Win10.
    let backend_ready = Arc::new(BackendReady::new());

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .manage(backend_ready)
        .plugin(tauri_plugin_process::init());

    #[cfg(feature = "auto-update")]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    builder
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let state = window.app_handle().state::<Arc<AppState>>();
                    api.prevent_close();
                    if !state.is_close_behavior_prompted() {
                        let os = if cfg!(target_os = "macos") {
                            "macos"
                        } else {
                            "windows"
                        };
                        let _ = window.emit("close-requested", os);
                    } else if state.is_close_to_tray() {
                        let _ = window.hide();
                    } else {
                        window.app_handle().exit(0);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_accounts,
            commands::toggle_autoswitch,
            commands::get_autoswitch_state,
            commands::focus_account,
            commands::focus_next_account,
            commands::focus_prev_account,
            commands::check_permissions,
            commands::request_screen_recording,
            commands::request_input_monitoring,
            commands::open_settings,
            commands::refresh_accounts,
            commands::toggle_group_invite,
            commands::get_group_invite_state,
            commands::toggle_trade,
            commands::get_trade_state,
            commands::toggle_taskbar_ungroup,
            commands::get_taskbar_ungroup_state,
            commands::apply_window_icon,
            commands::toggle_pm,
            commands::get_pm_state,
            commands::get_messages,
            commands::clear_messages,
            commands::toggle_auto_accept,
            commands::get_auto_accept_state,
            commands::reorder_account,
            commands::set_principal,
            commands::set_account_skipped,
            commands::update_account_profile,
            commands::get_profiles,
            commands::focus_principal,
            commands::get_hotkeys,
            commands::set_hotkey,
            commands::reset_hotkeys,
            commands::get_language,
            commands::set_language,
            commands::get_traces,
            commands::clear_traces,
            commands::get_notif_mode,
            commands::toggle_show_debug,
            commands::get_show_debug,
            commands::get_theme,
            commands::set_theme,
            commands::get_available_layouts,
            commands::apply_layout,
            commands::show_radial,
            commands::hide_radial,
            commands::get_update_consent,
            commands::set_update_consent,
            commands::get_close_to_tray,
            commands::set_close_to_tray,
            commands::set_close_behavior_prompted,
            commands::apply_close,
            commands::set_tray_icon,
            commands::wait_for_ready,
            commands::get_initial_state,
        ])
        .setup(|app| {
            // Always signal BackendReady on exit — even if setup() returns Err — so
            // any pending wait_for_ready future is unblocked rather than hanging forever.
            // (The process will panic shortly after on Err, but WebView2 runs out-of-process.)
            let ready = app.state::<Arc<BackendReady>>().inner().clone();
            let result = do_setup(app);
            ready.signal();
            result
        })
        .build(tauri::generate_context!())
        .expect("error while building FocusRetro")
        .run(|app_handle, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                if !has_visible_windows {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            #[cfg(target_os = "windows")]
            if let tauri::RunEvent::Exit = event {
                use crate::platform::windows::taskbar;
                let state = app_handle.state::<std::sync::Arc<AppState>>();
                let mut handles = state.taskbar_icon_handles.lock();
                taskbar::cleanup_all_icons(&mut handles);
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            let _ = (app_handle, event);
        });
}

fn start_hotkey_listener(app: &tauri::App) {
    let handle = app.handle().clone();
    let state = app.state::<Arc<AppState>>().inner().clone();

    #[cfg(target_os = "macos")]
    platform::macos::hotkeys::start_hotkey_listener(handle, state);

    #[cfg(target_os = "windows")]
    platform::windows::hotkeys::start_hotkey_listener(handle, state);

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (handle, state);
        log::warn!("[Hotkeys] Global hotkeys not available on this platform");
    }
}

fn do_setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = app.path().app_config_dir()?.join("config.json");

    crate::state::migrate_config_if_needed(&config_path);

    let app_state = Arc::new(AppState::new(config_path));
    app.manage(app_state);

    app.handle()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))?;

    setup_tray(app)?;
    start_hotkey_listener(app);
    core::autoswitch::setup(app)?;

    #[cfg(target_os = "macos")]
    setup_radial_panel(app);

    #[cfg(target_os = "windows")]
    setup_radial_window(app);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn setup_radial_window(app: &tauri::App) {
    use log::warn;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    match app.get_webview_window("radial-overlay") {
        None => warn!("[Radial] radial-overlay window not found during setup"),
        Some(overlay) => match overlay.hwnd() {
            Err(e) => warn!("[Radial] Failed to get HWND for overlay: {:?}", e),
            Ok(hwnd) => unsafe {
                let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                SetWindowLongPtrW(
                    hwnd,
                    GWL_EXSTYLE,
                    ex_style | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize,
                );
            },
        },
    }
}

#[cfg(target_os = "macos")]
fn setup_radial_panel(app: &tauri::App) {
    use log::warn;
    use tauri_nspanel::WebviewWindowExt as NSPanelExt;
    match app.get_webview_window("radial-overlay") {
        None => warn!("[Radial] radial-overlay window not found during setup"),
        Some(overlay) => match overlay.to_panel() {
            Err(e) => warn!("[Radial] to_panel() failed: {:?}", e),
            Ok(panel) => {
                panel.set_level(200);
                panel.set_becomes_key_only_if_needed(true);
            }
        },
    }
}

fn tray_t(lang: &str, key: &str) -> &'static str {
    match (lang, key) {
        ("fr", "autoswitch") => "Autoswitch",
        ("fr", "show_window") => "Afficher la fenêtre",
        ("fr", "quit") => "Quitter",
        ("fr", "tooltip_active") => "FocusRetro - Actif",
        ("fr", "tooltip_paused") => "FocusRetro - En pause",

        ("es", "autoswitch") => "Autoswitch",
        ("es", "show_window") => "Mostrar ventana",
        ("es", "quit") => "Salir",
        ("es", "tooltip_active") => "FocusRetro - Activo",
        ("es", "tooltip_paused") => "FocusRetro - En pausa",

        (_, "autoswitch") => "Autoswitch",
        (_, "show_window") => "Show Window",
        (_, "quit") => "Quit",
        (_, "tooltip_active") => "FocusRetro - Active",
        (_, "tooltip_paused") => "FocusRetro - Paused",
        _ => "",
    }
}

fn tray_accounts_label(lang: &str, count: usize) -> String {
    match lang {
        "fr" => match count {
            0 => "Aucun compte détecté".to_string(),
            1 => format!("{} compte détecté", count),
            _ => format!("{} comptes détectés", count),
        },
        "es" => match count {
            0 => "Ninguna cuenta detectada".to_string(),
            1 => format!("{} cuenta detectada", count),
            _ => format!("{} cuentas detectadas", count),
        },
        _ => match count {
            0 => "No accounts detected".to_string(),
            1 => format!("{} account detected", count),
            _ => format!("{} accounts detected", count),
        },
    }
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::tray::TrayIconBuilder;

    let state = app.state::<Arc<AppState>>().inner().clone();
    let is_active = state.is_autoswitch_enabled();
    let count = state.account_count();
    let lang = state.get_language();

    let count_label = tray_accounts_label(&lang, count);
    let tooltip_key = if is_active {
        "tooltip_active"
    } else {
        "tooltip_paused"
    };
    let menu = build_tray_menu(
        app.handle(),
        &count_label,
        tray_t(&lang, "autoswitch"),
        is_active,
        tray_t(&lang, "show_window"),
        tray_t(&lang, "quit"),
    )?;

    let icon = app
        .default_window_icon()
        .ok_or("no default window icon")?
        .clone();

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .tooltip(tray_t(&lang, tooltip_key))
        .on_menu_event({
            let handle = app.handle().clone();
            move |app, event| match event.id().as_ref() {
                "toggle" => {
                    let state = handle.state::<Arc<AppState>>();
                    let new_state = !state.is_autoswitch_enabled();
                    state.set_autoswitch(new_state);
                    let _ = app.emit("autoswitch-changed", new_state);
                    update_tray_display(&handle, &state);
                }
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

pub(crate) fn update_tray_display(handle: &AppHandle, state: &AppState) {
    let tray = match handle.tray_by_id("main") {
        Some(t) => t,
        None => return,
    };

    let is_active = state.is_autoswitch_enabled();
    let count = state.account_count();
    let lang = state.get_language();
    let snapshot = TraySnapshot {
        is_active,
        count,
        lang: lang.clone(),
    };

    {
        let mut last = state.last_tray_snapshot.lock();
        if last.as_ref() == Some(&snapshot) {
            return;
        }
        *last = Some(snapshot);
    }

    let tooltip_key = if is_active {
        "tooltip_active"
    } else {
        "tooltip_paused"
    };
    let _ = tray.set_tooltip(Some(tray_t(&lang, tooltip_key)));

    let count_label = tray_accounts_label(&lang, count);
    if let Ok(menu) = build_tray_menu(
        handle,
        &count_label,
        tray_t(&lang, "autoswitch"),
        is_active,
        tray_t(&lang, "show_window"),
        tray_t(&lang, "quit"),
    ) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn build_tray_menu(
    handle: &AppHandle,
    count_label: &str,
    autoswitch_label: &str,
    is_active: bool,
    show_label: &str,
    quit_label: &str,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};

    let count_item = MenuItemBuilder::with_id("count_info", count_label)
        .enabled(false)
        .build(handle)?;
    let toggle_item = CheckMenuItemBuilder::with_id("toggle", autoswitch_label)
        .checked(is_active)
        .build(handle)?;
    let show_item = MenuItemBuilder::with_id("show", show_label).build(handle)?;
    let quit_item = MenuItemBuilder::with_id("quit", quit_label).build(handle)?;

    let menu = MenuBuilder::new(handle)
        .item(&count_item)
        .separator()
        .item(&toggle_item)
        .separator()
        .item(&show_item)
        .item(&quit_item)
        .build()?;

    Ok(menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_t_returns_english_fallback() {
        assert_eq!(tray_t("en", "autoswitch"), "Autoswitch");
        assert_eq!(tray_t("en", "show_window"), "Show Window");
        assert_eq!(tray_t("en", "quit"), "Quit");
        assert_eq!(tray_t("en", "tooltip_active"), "FocusRetro - Active");
        assert_eq!(tray_t("en", "tooltip_paused"), "FocusRetro - Paused");
    }

    #[test]
    fn tray_t_returns_french() {
        assert_eq!(tray_t("fr", "show_window"), "Afficher la fenêtre");
        assert_eq!(tray_t("fr", "quit"), "Quitter");
        assert_eq!(tray_t("fr", "tooltip_active"), "FocusRetro - Actif");
        assert_eq!(tray_t("fr", "tooltip_paused"), "FocusRetro - En pause");
    }

    #[test]
    fn tray_t_returns_spanish() {
        assert_eq!(tray_t("es", "show_window"), "Mostrar ventana");
        assert_eq!(tray_t("es", "quit"), "Salir");
        assert_eq!(tray_t("es", "tooltip_active"), "FocusRetro - Activo");
        assert_eq!(tray_t("es", "tooltip_paused"), "FocusRetro - En pausa");
    }

    #[test]
    fn tray_accounts_label_zero() {
        assert_eq!(tray_accounts_label("en", 0), "No accounts detected");
        assert_eq!(tray_accounts_label("fr", 0), "Aucun compte détecté");
        assert_eq!(tray_accounts_label("es", 0), "Ninguna cuenta detectada");
    }

    #[test]
    fn tray_accounts_label_one() {
        assert_eq!(tray_accounts_label("en", 1), "1 account detected");
        assert_eq!(tray_accounts_label("fr", 1), "1 compte détecté");
        assert_eq!(tray_accounts_label("es", 1), "1 cuenta detectada");
    }

    #[test]
    fn tray_accounts_label_many() {
        assert_eq!(tray_accounts_label("en", 3), "3 accounts detected");
        assert_eq!(tray_accounts_label("fr", 3), "3 comptes détectés");
        assert_eq!(tray_accounts_label("es", 3), "3 cuentas detectadas");
    }

    #[test]
    fn tray_snapshot_eq() {
        let a = TraySnapshot {
            is_active: true,
            count: 2,
            lang: "en".into(),
        };
        let b = TraySnapshot {
            is_active: true,
            count: 2,
            lang: "en".into(),
        };
        let c = TraySnapshot {
            is_active: false,
            count: 2,
            lang: "en".into(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
