use crate::platform::GameWindow;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub receiver: String,
    pub sender: String,
    pub message: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    pub event_type: String,
    pub character_name: String,
    pub t_notification_ms: u64,
    pub t_parsed_ms: u64,
    pub t_focus_triggered_ms: u64,
    pub t_focus_done_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountProfile {
    pub character_name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon_path: Option<String>,
    #[serde(default)]
    pub is_principal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountView {
    pub character_name: String,
    pub window_id: u64,
    pub pid: u32,
    pub title: String,
    pub color: Option<String>,
    pub icon_path: Option<String>,
    pub is_principal: bool,
    pub is_current: bool,
    pub position: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyBinding {
    pub action: String,
    pub key: String,
    pub cmd: bool,
    pub alt: bool,
    pub shift: bool,
    pub ctrl: bool,
}

fn default_hotkeys() -> Vec<HotkeyBinding> {
    vec![
        HotkeyBinding {
            action: "prev".into(),
            key: "F1".into(),
            cmd: false,
            alt: false,
            shift: false,
            ctrl: false,
        },
        HotkeyBinding {
            action: "next".into(),
            key: "F2".into(),
            cmd: false,
            alt: false,
            shift: false,
            ctrl: false,
        },
        HotkeyBinding {
            action: "principal".into(),
            key: "F3".into(),
            cmd: false,
            alt: false,
            shift: false,
            ctrl: false,
        },
        HotkeyBinding {
            action: "radial".into(),
            key: "".into(),
            cmd: false,
            alt: false,
            shift: false,
            ctrl: false,
        },
    ]
}

fn detect_system_language() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLanguages"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for lang in ["fr", "es", "en"] {
                if text.contains(lang) {
                    return lang.into();
                }
            }
        }
    }
    if let Ok(lang) = std::env::var("LANG") {
        let lower = lang.to_lowercase();
        if lower.starts_with("fr") {
            return "fr".into();
        }
        if lower.starts_with("es") {
            return "es".into();
        }
    }
    "en".into()
}

fn default_language() -> String {
    detect_system_language()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub autoswitch_enabled: bool,
    pub group_invite_enabled: bool,
    pub trade_enabled: bool,
    pub pm_enabled: bool,
    pub auto_accept_enabled: bool,
    pub show_debug: bool,
    pub profiles: Vec<AccountProfile>,
    pub hotkeys: Vec<HotkeyBinding>,
    pub language: String,
    pub theme: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            autoswitch_enabled: true,
            group_invite_enabled: true,
            trade_enabled: true,
            pm_enabled: true,
            auto_accept_enabled: false,
            show_debug: false,
            profiles: Vec::new(),
            hotkeys: default_hotkeys(),
            language: default_language(),
            theme: "system".into(),
        }
    }
}

fn prefs_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".focusretro").join("config.json")
}

fn load_preferences() -> Preferences {
    let path = prefs_path();
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
            error!("[Prefs] Failed to parse {}: {}", path.display(), e);
            Preferences::default()
        }),
        Err(_) => {
            info!(
                "[Prefs] No config found at {}, using defaults",
                path.display()
            );
            Preferences::default()
        }
    }
}

fn save_preferences(prefs: &Preferences) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(prefs) {
        Ok(data) => {
            if let Err(e) = std::fs::write(&path, data) {
                error!("[Prefs] Failed to write {}: {}", path.display(), e);
            }
        }
        Err(e) => error!("[Prefs] Failed to serialize prefs: {}", e),
    }
}

pub struct AppState {
    pub autoswitch_enabled: AtomicBool,
    pub group_invite_enabled: AtomicBool,
    pub trade_enabled: AtomicBool,
    pub pm_enabled: AtomicBool,
    pub auto_accept_enabled: AtomicBool,
    pub show_debug: AtomicBool,
    pub radial_open: AtomicBool,
    pub radial_center: Mutex<Option<(f64, f64)>>,
    pub last_foreground_id: AtomicU64,
    pub profiles: Mutex<Vec<AccountProfile>>,
    pub accounts: Mutex<Vec<GameWindow>>,
    pub current_index: Mutex<usize>,
    pub messages: Mutex<Vec<StoredMessage>>,
    pub hotkeys: Mutex<Vec<HotkeyBinding>>,
    pub language: Mutex<String>,
    pub traces: Mutex<Vec<TraceEntry>>,
    pub notif_mode: Mutex<String>,
    pub theme: Mutex<String>,
}

