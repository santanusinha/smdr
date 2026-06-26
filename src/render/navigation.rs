//! Browser-style navigation history and in-document anchor/link navigation.
//!
//! This module hosts the full navigation logic: scroll-fraction conversion,
//! history stack management (push/restore), link handling (external, anchor,
//! local file), anchor resolution, and heading navigation.

use std::path::{Path, PathBuf};

use iced::Task;
use iced::widget::Id;
use iced::widget::operation::{self, RelativeOffset};

use smdr::markdown as md_helpers;

use super::images::load_file;
use super::state::{MdrApp, Message, NavEntry, Overlay, SCROLLABLE_ID};

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

// ---------------------------------------------------------------------------
// Navigation history helpers
// ---------------------------------------------------------------------------

/// Push a new navigation entry, truncating any forward history.
pub(super) fn push_nav(app: &mut MdrApp, file_path: PathBuf, scroll_y: f32) {
    app.nav_history[app.nav_index].scroll_y = app.current_scroll_y;
    app.nav_history.truncate(app.nav_index + 1);
    app.nav_history.push(NavEntry {
        file_path,
        scroll_y,
    });
    app.nav_index = app.nav_history.len() - 1;
}

/// Restore the view to the entry at `nav_index`.
pub(super) fn restore_nav_entry(app: &mut MdrApp) -> Task<Message> {
    let entry = app.nav_history[app.nav_index].clone();
    let image_task = if entry.file_path != app.file_path {
        load_file(app, &entry.file_path)
    } else {
        Task::none()
    };
    let offset = RelativeOffset {
        x: 0.0,
        y: entry.scroll_y,
    };
    Task::batch([
        image_task,
        operation::snap_to(Id::new(SCROLLABLE_ID), offset),
    ])
}

// ---------------------------------------------------------------------------
// Link handling
// ---------------------------------------------------------------------------

/// Handle a clicked or activated link URL.
///
/// - `http://` / `https://` → open in the system browser.
/// - `#anchor` → scroll to the matching heading in the current document.
/// - otherwise → treat as a local file path (relative to the current
///   document) and load it, pushing the current position onto history.
pub(super) fn handle_link(app: &mut MdrApp, url: String) -> Task<Message> {
    if url.starts_with("smdr-mermaid:") {
        let code = url.strip_prefix("smdr-mermaid:").unwrap();
        let code = urlencoding::decode(code)
            .unwrap_or(std::borrow::Cow::Borrowed(code))
            .into_owned();
        if let Some(handle) = app.mermaid_cache.get(&code) {
            app.overlay = Overlay::MermaidModal(handle.clone(), 1.0);
        }
        return Task::none();
    }

    if url.starts_with("http://") || url.starts_with("https://") {
        let _ = open::that(&url);
        return Task::none();
    }

    if let Some(anchor) = url.strip_prefix('#') {
        return navigate_to_anchor(app, anchor);
    }

    // Local file link
    let raw = url.strip_prefix("file://").unwrap_or(&url);
    let target = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        let base = app.file_path.parent().unwrap_or(Path::new("."));
        base.join(raw)
    };
    if target.exists() && target.is_file() {
        push_nav(app, target.clone(), 0.0);
        let image_task = load_file(app, &target);
        Task::batch([
            image_task,
            operation::snap_to(Id::new(SCROLLABLE_ID), RelativeOffset { x: 0.0, y: 0.0 }),
        ])
    } else {
        eprintln!("Warning: could not open '{}'", target.display());
        Task::none()
    }
}

/// Navigate to an in-document anchor and push to navigation history.
pub(super) fn navigate_to_anchor(app: &mut MdrApp, anchor: &str) -> Task<Message> {
    if let Some(target_y) = compute_anchor_y(app, anchor) {
        push_nav(app, app.file_path.clone(), target_y);
        let offset = RelativeOffset {
            x: 0.0,
            y: target_y,
        };
        operation::snap_to(Id::new(SCROLLABLE_ID), offset)
    } else {
        Task::none()
    }
}

/// Compute the scroll-Y offset for a document anchor (heading slug).
///
/// Uses pre-computed slugs stored in [`MdrApp::toc`] (populated at load
/// time by [`md_helpers::extract_toc`]) instead of re-scanning raw
/// markdown on every anchor click.  Tries an exact GitHub-style slug
/// match first, then falls back to a relaxed normalized comparison.
/// Returns `None` if no matching heading is found.
fn compute_anchor_y(app: &MdrApp, anchor: &str) -> Option<f32> {
    if app.toc.is_empty() {
        return None;
    }

    // Line-count is still needed to convert a heading line number to a
    // scroll fraction (same calculation used by navigate_to_heading).
    let total_lines = app.line_count as f32;
    if total_lines <= 0.0 {
        return None;
    }

    let target_anchor = anchor.to_lowercase();

    // Pass 1: exact slug match (O(H), no allocation).
    for entry in &app.toc {
        if entry.slug == target_anchor {
            let fraction = (entry.line as f32) / total_lines;
            return Some(content_fraction_to_scroll_y(app, fraction));
        }
    }

    // Pass 2: relaxed normalized match.
    let anchor_normalized = md_helpers::normalize_for_match(&target_anchor);
    if anchor_normalized.is_empty() {
        return None;
    }
    for entry in &app.toc {
        if entry.slug_normalized == anchor_normalized {
            let fraction = (entry.line as f32) / total_lines;
            return Some(content_fraction_to_scroll_y(app, fraction));
        }
    }

    None
}

/// Scroll so that the link at `idx` is visible at the top of the viewport.
pub(super) fn scroll_to_link(app: &MdrApp, idx: usize) -> Task<Message> {
    let total_lines = app.line_count as f32;
    if total_lines <= 0.0 {
        return Task::none();
    }
    let line = app.links[idx].line as f32;
    let fraction = line / total_lines;
    let y = content_fraction_to_scroll_y(app, fraction);
    let offset = RelativeOffset { x: 0.0, y };
    operation::snap_to(Id::new(SCROLLABLE_ID), offset)
}

/// Navigate to a TOC heading entry by index.
///
/// Shared helper used by both the sidebar handler (direct TOC clicks) and
/// the main message dispatch (re-dispatched from `SidebarActivate`).
pub(super) fn navigate_to_heading(app: &mut MdrApp, idx: usize) -> Task<Message> {
    if let Some(entry) = app.toc.get(idx) {
        let total_lines = app.line_count as f32;
        if total_lines > 0.0 {
            let fraction = (entry.line as f32) / total_lines;
            let target_y = content_fraction_to_scroll_y(app, fraction);
            app.last_scroll_y = app.current_scroll_y;
            push_nav(app, app.file_path.clone(), target_y);
            let offset = RelativeOffset {
                x: 0.0,
                y: target_y,
            };
            return operation::snap_to(Id::new(SCROLLABLE_ID), offset);
        }
    }
    Task::none()
}
