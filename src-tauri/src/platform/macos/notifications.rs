use crate::platform::NotificationListener;
use core_foundation::array::CFArrayRef;
use core_foundation::base::TCFType;
use core_foundation::runloop::*;
use core_foundation::string::CFString;
use log::{debug, error, info};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

type AXUIElementRef = *mut c_void;
type AXObserverRef = *mut c_void;
type AXError = i32;

const K_AX_SUCCESS: AXError = 0;

extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXObserverCreate(
        application: i32,
        callback: unsafe extern "C" fn(AXObserverRef, AXUIElementRef, *const c_void, *mut c_void),
        out_observer: *mut AXObserverRef,
    ) -> AXError;
    fn AXObserverAddNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: *const c_void,
        refcon: *mut c_void,
    ) -> AXError;
    fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFRunLoopSourceRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void,
        value: *mut *mut c_void,
    ) -> AXError;
    fn AXUIElementCopyActionNames(
        element: AXUIElementRef,
        action_names: *mut CFArrayRef,
    ) -> AXError;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: *const c_void) -> AXError;
    fn CFRelease(cf: *const c_void);
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
}

/// RAII guard for CF objects obtained via Copy rule (AXUIElementCopyAttributeValue,
/// AXUIElementCopyActionNames, etc.) where no typed RAII wrapper is available.
fn cf_guard(ptr: *const c_void) -> impl Drop {
    crate::platform::OnDrop::new(move || {
        if !ptr.is_null() {
            unsafe { CFRelease(ptr) }
        }
    })
}

struct CallbackContext {
    /// Called with text segments. Returns `true` if this is a turn notification
    /// and we should click the banner to focus the game window.
    on_notification: Box<dyn Fn(Vec<String>) -> bool + Send + 'static>,
}

unsafe extern "C" fn ax_observer_callback(
    _observer: AXObserverRef,
    element: AXUIElementRef,
    notification: *const c_void,
    context: *mut c_void,
) {
    let notif_name = CFString::wrap_under_get_rule(notification as *const _);
    info!("[AXObserver] callback fired: {}", notif_name);

    if notif_name != "AXWindowCreated" {
        return;
    }

    let segments = collect_text(element);
    debug!(
        "[AXObserver] collected {} text segments: {:?}",
        segments.len(),
        segments
    );

    if segments.is_empty() {
        return;
    }

    let ctx = &*(context as *const CallbackContext);
    let is_turn = (ctx.on_notification)(segments);

    if is_turn {
        click_notification_banner(element);
    }
}

/// Click the notification banner body to focus the source app + dismiss the banner.
/// This is equivalent to the user manually clicking the notification.
unsafe fn click_notification_banner(element: AXUIElementRef) {
    info!("[AXObserver] looking for AXNotificationCenterBanner to click...");

    if let Some(banner) = find_banner(element) {
        if has_action(banner, "AXPress") {
            let press = CFString::new("AXPress");
            let err =
                AXUIElementPerformAction(banner, press.as_concrete_TypeRef() as *const c_void);
            info!(
                "[AXObserver] pressed AXNotificationCenterBanner, result: {}",
                err
            );
            return;
        }
    }

    // Fallback: try AXPress directly on the root element
    if has_action(element, "AXPress") {
        let press = CFString::new("AXPress");
        let err = AXUIElementPerformAction(element, press.as_concrete_TypeRef() as *const c_void);
        info!(
            "[AXObserver] pressed root element as fallback, result: {}",
            err
        );
    } else {
        info!("[AXObserver] no pressable banner found, focus will rely on WindowManager fallback");
    }
}

/// Recursively find the AXNotificationCenterBanner element in the AX tree.
unsafe fn find_banner(element: AXUIElementRef) -> Option<AXUIElementRef> {
    let subrole = ax_copy_string(element, "AXSubrole");
    if subrole.as_deref() == Some("AXNotificationCenterBanner") {
        return Some(element);
    }

    let children_attr = CFString::new("AXChildren");
    let mut children_value: *mut c_void = std::ptr::null_mut();
    let err = AXUIElementCopyAttributeValue(
        element,
        children_attr.as_concrete_TypeRef() as *const c_void,
        &mut children_value,
    );
    if err != K_AX_SUCCESS || children_value.is_null() {
        return None;
    }
    let _guard = cf_guard(children_value as *const c_void);
    let count = CFArrayGetCount(children_value as *const c_void);
    for i in 0..count {
        let child = CFArrayGetValueAtIndex(children_value as *const c_void, i) as AXUIElementRef;
        if !child.is_null() {
            if let Some(banner) = find_banner(child) {
                return Some(banner);
            }
        }
    }
    None
}

