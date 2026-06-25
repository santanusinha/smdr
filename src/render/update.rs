//! Central message dispatch — routes messages to feature handlers.

use iced::Task;
use iced::widget::Id;

use iced::widget::operation::{self, AbsoluteOffset, RelativeOffset};

use smdr::theme::ThemeArg;

use super::images;
use super::navigation;
use super::search;
use super::sidebar;
use super::state::{MdrApp, Message, Overlay, SCROLLABLE_ID};

/// Handle an incoming [`Message`] and produce a [`Task<Message`].
///
/// Feature-specific messages (search, sidebar) are delegated first; unhandled
/// messages fall through to the main dispatch below.  The `Result`-returning
/// delegation avoids cloning the owned [`Message`].
pub(super) fn handle_message(app: &mut MdrApp, message: Message) -> Task<Message> {
    // Delegate to feature-specific handlers first (avoids cloning —
    // unhandled messages are returned via Err).
    let message = match search::handle_message(app, message) {
        Ok(task) => return task,
        Err(msg) => msg,
    };
    let message = match sidebar::handle_message(app, message) {
        Ok(task) => return task,
        Err(msg) => msg,
    };

    match message {
        Message::LinkClicked(url) => navigation::handle_link(app, url),
        Message::ScrollBy(delta) => {
            app.focused_link = None;
            operation::scroll_by(Id::new(SCROLLABLE_ID), AbsoluteOffset { x: 0.0, y: delta })
        }
        Message::HistoryBack => {
            if app.nav_index == 0 {
                return Task::none();
            }
            app.focused_link = None;
            app.nav_history[app.nav_index].scroll_y = app.current_scroll_y;
            app.nav_index -= 1;
            navigation::restore_nav_entry(app)
        }
        Message::HistoryForward => {
            if app.nav_index + 1 >= app.nav_history.len() {
                return Task::none();
            }
            app.focused_link = None;
            app.nav_history[app.nav_index].scroll_y = app.current_scroll_y;
            app.nav_index += 1;
            navigation::restore_nav_entry(app)
        }
        Message::FocusNextLink => {
            if app.links.is_empty() {
                return Task::none();
            }
            let next = match app.focused_link {
                Some(i) => (i + 1) % app.links.len(),
                None => 0,
            };
            app.focused_link = Some(next);
            navigation::scroll_to_link(app, next)
        }
        Message::FocusPrevLink => {
            if app.links.is_empty() {
                return Task::none();
            }
            let prev = match app.focused_link {
                Some(0) => app.links.len() - 1,
                Some(i) => i - 1,
                None => app.links.len() - 1,
            };
            app.focused_link = Some(prev);
            navigation::scroll_to_link(app, prev)
        }
        Message::ActivateLink => {
            if app.focused_link.is_some() {
                let idx = app.focused_link.unwrap();
                let url = app.links[idx].url.clone();
                app.focused_link = None;
                navigation::handle_link(app, url)
            } else if !app.search_hits.is_empty() {
                // Enter cycles to next search hit when no link is focused
                let next = match app.current_hit {
                    Some(i) => (i + 1) % app.search_hits.len(),
                    None => 0,
                };
                app.current_hit = Some(next);
                search::scroll_to_current_hit(app)
            } else {
                Task::none()
            }
        }
        Message::ThemeChanged(theme_arg) => {
            app.active_theme = theme_arg;
            Task::none()
        }
        Message::CycleTheme => {
            let all = ThemeArg::ALL;
            let idx = all.iter().position(|t| *t == app.active_theme).unwrap_or(0);
            app.active_theme = all[(idx + 1) % all.len()];
            Task::none()
        }
        Message::ShowShortcuts => {
            app.overlay = if app.overlay == Overlay::Shortcuts {
                Overlay::None
            } else {
                Overlay::Shortcuts
            };
            Task::none()
        }
        Message::ShowAbout => {
            app.overlay = if app.overlay == Overlay::About {
                Overlay::None
            } else {
                Overlay::About
            };
            Task::none()
        }
        Message::CloseOverlay => {
            app.overlay = Overlay::None;
            Task::none()
        }
        Message::NavigateToHeading(idx) => navigation::navigate_to_heading(app, idx),
        Message::Tick => images::poll_watcher(app),
        Message::Scrolled(viewport) => {
            app.current_scroll_y = viewport.relative_offset().y;
            app.content_height = viewport.content_bounds().height;
            app.viewport_height = viewport.bounds().height;
            Task::none()
        }
        Message::WindowResized(size) => {
            app.window_width = size.width;
            Task::none()
        }
        Message::ImageLoaded(url, data) => {
            app.image_pending.remove(&url);
            match data {
                Some(img_data) => {
                    app.image_cache.insert(url, img_data);
                }
                None => {
                    app.image_failed.insert(url);
                }
            }
            Task::none()
        }
        Message::MermaidRendered(code, svg_bytes) => {
            app.mermaid_pending.remove(&code);
            if let Some(svg_bytes) = svg_bytes {
                app.mermaid_cache
                    .insert(code, iced::widget::svg::Handle::from_memory(svg_bytes));
            }
            Task::none()
        }
        Message::ScrollToTop => {
            app.last_scroll_y = app.current_scroll_y;
            app.pending_key = None;
            operation::snap_to(Id::new(SCROLLABLE_ID), RelativeOffset { x: 0.0, y: 0.0 })
        }
        Message::ScrollToBottom => {
            app.last_scroll_y = app.current_scroll_y;
            app.pending_key = None;
            operation::snap_to(Id::new(SCROLLABLE_ID), RelativeOffset { x: 0.0, y: 1.0 })
        }
        Message::JumpToLastPosition => {
            let target = app.last_scroll_y;
            app.last_scroll_y = app.current_scroll_y;
            app.pending_key = None;
            operation::snap_to(Id::new(SCROLLABLE_ID), RelativeOffset { x: 0.0, y: target })
        }
        Message::ExitApp => iced::exit(),
        Message::ReloadFile => {
            let path = app.file_path.clone();
            images::load_file(app, &path)
        }
        Message::CopyToClipboard => iced::clipboard::write(app.raw_markdown.clone()),
        Message::PendingKey(ch) => {
            if app.pending_key == Some(ch) {
                app.pending_key = None;
                match ch {
                    'g' => handle_message(app, Message::ScrollToTop),
                    'G' => handle_message(app, Message::ScrollToBottom),
                    'q' | 'Z' => handle_message(app, Message::ExitApp),
                    '`' => handle_message(app, Message::JumpToLastPosition),
                    _ => Task::none(),
                }
            } else {
                app.pending_key = Some(ch);
                Task::none()
            }
        }
        // Search and sidebar messages are handled above and never reach here,
        // but Rust requires all variants covered.
        _ => Task::none(),
    }
}
