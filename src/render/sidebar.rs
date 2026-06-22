//! Sidebar visibility, TOC navigation, and resize handling.

use iced::widget::operation::{self, RelativeOffset};
use iced::widget::{Id, button, column, container, row, scrollable, text};
use iced::{Element, Length, Task};

use super::navigation;
use super::state::{
    MAX_SIDEBAR_RATIO, MIN_SIDEBAR_RATIO, MdrApp, Message, SCROLLABLE_ID, SIDEBAR_SCROLLABLE_ID,
};

// ---------------------------------------------------------------------------
// Message handler — dispatches sidebar-related messages.
// ---------------------------------------------------------------------------

/// Handle sidebar-related messages.
///
/// Returns `Ok(task)` if the message was a sidebar message,
/// `Err(message)` if it should be handled by the caller.
pub(super) fn handle_message(app: &mut MdrApp, message: Message) -> Result<Task<Message>, Message> {
    match message {
        Message::SidebarToggleVisibility => {
            // Ctrl-B: closed → open+focus+select; open → close+unfocus
            if app.sidebar_open {
                app.sidebar_open = false;
                app.sidebar_focused = false;
                Ok(Task::none())
            } else {
                app.sidebar_open = true;
                app.sidebar_focused = true;
                app.sidebar_selected = section_for_scroll_position(app);
                Ok(snap_sidebar_to_selected(app))
            }
        }
        Message::SidebarToggleFocus => {
            // 'o': closed → open+focus+select; open+unfocused → focus+select;
            //       open+focused → unfocus (sidebar stays visible)
            if !app.sidebar_open {
                app.sidebar_open = true;
                app.sidebar_focused = true;
                app.sidebar_selected = section_for_scroll_position(app);
                Ok(snap_sidebar_to_selected(app))
            } else if !app.sidebar_focused {
                app.sidebar_focused = true;
                app.sidebar_selected = section_for_scroll_position(app);
                Ok(snap_sidebar_to_selected(app))
            } else {
                app.sidebar_focused = false;
                Ok(Task::none())
            }
        }
        Message::UnfocusSidebar => {
            app.sidebar_focused = false;
            Ok(Task::none())
        }
        Message::SidebarNext => {
            if app.toc.is_empty() {
                return Ok(Task::none());
            }
            let next = match app.sidebar_selected {
                Some(i) if i + 1 < app.toc.len() => i + 1,
                Some(i) => i,
                None => 0,
            };
            app.sidebar_selected = Some(next);
            Ok(snap_sidebar_to_selected(app))
        }
        Message::SidebarPrev => {
            if app.toc.is_empty() {
                return Ok(Task::none());
            }
            let prev = match app.sidebar_selected {
                Some(0) | None => 0,
                Some(i) => i - 1,
            };
            app.sidebar_selected = Some(prev);
            Ok(snap_sidebar_to_selected(app))
        }
        Message::SidebarActivate => {
            if let Some(idx) = app.sidebar_selected {
                app.sidebar_focused = false;
                // Re-dispatch as NavigateToHeading
                return Err(Message::NavigateToHeading(idx));
            }
            Ok(Task::none())
        }
        Message::SidebarDragStart => {
            app.sidebar_dragging = true;
            Ok(Task::none())
        }
        Message::SidebarDragMove(x) => {
            if app.sidebar_dragging {
                app.sidebar_ratio =
                    (x / app.window_width).clamp(MIN_SIDEBAR_RATIO, MAX_SIDEBAR_RATIO);
            }
            Ok(Task::none())
        }
        Message::SidebarDragEnd => {
            app.sidebar_dragging = false;
            Ok(Task::none())
        }
        Message::NavigateToHeading(idx) => {
            if let Some(entry) = app.toc.get(idx) {
                let total_lines = app.raw_markdown.lines().count() as f32;
                if total_lines > 0.0 {
                    let fraction = (entry.line as f32) / total_lines;
                    let target_y = navigation::content_fraction_to_scroll_y(app, fraction);
                    app.last_scroll_y = app.current_scroll_y;
                    app.push_nav(app.file_path.clone(), target_y);
                    let offset = RelativeOffset {
                        x: 0.0,
                        y: target_y,
                    };
                    return Ok(operation::snap_to(Id::new(SCROLLABLE_ID), offset));
                }
            }
            Ok(Task::none())
        }
        other => Err(other),
    }
}

// ---------------------------------------------------------------------------
// Sidebar helpers
// ---------------------------------------------------------------------------

/// Build the collapsible left sidebar showing document outline.
pub(super) fn build_sidebar(app: &MdrApp) -> Element<'_, Message> {
    let min_level = app.toc.iter().map(|e| e.level).min().unwrap_or(1);

    let mut items = column![].spacing(2).padding([8, 4]);

    for (idx, entry) in app.toc.iter().enumerate() {
        let indent = ((entry.level - min_level) as u16) * 12;
        let is_selected = app.sidebar_focused && app.sidebar_selected == Some(idx);
        let label = text(&entry.text).size(13);
        let btn_style = if is_selected {
            button::primary
        } else {
            button::text
        };
        let btn = button(label)
            .on_press(Message::NavigateToHeading(idx))
            .padding([2, 4])
            .style(btn_style);

        let left_pad = iced::Padding::ZERO.left((indent as f32) * 1.0);
        items = items.push(container(btn).padding(left_pad));
    }

    let header_text = if app.sidebar_focused {
        "Outline ●"
    } else {
        "Outline"
    };
    let header = row![
        text(header_text).size(13),
        container(
            button(text("✕").size(11))
                .on_press(Message::SidebarToggleVisibility)
                .padding(2)
                .style(button::text)
        )
        .width(Length::Fill)
        .align_x(iced::Alignment::End),
    ]
    .align_y(iced::Alignment::Center)
    .padding([4, 8])
    .width(Length::Fill);

    container(
        column![
            header,
            scrollable(items)
                .id(Id::new(SIDEBAR_SCROLLABLE_ID))
                .height(Length::Fill)
        ]
        .height(Length::Fill)
        .width(Length::Fixed(app.window_width * app.sidebar_ratio)),
    )
    .height(Length::Fill)
    .style(container::rounded_box)
    .into()
}

/// Determine which section (heading index) is currently visible.
pub(super) fn section_for_scroll_position(app: &MdrApp) -> Option<usize> {
    if app.toc.is_empty() {
        return None;
    }
    let total_lines = app.raw_markdown.lines().count() as f32;
    if total_lines <= 0.0 {
        return Some(0);
    }
    let content_fraction = navigation::scroll_y_to_content_fraction(app);
    let current_line = (content_fraction * total_lines) as usize;
    let mut best = 0;
    for (i, entry) in app.toc.iter().enumerate() {
        if entry.line <= current_line {
            best = i;
        } else {
            break;
        }
    }
    Some(best)
}

/// Scroll the sidebar so the currently selected heading is visible.
pub(super) fn snap_sidebar_to_selected(app: &MdrApp) -> Task<Message> {
    let selected = match app.sidebar_selected {
        Some(i) => i,
        None => return Task::none(),
    };
    let total = app.toc.len();
    if total == 0 {
        return Task::none();
    }
    let y = if total <= 1 {
        0.0
    } else {
        (selected as f32) / ((total - 1) as f32)
    };
    operation::snap_to(Id::new(SIDEBAR_SCROLLABLE_ID), RelativeOffset { x: 0.0, y })
}