impl AppState {
    pub fn new() -> Self {
        let prefs = load_preferences();
        info!("[Prefs] Loaded {} profiles", prefs.profiles.len());
        // Start from loaded hotkeys, then add any default actions that are missing.
        // This ensures new actions (e.g. "radial") appear on first launch after an upgrade.
        let mut hotkeys = if prefs.hotkeys.is_empty() {
            default_hotkeys()
        } else {
            prefs.hotkeys
        };
        for default_hk in default_hotkeys() {
            if !hotkeys.iter().any(|h| h.action == default_hk.action) {
                hotkeys.push(default_hk);
            }
        }
        Self {
            autoswitch_enabled: AtomicBool::new(prefs.autoswitch_enabled),
            group_invite_enabled: AtomicBool::new(prefs.group_invite_enabled),
            trade_enabled: AtomicBool::new(prefs.trade_enabled),
            pm_enabled: AtomicBool::new(prefs.pm_enabled),
            auto_accept_enabled: AtomicBool::new(prefs.auto_accept_enabled),
            show_debug: AtomicBool::new(prefs.show_debug),
            radial_open: AtomicBool::new(false),
            radial_center: Mutex::new(None),
            last_foreground_id: AtomicU64::new(0),
            profiles: Mutex::new(prefs.profiles),
            accounts: Mutex::new(Vec::new()),
            current_index: Mutex::new(0),
            messages: Mutex::new(Vec::new()),
            hotkeys: Mutex::new(hotkeys),
            language: Mutex::new(prefs.language),
            traces: Mutex::new(Vec::new()),
            notif_mode: Mutex::new("unknown".into()),
            theme: Mutex::new(prefs.theme),
        }
    }

    fn save(&self) {
        let prefs = Preferences {
            autoswitch_enabled: self.autoswitch_enabled.load(Ordering::Relaxed),
            group_invite_enabled: self.group_invite_enabled.load(Ordering::Relaxed),
            trade_enabled: self.trade_enabled.load(Ordering::Relaxed),
            pm_enabled: self.pm_enabled.load(Ordering::Relaxed),
            auto_accept_enabled: self.auto_accept_enabled.load(Ordering::Relaxed),
            show_debug: self.show_debug.load(Ordering::Relaxed),
            profiles: self.profiles.lock().unwrap().clone(),
            hotkeys: self.hotkeys.lock().unwrap().clone(),
            language: self.language.lock().unwrap().clone(),
            theme: self.theme.lock().unwrap().clone(),
        };
        std::thread::spawn(move || {
            save_preferences(&prefs);
        });
    }

    pub fn is_autoswitch_enabled(&self) -> bool {
        self.autoswitch_enabled.load(Ordering::Relaxed)
    }

    pub fn set_autoswitch(&self, enabled: bool) {
        self.autoswitch_enabled.store(enabled, Ordering::Relaxed);
        self.save();
    }

    pub fn is_group_invite_enabled(&self) -> bool {
        self.group_invite_enabled.load(Ordering::Relaxed)
    }

    pub fn set_group_invite(&self, enabled: bool) {
        self.group_invite_enabled.store(enabled, Ordering::Relaxed);
        self.save();
    }

    pub fn is_trade_enabled(&self) -> bool {
        self.trade_enabled.load(Ordering::Relaxed)
    }

    pub fn set_trade(&self, enabled: bool) {
        self.trade_enabled.store(enabled, Ordering::Relaxed);
        self.save();
    }

    pub fn is_pm_enabled(&self) -> bool {
        self.pm_enabled.load(Ordering::Relaxed)
    }

    pub fn set_pm(&self, enabled: bool) {
        self.pm_enabled.store(enabled, Ordering::Relaxed);
        self.save();
    }

    pub fn is_auto_accept_enabled(&self) -> bool {
        self.auto_accept_enabled.load(Ordering::Relaxed)
    }

    pub fn set_auto_accept(&self, enabled: bool) {
        self.auto_accept_enabled.store(enabled, Ordering::Relaxed);
        self.save();
    }

    pub fn is_show_debug(&self) -> bool {
        self.show_debug.load(Ordering::Relaxed)
    }

    pub fn set_show_debug(&self, enabled: bool) {
        self.show_debug.store(enabled, Ordering::Relaxed);
        self.save();
    }

    pub fn get_hotkeys(&self) -> Vec<HotkeyBinding> {
        self.hotkeys.lock().unwrap().clone()
    }

    pub fn reset_hotkeys(&self) {
        *self.hotkeys.lock().unwrap() = default_hotkeys();
        self.save();
    }

