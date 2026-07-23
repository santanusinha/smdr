//! UI composition — view tree, status bar, overlays, and subscriptions.

use iced::border;
use iced::event;
use iced::keyboard;
use iced::mouse;
use iced::widget::{
    Id, button, column, container, markdown, mouse_area, pick_list, row, rule, scrollable, text,
    text_editor, text_input,
};
use iced::{Alignment, Background, Color, Element, Event, Font, Length, Pixels, Subscription};

use smdr::theme::ThemeArg;

use super::sidebar::build_sidebar;
use super::state::{
    COMMENT_INPUT_ID, LINE_SCROLL, MdrApp, Message, Overlay, SCROLLABLE_ID, SEARCH_INPUT_ID,
    SOURCE_SCROLLABLE_ID,
};
use super::widget::MdrViewer;

/// Monospace text size for the read-only source (comment) view.
const SOURCE_TEXT_SIZE: f32 = 14.0;
/// Line-height factor for the source view; also drives gutter row height so
/// numbers align 1:1 with editor lines (including inside tables/code blocks).
const SOURCE_LINE_HEIGHT: f32 = 1.4;
/// Vertical padding above the first line, shared by editor and gutter so their
/// baselines line up.
const SOURCE_TOP_PAD: f32 = 8.0;

/// Build the main UI element tree.
pub(super) fn build_ui(app: &MdrApp) -> Element<'_, Message> {
    // --- Tab bar (shown only when more than one tab is open) ---
    let tab_bar: Option<Element<'_, Message>> = if app.tabs.is_empty() {
        None
    } else {
        Some(build_tab_bar(app))
    };

    let theme = app.active_theme.to_theme();
    let is_dark = theme.extended_palette().is_dark;
    let mut style = markdown::Style::from(&theme);

    // Theme-adaptive inline code styling (replaces harsh #111111 default)
    if is_dark {
        style.inline_code_highlight = markdown::Highlight {
            background: Background::Color(Color::from_rgb(0.18, 0.20, 0.25)),
            border: border::rounded(4),
        };
        style.inline_code_color = Color::from_rgb(0.85, 0.87, 0.91);
    } else {
        style.inline_code_highlight = markdown::Highlight {
            background: Background::Color(Color::from_rgb(0.91, 0.92, 0.94)),
            border: border::rounded(4),
        };
        style.inline_code_color = Color::from_rgb(0.15, 0.16, 0.18);
    }

    let settings = markdown::Settings {
        text_size: Pixels(16.0),
        h1_size: Pixels(24.0),
        h2_size: Pixels(21.0),
        h3_size: Pixels(18.0),
        h4_size: Pixels(17.0),
        h5_size: Pixels(16.0),
        h6_size: Pixels(16.0),
        code_size: Pixels(15.0),
        spacing: Pixels(14.0),
        style,
    };
    let viewer = MdrViewer {
        image_cache: &app.image_cache,
        image_pending: &app.image_pending,
        image_failed: &app.image_failed,
        mermaid_cache: &app.mermaid_cache,
        mermaid_pending: &app.mermaid_pending,
        search_query: &app.search_query,
        search_query_lower: &app.search_query_lower,
    };
    let md_view: Element<'_, Message> =
        markdown::view_with(app.content.items(), settings, &viewer).map(Message::LinkClicked);

    let content_area: Element<'_, Message> = if app.comment_mode {
        build_source_view(app, &theme)
    } else {
        scrollable(
            container(md_view)
                .padding(20)
                .max_width(860)
                .center_x(Length::Fill),
        )
        .id(Id::new(SCROLLABLE_ID))
        .on_scroll(Message::Scrolled)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    };

    // --- Search bar (shown above content when in search mode) ---
    let search_bar: Option<Element<'_, Message>> = if app.search_mode {
        let hit_info = if app.search_hits.is_empty() {
            if app.search_query.is_empty() {
                String::new()
            } else {
                "No matches".to_string()
            }
        } else {
            let idx = app.current_hit.map_or(0, |i| i + 1);
            format!("{}/{}", idx, app.search_hits.len())
        };

        Some(
            container(
                row![
                    text("/").size(14),
                    text_input("Search...", &app.search_query)
                        .id(Id::new(SEARCH_INPUT_ID))
                        .on_input(Message::SearchInput)
                        .on_submit(Message::SearchSubmit)
                        .width(Length::Fill)
                        .size(14),
                    text(hit_info).size(12),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding(6)
            .width(Length::Fill)
            .into(),
        )
    } else {
        None
    };

    // --- Permanent status bar (bottom) ---
    let status_bar = build_status_bar(app);

    // --- Overlay panel ---
    let overlay_panel: Option<Element<'_, Message>> = match &app.overlay {
        Overlay::None => None,
        Overlay::Shortcuts => Some(build_shortcuts_panel(app)),
        Overlay::About => Some(build_about_panel(app)),
        Overlay::MermaidModal(handle, zoom) => {
            Some(build_mermaid_modal(app, handle.clone(), *zoom))
        }
    };

    // --- Sidebar + content area ---
    let main_body: Element<'_, Message> = if app.sidebar_open && !app.toc.is_empty() {
        let sidebar = build_sidebar(app);

        // Drag handle: a narrow vertical rule wrapped in a MouseArea
        let drag_handle: Element<'_, Message> = mouse_area(
            container(rule::vertical(1))
                .height(Length::Fill)
                .padding([0, 2]),
        )
        .on_press(Message::SidebarDragStart)
        .interaction(mouse::Interaction::ResizingColumn)
        .into();

        row![sidebar, drag_handle, content_area]
            .height(Length::Fill)
            .into()
    } else {
        content_area
    };

    // Assemble the full layout
    let mut layout = column![];

    if let Some(bar) = tab_bar {
        layout = layout.push(bar);
    }

    if let Some(bar) = search_bar {
        layout = layout.push(bar);
    }

    if let Some(panel) = overlay_panel {
        if matches!(app.overlay, Overlay::MermaidModal(_, _)) {
            // Full-screen overlay for Mermaid diagram
            layout = layout.push(panel);
        } else {
            // Other panels just get stacked below main content (like a bottom sheet)
            layout = layout.push(main_body);
            layout = layout.push(panel);
        }
    } else {
        layout = layout.push(main_body);
    }

    layout = layout.push(status_bar);
    layout.into()
}

