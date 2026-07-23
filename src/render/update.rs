//! Central message dispatch — routes messages to feature handlers.

use iced::Task;
use iced::widget::Id;

use iced::widget::operation::{self, AbsoluteOffset, RelativeOffset};

use smdr::persist::{self, PersistedState};
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
            persist::save(&PersistedState { theme: theme_arg });
            Task::none()
        }
        Message::CycleTheme => {
            let all = ThemeArg::ALL;
            let idx = all.iter().position(|t| *t == app.active_theme).unwrap_or(0);
            app.active_theme = all[(idx + 1) % all.len()];
            persist::save(&PersistedState {
                theme: app.active_theme,
            });
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
        Message::MermaidZoomIn => {
            if let Overlay::MermaidModal(handle, zoom) = &app.overlay {
                app.overlay = Overlay::MermaidModal(handle.clone(), (*zoom * 1.2).min(5.0));
            }
            Task::none()
        }
        Message::MermaidZoomOut => {
            if let Overlay::MermaidModal(handle, zoom) = &app.overlay {
                app.overlay = Overlay::MermaidModal(handle.clone(), (*zoom / 1.2).max(0.2));
            }
            Task::none()
        }
        Message::MermaidScrollBy(dx, dy) => operation::scroll_by(
            Id::new(super::state::MERMAID_SCROLLABLE_ID),
            AbsoluteOffset { x: dx, y: dy },
        ),
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
        // --- Tab messages ---
        //
        // Tab model invariant: the *active* document lives inline in `MdrApp`
        // and logically occupies visual slot `active_tab`.  `app.tabs` holds
        // every *other* open document in visual order.  A background tab at
        // visual slot `v` therefore maps to `tabs[v]` when `v < active_tab`
        // and `tabs[v - 1]` when `v > active_tab`.
        Message::OpenInNewTab(path) => {
            // If this file is already open, don't create a duplicate tab:
            // switch to the existing tab (if needed) and reload it from disk so
            // the user sees the freshest content.  Paths are canonicalized
            // before comparison so `./a.md`, `a.md`, and an absolute path all
            // resolve to the same document.
            let target = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            let same = |p: &std::path::Path| {
                std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()) == target
            };

            // Already showing in the active tab → reload in place.
            if same(&app.file_path) {
                return images::load_file(app, &path);
            }

            // Open in a background tab → switch there, then reload.
            if let Some(vec_idx) = app.tabs.iter().position(|t| same(&t.file_path)) {
                // Map the vector index to its visual slot for `SwitchTab`.
                let visual = if vec_idx < app.active_tab {
                    vec_idx
                } else {
                    vec_idx + 1
                };
                return handle_message(app, Message::SwitchTab(visual))
                    .chain(images::load_file(app, &path));
            }

            // Not open anywhere — create a new tab.
            // Snapshot the current (outgoing) tab and re-insert it at its own
            // visual slot so ordering is preserved, then append the new tab at
            // the end and make it active.
            let old_active = app.active_tab;
            let saved = app.save_current_tab();
            app.tabs.insert(old_active, saved);
            app.active_tab = app.tabs.len();
            // Reset per-document UI state for the new tab.
            app.search_mode = false;
            app.search_query.clear();
            app.search_query_lower.clear();
            app.search_hits.clear();
            app.current_hit = None;
            app.overlay = Overlay::None;
            app.focused_link = None;
            images::load_file(app, &path)
        }
        Message::SwitchTab(index) => {
            let total = app.tabs.len() + 1;
            if index == app.active_tab || index >= total {
                return Task::none();
            }
            // Put the outgoing tab back at its visual slot, producing a fully
            // ordered `tabs` vector, then pull the target out of that vector.
            let old_active = app.active_tab;
            let saved = app.save_current_tab();
            app.tabs.insert(old_active, saved);
            let target = app.tabs.remove(index);
            app.restore_tab(target);
            app.active_tab = index;
            Task::none()
        }
        Message::CloseTab(index) => {
            let total = app.tabs.len() + 1;
            if total == 1 {
                // Only one tab open — closing it exits the app.
                return iced::exit();
            }
            if index == app.active_tab {
                // Closing the active tab: promote an adjacent background tab.
                // Prefer the next tab (visual active_tab + 1 → tabs[active_tab]);
                // if the active tab is the last one, promote the previous.
                if app.active_tab < app.tabs.len() {
                    let target = app.tabs.remove(app.active_tab);
                    app.restore_tab(target);
                    // active_tab stays: the promoted tab now occupies this slot.
                } else {
                    let target = app.tabs.remove(app.active_tab - 1);
                    app.restore_tab(target);
                    app.active_tab -= 1;
                }
            } else {
                // Closing a background tab: map its visual slot to a vec index.
                let vec_idx = if index < app.active_tab {
                    index
                } else {
                    index - 1
                };
                if vec_idx < app.tabs.len() {
                    app.tabs.remove(vec_idx);
                }
                // Removing a tab left of the active one shifts it one slot left.
                if index < app.active_tab {
                    app.active_tab -= 1;
                }
            }
            Task::none()
        }
        Message::NextTab => {
            app.pending_key = None;
            let total = app.tabs.len() + 1; // background tabs + active
            if total <= 1 {
                return Task::none();
            }
            let next = (app.active_tab + 1) % total;
            handle_message(app, Message::SwitchTab(next))
        }
        Message::PrevTab => {
            app.pending_key = None;
            let total = app.tabs.len() + 1;
            if total <= 1 {
                return Task::none();
            }
            let prev = if app.active_tab == 0 {
                total - 1
            } else {
                app.active_tab - 1
            };
            handle_message(app, Message::SwitchTab(prev))
        }
        Message::IpcFileReceived(path) => {
            // Open the received file path in a new tab.
            handle_message(app, Message::OpenInNewTab(path))
        }
        // --- Comment / review mode ---
        Message::ToggleCommentMode => {
            app.comment_mode = !app.comment_mode;
            // Leaving comment mode discards any in-progress composer draft.
            if !app.comment_mode {
                app.comment_target_line = None;
                app.comment_draft.clear();
            }
            Task::none()
        }
        Message::SourceEditorAction(action) => {
            // Read-only source view: apply navigation/selection/scroll actions
            // but ignore edits so the buffer always mirrors `raw_markdown`.
            // A click doubles as line selection for commenting: the resulting
            // cursor line seeds the composer target.
            if !action.is_edit() {
                let is_click = matches!(action, iced::widget::text_editor::Action::Click(_));
                app.source_content.perform(action);
                if is_click {
                    app.comment_target_line = Some(app.source_content.cursor().position.line);
                    return operation::focus(Id::new(super::state::COMMENT_INPUT_ID));
                }
            }
            Task::none()
        }
        Message::GutterLineClicked(line) => {
            app.comment_target_line = Some(line);
            app.comment_draft.clear();
            // Prefill the composer with any existing comment on this line.
            if let Some(existing) = app.comments.iter().find(|c| c.line == line) {
                app.comment_draft = existing.text.clone();
            }
            operation::focus(Id::new(super::state::COMMENT_INPUT_ID))
        }
        Message::CommentDraftChanged(text) => {
            app.comment_draft = text;
            Task::none()
        }
        Message::CommentSubmit => {
            if let Some(line) = app.comment_target_line {
                let text = app.comment_draft.trim().to_string();
                // Remove any prior comment on this line, then re-add if non-empty
                // (an empty submission deletes the comment).
                app.comments.retain(|c| c.line != line);
                if !text.is_empty() {
                    app.comments.push(super::state::LineComment { line, text });
                    app.comments.sort_by_key(|c| c.line);
                }
            }
            app.comment_target_line = None;
            app.comment_draft.clear();
            Task::none()
        }
        Message::CommentCancel => {
            app.comment_target_line = None;
            app.comment_draft.clear();
            Task::none()
        }
        // Search and sidebar messages are handled above and never reach here,
        // but Rust requires all variants covered.
        _ => Task::none(),
    }
}
