use crate::platform::NotificationListener;
use log::{error, info, warn};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::{mpsc, Arc, LazyLock};

use regex::Regex;

static RE_TOAST_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<text[^>]*>([^<]*)</text>").unwrap());

use windows::core::HSTRING;
use windows::Foundation::TypedEventHandler;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::{
    GetCurrentThread, GetCurrentThreadId, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL,
};
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{
    NotificationKinds, UserNotification, UserNotificationChangedEventArgs,
    UserNotificationChangedKind,
};

/// Wraps a `Send`-only `Fn` so it can be shared across WinRT threads.
/// Safety: `Fn` uses `&self`, so concurrent calls are safe given captured data is `Send`.
struct SyncCallback(Box<dyn Fn(Vec<String>) -> bool + Send + 'static>);
unsafe impl Sync for SyncCallback {}

pub struct WinNotificationListener {
    thread_id: Arc<Mutex<Option<u32>>>,
}

impl WinNotificationListener {
    pub fn new() -> Self {
        Self {
            thread_id: Arc::new(Mutex::new(None)),
        }
    }
}

/// Extract text segments from a notification on the calling thread.
///
/// Must be called on the same STA thread that owns `listener` — `GetNotification` cannot
/// be marshaled to a different apartment without a running message pump, which would cause
/// `RPC_E_WRONG_THREAD` (0x8001010E). Only the autoswitch callback is dispatched off-thread.
fn extract_segments(listener: &UserNotificationListener, notif_id: u32) -> Option<Vec<String>> {
    let notif = match listener.GetNotification(notif_id) {
        Ok(n) => n,
        Err(e) => {
            error!("[WinNotif] GetNotification({}) failed: {:?}", notif_id, e);
            return None;
        }
    };

    let notification = match notif.Notification() {
        Ok(n) => n,
        Err(e) => {
            error!("[WinNotif] notif.Notification() failed: {:?}", e);
            return None;
        }
    };

    let visual = match notification.Visual() {
        Ok(v) => v,
        Err(e) => {
            error!("[WinNotif] notification.Visual() failed: {:?}", e);
            return None;
        }
    };

    let binding = match visual.GetBinding(&HSTRING::from("ToastGeneric")) {
        Ok(b) => b,
        Err(e) => {
            error!("[WinNotif] GetBinding(ToastGeneric) failed: {:?}", e);
            return None;
        }
    };

    let elements = match binding.GetTextElements() {
        Ok(e) => e,
        Err(e) => {
            error!("[WinNotif] GetTextElements() failed: {:?}", e);
            return None;
        }
    };

    let count = elements.Size().unwrap_or(0);
    info!(
        "[WinNotif] Notification {} has {} text element(s)",
        notif_id, count
    );

    let mut texts: Vec<String> = Vec::new();
    for i in 0..count {
        match elements.GetAt(i) {
            Ok(elem) => match elem.Text() {
                Ok(t) => {
                    let s = t.to_string();
                    info!("[WinNotif]   text[{}]: {:?}", i, s);
                    if !s.is_empty() {
                        texts.push(s);
                    }
                }
                Err(e) => error!("[WinNotif] elem.Text() failed at {}: {:?}", i, e),
            },
            Err(e) => error!("[WinNotif] GetAt({}) failed: {:?}", i, e),
        }
    }

    if texts.is_empty() {
        warn!("[WinNotif] Notification {} had no text, skipping", notif_id);
        return None;
    }

    let title = texts[0].clone();
    let body = texts.get(1).cloned().unwrap_or_default();
    let combined = if body.is_empty() {
        title.clone()
    } else {
        format!("{} {}", title, body)
    };
    // segments[0]: "Dofus Retro" is used as the source tag on Windows (both WinRT and DB paths).
    // The parser never reads segments[0] — it iterates in reverse over segments[1..] — so this
    // differs from the macOS convention ("Notification Center") without affecting behavior.
    Some(vec!["Dofus Retro".to_string(), combined, title, body])
}

fn seed_last_id(conn: &rusqlite::Connection) -> u64 {
    conn.query_row("SELECT COALESCE(MAX(Id), 0) FROM Notification", [], |row| {
        row.get::<_, i64>(0)
    })
    .map(|v| v as u64)
    .unwrap_or(0)
}

/// Returns `false` if the connection appears broken (caller should reopen).
fn poll_db_notifications(
    conn: &rusqlite::Connection,
    last_id: &mut u64,
    cb: &SyncCallback,
) -> bool {
    let mut stmt = match conn
        .prepare("SELECT Id, CAST(Payload AS TEXT) FROM Notification WHERE Id > ?1 ORDER BY Id ASC")
    {
        Ok(s) => s,
        Err(e) => {
            warn!("[WinNotif] poll: prepare failed: {:?}", e);
            return false;
        }
    };

    let rows: Vec<(u64, String)> = match stmt.query_map([*last_id as i64], |row| {
        Ok((row.get::<_, i64>(0)? as u64, row.get::<_, String>(1)?))
    }) {
        Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            warn!("[WinNotif] poll: query failed: {:?}", e);
            return false;
        }
    };

    for (id, payload) in rows {
        let texts: Vec<String> = RE_TOAST_TEXT
            .captures_iter(&payload)
            .map(|cap| cap[1].trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if texts.is_empty() {
            if id > *last_id {
                *last_id = id;
            }
            continue;
        }

        let title = texts[0].clone();
        let body = texts.get(1).cloned().unwrap_or_default();
        let combined = if body.is_empty() {
            title.clone()
        } else {
            format!("{} {}", title, body)
        };

        // segments[0]: "Dofus Retro" is used as the source tag on Windows (both WinRT and DB paths).
        // The parser never reads segments[0] — it iterates in reverse over segments[1..] — so this
        // differs from the macOS convention ("Notification Center") without affecting behavior.
        let segments = vec!["Dofus Retro".to_string(), combined, title, body];
        (cb.0)(segments);
        // Advance last_id only after the callback has returned to avoid skipping rows on panic.
        if id > *last_id {
            *last_id = id;
        }
    }
    true
}

impl NotificationListener for WinNotificationListener {
    fn start(
        &self,
        on_notification: Box<dyn Fn(Vec<String>) -> bool + Send + 'static>,
        on_mode: Box<dyn Fn(String) + Send + 'static>,
    ) -> anyhow::Result<()> {
        let thread_id_store = Arc::clone(&self.thread_id);
        let callback = Arc::new(SyncCallback(on_notification));

        // `done_tx` is sent exactly once when the listener thread exits:
        //   Ok(())  — clean shutdown via WM_QUIT
        //   Err(_)  — unexpected failure (API error, DB unavailable, etc.)
        //
        // `start()` blocks on `done_rx` so the watchdog in autoswitch.rs can detect
        // thread exit and retry on error, matching the macOS AXObserver reconnect behavior.
        let (done_tx, done_rx) = mpsc::channel::<anyhow::Result<()>>();

        std::thread::spawn(move || {
            let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            if hr.is_err() {
                error!("[WinNotif] CoInitializeEx failed: {:?}", hr);
                done_tx
                    .send(Err(anyhow::anyhow!("CoInitializeEx failed: {:?}", hr)))
                    .ok();
                return;
            }
            // Guard covers all return paths in this thread closure.
            let _com = crate::platform::OnDrop::new(|| unsafe { CoUninitialize() });
            // Boost scheduling priority so poll wakeups are prompt even under system load.
            if let Err(e) =
                unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL) }
            {
                warn!("[WinNotif] SetThreadPriority failed: {:?}", e);
            }

            let tid = unsafe { GetCurrentThreadId() };
            {
                let mut guard = thread_id_store.lock();
                *guard = Some(tid);
            }

            let listener = match UserNotificationListener::Current() {
                Ok(l) => l,
                Err(e) => {
                    error!("[WinNotif] Failed to get UserNotificationListener: {:?}", e);
                    done_tx
                        .send(Err(anyhow::anyhow!(
                            "UserNotificationListener::Current() failed: {:?}",
                            e
                        )))
                        .ok();
                    return;
                }
            };

            let status = match listener.RequestAccessAsync().and_then(|op| op.get()) {
                Ok(s) => s,
                Err(e) => {
                    error!("[WinNotif] Failed to request notification access: {:?}", e);
                    done_tx
                        .send(Err(anyhow::anyhow!("RequestAccessAsync failed: {:?}", e)))
                        .ok();
                    return;
                }
            };

            if status != UserNotificationListenerAccessStatus::Allowed {
                warn!(
                    "[WinNotif] Notification access not granted (status: {:?}), trying DB fallback",
                    status
                );

                let db_path = match std::env::var("LOCALAPPDATA") {
                    Ok(p) => std::path::PathBuf::from(p)
                        .join("Microsoft")
                        .join("Windows")
                        .join("Notifications")
                        .join("wpndatabase.db"),
                    Err(_) => {
                        error!("[WinNotif] LOCALAPPDATA not set");
                        done_tx
                            .send(Err(anyhow::anyhow!("LOCALAPPDATA env var not set")))
                            .ok();
                        return;
                    }
                };

                use rusqlite::{Connection, OpenFlags};
                let mut conn = match Connection::open_with_flags(
                    &db_path,
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                ) {
                    Ok(c) => c,
                    Err(e) => {
                        error!("[WinNotif] poll-db: cannot open DB: {:?}", e);
                        done_tx
                            .send(Err(anyhow::anyhow!("Cannot open notification DB: {:?}", e)))
                            .ok();
                        return;
                    }
                };
                let _ = conn.busy_timeout(std::time::Duration::from_millis(1000));

                on_mode("poll-db".into());
                let mut last_id = seed_last_id(&conn);
                let poll_interval = std::time::Duration::from_millis(200);
                let mut consecutive_failures: u32 = 0;

                loop {
                    std::thread::sleep(poll_interval);

                    if poll_db_notifications(&conn, &mut last_id, &callback) {
                        consecutive_failures = 0;
                    } else {
                        consecutive_failures += 1;
                        // After 3 consecutive failures (~600ms), the connection is likely stale
                        // (e.g., notification service restarted the DB after sleep/wake).
                        // Reopen to recover automatically.
                        if consecutive_failures >= 3 {
                            warn!(
                                "[WinNotif] poll-db: {} consecutive failures, reopening connection",
                                consecutive_failures
                            );
                            match Connection::open_with_flags(
                                &db_path,
                                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                            ) {
                                Ok(new_conn) => {
                                    let _ = new_conn
                                        .busy_timeout(std::time::Duration::from_millis(1000));
                                    conn = new_conn;
                                    consecutive_failures = 0;
                                    info!("[WinNotif] poll-db: connection reopened successfully");
                                }
                                Err(e) => {
                                    warn!("[WinNotif] poll-db: reopen failed: {:?}", e);
                                }
                            }
                        }
                    }

                    unsafe {
                        use windows::Win32::UI::WindowsAndMessaging::{
                            PeekMessageW, MSG, PM_REMOVE,
                        };
                        let mut msg = MSG::default();
                        if PeekMessageW(&mut msg, None, WM_QUIT, WM_QUIT, PM_REMOVE).as_bool() {
                            break;
                        }
                    }
                }
                done_tx.send(Ok(())).ok();
                info!("[WinNotif] DB poll thread exiting");
                return;
            }

            // Shared seen_ids between event handler and poll loop so they never
            // double-fire the same notification.
            let seen_ids = Arc::new(Mutex::new(HashSet::<u32>::new()));

            // Seed with notifications already present so we don't replay old ones.
            if let Ok(op) = listener.GetNotificationsAsync(NotificationKinds::Toast) {
                if let Ok(existing) = op.get() {
                    let count = existing.Size().unwrap_or(0);
                    let mut ids = seen_ids.lock();
                    for i in 0..count {
                        if let Ok(n) = existing.GetAt(i) {
                            if let Ok(id) = n.Id() {
                                ids.insert(id);
                            }
                        }
                    }
                    info!("[WinNotif] Seeded {} existing notification IDs", ids.len());
                }
            }

            // Processor thread: runs the autoswitch callback off the poll hot path so the poll
            // loop is never blocked by focus logic or auto-accept delays.
            // Receives pre-extracted Vec<String> segments — NO WinRT calls here.
            // All GetNotification / text-extraction calls stay on the STA listener thread to
            // avoid RPC_E_WRONG_THREAD (cross-apartment marshal without a message pump).
            let (tx, rx) = mpsc::channel::<Vec<String>>();
            let cb_p = Arc::clone(&callback);
            std::thread::spawn(move || {
                if let Err(e) =
                    unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL) }
                {
                    warn!("[WinNotif] Processor SetThreadPriority failed: {:?}", e);
                }
                while let Ok(segments) = rx.recv() {
                    (cb_p.0)(segments);
                }
                info!("[WinNotif] Processor thread exiting");
            });

            // Try event-based detection first (requires MSIX package identity — falls back to
            // polling if unavailable). The event handler only marks the ID as seen so the poll
            // loop picks it up on the next cycle via GetNotificationsAsync (still on the STA
            // thread). We do not call GetNotification from the MTA thread-pool callback to avoid
            // the same RPC_E_WRONG_THREAD issue.
            let event_seen_ids = Arc::clone(&seen_ids);
            let event_result = listener.NotificationChanged(&TypedEventHandler::<
                UserNotificationListener,
                UserNotificationChangedEventArgs,
            >::new(move |_, args| {
                if let Some(args) = &*args {
                    if args.ChangeKind()? == UserNotificationChangedKind::Added {
                        let id = args.UserNotificationId()?;
                        // Insert but do NOT extract segments here — extraction happens on the
                        // STA poll thread. Removing the ID from seen_ids is intentionally
                        // skipped: the poll loop will call extract_segments for any ID not yet
                        // seen, so we must not pre-insert it here.
                        info!(
                            "[WinNotif] Event: new notification ID {} (deferred to poll)",
                            id
                        );
                        let _ = id; // future: wake poll early via a condvar when MSIX is enabled
                    }
                }
                Ok(())
            }));

            // Keep the token alive for the entire poll loop — dropping it would unregister the
            // event handler before it ever fires. Using `ok()` instead of a match arm prevents
            // the token from being scoped to the arm and dropped prematurely.
            let _event_token = event_result.ok();
            let poll_interval = if _event_token.is_some() {
                info!("[WinNotif] Subscribed to NotificationChanged — poll is backup only (200ms)");
                on_mode("event".into());
                std::time::Duration::from_millis(200)
            } else {
                info!("[WinNotif] NotificationChanged unavailable (unpackaged app), poll-only mode (100ms)");
                on_mode("poll".into());
                std::time::Duration::from_millis(100)
            };

            loop {
                std::thread::sleep(poll_interval);

                let notifications = match listener
                    .GetNotificationsAsync(NotificationKinds::Toast)
                    .and_then(|op| op.get())
                {
                    Ok(n) => n,
                    Err(e) => {
                        error!("[WinNotif] GetNotificationsAsync failed: {:?}", e);
                        // Signal an error so the watchdog in autoswitch.rs retries.
                        drop(tx);
                        done_tx
                            .send(Err(anyhow::anyhow!(
                                "GetNotificationsAsync failed: {:?}",
                                e
                            )))
                            .ok();
                        return;
                    }
                };

                let count = notifications.Size().unwrap_or(0);
                // Collect new IDs with the lock held (fast), then release before WinRT calls.
                let mut new_notif_ids: Vec<u32> = Vec::new();
                {
                    let mut ids = seen_ids.lock();
                    for i in 0..count {
                        let notif: UserNotification = match notifications.GetAt(i) {
                            Ok(n) => n,
                            Err(_) => continue,
                        };
                        let id = match notif.Id() {
                            Ok(id) => id,
                            Err(_) => continue,
                        };
                        if ids.insert(id) {
                            new_notif_ids.push(id);
                        }
                    }
                }

                if !new_notif_ids.is_empty() {
                    info!(
                        "[WinNotif] Poll: {} new notification(s)",
                        new_notif_ids.len()
                    );
                    // Extract segments on this STA thread — GetNotification must not be called
                    // from a different apartment. Send all new segments so a Dofus notification
                    // is never dropped because another app posted in the same poll window.
                    for id in new_notif_ids {
                        if let Some(segments) = extract_segments(&listener, id) {
                            tx.send(segments).ok();
                        }
                    }
                }

                unsafe {
                    use windows::Win32::UI::WindowsAndMessaging::{PeekMessageW, MSG, PM_REMOVE};
                    let mut msg = MSG::default();
                    if PeekMessageW(&mut msg, None, WM_QUIT, WM_QUIT, PM_REMOVE).as_bool() {
                        break;
                    }
                }
            }

            // Dropping tx causes rx.recv() in the processor thread to return Err, triggering a clean exit.
            drop(tx);
            done_tx.send(Ok(())).ok();
            info!("[WinNotif] Notification listener thread exiting");
        });

        // Block until the listener thread exits so the watchdog in autoswitch.rs can detect
        // unexpected failures and restart. Ok(()) on WM_QUIT (watchdog breaks); Err on failure
        // (watchdog sleeps 2s and retries). Treat a dropped sender (thread panic) as an error.
        done_rx
            .recv()
            .unwrap_or_else(|_| Err(anyhow::anyhow!("listener thread exited unexpectedly")))
    }

    fn stop(&self) {
        let tid = {
            let guard = self.thread_id.lock();
            *guard
        };
        if let Some(thread_id) = tid {
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            info!(
                "[WinNotif] Posted WM_QUIT to notification thread {}",
                thread_id
            );
        }
    }
}
