// Windows taskbar identity: AUMID ungrouping + custom icon per Dofus window
//
// Icon compositing (disc, base icon, overlay) is handled by the frontend via HTML Canvas.
// This module only handles Win32 concerns: HICON creation, AUMID, WM_SETICON, handle cleanup.

use std::collections::{HashMap, HashSet};

use crate::platform::GameWindow;
use log::warn;

#[cfg(target_os = "windows")]
use windows::{
    core::BOOL,
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        Graphics::Gdi::{
            CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
            DIB_RGB_COLORS, HBITMAP,
        },
        Storage::EnhancedStorage::PKEY_AppUserModel_ID,
        System::Com::StructuredStorage::PROPVARIANT,
        System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
        UI::Shell::PropertiesSystem::{IPropertyStore, SHGetPropertyStoreForWindow},
        UI::WindowsAndMessaging::{
            CreateIconIndirect, DestroyIcon, IsWindow, SendMessageW, HICON, ICONINFO, ICON_BIG,
            ICON_SMALL, WM_SETICON,
        },
    },
};

/// Initializes COM on the current thread and returns a guard that calls
/// CoUninitialize on drop. Returns None if COM was already initialized with a
/// different threading model (RPC_E_CHANGED_MODE) — in that case no cleanup needed.
#[cfg(target_os = "windows")]
fn com_init() -> Option<impl Drop> {
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // S_OK (0) or S_FALSE (1): we initialized or re-entered — must uninit on drop.
    // Negative HRESULT (e.g. RPC_E_CHANGED_MODE): already initialized differently — skip.
    hr.is_ok()
        .then(|| crate::platform::OnDrop::new(|| unsafe { CoUninitialize() }))
}

/// Converts a flat RGBA byte slice (size×size pixels) into a Windows HICON.
/// Caller owns the returned HICON and must call DestroyIcon when done.
#[cfg(target_os = "windows")]
pub(crate) unsafe fn rgba_to_hicon(rgba: &[u8], size: u32) -> anyhow::Result<HICON> {
    assert_eq!(rgba.len() as u32, size * size * 4);

    // Convert RGBA → premultiplied BGRA (top-down order, matching negative biHeight below).
    let mut bgra = vec![0u8; (size * size * 4) as usize];
    for i in 0..(size * size) as usize {
        let src = i * 4;
        let a = rgba[src + 3];
        let pm = |c: u8| -> u8 { ((c as u16 * a as u16 + 127) / 255) as u8 };
        bgra[src] = pm(rgba[src + 2]); // B
        bgra[src + 1] = pm(rgba[src + 1]); // G
        bgra[src + 2] = pm(rgba[src]); // R
        bgra[src + 3] = a;
    }

    // CreateDIBSection for 32bpp BGRA with alpha.
    // Negative biHeight = top-down DIB, memory order matches our bgra buffer.
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size as i32,
            biHeight: -(size as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits_ptr = std::ptr::null_mut::<std::ffi::c_void>();
    let hbm_color: HBITMAP = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0)?;
    std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits_ptr as *mut u8, bgra.len());

    // 1bpp all-zeros AND mask (size × size).
    let mask_row_bytes = size.div_ceil(32) * 4;
    let mask_data = vec![0u8; (mask_row_bytes * size) as usize];
    let hbm_mask: HBITMAP = CreateBitmap(
        size as i32,
        size as i32,
        1,
        1,
        Some(mask_data.as_ptr() as *const _),
    );
    if hbm_mask.0.is_null() {
        let _ = DeleteObject(hbm_color.into());
        return Err(anyhow::anyhow!("CreateBitmap for icon mask returned null"));
    }

    let icon_info = ICONINFO {
        fIcon: BOOL(1),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };
    let hicon = CreateIconIndirect(&icon_info)?;

    // Icon owns copies of the bitmaps — safe to delete originals
    let _ = DeleteObject(hbm_color.into());
    let _ = DeleteObject(hbm_mask.into());

    Ok(hicon)
}

/// Sets a unique AUMID on a window via IPropertyStore, causing the Windows taskbar
/// to group this window separately from other Dofus windows.
#[cfg(target_os = "windows")]
pub(crate) unsafe fn set_window_aumid(hwnd: HWND, character_name: &str) -> anyhow::Result<()> {
    let _com = com_init();

    let store: IPropertyStore = SHGetPropertyStoreForWindow(hwnd)
        .map_err(|e| anyhow::anyhow!("SHGetPropertyStoreForWindow: {e:?}"))?;

    let aumid = format!("focusretro.dofus.{}", character_name);
    let pv = PROPVARIANT::from(aumid.as_str());

    store
        .SetValue(&PKEY_AppUserModel_ID, &pv)
        .map_err(|e| anyhow::anyhow!("IPropertyStore::SetValue: {e:?}"))?;
    store
        .Commit()
        .map_err(|e| anyhow::anyhow!("IPropertyStore::Commit: {e:?}"))?;

    Ok(())
}

/// Applies a pre-composed RGBA icon (provided by the frontend) to a window's taskbar button.
/// Creates a new HICON, sends WM_SETICON, destroys the previous handle for this window.
#[cfg(target_os = "windows")]
pub fn set_window_icon(hwnd_isize: isize, rgba: &[u8], icon_handles: &mut HashMap<isize, isize>) {
    let hwnd = HWND(hwnd_isize as usize as *mut _);
    let new_hicon = match unsafe { rgba_to_hicon(rgba, 24) } {
        Ok(h) => h,
        Err(e) => {
            warn!("set_window_icon: rgba_to_hicon: {e}");
            return;
        }
    };
    // Validate HWND before sending: if the window has closed and its handle has been
    // reused by a different process, we must not apply the icon to the wrong window
    // or cache a handle that will be destroyed on the next refresh.
    if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
        unsafe {
            let _ = DestroyIcon(new_hicon);
        }
        icon_handles.remove(&hwnd_isize);
        return;
    }
    unsafe {
        SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_BIG as usize)),
            Some(LPARAM(new_hicon.0 as isize)),
        );
        SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_SMALL as usize)),
            Some(LPARAM(new_hicon.0 as isize)),
        );
    }
    if let Some(old_raw) = icon_handles.get(&hwnd_isize).copied() {
        unsafe {
            let _ = DestroyIcon(HICON(old_raw as *mut _));
        }
    }
    icon_handles.insert(hwnd_isize, new_hicon.0 as isize);
}

