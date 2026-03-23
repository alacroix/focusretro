use crate::platform::{GameWindow, WindowManager};
use log::info;
use std::mem;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, SetActiveWindow, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS,
    KEYBDINPUT, KEYEVENTF_KEYUP, VK_RETURN,
};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoA, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, IsZoomed, MSG, PeekMessageW, PM_NOREMOVE, SetForegroundWindow,
    SetWindowPos, ShowWindow, SW_MAXIMIZE, SW_RESTORE, SWP_NOACTIVATE, SWP_NOZORDER,
};

pub fn get_foreground_window_id() -> u64 {
    unsafe { GetForegroundWindow() }.0 as usize as u64
}

pub struct WinWindowManager;

impl WinWindowManager {
    pub fn new() -> Self {
        Self
    }
}

struct EnumData {
    windows: Vec<HWND>,
}

unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let data = &mut *(lparam.0 as *mut EnumData);
    data.windows.push(hwnd);
    TRUE
}

fn enum_all_windows() -> Vec<HWND> {
    let mut data = EnumData { windows: Vec::new() };
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_callback),
            LPARAM(&mut data as *mut EnumData as isize),
        );
    }
    data.windows
}

fn get_window_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..len as usize])
}

impl WindowManager for WinWindowManager {
    fn list_dofus_windows(&self) -> Vec<GameWindow> {
        let mut result = Vec::new();
        for hwnd in enum_all_windows() {
            if unsafe { !IsWindowVisible(hwnd).as_bool() } {
                continue;
            }
            let title = get_window_text(hwnd);
            let idx = match title.find(" - Dofus Retro") {
                Some(i) => i,
                None => continue,
            };
            let character_name = title[..idx].trim().to_string();
            if character_name.is_empty() {
                continue;
            }
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            let window_id = hwnd.0 as usize as u64;
            result.push(GameWindow {
                character_name,
                window_id,
                pid,
                title,
            });
        }
        result
    }

    fn focus_window(&self, window: &GameWindow) -> anyhow::Result<()> {
        // window_id is the HWND captured at enumeration time — use it directly.
        let hwnd = HWND(window.window_id as usize as *mut _);

        unsafe {
            // Disable DWM transition animation for instant focus appearance.
            let disable: u32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_TRANSITIONS_FORCEDISABLED,
                &disable as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );

            // Unminimize only if actually minimized — calling SW_RESTORE on a
            // fullscreen window would exit fullscreen and shrink it to windowed.
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }

            // Ensure this thread has a Win32 message queue before calling AttachThreadInput.
            // AttachThreadInput requires both threads to have a queue; on threads that never
            // called PeekMessage/GetMessage (e.g. the poll-db notification thread on Win10)
            // the queue does not exist yet and AttachThreadInput fails silently, causing
            // SetFocus to have no effect and leaving the window focused-but-unresponsive.
            // PeekMessage with PM_NOREMOVE on a null HWND creates the queue lazily if absent.
            let mut _msg = MSG::default();
            let _ = PeekMessageW(&mut _msg, None, 0, 0, PM_NOREMOVE);

            // Attach our thread to both the foreground and target threads:
            //   cur → fg_tid  : makes SetForegroundWindow bypass focus-stealing prevention
            //   cur → target  : makes SetFocus effective (SetFocus only works within the
            //                   calling thread's input queue)
            let cur_tid = GetCurrentThreadId();
            let fg_tid = GetWindowThreadProcessId(GetForegroundWindow(), None);
            let target_tid = GetWindowThreadProcessId(hwnd, None);

            if cur_tid != fg_tid     { let _ = AttachThreadInput(cur_tid, fg_tid, true); }
            if cur_tid != target_tid { let _ = AttachThreadInput(cur_tid, target_tid, true); }

            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetActiveWindow(hwnd);
            let _ = SetFocus(Some(hwnd));

            if cur_tid != target_tid { let _ = AttachThreadInput(cur_tid, target_tid, false); }
            if cur_tid != fg_tid     { let _ = AttachThreadInput(cur_tid, fg_tid, false); }

