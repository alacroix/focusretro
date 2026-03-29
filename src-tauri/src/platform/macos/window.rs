use crate::platform::{GameWindow, WindowManager};
use core_foundation::base::TCFType;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::display::{
    CGDisplayBounds, CGMainDisplayID, CGPoint, CGRect, CGSize, CGWindowListCopyWindowInfo,
};
use core_graphics::window::{
    kCGNullWindowID, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
};
use log::{debug, info};
use std::ffi::c_void;
use std::ptr;

type AXUIElementRef = *mut c_void;
type AXError = i32;

const K_AX_SUCCESS: AXError = 0;
const K_AX_VALUE_CG_POINT_TYPE: i32 = 1;
const K_AX_VALUE_CG_SIZE_TYPE: i32 = 2;

type CGEventRef = *mut c_void;
type CGEventSourceRef = *mut c_void;

extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void,
        value: *mut *mut c_void,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void,
        value: *const c_void,
    ) -> AXError;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: *const c_void) -> AXError;
    fn AXValueCreate(value_type: i32, value_ptr: *const c_void) -> *mut c_void;
    fn CFRelease(cf: *const c_void);
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventPost(tap: u32, event: CGEventRef);
}

const DOFUS_TITLE_PATTERN: &str = " - Dofus Retro";

pub struct MacWindowManager;

impl MacWindowManager {
    pub fn new() -> Self {
        Self
    }

    fn parse_character_name(title: &str) -> Option<String> {
        if let Some(idx) = title.find(DOFUS_TITLE_PATTERN) {
            let name = title[..idx].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        None
    }
}

impl WindowManager for MacWindowManager {
    fn list_dofus_windows(&self) -> Vec<GameWindow> {
        let mut result = Vec::new();

        let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
        let window_list = unsafe { CGWindowListCopyWindowInfo(options, kCGNullWindowID) };

        if window_list.is_null() {
            return result;
        }

        let window_list_ptr = window_list as *const c_void;
        let count = unsafe { CFArrayGetCount(window_list_ptr) };

        for i in 0..count {
            let dict = unsafe { CFArrayGetValueAtIndex(window_list_ptr, i) };
            if dict.is_null() {
                continue;
            }

            let title = get_dict_string(dict, "kCGWindowName");
            let owner = get_dict_string(dict, "kCGWindowOwnerName");
            let window_id = get_dict_i64(dict, "kCGWindowNumber").unwrap_or(0) as u64;
            let pid = get_dict_i64(dict, "kCGWindowOwnerPID").unwrap_or(0) as u32;

            if let Some(title) = title {
                if let Some(char_name) = Self::parse_character_name(&title) {
                    debug!(
                        "Found Dofus window: {} (owner={}, pid={}, wid={})",
                        title,
                        owner.as_deref().unwrap_or("?"),
                        pid,
                        window_id
                    );
                    result.push(GameWindow {
                        character_name: char_name,
                        window_id,
                        pid,
                        title,
                    });
                }
            }
        }

        unsafe { CFRelease(window_list_ptr) };
        result
    }

    fn focus_window(&self, window: &GameWindow) -> anyhow::Result<()> {
        let pid = window.pid as i32;
        activate_app(pid)?;
        raise_window_ax(pid, &window.title)?;
        Ok(())
    }

    fn send_enter_key(&self) -> anyhow::Result<()> {
        const K_VK_RETURN: u16 = 0x24;
        const K_CG_HID_EVENT_TAP: u32 = 0;

        unsafe {
            let key_down = CGEventCreateKeyboardEvent(ptr::null_mut(), K_VK_RETURN, true);
            let key_up = CGEventCreateKeyboardEvent(ptr::null_mut(), K_VK_RETURN, false);
            if key_down.is_null() || key_up.is_null() {
                return Err(anyhow::anyhow!("Failed to create CGEvent for Enter key"));
            }
            CGEventPost(K_CG_HID_EVENT_TAP, key_down);
            CGEventPost(K_CG_HID_EVENT_TAP, key_up);
            CFRelease(key_down as *const c_void);
            CFRelease(key_up as *const c_void);
        }
        info!("[WindowManager] Sent Enter keypress");
        Ok(())
    }

