#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameWindow {
    pub character_name: String,
    pub window_id: u64,
    pub pid: u32,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub accessibility: bool,
    pub screen_recording: bool,
}

pub trait WindowManager: Send + Sync {
    fn list_dofus_windows(&self) -> Vec<GameWindow>;
    fn focus_window(&self, window: &GameWindow) -> anyhow::Result<()>;
    fn send_enter_key(&self) -> anyhow::Result<()>;
}

pub trait NotificationListener: Send + Sync {
    /// Start listening. Calls `on_notification` with text segments from the notification.
    /// Calls `on_mode` once with either `"event"` or `"poll"` to indicate detection mode.
    fn start(
        &self,
        on_notification: Box<dyn Fn(Vec<String>) -> bool + Send + 'static>,
        on_mode: Box<dyn Fn(String) + Send + 'static>,
    ) -> anyhow::Result<()>;
    fn stop(&self);
}

pub fn create_window_manager() -> Box<dyn WindowManager> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::window::MacWindowManager::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::window::WinWindowManager::new())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Unsupported platform")
    }
}

pub fn create_notification_listener() -> Box<dyn NotificationListener> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::notifications::MacNotificationListener::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::notifications::WinNotificationListener::new())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        compile_error!("Unsupported platform")
    }
}

pub fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::permissions::is_accessibility_enabled()
    }
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

pub fn check_screen_recording_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::permissions::is_screen_recording_enabled()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

pub fn request_screen_recording_permission() {
    #[cfg(target_os = "macos")]
    macos::permissions::request_screen_recording();
}

#[allow(dead_code)]
pub fn request_accessibility_permission() {
    #[cfg(target_os = "macos")]
    macos::permissions::request_accessibility();
}

#[allow(dead_code)]
pub fn stop_notification_listener(listener: &dyn NotificationListener) {
    listener.stop();
}