/// Sets AUMID for new windows and evicts closed windows from the cache,
/// destroying their HICON handles. Icon compositing is the frontend's responsibility.
#[cfg(target_os = "windows")]
pub fn apply_taskbar_identities(
    windows: &[GameWindow],
    aumid_cache: &mut HashSet<isize>,
    icon_handles: &mut HashMap<isize, isize>,
) {
    let active: HashSet<isize> = windows.iter().map(|w| w.window_id as isize).collect();

    // Evict closed windows: remove from AUMID cache and destroy their HICON handles
    let stale: Vec<isize> = aumid_cache
        .iter()
        .copied()
        .filter(|h| !active.contains(h))
        .collect();
    for hwnd in stale {
        aumid_cache.remove(&hwnd);
        if let Some(raw) = icon_handles.remove(&hwnd) {
            unsafe {
                let _ = DestroyIcon(HICON(raw as *mut _));
            }
        }
    }

    // Set AUMID for windows not yet processed
    for window in windows {
        let hwnd_isize = window.window_id as isize;
        if aumid_cache.contains(&hwnd_isize) {
            continue;
        }
        let hwnd = HWND(hwnd_isize as usize as *mut _);
        if let Err(e) = unsafe { set_window_aumid(hwnd, &window.character_name) } {
            warn!(
                "apply_taskbar_identities: AUMID for '{}': {e}",
                window.character_name
            );
            continue;
        }
        aumid_cache.insert(hwnd_isize);
    }
}

/// Resets all windows back to default taskbar grouping (clears AUMID, resets icons).
/// No-op if the cache is already empty (nothing was applied).
#[cfg(target_os = "windows")]
pub fn reset_taskbar_identities(
    windows: &[GameWindow],
    aumid_cache: &mut HashSet<isize>,
    icon_handles: &mut HashMap<isize, isize>,
) {
    if aumid_cache.is_empty() {
        return;
    }
    let _com = com_init();
    for window in windows {
        let hwnd = HWND(window.window_id as usize as *mut _);
        unsafe {
            if let Ok(store) = SHGetPropertyStoreForWindow::<IPropertyStore>(hwnd) {
                let _ = store.SetValue(&PKEY_AppUserModel_ID, &PROPVARIANT::default());
                let _ = store.Commit();
            }
            SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_BIG as usize)),
                Some(LPARAM(0)),
            );
            SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_SMALL as usize)),
                Some(LPARAM(0)),
            );
        }
    }
    cleanup_all_icons(icon_handles);
    aumid_cache.clear();
}

/// Destroys all tracked HICON handles and clears the map.
#[cfg(target_os = "windows")]
pub fn cleanup_all_icons(icon_handles: &mut HashMap<isize, isize>) {
    for (_hwnd, raw) in icon_handles.iter() {
        unsafe {
            let _ = DestroyIcon(HICON(*raw as *mut _));
        }
    }
    icon_handles.clear();
}

// {56FDF344-FD6D-11D0-958A-006097C9A090}
#[cfg(target_os = "windows")]
const CLSID_TASKBAR_LIST: windows::core::GUID =
    windows::core::GUID::from_u128(0x56fdf344_fd6d_11d0_958a_006097c9a090);

/// Reorders taskbar buttons to match `windows_in_order` by cycling
/// ITaskbarList::DeleteTab + AddTab for all windows in the AUMID cache.
/// Since AddTab appends to the end, deleting all then re-adding in order
/// sets the desired left-to-right sequence.
/// Only processes windows already in `aumid_cache` (AUMID set in a prior cycle).
/// Gracefully no-ops if ITaskbarList is unavailable (e.g. Explorer not running).
#[cfg(target_os = "windows")]
pub fn reorder_taskbar_buttons(windows_in_order: &[GameWindow], aumid_cache: &HashSet<isize>) {
    use windows::Win32::{
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
        UI::Shell::ITaskbarList,
    };

    let ready: Vec<&GameWindow> = windows_in_order
        .iter()
        .filter(|w| aumid_cache.contains(&(w.window_id as isize)))
        .collect();

    if ready.len() < 2 {
        return;
    }

    let _com = com_init();
    unsafe {
        let taskbar_list: ITaskbarList =
            match CoCreateInstance(&CLSID_TASKBAR_LIST, None, CLSCTX_INPROC_SERVER) {
                Ok(tl) => tl,
                Err(e) => {
                    warn!("reorder_taskbar_buttons: CoCreateInstance: {e}");
                    return;
                }
            };

        if let Err(e) = taskbar_list.HrInit() {
            warn!("reorder_taskbar_buttons: HrInit: {e}");
            return;
        }

        // Remove all our windows from the taskbar
        for window in &ready {
            let hwnd = HWND(window.window_id as usize as *mut _);
            let _ = taskbar_list.DeleteTab(hwnd);
        }

        // Re-add in desired order — AddTab appends each to the end
        for window in &ready {
            let hwnd = HWND(window.window_id as usize as *mut _);
            let _ = taskbar_list.AddTab(hwnd);
        }
    }
}