            // Re-enable DWM transitions.
            let enable: u32 = 0;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_TRANSITIONS_FORCEDISABLED,
                &enable as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }

        info!("[WinWindow] Focused window: {}", window.character_name);
        Ok(())
    }

    fn arrange_windows(&self, windows: &[GameWindow], layout: &str) -> anyhow::Result<()> {
        if windows.is_empty() {
            return Ok(());
        }

        if layout == "maximize" {
            for window in windows {
                let hwnd = HWND(window.window_id as usize as *mut _);
                unsafe { let _ = ShowWindow(hwnd, SW_MAXIMIZE); }
            }
            return Ok(());
        }

        let first_hwnd = HWND(windows[0].window_id as usize as *mut _);
        let monitor = unsafe { MonitorFromWindow(first_hwnd, MONITOR_DEFAULTTONEAREST) };

        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        unsafe { let _ = GetMonitorInfoA(monitor, &mut info); };

        let l = info.rcWork.left;
        let t = info.rcWork.top;
        let w = info.rcWork.right - info.rcWork.left;
        let h = info.rcWork.bottom - info.rcWork.top;

        // Windows 10/11 DWM adds ~8px invisible resize border on left/right/bottom.
        // Expand every slot by this amount so adjacent windows appear seamless.
        const B: i32 = 8;
        let expand = |(x, y, cw, ch): (i32, i32, i32, i32)| {
            (x - B, y, cw + B * 2, ch + B)
        };

        // Build (x, y, width, height) slots for each layout
        let slots: Vec<(i32, i32, i32, i32)> = match layout {
            "split-h" => vec![
                expand((l,           t, w / 2,     h)),
                expand((l + w / 2,   t, w - w / 2, h)),
            ],
            "split-v" => vec![
                expand((l, t,           w, h / 2)),
                expand((l, t + h / 2,   w, h - h / 2)),
            ],
            "grid-2x2" => vec![
                expand((l,           t,           w / 2,     h / 2)),
                expand((l + w / 2,   t,           w - w / 2, h / 2)),
                expand((l,           t + h / 2,   w / 2,     h - h / 2)),
                expand((l + w / 2,   t + h / 2,   w - w / 2, h - h / 2)),
            ],
            "grid-3x2" => {
                let cw = w / 3;
                let rh = h / 2;
                vec![
                    expand((l,          t,       cw,          rh)),
                    expand((l + cw,     t,       cw,          rh)),
                    expand((l + cw * 2, t,       w - cw * 2,  rh)),
                    expand((l,          t + rh,  cw,          h - rh)),
                    expand((l + cw,     t + rh,  cw,          h - rh)),
                    expand((l + cw * 2, t + rh,  w - cw * 2,  h - rh)),
                ]
            }
            "grid-4x2" => {
                let cw = w / 4;
                let rh = h / 2;
                vec![
                    expand((l,          t,       cw,          rh)),
                    expand((l + cw,     t,       cw,          rh)),
                    expand((l + cw * 2, t,       cw,          rh)),
                    expand((l + cw * 3, t,       w - cw * 3,  rh)),
                    expand((l,          t + rh,  cw,          h - rh)),
                    expand((l + cw,     t + rh,  cw,          h - rh)),
                    expand((l + cw * 2, t + rh,  cw,          h - rh)),
                    expand((l + cw * 3, t + rh,  w - cw * 3,  h - rh)),
                ]
            }
            _ => return Err(anyhow::anyhow!("Unknown layout: {}", layout)),
        };

        for (window, (x, y, cw, ch)) in windows.iter().zip(slots.iter()) {
            let hwnd = HWND(window.window_id as usize as *mut _);
            unsafe {
                if IsZoomed(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                let _ = SetWindowPos(hwnd, None, *x, *y, *cw, *ch, SWP_NOZORDER | SWP_NOACTIVATE);
            }
        }

        Ok(())
    }

    fn send_enter_key(&self) -> anyhow::Result<()> {
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_RETURN,
                        wScan: 0,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_RETURN,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        let sent = unsafe { SendInput(&inputs, mem::size_of::<INPUT>() as i32) };
        if sent == inputs.len() as u32 {
            info!("[WinWindow] Sent Enter key via SendInput");
            Ok(())
        } else {
            Err(anyhow::anyhow!("SendInput failed (sent {} of 2)", sent))
        }
    }
}