/// Build the read-only, line-numbered source view used for commenting.
///
/// The markdown *source* is shown in a read-only [`text_editor`] (edit actions
/// are dropped in [`update`](super::update)).  Because every source line maps to
/// exactly one editor line, a sibling gutter column of clickable line numbers
/// stays aligned even inside tables, lists, and fenced code blocks — which is
/// impossible with the flowed, rendered markdown view.  Clicking a gutter line
/// (or a line in the editor) opens the composer for that 0-based line.
fn build_source_view<'a>(app: &'a MdrApp, theme: &iced::Theme) -> Element<'a, Message> {
    let is_dark = theme.extended_palette().is_dark;
    let line_px = SOURCE_TEXT_SIZE * SOURCE_LINE_HEIGHT;
    let line_count = app.source_content.line_count();

    // Editor height is pinned so the outer scrollable (not the editor) drives
    // scrolling, keeping the gutter and text in lockstep.
    let editor_height = line_count as f32 * line_px + 2.0 * SOURCE_TOP_PAD;

    let hl_theme = if is_dark {
        iced::highlighter::Theme::Base16Ocean
    } else {
        iced::highlighter::Theme::InspiredGitHub
    };

    let editor = text_editor(&app.source_content)
        .on_action(Message::SourceEditorAction)
        .font(Font::MONOSPACE)
        .size(SOURCE_TEXT_SIZE)
        .line_height(SOURCE_LINE_HEIGHT)
        .padding([SOURCE_TOP_PAD, 8.0])
        .wrapping(text::Wrapping::None)
        .highlight("markdown", hl_theme)
        .height(Length::Fixed(editor_height));

    // --- Gutter: one clickable row per source line ---
    let target = app.comment_target_line;
    let mut gutter = column![].width(Length::Fixed(56.0));
    for i in 0..line_count {
        let has_comment = app.comments.iter().any(|c| c.line == i);
        let is_target = target == Some(i);

        // A leading marker column keeps the numbers right-aligned while showing
        // a bullet on commented lines.
        let marker = if has_comment { "●" } else { " " };
        let label = row![
            text(marker).size(SOURCE_TEXT_SIZE - 2.0).color(if is_dark {
                Color::from_rgb(0.45, 0.70, 1.0)
            } else {
                Color::from_rgb(0.15, 0.40, 0.85)
            }),
            container(
                text(format!("{}", i + 1))
                    .size(SOURCE_TEXT_SIZE - 2.0)
                    .color(if is_dark {
                        Color::from_rgb(0.55, 0.58, 0.65)
                    } else {
                        Color::from_rgb(0.50, 0.52, 0.58)
                    })
            )
            .width(Length::Fill)
            .align_x(Alignment::End),
        ]
        .spacing(3);

        let row_bg = if is_target {
            Some(if is_dark {
                Color::from_rgb(0.20, 0.24, 0.32)
            } else {
                Color::from_rgb(0.85, 0.90, 0.98)
            })
        } else {
            None
        };

        let cell = mouse_area(
            container(label)
                .width(Length::Fill)
                .height(Length::Fixed(line_px))
                .padding([0, 6])
                .align_y(Alignment::Center)
                .style(move |_t: &iced::Theme| container::Style {
                    background: row_bg.map(Background::Color),
                    ..container::Style::default()
                }),
        )
        .interaction(mouse::Interaction::Pointer)
        .on_press(Message::GutterLineClicked(i));

        gutter = gutter.push(cell);
    }

    let gutter_col =
        container(gutter)
            .padding([SOURCE_TOP_PAD, 0.0])
            .style(move |_t: &iced::Theme| container::Style {
                background: Some(Background::Color(if is_dark {
                    Color::from_rgb(0.10, 0.11, 0.13)
                } else {
                    Color::from_rgb(0.96, 0.97, 0.98)
                })),
                ..container::Style::default()
            });

    let body = scrollable(row![gutter_col, editor].width(Length::Fill))
        .id(Id::new(SOURCE_SCROLLABLE_ID))
        .width(Length::Fill)
        .height(Length::Fill);

    // --- Optional inline composer for the targeted line ---
    let mut col = column![].height(Length::Fill);

    // In review mode, a top toolbar lets the reviewer submit the turn: it
    // serializes every gutter-authored comment into the review envelope and
    // exits. Shown only when launched with `--review`.
    if app.review_mode {
        let count = app.comments.len();
        let submit_btn = button(text(format!("Submit review ({count})")).size(12))
            .on_press(Message::ReviewSubmit)
            .padding([4, 12])
            .style(button::primary);
        let toolbar = container(
            row![
                text("Review mode — click a gutter line to comment").size(12),
                container(submit_btn)
                    .width(Length::Fill)
                    .align_x(Alignment::End),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
        )
        .padding([6, 10])
        .width(Length::Fill)
        .style(container::rounded_box);
        col = col.push(toolbar);
    }

    col = col.push(body);
    if let Some(line) = target {
        col = col.push(build_comment_composer(app, line));
    }

    col.into()
}

