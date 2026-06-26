//! Search mode, query handling, and hit navigation.

use iced::Task;
use iced::widget::Id;
use iced::widget::operation::{self, RelativeOffset};

use super::navigation;
use super::state::{MdrApp, Message, SCROLLABLE_ID, SEARCH_INPUT_ID};

// ---------------------------------------------------------------------------
// Message handler — dispatches search-related messages.
// ---------------------------------------------------------------------------

/// Handle search-related messages.
///
/// Returns `Ok(task)` if the message was a search message,
/// `Err(message)` if it should be handled by the caller.
pub(super) fn handle_message(app: &mut MdrApp, message: Message) -> Result<Task<Message>, Message> {
    match message {
        Message::SearchOpen => {
            app.focused_link = None;
            app.search_mode = true;
            Ok(operation::focus(Id::new(SEARCH_INPUT_ID)))
        }
        Message::SearchClose => {
            app.search_mode = false;
            app.search_query.clear();
            app.search_query_lower.clear();
            app.search_hits.clear();
            app.current_hit = None;
            Ok(Task::none())
        }
        Message::SearchInput(q) => {
            app.search_query_lower = q.to_lowercase();
            app.search_query = q;
            recompute_search_hits(app);
            Ok(scroll_to_current_hit(app))
        }
        Message::SearchSubmit => {
            recompute_search_hits(app);
            app.search_mode = false;
            Ok(scroll_to_current_hit(app))
        }
        Message::SearchNext => {
            if !app.search_hits.is_empty() {
                let next = match app.current_hit {
                    Some(i) => (i + 1) % app.search_hits.len(),
                    None => 0,
                };
                app.current_hit = Some(next);
            }
            Ok(scroll_to_current_hit(app))
        }
        Message::SearchPrev => {
            if !app.search_hits.is_empty() {
                let prev = match app.current_hit {
                    Some(0) => app.search_hits.len() - 1,
                    Some(i) => i - 1,
                    None => app.search_hits.len() - 1,
                };
                app.current_hit = Some(prev);
            }
            Ok(scroll_to_current_hit(app))
        }
        other => Err(other),
    }
}

// ---------------------------------------------------------------------------
// Search helpers
// ---------------------------------------------------------------------------

/// Recompute the set of line indices matching the current search query.
/// Recompute the set of line indices matching the current search query.
pub(super) fn recompute_search_hits(app: &mut MdrApp) {
    app.search_hits.clear();
    app.current_hit = None;

    if app.search_query_lower.is_empty() {
        return;
    }

    for (i, line) in app.raw_markdown.lines().enumerate() {
        if line
            .to_lowercase()
            .contains(app.search_query_lower.as_str())
        {
            app.search_hits.push(i);
        }
    }

    if !app.search_hits.is_empty() {
        app.current_hit = Some(0);
    }
}

/// Snap the content scrollable so the current search hit is visible.
pub(super) fn scroll_to_current_hit(app: &MdrApp) -> Task<Message> {
    let Some(hit_idx) = app.current_hit else {
        return Task::none();
    };
    let Some(&line_num) = app.search_hits.get(hit_idx) else {
        return Task::none();
    };

    let total_lines = app.line_count as f32;
    if total_lines <= 0.0 {
        return Task::none();
    }

    let fraction = (line_num as f32) / total_lines;
    let y = navigation::content_fraction_to_scroll_y(app, fraction);
    let offset = RelativeOffset { x: 0.0, y };
    operation::snap_to(Id::new(SCROLLABLE_ID), offset)
}