/// Check if an element supports a given action without performing it.
unsafe fn has_action(element: AXUIElementRef, action_name: &str) -> bool {
    let mut actions: CFArrayRef = std::ptr::null();
    let err = AXUIElementCopyActionNames(element, &mut actions);
    if err != K_AX_SUCCESS || actions.is_null() {
        return false;
    }
    let _guard = cf_guard(actions as *const c_void);
    let count = CFArrayGetCount(actions as *const c_void);
    for i in 0..count {
        let name = CFArrayGetValueAtIndex(actions as *const c_void, i);
        if !name.is_null() {
            let cf_name = CFString::wrap_under_get_rule(name as *const _);
            if cf_name == action_name {
                return true;
            }
        }
    }
    false
}

unsafe fn collect_text(element: AXUIElementRef) -> Vec<String> {
    let mut out = Vec::new();

    for attr_name in ["AXTitle", "AXValue", "AXDescription"] {
        if let Some(s) = ax_copy_string(element, attr_name) {
            if !s.is_empty() {
                out.push(s);
            }
        }
    }

    let children_attr = CFString::new("AXChildren");
    let mut children_value: *mut c_void = std::ptr::null_mut();
    let err = AXUIElementCopyAttributeValue(
        element,
        children_attr.as_concrete_TypeRef() as *const c_void,
        &mut children_value,
    );

    if err == K_AX_SUCCESS && !children_value.is_null() {
        let _guard = cf_guard(children_value as *const c_void);
        let count = CFArrayGetCount(children_value as *const c_void);
        for i in 0..count {
            let child =
                CFArrayGetValueAtIndex(children_value as *const c_void, i) as AXUIElementRef;
            if !child.is_null() {
                out.extend(collect_text(child));
            }
        }
    }

    out
}

unsafe fn ax_copy_string(element: AXUIElementRef, attr_name: &str) -> Option<String> {
    let attr = CFString::new(attr_name);
    let mut value: *mut c_void = std::ptr::null_mut();
    let err = AXUIElementCopyAttributeValue(
        element,
        attr.as_concrete_TypeRef() as *const c_void,
        &mut value,
    );
    if err == K_AX_SUCCESS && !value.is_null() {
        let cf_str = CFString::wrap_under_create_rule(value as *const _);
        Some(cf_str.to_string())
    } else {
        None
    }
}

fn find_notification_center_pid() -> Option<i32> {
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::NSString;

    let bundle_id = NSString::from_str("com.apple.notificationcenterui");
    let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&bundle_id);

    if apps.count() > 0 {
        let app: &NSRunningApplication = &apps.objectAtIndex(0);
        Some(app.processIdentifier())
    } else {
        None
    }
}

pub struct MacNotificationListener {
    running: Arc<AtomicBool>,
}

impl MacNotificationListener {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl NotificationListener for MacNotificationListener {
    fn start(
        &self,
        on_notification: Box<dyn Fn(Vec<String>) -> bool + Send + 'static>,
        on_mode: Box<dyn Fn(String) + Send + 'static>,
    ) -> anyhow::Result<()> {
        on_mode("event".into()); // macOS always uses AXObserver events
        let pid = find_notification_center_pid()
            .ok_or_else(|| anyhow::anyhow!("NotificationCenter process not found"))?;

        info!(
            "[NotificationListener] Found NotificationCenter PID: {}",
            pid
        );
        self.running.store(true, Ordering::Relaxed);

        let ctx = Box::new(CallbackContext { on_notification });
        let ctx_ptr = Box::into_raw(ctx) as *mut c_void;

        unsafe {
            let app_element = AXUIElementCreateApplication(pid);
            if app_element.is_null() {
                return Err(anyhow::anyhow!(
                    "Failed to create AXUIElement for NotificationCenter"
                ));
            }

            let mut observer: AXObserverRef = std::ptr::null_mut();
            let err = AXObserverCreate(pid, ax_observer_callback, &mut observer);
            if err != K_AX_SUCCESS || observer.is_null() {
                CFRelease(app_element as *const c_void);
                return Err(anyhow::anyhow!(
                    "Failed to create AXObserver (error {})",
                    err
                ));
            }

            let window_created = CFString::new("AXWindowCreated");
            let err = AXObserverAddNotification(
                observer,
                app_element,
                window_created.as_concrete_TypeRef() as *const c_void,
                ctx_ptr,
            );
            if err != K_AX_SUCCESS {
                error!(
                    "[NotificationListener] Failed to add AXWindowCreated notification (error {})",
                    err
                );
            } else {
                info!("[NotificationListener] AXWindowCreated notification registered OK");
            }

            let run_loop_source = AXObserverGetRunLoopSource(observer);
            CFRunLoopAddSource(
                CFRunLoopGetCurrent(),
                run_loop_source,
                kCFRunLoopDefaultMode,
            );

            info!(
                "[NotificationListener] AXObserver attached to CFRunLoop, listening for banners..."
            );
            CFRunLoopRun();
            info!("[NotificationListener] CFRunLoop exited");
        }

        Ok(())
    }

    fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        unsafe {
            CFRunLoopStop(CFRunLoopGetCurrent());
        }
    }
}