/// Build the line-comment composer shown at the bottom of the source view.
fn build_comment_composer(app: &MdrApp, line: usize) -> Element<'_, Message> {
    let header = text(format!("Comment on line {}", line + 1)).size(12);
    let input = text_input("Write a comment…", &app.comment_draft)
        .id(Id::new(COMMENT_INPUT_ID))
        .on_input(Message::CommentDraftChanged)
        .on_submit(Message::CommentSubmit)
        .padding(6)
        .size(13)
        .width(Length::Fill);

    let save_btn = button(text("Save").size(12))
        .on_press(Message::CommentSubmit)
        .padding([4, 10])
        .style(button::primary);
    let cancel_btn = button(text("Cancel").size(12))
        .on_press(Message::CommentCancel)
        .padding([4, 10])
        .style(button::text);

    container(
        column![
            header,
            row![input, save_btn, cancel_btn]
                .spacing(8)
                .align_y(Alignment::Center),
        ]
        .spacing(6),
    )
    .padding(10)
    .width(Length::Fill)
    .style(container::rounded_box)
    .into()
}

/// Build the tab bar (shown when more than one tab is open).
///
/// Tabs are rendered in visual-slot order.  The *active* document lives inline
/// in [`MdrApp`] and occupies visual slot `active_tab`; every other slot is a
/// background tab pulled from `app.tabs`.  A background tab at vector index `k`
/// therefore appears at visual slot `k` when `k < active_tab` and `k + 1`
/// otherwise.  The active tab is highlighted; each tab carries a close (✕)
/// button.
fn build_tab_bar(app: &MdrApp) -> Element<'_, Message> {
    let mut tabs_row = row![].spacing(0);
    let total = app.tabs.len() + 1;

    for visual in 0..total {
        let is_active = visual == app.active_tab;

        // Resolve the label for this visual slot.
        let label: String = if is_active {
            app.file_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "stdin".to_string())
        } else {
            let vec_idx = if visual < app.active_tab {
                visual
            } else {
                visual - 1
            };
            app.tabs[vec_idx].label.clone()
        };

        let tab_btn = button(
            row![
                text(label).size(12),
                button(text("✕").size(10))
                    .on_press(Message::CloseTab(visual))
                    .padding([0, 2])
                    .style(button::text),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .on_press(Message::SwitchTab(visual))
        .padding([4, 10])
        .style(if is_active {
            button::primary
        } else {
            button::text
        });

        tabs_row = tabs_row.push(tab_btn);
    }

    container(tabs_row)
        .width(Length::Fill)
        .style(container::rounded_box)
        .into()
}

