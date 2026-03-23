use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

pub struct BackendReady {
    pub is_ready: AtomicBool,
    pub notify: Notify,
}

impl Default for BackendReady {
    fn default() -> Self {
        Self {
            is_ready: AtomicBool::new(false),
            notify: Notify::new(),
        }
    }
}

impl BackendReady {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn signal(&self) {
        self.is_ready.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }
}
