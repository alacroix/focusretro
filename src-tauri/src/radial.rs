use crate::platform;
use crate::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

// Radial geometry — must match RadialSelector.tsx constants
pub const OUTER_R: f64 = 124.0;
pub const INNER_R: f64 = 34.0;
pub const RADIAL_WIN_SIZE: f64 = 420.0;
pub const RADIAL_WIN_CX: f64 = RADIAL_WIN_SIZE / 2.0;

/// Compute the account segment index under the cursor, or None if outside the wheel.
pub fn radial_segment_at(
    cursor_x: f64,
    cursor_y: f64,
    center_x: f64,
    center_y: f64,
    n: usize,
) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let dx = cursor_x - center_x;
    let dy = cursor_y - center_y;
    let dist = (dx * dx + dy * dy).sqrt();
    if !(INNER_R..=OUTER_R).contains(&dist) {
        return None;
    }
    let mut angle = dy.atan2(dx) + std::f64::consts::PI / 2.0;
    if angle < 0.0 {
        angle += 2.0 * std::f64::consts::PI;
    }
    if angle >= 2.0 * std::f64::consts::PI {
        angle -= 2.0 * std::f64::consts::PI;
    }
    Some((angle / (2.0 * std::f64::consts::PI) * n as f64).floor() as usize % n)
}

/// Resolve the selected account name from the cursor's logical position at key/button release.
/// Returns None if the cursor is outside the wheel or no accounts are registered.
pub fn resolve_selection(state: &AppState, logical_x: f64, logical_y: f64) -> Option<String> {
    let keydown = state.get_radial_center()?;
    let accounts = state.get_account_views();
    let n = accounts.len();
    if n == 0 {
        return None;
    }
    let rel_x = RADIAL_WIN_CX + (logical_x - keydown.0);
    let rel_y = RADIAL_WIN_CX + (logical_y - keydown.1);
    let seg = radial_segment_at(rel_x, rel_y, RADIAL_WIN_CX, RADIAL_WIN_CX, n)?;
    Some(accounts[seg].character_name.clone())
}

/// Focus the selected account by name, or fall back to the current window.
/// Emits "focus-changed" on success. Intended to be called from a spawned thread.
pub fn focus_selected_or_current(
    handle: AppHandle,
    state: Arc<AppState>,
    selected: Option<String>,
) {
    let wm = platform::create_window_manager();
    if let Some(name) = selected {
        let windows = wm.list_dofus_windows();
        if let Some(win) = windows
            .iter()
            .find(|w| w.character_name.eq_ignore_ascii_case(&name))
        {
            let _ = wm.focus_window(win);
            state.set_current_by_name(&name);
            let _ = handle.emit("focus-changed", &name);
        }
    } else if let Some(win) = state.get_current_window() {
        let _ = wm.focus_window(&win);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CX: f64 = RADIAL_WIN_CX;
    const CY: f64 = RADIAL_WIN_SIZE / 2.0;
    // A radius comfortably inside the valid ring
    const R: f64 = (INNER_R + OUTER_R) / 2.0;

    #[test]
    fn n_zero_returns_none() {
        assert_eq!(radial_segment_at(CX, CY - R, CX, CY, 0), None);
    }

    #[test]
    fn inside_inner_ring_returns_none() {
        // dist = INNER_R - 1, just inside the dead zone
        assert_eq!(radial_segment_at(CX, CY - (INNER_R - 1.0), CX, CY, 4), None);
    }

    #[test]
    fn outside_outer_ring_returns_none() {
        assert_eq!(radial_segment_at(CX, CY - (OUTER_R + 1.0), CX, CY, 4), None);
    }

    #[test]
    fn n_one_always_returns_segment_zero() {
        // Any point in the valid ring maps to segment 0
        assert_eq!(radial_segment_at(CX, CY - R, CX, CY, 1), Some(0)); // top
        assert_eq!(radial_segment_at(CX + R, CY, CX, CY, 1), Some(0)); // right
        assert_eq!(radial_segment_at(CX, CY + R, CX, CY, 1), Some(0)); // bottom
        assert_eq!(radial_segment_at(CX - R, CY, CX, CY, 1), Some(0)); // left
    }

    #[test]
    fn n4_clockwise_segments() {
        // Segment 0 = top, 1 = right, 2 = bottom, 3 = left
        assert_eq!(radial_segment_at(CX, CY - R, CX, CY, 4), Some(0));
        assert_eq!(radial_segment_at(CX + R, CY, CX, CY, 4), Some(1));
        assert_eq!(radial_segment_at(CX, CY + R, CX, CY, 4), Some(2));
        assert_eq!(radial_segment_at(CX - R, CY, CX, CY, 4), Some(3));
    }

    #[test]
    fn n8_all_segments_reachable() {
        use std::f64::consts::PI;
        let mut seen = std::collections::HashSet::new();
        for i in 0..8usize {
            // Place cursor at centre of each 45-degree slice
            let angle = (i as f64) * (2.0 * PI / 8.0);
            // Rotate so segment 0 is at the top: angle 0 → dy = -R (up)
            let cursor_x = CX + R * angle.sin();
            let cursor_y = CY - R * angle.cos();
            let seg = radial_segment_at(cursor_x, cursor_y, CX, CY, 8).unwrap();
            seen.insert(seg);
        }
        assert_eq!(seen.len(), 8, "all 8 segments must be reachable");
    }

    #[test]
    fn non_zero_center_offset() {
        // Shifting both cursor and center by the same amount must yield the same segment
        let offset_x = 50.0;
        let offset_y = 30.0;
        let seg_origin = radial_segment_at(CX, CY - R, CX, CY, 4);
        let seg_offset = radial_segment_at(
            CX + offset_x,
            CY - R + offset_y,
            CX + offset_x,
            CY + offset_y,
            4,
        );
        assert_eq!(seg_origin, seg_offset);
    }
}