/// Build the permanent bottom status bar.
pub(super) fn build_status_bar(app: &MdrApp) -> Element<'_, Message> {
    // Left side: contextual messages
    let left_content: Element<'_, Message> = if !app.search_hits.is_empty() && !app.search_mode {
        let idx = app.current_hit.map_or(0, |i| i + 1);
        text(format!(
            "[{}/{}] \"{}\"",
            idx,
            app.search_hits.len(),
            app.search_query
        ))
        .size(12)
        .into()
    } else if let Some(idx) = app.focused_link {
        let link = &app.links[idx];
        text(format!(
            "[{}/{}] {} → {}",
            idx + 1,
            app.links.len(),
            link.text,
            link.url
        ))
        .size(12)
        .into()
    } else {
        text("").size(12).into()
    };

    // Right side: sidebar toggle + theme selector + shortcuts + about buttons
    let sidebar_btn = button(text("☰").size(14))
        .on_press(Message::SidebarToggleVisibility)
        .padding(4);

    let theme_picker = pick_list(ThemeArg::ALL, Some(app.active_theme), Message::ThemeChanged)
        .text_size(12)
        .padding([4, 8]);

    let shortcuts_btn = button(text("⌨").size(14))
        .on_press(Message::ShowShortcuts)
        .padding(4);

    let about_btn = button(text("ℹ").size(14))
        .on_press(Message::ShowAbout)
        .padding(4);

    let right_side = row![sidebar_btn, theme_picker, shortcuts_btn, about_btn]
        .spacing(6)
        .align_y(Alignment::Center);

    container(
        row![container(left_content).width(Length::Fill), right_side,]
            .align_y(Alignment::Center)
            .spacing(8),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .style(container::rounded_box)
    .into()
}

/// Build the keyboard shortcuts overlay panel.
pub(super) fn build_shortcuts_panel(app: &MdrApp) -> Element<'_, Message> {
    let _ = app; // shortcuts panel is static

    // (Action, Primary key, Vim-style key)
    let shortcuts: &[(&str, &str, &str)] = &[
        ("Scroll down", "↓", "j"),
        ("Scroll up", "↑", "k"),
        ("Page down", "PgDn / Space", "Ctrl-D"),
        ("Page up", "PgUp", "Ctrl-U"),
        ("Scroll to top", "Home", "gg"),
        ("Scroll to bottom", "End", "GG"),
        ("Jump to last position", "", "``"),
        ("Navigate back", "←", "h"),
        ("Navigate forward", "→", "l"),
        ("Next link", "Tab", ""),
        ("Previous link", "Shift-Tab", ""),
        ("Activate link / next hit", "Enter", ""),
        ("Open search", "Ctrl-F", "/"),
        ("Next search hit", "", "n"),
        ("Previous search hit", "", "p"),
        ("Toggle sidebar", "Ctrl-B", ""),
        ("Focus outline sidebar", "", "o"),
        ("Toggle comment (source) view", "", "c"),
        ("Cycle theme", "Ctrl-T", ""),
        ("Reload file", "Ctrl-R", ""),
        ("Copy document", "Ctrl-C", ""),
        ("Next tab", "Ctrl-Tab", "gt"),
        ("Previous tab", "Ctrl-Shift-Tab", "gT"),
        ("Close tab", "Ctrl-W", ""),
        ("Show keymap", "", "?"),
        ("Exit", "", "qq / ZZ"),
        ("Close search / overlay", "Esc", ""),
    ];

    // Table header
    let table_header = row![
        container(text("Action").size(11)).width(Length::Fixed(180.0)),
        container(text("Primary").size(11)).width(Length::Fixed(110.0)),
        container(text("Vim").size(11)).width(Length::Fixed(80.0)),
    ]
    .spacing(8);

    let separator = rule::horizontal(1);

    let mut table_rows = column![].spacing(3).padding(8);
    table_rows = table_rows.push(table_header);
    table_rows = table_rows.push(separator);

    for (action, primary, vim) in shortcuts {
        table_rows = table_rows.push(
            row![
                container(text(*action).size(12)).width(Length::Fixed(180.0)),
                container(text(*primary).size(12)).width(Length::Fixed(110.0)),
                container(text(*vim).size(12)).width(Length::Fixed(80.0)),
            ]
            .spacing(8),
        );
    }

    let header = row![
        text("Keyboard Shortcuts").size(14),
        container(
            button(text("✕").size(12))
                .on_press(Message::CloseOverlay)
                .padding(2)
        )
        .width(Length::Fill)
        .align_x(Alignment::End),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    container(column![header, table_rows].spacing(8).padding(12))
        .width(Length::Fill)
        .max_width(500)
        .center_x(Length::Fill)
        .style(container::rounded_box)
        .into()
}