    fn arrange_windows(&self, windows: &[GameWindow], layout: &str) -> anyhow::Result<()> {
        if windows.is_empty() {
            return Ok(());
        }

        if layout == "maximize" {
            let (l, b, w, h) = main_display_bounds();
            for window in windows {
                set_window_frame_ax(window.pid as i32, &window.title, l, b, w, h)?;
            }
            info!(
                "[MacWindowManager] Arranged {} windows: maximize",
                windows.len()
            );
            return Ok(());
        }

        let (l, b, w, h) = main_display_bounds();

        type Slot = (f64, f64, f64, f64); // left, bottom, width, height
        let slots: Vec<Slot> = match layout {
            "split-h" => vec![(l, b, w / 2.0, h), (l + w / 2.0, b, w - w / 2.0, h)],
            "split-v" => vec![(l, b + h / 2.0, w, h / 2.0), (l, b, w, h - h / 2.0)],
            "grid-2x2" => vec![
                (l, b + h / 2.0, w / 2.0, h / 2.0),
                (l + w / 2.0, b + h / 2.0, w - w / 2.0, h / 2.0),
                (l, b, w / 2.0, h - h / 2.0),
                (l + w / 2.0, b, w - w / 2.0, h - h / 2.0),
            ],
            "grid-3x2" => {
                let cw = w / 3.0;
                let rh = h / 2.0;
                vec![
                    (l, b + rh, cw, rh),
                    (l + cw, b + rh, cw, rh),
                    (l + cw * 2.0, b + rh, w - cw * 2.0, rh),
                    (l, b, cw, h - rh),
                    (l + cw, b, cw, h - rh),
                    (l + cw * 2.0, b, w - cw * 2.0, h - rh),
                ]
            }
            "grid-4x2" => {
                let cw = w / 4.0;
                let rh = h / 2.0;
                vec![
                    (l, b + rh, cw, rh),
                    (l + cw, b + rh, cw, rh),
                    (l + cw * 2.0, b + rh, cw, rh),
                    (l + cw * 3.0, b + rh, w - cw * 3.0, rh),
                    (l, b, cw, h - rh),
                    (l + cw, b, cw, h - rh),
                    (l + cw * 2.0, b, cw, h - rh),
                    (l + cw * 3.0, b, w - cw * 3.0, h - rh),
                ]
            }
            _ => return Err(anyhow::anyhow!("Unknown layout: {}", layout)),
        };

        for (window, (left, bottom, cw, ch)) in windows.iter().zip(slots.iter()) {
            set_window_frame_ax(window.pid as i32, &window.title, *left, *bottom, *cw, *ch)?;
        }
        info!(
            "[MacWindowManager] Arranged {} windows: {}",
            windows.len(),
            layout
        );
        Ok(())
    }
}

/// Returns (left, bottom, width, height) of the main display in screen coordinates (origin bottom-left).
fn main_display_bounds() -> (f64, f64, f64, f64) {
    let display_id = unsafe { CGMainDisplayID() };
    let bounds: CGRect = unsafe { CGDisplayBounds(display_id) };
    (
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    )
}

/// Returns the frontmost application's process identifier, or None if unavailable.
fn frontmost_application_pid() -> Option<i32> {
    let out = std::process::Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to return unix id of first process whose frontmost is true"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim().parse().ok()
}

/// Returns the main window's title for the given application PID via Accessibility API.
fn main_window_title_ax(pid: i32) -> Option<String> {
    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return None;
        }
        let main_attr = CFString::new("AXMainWindow");
        let mut main_value: *mut c_void = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            app_element,
            main_attr.as_concrete_TypeRef() as *const c_void,
            &mut main_value,
        );
        CFRelease(app_element as *const c_void);
        if err != K_AX_SUCCESS || main_value.is_null() {
            return None;
        }
        let win_element = main_value as AXUIElementRef;
        let title_attr = CFString::new("AXTitle");
        let mut title_value: *mut c_void = ptr::null_mut();
        let title_err = AXUIElementCopyAttributeValue(
            win_element,
            title_attr.as_concrete_TypeRef() as *const c_void,
            &mut title_value,
        );
        CFRelease(main_value as *const c_void);
        if title_err != K_AX_SUCCESS || title_value.is_null() {
            return None;
        }
        let cf_title = CFString::wrap_under_create_rule(title_value as *const _);
        Some(cf_title.to_string())
    }
}

