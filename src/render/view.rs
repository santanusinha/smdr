//! UI composition — view tree, status bar, overlays, and subscriptions.

use iced::border;
use iced::event;
use iced::keyboard;
use iced::mouse;
use iced::widget::{
    Id, button, column, container, markdown, mouse_area, pick_list, row, rule, scrollable, text,
    text_input,
};
use iced::{Alignment, Background, Color, Element, Event, Length, Pixels, Subscription};

use mdr::theme::ThemeArg;

use super::sidebar::build_sidebar;
use super::state::{LINE_SCROLL, MdrApp, Message, Overlay, SCROLLABLE_ID, SEARCH_INPUT_ID};
use super::widget::MdrViewer;

/// Build the main UI element tree.
pub(super) fn build_ui(app: &MdrApp) -> Element<'_, Message> {
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
    };
    let md_view: Element<Message> =
        markdown::view_with(app.content.items(), settings, &viewer).map(Message::LinkClicked);

    let content_area = scrollable(
        container(md_view)
            .padding(20)
            .max_width(860)
            .center_x(Length::Fill),
    )
    .id(Id::new(SCROLLABLE_ID))
    .on_scroll(Message::Scrolled)
    .width(Length::Fill)
    .height(Length::Fill);

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
        content_area.into()
    };

    // Assemble the full layout
    let mut layout = column![];

    if let Some(bar) = search_bar {
        layout = layout.push(bar);
    }

    if let Some(panel) = overlay_panel {
        layout = layout.push(main_body);
        layout = layout.push(panel);
    } else {
        layout = layout.push(main_body);
    }

    layout = layout.push(status_bar);
    layout.into()
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
        ("Cycle theme", "Ctrl-T", ""),
        ("Reload file", "Ctrl-R", ""),
        ("Copy document", "Ctrl-C", ""),
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
        text("About mdr").size(14),
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
        text(format!("mdr v{version}")).size(13),
        text("Minimal Desktop Markdown Reader").size(12),
        text("").size(6),
        text("Built with iced + pulldown-cmark").size(12),
        text("https://github.com/user/mdr").size(11),
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

/// Build the event subscription stream (keyboard, mouse, timer, resize).
pub(super) fn build_subscription(app: &MdrApp) -> Subscription<Message> {
    let search_mode = app.search_mode;
    let has_overlay = app.overlay != Overlay::None;

    let sidebar_focused = app.sidebar_focused;

    let keys = keyboard::listen()
        .with((search_mode, has_overlay, sidebar_focused))
        .filter_map(|((search_mode, has_overlay, sidebar_focused), event)| {
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
                        if modifiers.shift() {
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
                            "g" | "G" | "q" | "Z" | "`" => {
                                Some(Message::PendingKey(s.chars().next().unwrap()))
                            }
                            _ => None,
                        }
                    }
                }
                _ => None,
            }
        });

    // Global mouse event subscription for sidebar drag tracking
    let mouse_events = event::listen_with(|event, _status, _window| match event {
        Event::Mouse(mouse::Event::CursorMoved { position }) => {
            Some(Message::SidebarDragMove(position.x))
        }
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(Message::SidebarDragEnd)
        }
        _ => None,
    });

    let ticker = iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::Tick);

    let window_resize =
        iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size));
    Subscription::batch([keys, mouse_events, window_resize, ticker])
}