    pub fn set_hotkey(&self, action: &str, key: String, cmd: bool, alt: bool, shift: bool, ctrl: bool) {
        let mut hotkeys = self.hotkeys.lock().unwrap();
        if let Some(hk) = hotkeys.iter_mut().find(|h| h.action == action) {
            hk.key = key;
            hk.cmd = cmd;
            hk.alt = alt;
            hk.shift = shift;
            hk.ctrl = ctrl;
        } else {
            hotkeys.push(HotkeyBinding { action: action.into(), key, cmd, alt, shift, ctrl });
        }
        drop(hotkeys);
        self.save();
    }

    pub fn get_language(&self) -> String {
        self.language.lock().unwrap().clone()
    }

    pub fn set_language(&self, lang: String) {
        *self.language.lock().unwrap() = lang;
        self.save();
    }

    pub fn get_theme(&self) -> String {
        self.theme.lock().unwrap().clone()
    }

    pub fn set_theme(&self, theme: String) {
        *self.theme.lock().unwrap() = theme;
        self.save();
    }

    pub fn update_accounts(&self, windows: Vec<GameWindow>) {
        let mut profiles = self.profiles.lock().unwrap();
        let mut accounts = self.accounts.lock().unwrap();

        // Add newly detected windows not yet in profiles (preserves existing profiles)
        let mut new_profiles_added = false;
        for win in &windows {
            if !profiles.iter().any(|p| p.character_name.eq_ignore_ascii_case(&win.character_name)) {
                profiles.push(AccountProfile {
                    character_name: win.character_name.clone(),
                    color: None,
                    icon_path: None,
                    is_principal: false,
                });
                new_profiles_added = true;
            }
        }

        // Rebuild accounts in profile order, only for currently open windows
        *accounts = profiles
            .iter()
            .filter_map(|p| {
                windows.iter().find(|w| w.character_name.eq_ignore_ascii_case(&p.character_name)).cloned()
            })
            .collect();

        if !profiles.is_empty() && !profiles.iter().any(|p| p.is_principal) {
            profiles[0].is_principal = true;
        }

        drop(profiles);
        drop(accounts);

        if new_profiles_added {
            self.save();
        }
    }

    #[allow(dead_code)]
    pub fn get_accounts(&self) -> Vec<GameWindow> {
        self.accounts.lock().unwrap().clone()
    }

    pub fn get_account_views(&self) -> Vec<AccountView> {
        let profiles = self.profiles.lock().unwrap();
        let accounts = self.accounts.lock().unwrap();
        let current_idx = *self.current_index.lock().unwrap();

        accounts
            .iter()
            .enumerate()
            .map(|(i, win)| {
                let profile = profiles
                    .iter()
                    .find(|p| p.character_name.eq_ignore_ascii_case(&win.character_name));
                AccountView {
                    character_name: win.character_name.clone(),
                    window_id: win.window_id,
                    pid: win.pid,
                    title: win.title.clone(),
                    color: profile.and_then(|p| p.color.clone()),
                    icon_path: profile.and_then(|p| p.icon_path.clone()),
                    is_principal: profile.is_some_and(|p| p.is_principal),
                    is_current: i == current_idx,
                    position: i,
                }
            })
            .collect()
    }

    pub fn has_account(&self, name: &str) -> bool {
        self.accounts
            .lock()
            .unwrap()
            .iter()
            .any(|w| w.character_name.eq_ignore_ascii_case(name))
    }

    pub fn reorder_account(&self, name: &str, new_position: usize) -> bool {
        let mut profiles = self.profiles.lock().unwrap();
        let mut accounts = self.accounts.lock().unwrap();

        // Find and remove the source profile
        let source_profile_idx = match profiles.iter().position(|p| p.character_name.eq_ignore_ascii_case(name)) {
            Some(i) => i,
            None => return false,
        };
        let profile = profiles.remove(source_profile_idx);

        // Build the ordered list of open account names after removal (in profile order)
        let open_after_removal: Vec<String> = profiles
            .iter()
            .filter(|p| accounts.iter().any(|w| w.character_name.eq_ignore_ascii_case(&p.character_name)))
            .map(|p| p.character_name.clone())
            .collect();

        // Map the visual new_position to an insertion index in the full profiles list
        let insert_profile_idx = if new_position >= open_after_removal.len() {
            profiles.len()
        } else {
            let target_name = &open_after_removal[new_position];
            match profiles.iter().position(|p| p.character_name.eq_ignore_ascii_case(target_name)) {
                Some(i) => i,
                None => {
                    profiles.insert(source_profile_idx, profile);
                    return false;
                }
            }
        };

        profiles.insert(insert_profile_idx, profile);

        // Rebuild accounts in new profile order
        let old_accounts = accounts.clone();
        *accounts = profiles
            .iter()
            .filter_map(|p| old_accounts.iter().find(|w| w.character_name.eq_ignore_ascii_case(&p.character_name)).cloned())
            .collect();

        drop(profiles);
        drop(accounts);
        self.save();
        true
    }