/// Returns the CGWindowID of the frontmost window, or 0 if it cannot be determined.
/// Used to sync the "current" account from the actual focused Dofus window.
pub fn get_foreground_window_id() -> u64 {
    let pid = match frontmost_application_pid() {
        Some(p) => p,
        None => return 0,
    };
    let title = match main_window_title_ax(pid) {
        Some(t) => t,
        None => return 0,
    };
    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let window_list = unsafe { CGWindowListCopyWindowInfo(options, kCGNullWindowID) };
    if window_list.is_null() {
        return 0;
    }
    let window_list_ptr = window_list as *const c_void;
    let count = unsafe { CFArrayGetCount(window_list_ptr) };
    let mut result = 0u64;
    for i in 0..count {
        let dict = unsafe { CFArrayGetValueAtIndex(window_list_ptr, i) };
        if dict.is_null() {
            continue;
        }
        let owner_pid = get_dict_i64(dict, "kCGWindowOwnerPID").unwrap_or(0) as i32;
        let name = get_dict_string(dict, "kCGWindowName");
        if owner_pid == pid && name.as_deref() == Some(title.as_str()) {
            result = get_dict_i64(dict, "kCGWindowNumber").unwrap_or(0) as u64;
            break;
        }
    }
    unsafe { CFRelease(window_list_ptr) };
    result
}

/// Set a window's position and size via Accessibility API. (left, bottom) is bottom-left in screen coords.
fn set_window_frame_ax(
    pid: i32,
    title: &str,
    left: f64,
    bottom: f64,
    width: f64,
    height: f64,
) -> anyhow::Result<()> {
    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return Err(anyhow::anyhow!(
                "Failed to create AXUIElement for pid {}",
                pid
            ));
        }

        let windows_attr = CFString::new("AXWindows");
        let mut windows_value: *mut c_void = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            app_element,
            windows_attr.as_concrete_TypeRef() as *const c_void,
            &mut windows_value,
        );

        if err != K_AX_SUCCESS || windows_value.is_null() {
            CFRelease(app_element as *const c_void);
            return Err(anyhow::anyhow!("Failed to get AXWindows (error {})", err));
        }

        let count = CFArrayGetCount(windows_value as *const c_void);
        let mut win_element: AXUIElementRef = ptr::null_mut();

        for i in 0..count {
            let elem = CFArrayGetValueAtIndex(windows_value as *const c_void, i) as AXUIElementRef;
            if elem.is_null() {
                continue;
            }

            let title_attr = CFString::new("AXTitle");
            let mut title_value: *mut c_void = ptr::null_mut();
            let title_err = AXUIElementCopyAttributeValue(
                elem,
                title_attr.as_concrete_TypeRef() as *const c_void,
                &mut title_value,
            );

            if title_err == K_AX_SUCCESS && !title_value.is_null() {
                let cf_title = CFString::wrap_under_create_rule(title_value as *const _);
                if cf_title == title {
                    win_element = elem;
                    break;
                }
            }
        }

        if win_element.is_null() {
            CFRelease(windows_value as *const c_void);
            CFRelease(app_element as *const c_void);
            return Err(anyhow::anyhow!(
                "Window not found for title: {} (pid {})",
                title,
                pid
            ));
        }

        // Set position/size while window element is still valid (before releasing the array that owns it).
        let pos = CGPoint { x: left, y: bottom };
        let size = CGSize { width, height };
        let pos_value = AXValueCreate(
            K_AX_VALUE_CG_POINT_TYPE,
            &pos as *const CGPoint as *const c_void,
        );
        let size_value = AXValueCreate(
            K_AX_VALUE_CG_SIZE_TYPE,
            &size as *const CGSize as *const c_void,
        );

        if pos_value.is_null() || size_value.is_null() {
            CFRelease(windows_value as *const c_void);
            CFRelease(app_element as *const c_void);
            return Err(anyhow::anyhow!("AXValueCreate failed"));
        }

        let pos_attr = CFString::new("AXPosition");
        let size_attr = CFString::new("AXSize");
        let pos_err = AXUIElementSetAttributeValue(
            win_element,
            pos_attr.as_concrete_TypeRef() as *const c_void,
            pos_value as *const c_void,
        );
        let size_err = AXUIElementSetAttributeValue(
            win_element,
            size_attr.as_concrete_TypeRef() as *const c_void,
            size_value as *const c_void,
        );

        CFRelease(pos_value as *const c_void);
        CFRelease(size_value as *const c_void);
        CFRelease(windows_value as *const c_void);
        CFRelease(app_element as *const c_void);

        if pos_err != K_AX_SUCCESS {
            return Err(anyhow::anyhow!(
                "AXUIElementSetAttributeValue AXPosition failed: {}",
                pos_err
            ));
        }
        if size_err != K_AX_SUCCESS {
            return Err(anyhow::anyhow!(
                "AXUIElementSetAttributeValue AXSize failed: {}",
                size_err
            ));
        }
    }
    Ok(())
}

