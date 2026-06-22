//! Navigation helpers — scroll fraction conversion, history, and link handling.
//!
//! This module is expanded in Phase 3 to include the full navigation logic
//! (push_nav, restore_nav_entry, handle_link, navigate_to_anchor, etc.).
//! For now it hosts only the scroll-fraction conversion helpers shared by
//! the search and sidebar modules.

use super::state::MdrApp;

// ---------------------------------------------------------------------------
// Scroll fraction conversion helpers
// ---------------------------------------------------------------------------

/// Convert a content fraction (line / total_lines, in 0.0..1.0) to a
/// `RelativeOffset.y` suitable for `snap_to`. This accounts for the
/// viewport height so the target line lands at the **top** of the window.
/// Naturally clamps to 1.0, which handles the "last section" case where
/// there isn't enough content below to fill the viewport.
pub(super) fn content_fraction_to_scroll_y(app: &MdrApp, fraction: f32) -> f32 {
    let max_scroll = app.content_height - app.viewport_height;
    if max_scroll <= 0.0 || app.content_height <= 0.0 {
        // Content fits in viewport or dimensions unknown yet — use fraction directly
        return fraction.clamp(0.0, 1.0);
    }
    ((fraction * app.content_height) / max_scroll).clamp(0.0, 1.0)
}

/// Convert the current relative scroll offset back to an approximate
/// content fraction (for determining which section is visible).
pub(super) fn scroll_y_to_content_fraction(app: &MdrApp) -> f32 {
    let max_scroll = app.content_height - app.viewport_height;
    if max_scroll <= 0.0 || app.content_height <= 0.0 {
        return app.current_scroll_y;
    }
    (app.current_scroll_y * max_scroll) / app.content_height
}