    pub fn set_principal(&self, name: &str) {
        let mut profiles = self.profiles.lock().unwrap();
        for p in profiles.iter_mut() {
            p.is_principal = p.character_name.eq_ignore_ascii_case(name);
        }
        drop(profiles);
        self.save();
    }

    pub fn get_principal(&self) -> Option<GameWindow> {
        let profiles = self.profiles.lock().unwrap();
        let accounts = self.accounts.lock().unwrap();
        profiles
            .iter()
            .find(|p| p.is_principal)
            .and_then(|p| {
                accounts.iter().find(|w| w.character_name.eq_ignore_ascii_case(&p.character_name))
            })
            .or_else(|| accounts.first())
            .cloned()
    }

    pub fn get_principal_name(&self) -> Option<String> {
        let profiles = self.profiles.lock().unwrap();
        profiles.iter().find(|p| p.is_principal).map(|p| p.character_name.clone())
    }

    pub fn account_count(&self) -> usize {
        self.accounts.lock().unwrap().len()
    }

    pub fn update_profile(&self, name: &str, color: Option<String>, icon_path: Option<String>) {
        let mut profiles = self.profiles.lock().unwrap();
        if let Some(p) = profiles.iter_mut().find(|p| p.character_name.eq_ignore_ascii_case(name)) {
            p.color = color;
            p.icon_path = icon_path;
        }
        drop(profiles);
        self.save();
    }

    #[allow(dead_code)]
    pub fn get_profiles(&self) -> Vec<AccountProfile> {
        self.profiles.lock().unwrap().clone()
    }

    pub fn add_message(&self, msg: StoredMessage) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(msg);
        if messages.len() > 500 {
            messages.drain(0..100);
        }
    }

    pub fn get_messages(&self) -> Vec<StoredMessage> {
        self.messages.lock().unwrap().clone()
    }

    pub fn clear_messages(&self) {
        self.messages.lock().unwrap().clear();
    }

    pub fn set_current_by_name(&self, name: &str) {
        let accounts = self.accounts.lock().unwrap();
        if let Some(idx) = accounts.iter().position(|w| w.character_name.eq_ignore_ascii_case(name)) {
            *self.current_index.lock().unwrap() = idx;
        }
    }

    pub fn get_current_window(&self) -> Option<GameWindow> {
        let accounts = self.accounts.lock().unwrap();
        if accounts.is_empty() {
            return None;
        }
        let idx = *self.current_index.lock().unwrap();
        accounts.get(idx).cloned()
    }

    pub fn set_radial_center(&self, x: f64, y: f64) {
        *self.radial_center.lock().unwrap() = Some((x, y));
    }

    pub fn get_radial_center(&self) -> Option<(f64, f64)> {
        *self.radial_center.lock().unwrap()
    }

    pub fn sync_current_from_window_id(&self, window_id: u64) {
        let accounts = self.accounts.lock().unwrap();
        if let Some(idx) = accounts.iter().position(|w| w.window_id == window_id) {
            *self.current_index.lock().unwrap() = idx;
        }
    }

    pub fn cycle_next(&self) -> Option<GameWindow> {
        let accounts = self.accounts.lock().unwrap();
        if accounts.is_empty() {
            return None;
        }
        let mut idx = self.current_index.lock().unwrap();
        *idx = (*idx + 1) % accounts.len();
        Some(accounts[*idx].clone())
    }

    pub fn cycle_prev(&self) -> Option<GameWindow> {
        let accounts = self.accounts.lock().unwrap();
        if accounts.is_empty() {
            return None;
        }
        let mut idx = self.current_index.lock().unwrap();
        *idx = if *idx == 0 { accounts.len() - 1 } else { *idx - 1 };
        Some(accounts[*idx].clone())
    }

    pub fn add_trace(&self, entry: TraceEntry) {
        let mut traces = self.traces.lock().unwrap();
        if traces.len() >= 100 {
            traces.remove(0);
        }
        traces.push(entry);
    }

    pub fn get_traces(&self) -> Vec<TraceEntry> {
        self.traces.lock().unwrap().clone()
    }

    pub fn clear_traces(&self) {
        self.traces.lock().unwrap().clear();
    }

    pub fn set_notif_mode(&self, mode: String) {
        *self.notif_mode.lock().unwrap() = mode;
    }

    pub fn get_notif_mode(&self) -> String {
        self.notif_mode.lock().unwrap().clone()
    }
}