fn activate_app(pid: i32) -> anyhow::Result<()> {
    let output = std::process::Command::new("osascript")
        .args([
            "-e",
            &format!(
                "tell application \"System Events\" to set frontmost of (first process whose unix id is {}) to true",
                pid
            ),
        ])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to activate app: {}", e))?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "osascript failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

fn raise_window_ax(pid: i32, target_title: &str) -> anyhow::Result<()> {
    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return Err(anyhow::anyhow!(
                "Failed to create AXUIElement for pid {}",
                pid
            ));
        }

        let windows_attr = CFString::new("AXWindows");
        let mut windows_value: *mut c_void = std::ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            app_element,
            windows_attr.as_concrete_TypeRef() as *const c_void,
            &mut windows_value,
        );

        if err != K_AX_SUCCESS || windows_value.is_null() {
            CFRelease(app_element as *const c_void);
            return Err(anyhow::anyhow!("Failed to get AXWindows (error {})", err));
        }

        let count = CFArrayGetCount(windows_value as *const c_void);

        for i in 0..count {
            let win_element =
                CFArrayGetValueAtIndex(windows_value as *const c_void, i) as AXUIElementRef;
            if win_element.is_null() {
                continue;
            }

            let title_attr = CFString::new("AXTitle");
            let mut title_value: *mut c_void = std::ptr::null_mut();
            let title_err = AXUIElementCopyAttributeValue(
                win_element,
                title_attr.as_concrete_TypeRef() as *const c_void,
                &mut title_value,
            );

            if title_err == K_AX_SUCCESS && !title_value.is_null() {
                let cf_title = CFString::wrap_under_create_rule(title_value as *const _);
                let win_title = cf_title.to_string();

                if win_title == target_title {
                    let raise_action = CFString::new("AXRaise");
                    AXUIElementPerformAction(
                        win_element,
                        raise_action.as_concrete_TypeRef() as *const c_void,
                    );
                    break;
                }
            }
        }

        CFRelease(windows_value as *const c_void);
        CFRelease(app_element as *const c_void);
    }

    Ok(())
}

fn get_dict_string(dict: *const c_void, key: &str) -> Option<String> {
    unsafe {
        let cf_key = CFString::new(key);
        let value = CFDictionaryGetValue(dict, cf_key.as_concrete_TypeRef() as *const c_void);
        if value.is_null() {
            return None;
        }
        let cf_str = CFString::wrap_under_get_rule(value as *const _);
        Some(cf_str.to_string())
    }
}

fn get_dict_i64(dict: *const c_void, key: &str) -> Option<i64> {
    unsafe {
        let cf_key = CFString::new(key);
        let value = CFDictionaryGetValue(dict, cf_key.as_concrete_TypeRef() as *const c_void);
        if value.is_null() {
            return None;
        }
        let cf_num = CFNumber::wrap_under_get_rule(value as *const _);
        cf_num.to_i64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_character_name() {
        assert_eq!(
            MacWindowManager::parse_character_name("Craette - Dofus Retro v1.40.0"),
            Some("Craette".to_string())
        );
        assert_eq!(
            MacWindowManager::parse_character_name("My-Char_123 - Dofus Retro v1.39.0"),
            Some("My-Char_123".to_string())
        );
        assert_eq!(
            MacWindowManager::parse_character_name("Some Random Window"),
            None
        );
    }
}