/// Build the about overlay panel.
pub(super) fn build_about_panel(app: &MdrApp) -> Element<'_, Message> {
    let _ = app; // about panel is static
    let version = env!("CARGO_PKG_VERSION");

    let header = row![
        text("About smdr").size(14),
        container(
            button(text("✕").size(12))
                .on_press(Message::CloseOverlay)
                .padding(2)
        )
        .width(Length::Fill)
        .align_x(Alignment::End),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let info = column![
        text(format!("smdr v{version}")).size(13),
        text("Simple Markdown Reader").size(12),
        text("").size(6),
        text("Built with iced + pulldown-cmark").size(12),
        text("https://github.com/santanusinha/smdr").size(11),
    ]
    .spacing(4)
    .padding(8);

    container(column![header, info].spacing(8).padding(12))
        .width(Length::Fill)
        .max_width(400)
        .center_x(Length::Fill)
        .style(container::rounded_box)
        .into()
}

/// Build the mermaid overlay panel.
pub(super) fn build_mermaid_modal(
    app: &MdrApp,
    handle: iced::widget::svg::Handle,
    zoom: f32,
) -> Element<'_, Message> {
    let _ = app;

    let header = row![
        text(format!("Mermaid Diagram (Zoom: {:.1}x)", zoom)).size(14),
        container(
            row![
                button(text("−").size(14))
                    .on_press(Message::MermaidZoomOut)
                    .padding([2, 6]),
                button(text("+").size(14))
                    .on_press(Message::MermaidZoomIn)
                    .padding([2, 6]),
                button(text("✕").size(12))
                    .on_press(Message::CloseOverlay)
                    .padding([2, 6])
            ]
            .spacing(8)
        )
        .width(Length::Fill)
        .align_x(Alignment::End),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let base_size = 1000.0;
    let scaled_size = base_size * zoom;
    let svg_view = scrollable(
        container(
            iced::widget::svg(handle)
                .width(Length::Fixed(scaled_size))
                .height(Length::Fixed(scaled_size))
                .content_fit(iced::ContentFit::Contain),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Left)
        .align_y(iced::alignment::Vertical::Top),
    )
    .id(Id::new(super::state::MERMAID_SCROLLABLE_ID))
    .direction(iced::widget::scrollable::Direction::Both {
        vertical: iced::widget::scrollable::Scrollbar::new(),
        horizontal: iced::widget::scrollable::Scrollbar::new(),
    })
    .width(Length::Fill)
    .height(Length::Fill);

    container(column![header, svg_view].spacing(16).padding(16))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(container::rounded_box)
        .into()
}

