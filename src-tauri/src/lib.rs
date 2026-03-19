mod commands;
mod core;
mod platform;
mod radial;
mod state;

use state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

pub fn run() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("focusretro_lib=info"),
    )
    .init();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
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
                    if state.is_close_to_tray() {
                        api.prevent_close();
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
            commands::toggle_pm,
            commands::get_pm_state,
            commands::get_messages,
            commands::clear_messages,
            commands::toggle_auto_accept,
            commands::get_auto_accept_state,
            commands::reorder_account,
            commands::set_principal,
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
        ])
        .setup(|app| {
            let config_path = app
                .path()
                .app_config_dir()?
                .join("config.json");

            crate::state::migrate_config_if_needed(&config_path);

            let app_state = Arc::new(AppState::new(config_path));
            app.manage(app_state);

            setup_tray(app)?;
            start_hotkey_listener(app);
            core::autoswitch::setup(app)?;

            #[cfg(target_os = "macos")]
            setup_radial_panel(app);

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building FocusRetro")
        .run(|app_handle, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { has_visible_windows, .. } = event {
                if !has_visible_windows {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
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

#[cfg(target_os = "macos")]
fn setup_radial_panel(app: &tauri::App) {
    use tauri_nspanel::WebviewWindowExt as NSPanelExt;
    use log::warn;
    match app.get_webview_window("radial-overlay") {
        None => warn!("[Radial] radial-overlay window not found during setup"),
        Some(overlay) => {
            match overlay.to_panel() {
                Err(e) => warn!("[Radial] to_panel() failed: {:?}", e),
                Ok(panel) => {
                    panel.set_level(200);
                    panel.set_becomes_key_only_if_needed(true);
                }
            }
        }
    }
}

fn make_circle_icon(r: u8, g: u8, b: u8, size: u32) -> Vec<u8> {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let center = size as f32 / 2.0;
    let radius = center - 1.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            if dx * dx + dy * dy <= radius * radius {
                let idx = ((y * size + x) * 4) as usize;
                pixels[idx] = r;
                pixels[idx + 1] = g;
                pixels[idx + 2] = b;
                pixels[idx + 3] = 255;
            }
        }
    }
    pixels
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::TrayIconBuilder;

    let state = app.state::<Arc<AppState>>().inner().clone();
    let is_active = state.is_autoswitch_enabled();

    let principal_label = state
        .get_principal_name()
        .map(|n| format!("{} (principal)", n))
        .unwrap_or_else(|| "No principal".into());
    let count = state.account_count();
    let count_label = format!("{} accounts detected", count);

    let principal_item = MenuItemBuilder::with_id("principal_info", &principal_label)
        .enabled(false)
        .build(app)?;
    let count_item = MenuItemBuilder::with_id("count_info", &count_label)
        .enabled(false)
        .build(app)?;

    let toggle_label = if is_active {
        "Autoswitch ON"
    } else {
        "Autoswitch OFF"
    };
    let toggle_item = MenuItemBuilder::with_id("toggle", toggle_label).build(app)?;
    let show_item = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&principal_item)
        .item(&count_item)
        .separator()
        .item(&toggle_item)
        .separator()
        .item(&show_item)
        .item(&quit_item)
        .build()?;

    let icon_pixels = if is_active {
        make_circle_icon(246, 168, 0, 22) // brand-500
    } else {
        make_circle_icon(107, 114, 128, 22) // gray
    };
    let icon = tauri::image::Image::new_owned(icon_pixels, 22, 22);

    let tooltip = if is_active {
        "FocusRetro — Active"
    } else {
        "FocusRetro — Paused"
    };

    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .tooltip(tooltip)
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

    let icon_pixels = if is_active {
        make_circle_icon(99, 102, 241, 22)
    } else {
        make_circle_icon(107, 114, 128, 22)
    };
    let icon = tauri::image::Image::new_owned(icon_pixels, 22, 22);
    let _ = tray.set_icon(Some(icon));

    let tooltip = if is_active {
        "FocusRetro — Active"
    } else {
        "FocusRetro — Paused"
    };
    let _ = tray.set_tooltip(Some(tooltip));

    let principal_label = state
        .get_principal_name()
        .map(|n| format!("{} (principal)", n))
        .unwrap_or_else(|| "No principal".into());
    let count = state.account_count();
    let count_label = format!("{} accounts detected", count);
    let toggle_label = if is_active {
        "Autoswitch ON"
    } else {
        "Autoswitch OFF"
    };

    if let Ok(menu) = build_tray_menu(handle, &principal_label, &count_label, toggle_label) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn build_tray_menu(
    handle: &AppHandle,
    principal_label: &str,
    count_label: &str,
    toggle_label: &str,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};

    let principal_item = MenuItemBuilder::with_id("principal_info", principal_label)
        .enabled(false)
        .build(handle)?;
    let count_item = MenuItemBuilder::with_id("count_info", count_label)
        .enabled(false)
        .build(handle)?;
    let toggle_item = MenuItemBuilder::with_id("toggle", toggle_label).build(handle)?;
    let show_item = MenuItemBuilder::with_id("show", "Show Window").build(handle)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(handle)?;

    let menu = MenuBuilder::new(handle)
        .item(&principal_item)
        .item(&count_item)
        .separator()
        .item(&toggle_item)
        .separator()
        .item(&show_item)
        .item(&quit_item)
        .build()?;

    Ok(menu)
}