/// Build the event subscription stream (keyboard, mouse, timer, resize).
pub(super) fn build_subscription(app: &MdrApp) -> Subscription<Message> {
    let search_mode = app.search_mode;
    let has_overlay = app.overlay != Overlay::None;

    let sidebar_focused = app.sidebar_focused;
    let sidebar_dragging = app.sidebar_dragging;

    let is_mermaid_modal = matches!(app.overlay, Overlay::MermaidModal(_, _));

    let keys = keyboard::listen()
        .with((
            search_mode,
            has_overlay,
            sidebar_focused,
            is_mermaid_modal,
            app.active_tab,
            app.pending_key,
        ))
        .filter_map(
            |(
                (
                    search_mode,
                    has_overlay,
                    sidebar_focused,
                    is_mermaid_modal,
                    active_tab,
                    pending_key,
                ),
                event,
            )| {
                let keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    text: _,
                    modified_key,
                    physical_key: _,
                    location: _,
                    repeat: _,
                } = event
                else {
                    return None;
                };

                // Escape always closes overlay, search, or sidebar focus
                if matches!(&key, keyboard::Key::Named(keyboard::key::Named::Escape)) {
                    if has_overlay {
                        return Some(Message::CloseOverlay);
                    }
                    if search_mode {
                        return Some(Message::SearchClose);
                    }
                    if sidebar_focused {
                        return Some(Message::UnfocusSidebar);
                    }
                    return None;
                }

                if search_mode {
                    return None;
                }

                if has_overlay {
                    if is_mermaid_modal {
                        if modifiers.control() {
                            let ctrl_s = match &key {
                                keyboard::Key::Character(c) => c.as_str(),
                                _ => "",
                            };
                            match ctrl_s {
                                "=" | "+" => return Some(Message::MermaidZoomIn),
                                "-" => return Some(Message::MermaidZoomOut),
                                _ => {}
                            }
                        } else {
                            match &key {
                                keyboard::Key::Named(named) => match named {
                                    keyboard::key::Named::ArrowDown => {
                                        return Some(Message::MermaidScrollBy(0.0, LINE_SCROLL));
                                    }
                                    keyboard::key::Named::ArrowUp => {
                                        return Some(Message::MermaidScrollBy(0.0, -LINE_SCROLL));
                                    }
                                    keyboard::key::Named::ArrowLeft => {
                                        return Some(Message::MermaidScrollBy(-LINE_SCROLL, 0.0));
                                    }
                                    keyboard::key::Named::ArrowRight => {
                                        return Some(Message::MermaidScrollBy(LINE_SCROLL, 0.0));
                                    }
                                    _ => {}
                                },
                                keyboard::Key::Character(c) => match c.as_str() {
                                    "j" => return Some(Message::MermaidScrollBy(0.0, LINE_SCROLL)),
                                    "k" => {
                                        return Some(Message::MermaidScrollBy(0.0, -LINE_SCROLL));
                                    }
                                    "h" => {
                                        return Some(Message::MermaidScrollBy(-LINE_SCROLL, 0.0));
                                    }
                                    "l" => return Some(Message::MermaidScrollBy(LINE_SCROLL, 0.0)),
                                    _ => {}
                                },
                                _ => {}
                            }
                        }
                    }
                    return None;
                }

                // Sidebar-focused mode: j/k/arrows navigate headings
                if sidebar_focused {
                    return match &key {
                        keyboard::Key::Named(named) => match named {
                            keyboard::key::Named::ArrowDown => Some(Message::SidebarNext),
                            keyboard::key::Named::ArrowUp => Some(Message::SidebarPrev),
                            keyboard::key::Named::Enter => Some(Message::SidebarActivate),
                            _ => None,
                        },
                        keyboard::Key::Character(c) => match c.as_str() {
                            "j" => Some(Message::SidebarNext),
                            "k" => Some(Message::SidebarPrev),
                            "o" => Some(Message::SidebarToggleFocus),
                            _ => {
                                if modifiers.control() && c.as_str() == "b" {
                                    Some(Message::SidebarToggleVisibility)
                                } else {
                                    None
                                }
                            }
                        },
                        _ => None,
                    };
                }

                match &key {
                    keyboard::Key::Named(named) => match named {
                        keyboard::key::Named::ArrowDown => Some(Message::ScrollBy(LINE_SCROLL)),
                        keyboard::key::Named::ArrowUp => Some(Message::ScrollBy(-LINE_SCROLL)),
                        keyboard::key::Named::ArrowLeft => Some(Message::HistoryBack),
                        keyboard::key::Named::ArrowRight => Some(Message::HistoryForward),
                        keyboard::key::Named::PageDown => Some(Message::ScrollBy(360.0)),
                        keyboard::key::Named::PageUp => Some(Message::ScrollBy(-360.0)),
                        keyboard::key::Named::Home => Some(Message::ScrollToTop),
                        keyboard::key::Named::End => Some(Message::ScrollToBottom),
                        keyboard::key::Named::Space => Some(Message::ScrollBy(360.0)),
                        keyboard::key::Named::Tab => {
                            if modifiers.control() {
                                if modifiers.shift() {
                                    Some(Message::PrevTab)
                                } else {
                                    Some(Message::NextTab)
                                }
                            } else if modifiers.shift() {
                                Some(Message::FocusPrevLink)
                            } else {
                                Some(Message::FocusNextLink)
                            }
                        }
                        keyboard::key::Named::Enter => Some(Message::ActivateLink),
                        _ => None,
                    },
                    keyboard::Key::Character(_) => {
                        // Use `key` (unmodified) for Ctrl combos, `modified_key`
                        // (shift-aware) for plain keystrokes so '?' (Shift+/) is
                        // distinguished from '/' and 'G' (Shift+g) from 'g'.
                        let ctrl_s = match &key {
                            keyboard::Key::Character(c) => c.as_str(),
                            _ => "",
                        };
                        let s = match &modified_key {
                            keyboard::Key::Character(c) => c.as_str(),
                            _ => "",
                        };
                        if modifiers.control() {
                            match ctrl_s {
                                "d" => Some(Message::ScrollBy(360.0)),
                                "u" => Some(Message::ScrollBy(-360.0)),
                                "f" => Some(Message::SearchOpen),
                                "b" => Some(Message::SidebarToggleVisibility),
                                "t" => Some(Message::CycleTheme),
                                "r" => Some(Message::ReloadFile),
                                "c" => Some(Message::CopyToClipboard),
                                "w" => Some(Message::CloseTab(active_tab)),
                                _ => None,
                            }
                        } else if modifiers.alt() {
                            None
                        } else {
                            match s {
                                "j" => Some(Message::ScrollBy(LINE_SCROLL)),
                                "k" => Some(Message::ScrollBy(-LINE_SCROLL)),
                                "h" => Some(Message::HistoryBack),
                                "l" => Some(Message::HistoryForward),
                                "n" => Some(Message::SearchNext),
                                "p" => Some(Message::SearchPrev),
                                "/" => Some(Message::SearchOpen),
                                "?" => Some(Message::ShowShortcuts),
                                "o" => Some(Message::SidebarToggleFocus),
                                "c" => Some(Message::ToggleCommentMode),
                                // Vim-style tab switching: `gt` → next tab,
                                // `gT` → previous tab (both wrap around).  These
                                // fire only when `g` is the pending prefix key.
                                "t" if pending_key == Some('g') => Some(Message::NextTab),
                                "T" if pending_key == Some('g') => Some(Message::PrevTab),
                                "g" | "G" | "q" | "Z" | "`" => {
                                    Some(Message::PendingKey(s.chars().next().unwrap()))
                                }
                                _ => None,
                            }
                        }
                    }
                    _ => None,
                }
            },
        );
    // Mouse events for sidebar drag tracking.
    //
    // CRITICAL: We gate on `sidebar_dragging` so that cursor-move events only
    // produce messages while an active drag is in progress.  Without this gate,
    // every mouse movement would emit a `SidebarDragMove` message, causing iced
    // to rebuild the view on each mouse-motion event — an infinite re-render
    // loop that pins the CPU at 100%.
    //
    // We use `event::listen()` (returns `Subscription<Event>`) chained with
    // `.with(sidebar_dragging)` and `.filter_map()` because `listen_with`
    // requires a plain `fn` pointer that cannot capture state.
    let mouse_events =
        event::listen()
            .with(sidebar_dragging)
            .filter_map(|(sidebar_dragging, event)| {
                if !sidebar_dragging {
                    return None;
                }
                match event {
                    Event::Mouse(mouse::Event::CursorMoved { position }) => {
                        Some(Message::SidebarDragMove(position.x))
                    }
                    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                        Some(Message::SidebarDragEnd)
                    }
                    _ => None,
                }
            });

    let ticker = iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::Tick);

    let window_resize =
        iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size));

    // IPC: receive file paths sent by later smdr invocations and open them as
    // new tabs.  Only the first instance binds the socket; later invocations
    // hand off via `ipc::client_send` and exit before reaching the GUI.
    let ipc = Subscription::run(crate::ipc::server_worker).map(Message::IpcFileReceived);

    Subscription::batch([keys, mouse_events, window_resize, ticker, ipc])
}
