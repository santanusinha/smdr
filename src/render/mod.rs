//! iced-based markdown viewer — window, rendering, keyboard navigation, and search.
//!
//! Responsibilities:
//! - Create a native OS window via `iced::application`.
//! - Render the markdown document using `iced::widget::markdown`.
//! - Poll the file-watcher channel and hot-reload on changes (`--watch`).
//! - Intercept link clicks: open external URLs in the browser, navigate local
//!   links within the viewer.
//! - Vim-style navigation keys (j/k, Ctrl-U/D, arrows, PageUp/PageDown).
//! - Browser-like navigation history: clicking links or anchors pushes to
//!   history; h/Left (back) and l/Right (forward) traverse that history.
//! - Tab/Shift-Tab to cycle through document links, Enter to activate.
//! - `/` or `?` to search, `n`/`p` to cycle through matches.
//! - Permanent bottom status bar with theme selector, shortcuts, and about.
//! - Collapsible, resizable left sidebar showing document outline (headings).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use iced::border;
use iced::event;
use iced::keyboard;
use iced::mouse;
use iced::widget::Id;
use iced::widget::operation::{self, AbsoluteOffset, RelativeOffset};
use iced::widget::{
    button, column, container, markdown, mouse_area, pick_list, row, rule, scrollable, text,
    text_input,
};
use iced::{Background, Color, Element, Event, Length, Pixels, Subscription, Task};

use mdr::markdown::{self as md_helpers};
use mdr::theme::ThemeArg;
use mdr::watcher;

mod images;
mod navigation;
mod search;
mod sidebar;
mod state;
mod styles;
mod widget;

use navigation as nav;
pub use state::ViewerConfig;
use state::{
    DEFAULT_SIDEBAR_RATIO, INITIAL_WINDOW_WIDTH, LINE_SCROLL, MdrApp, Message, NavEntry, Overlay,
    SCROLLABLE_ID, SEARCH_INPUT_ID,
};
use widget::MdrViewer;

/// Launches the viewer window and blocks until it is closed.
///
/// # Errors
/// Returns an error if the window cannot be created.
pub fn launch(file_path: &Path, config: &ViewerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let markdown_src = std::fs::read_to_string(file_path)?;

    let watcher_rx: Option<Receiver<()>> = if config.watch {
        match watcher::watch_file(file_path) {
            Ok((w, rx)) => {
                Box::leak(Box::new(w));
                Some(rx)
            }
            Err(e) => {
                eprintln!("Warning: could not set up file watcher: {e}");
                None
            }
        }
    } else {
        None
    };

    let title = format!(
        "mdr — {}",
        file_path.file_name().unwrap_or_default().to_string_lossy()
    );

    let theme_arg = config.theme;
    let file_path = file_path.to_path_buf();

    let app_state = AppInit {
        markdown_src,
        file_path,
        watcher_rx,
        theme: theme_arg,
        title,
        network_enabled: config.network_enabled,
    };

    // iced requires Fn (not FnOnce) for boot.  We use a Mutex<Option<_>> to
    // move the one-shot init data out on the first (and only) invocation.
    let init = std::sync::Mutex::new(Some(app_state));

    iced::application(
        move || {
            init.lock()
                .unwrap()
                .take()
                .expect("boot called more than once")
                .build()
        },
        MdrApp::update,
        MdrApp::view,
    )
    .subscription(MdrApp::subscription)
    .theme(|app: &MdrApp| app.active_theme.to_theme())
    .title(|app: &MdrApp| app.title.clone())
    .window_size((960.0, 720.0))
    .run()
    .map_err(|e| e.to_string().into())
}

/// Launches the viewer window with content read from stdin.
///
/// Behaves identically to [`launch`] but does not require a file path.
/// File watching is not available in this mode.
///
/// # Errors
/// Returns an error if the window cannot be created.
pub fn launch_stdin(
    content: String,
    config: &ViewerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let title = String::from("mdr \u{2014} stdin");
    // Use cwd so relative links in the document resolve against the invoking directory.
    let file_path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("stdin");

    let app_state = AppInit {
        markdown_src: content,
        file_path,
        watcher_rx: None,
        theme: config.theme,
        title,
        network_enabled: config.network_enabled,
    };

    let init = std::sync::Mutex::new(Some(app_state));

    iced::application(
        move || {
            init.lock()
                .unwrap()
                .take()
                .expect("boot called more than once")
                .build()
        },
        MdrApp::update,
        MdrApp::view,
    )
    .subscription(MdrApp::subscription)
    .theme(|app: &MdrApp| app.active_theme.to_theme())
    .title(|app: &MdrApp| app.title.clone())
    .window_size((960.0, 720.0))
    .run()
    .map_err(|e| e.to_string().into())
}

/// Initialization data passed into the iced application.
struct AppInit {
    markdown_src: String,
    file_path: PathBuf,
    watcher_rx: Option<Receiver<()>>,
    theme: ThemeArg,
    title: String,
    network_enabled: bool,
}

impl AppInit {
    fn build(self) -> (MdrApp, Task<Message>) {
        let links = md_helpers::extract_links(&self.markdown_src);
        let toc = md_helpers::extract_toc(&self.markdown_src);
        let content = markdown::Content::parse(&self.markdown_src);
        let base_dir = self
            .file_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let network_enabled = self.network_enabled;

        let mut app = MdrApp {
            raw_markdown: self.markdown_src,
            content,
            file_path: self.file_path.clone(),
            watcher_rx: self.watcher_rx,
            active_theme: self.theme,
            title: self.title,
            nav_history: vec![NavEntry {
                file_path: self.file_path,
                scroll_y: 0.0,
            }],
            nav_index: 0,
            current_scroll_y: 0.0,
            links,
            focused_link: None,
            search_mode: false,
            search_query: String::new(),
            search_hits: Vec::new(),
            current_hit: None,
            overlay: Overlay::None,
            toc,
            sidebar_open: true,
            sidebar_focused: false,
            sidebar_selected: None,
            sidebar_ratio: DEFAULT_SIDEBAR_RATIO,
            sidebar_dragging: false,
            window_width: INITIAL_WINDOW_WIDTH,
            image_cache: HashMap::new(),
            image_pending: HashSet::new(),
            image_failed: HashSet::new(),
            mermaid_cache: HashMap::new(),
            network_enabled,
            base_dir: base_dir.clone(),
            pending_key: None,
            last_scroll_y: 0.0,
            content_height: 0.0,
            viewport_height: 0.0,
        };

        // Pre-render mermaid diagrams
        images::prerender_mermaid(&mut app);

        // Mark image URLs as pending and spawn loading tasks
        let task = images::spawn_image_loads(&mut app);

        (app, task)
    }
}

impl MdrApp {
    // -----------------------------------------------------------------------
    // Update
    // -----------------------------------------------------------------------

    fn update(&mut self, message: Message) -> Task<Message> {
        // Delegate to feature-specific handlers first (avoids cloning —
        // unhandled messages are returned via Err).
        let message = match search::handle_message(self, message) {
            Ok(task) => return task,
            Err(msg) => msg,
        };
        let message = match sidebar::handle_message(self, message) {
            Ok(task) => return task,
            Err(msg) => msg,
        };
        match message {
            Message::LinkClicked(url) => self.handle_link(url),
            Message::ScrollBy(delta) => {
                self.focused_link = None;
                operation::scroll_by(Id::new(SCROLLABLE_ID), AbsoluteOffset { x: 0.0, y: delta })
            }
            Message::HistoryBack => {
                if self.nav_index == 0 {
                    return Task::none();
                }
                self.focused_link = None;
                self.nav_history[self.nav_index].scroll_y = self.current_scroll_y;
                self.nav_index -= 1;
                self.restore_nav_entry()
            }
            Message::HistoryForward => {
                if self.nav_index + 1 >= self.nav_history.len() {
                    return Task::none();
                }
                self.focused_link = None;
                self.nav_history[self.nav_index].scroll_y = self.current_scroll_y;
                self.nav_index += 1;
                self.restore_nav_entry()
            }
            Message::FocusNextLink => {
                if self.links.is_empty() {
                    return Task::none();
                }
                let next = match self.focused_link {
                    Some(i) => (i + 1) % self.links.len(),
                    None => 0,
                };
                self.focused_link = Some(next);
                self.scroll_to_link(next)
            }
            Message::FocusPrevLink => {
                if self.links.is_empty() {
                    return Task::none();
                }
                let prev = match self.focused_link {
                    Some(0) => self.links.len() - 1,
                    Some(i) => i - 1,
                    None => self.links.len() - 1,
                };
                self.focused_link = Some(prev);
                self.scroll_to_link(prev)
            }
            Message::ActivateLink => {
                if self.focused_link.is_some() {
                    let idx = self.focused_link.unwrap();
                    let url = self.links[idx].url.clone();
                    self.focused_link = None;
                    self.handle_link(url)
                } else if !self.search_hits.is_empty() {
                    // Enter cycles to next search hit when no link is focused
                    let next = match self.current_hit {
                        Some(i) => (i + 1) % self.search_hits.len(),
                        None => 0,
                    };
                    self.current_hit = Some(next);
                    search::scroll_to_current_hit(self)
                } else {
                    Task::none()
                }
            }
            Message::ThemeChanged(theme_arg) => {
                self.active_theme = theme_arg;
                Task::none()
            }
            Message::CycleTheme => {
                let all = ThemeArg::ALL;
                let idx = all
                    .iter()
                    .position(|t| *t == self.active_theme)
                    .unwrap_or(0);
                self.active_theme = all[(idx + 1) % all.len()];
                Task::none()
            }
            Message::ShowShortcuts => {
                self.overlay = if self.overlay == Overlay::Shortcuts {
                    Overlay::None
                } else {
                    Overlay::Shortcuts
                };
                Task::none()
            }
            Message::ShowAbout => {
                self.overlay = if self.overlay == Overlay::About {
                    Overlay::None
                } else {
                    Overlay::About
                };
                Task::none()
            }
            Message::CloseOverlay => {
                self.overlay = Overlay::None;
                Task::none()
            }
            Message::NavigateToHeading(idx) => {
                if let Some(entry) = self.toc.get(idx) {
                    let total_lines = self.raw_markdown.lines().count() as f32;
                    if total_lines > 0.0 {
                        let fraction = (entry.line as f32) / total_lines;
                        let target_y = nav::content_fraction_to_scroll_y(self, fraction);
                        self.last_scroll_y = self.current_scroll_y;
                        self.push_nav(self.file_path.clone(), target_y);
                        let offset = RelativeOffset {
                            x: 0.0,
                            y: target_y,
                        };
                        return operation::snap_to(Id::new(SCROLLABLE_ID), offset);
                    }
                }
                Task::none()
            }
            Message::Tick => images::poll_watcher(self),
            Message::Scrolled(viewport) => {
                self.current_scroll_y = viewport.relative_offset().y;
                self.content_height = viewport.content_bounds().height;
                self.viewport_height = viewport.bounds().height;
                Task::none()
            }
            Message::WindowResized(size) => {
                self.window_width = size.width;
                Task::none()
            }
            Message::ImageLoaded(url, data) => {
                self.image_pending.remove(&url);
                match data {
                    Some(img_data) => {
                        self.image_cache.insert(url, img_data);
                    }
                    None => {
                        self.image_failed.insert(url);
                    }
                }
                Task::none()
            }
            Message::ScrollToTop => {
                self.last_scroll_y = self.current_scroll_y;
                self.pending_key = None;
                operation::snap_to(Id::new(SCROLLABLE_ID), RelativeOffset { x: 0.0, y: 0.0 })
            }
            Message::ScrollToBottom => {
                self.last_scroll_y = self.current_scroll_y;
                self.pending_key = None;
                operation::snap_to(Id::new(SCROLLABLE_ID), RelativeOffset { x: 0.0, y: 1.0 })
            }
            Message::JumpToLastPosition => {
                let target = self.last_scroll_y;
                self.last_scroll_y = self.current_scroll_y;
                self.pending_key = None;
                operation::snap_to(Id::new(SCROLLABLE_ID), RelativeOffset { x: 0.0, y: target })
            }
            Message::ExitApp => iced::exit(),
            Message::PendingKey(ch) => {
                if self.pending_key == Some(ch) {
                    self.pending_key = None;
                    match ch {
                        'g' => self.update(Message::ScrollToTop),
                        'G' => self.update(Message::ScrollToBottom),
                        'q' | 'Z' => self.update(Message::ExitApp),
                        '`' => self.update(Message::JumpToLastPosition),
                        _ => Task::none(),
                    }
                } else {
                    self.pending_key = Some(ch);
                    Task::none()
                }
            }
            _ => Task::none(),
        }
    }

    // -----------------------------------------------------------------------
    // View
    // -----------------------------------------------------------------------
    fn view(&self) -> Element<'_, Message> {
        let theme = self.active_theme.to_theme();
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
            image_cache: &self.image_cache,
            image_pending: &self.image_pending,
            image_failed: &self.image_failed,
            mermaid_cache: &self.mermaid_cache,
        };
        let md_view: Element<Message> =
            markdown::view_with(self.content.items(), settings, &viewer).map(Message::LinkClicked);

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
        let search_bar: Option<Element<'_, Message>> = if self.search_mode {
            let hit_info = if self.search_hits.is_empty() {
                if self.search_query.is_empty() {
                    String::new()
                } else {
                    "No matches".to_string()
                }
            } else {
                let idx = self.current_hit.map_or(0, |i| i + 1);
                format!("{}/{}", idx, self.search_hits.len())
            };

            Some(
                container(
                    row![
                        text("/").size(14),
                        text_input("Search...", &self.search_query)
                            .id(Id::new(SEARCH_INPUT_ID))
                            .on_input(Message::SearchInput)
                            .on_submit(Message::SearchSubmit)
                            .width(Length::Fill)
                            .size(14),
                        text(hit_info).size(12),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center),
                )
                .padding(6)
                .width(Length::Fill)
                .into(),
            )
        } else {
            None
        };

        // --- Permanent status bar (bottom) ---
        let status_bar = self.build_status_bar();

        // --- Overlay panel ---
        let overlay_panel: Option<Element<'_, Message>> = match &self.overlay {
            Overlay::None => None,
            Overlay::Shortcuts => Some(self.build_shortcuts_panel()),
            Overlay::About => Some(self.build_about_panel()),
        };

        // --- Sidebar + content area ---
        let main_body: Element<'_, Message> = if self.sidebar_open && !self.toc.is_empty() {
            let sidebar = sidebar::build_sidebar(self);

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
    fn build_status_bar(&self) -> Element<'_, Message> {
        // Left side: contextual messages
        let left_content: Element<'_, Message> =
            if !self.search_hits.is_empty() && !self.search_mode {
                let idx = self.current_hit.map_or(0, |i| i + 1);
                text(format!(
                    "[{}/{}] \"{}\"",
                    idx,
                    self.search_hits.len(),
                    self.search_query
                ))
                .size(12)
                .into()
            } else if let Some(idx) = self.focused_link {
                let link = &self.links[idx];
                text(format!(
                    "[{}/{}] {} → {}",
                    idx + 1,
                    self.links.len(),
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

        let theme_picker = pick_list(
            ThemeArg::ALL,
            Some(self.active_theme),
            Message::ThemeChanged,
        )
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
            .align_y(iced::Alignment::Center);

        container(
            row![container(left_content).width(Length::Fill), right_side,]
                .align_y(iced::Alignment::Center)
                .spacing(8),
        )
        .padding([4, 8])
        .width(Length::Fill)
        .style(container::rounded_box)
        .into()
    }

    /// Build the keyboard shortcuts overlay panel.
    fn build_shortcuts_panel(&self) -> Element<'_, Message> {
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
            .align_x(iced::Alignment::End),
        ]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        container(column![header, table_rows].spacing(8).padding(12))
            .width(Length::Fill)
            .max_width(500)
            .center_x(Length::Fill)
            .style(container::rounded_box)
            .into()
    }

    /// Build the about overlay panel.
    fn build_about_panel(&self) -> Element<'_, Message> {
        let version = env!("CARGO_PKG_VERSION");

        let header = row![
            text("About mdr").size(14),
            container(
                button(text("✕").size(12))
                    .on_press(Message::CloseOverlay)
                    .padding(2)
            )
            .width(Length::Fill)
            .align_x(iced::Alignment::End),
        ]
        .align_y(iced::Alignment::Center)
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

    // -----------------------------------------------------------------------
    // Subscription
    // -----------------------------------------------------------------------

    fn subscription(&self) -> Subscription<Message> {
        let search_mode = self.search_mode;
        let has_overlay = self.overlay != Overlay::None;

        let sidebar_focused = self.sidebar_focused;

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

        let ticker =
            iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::Tick);

        let window_resize =
            iced::window::resize_events().map(|(_id, size)| Message::WindowResized(size));
        Subscription::batch([keys, mouse_events, window_resize, ticker])
    }

    // -----------------------------------------------------------------------
    // Navigation history helpers
    // -----------------------------------------------------------------------

    /// Push a new navigation entry, truncating any forward history.
    fn push_nav(&mut self, file_path: PathBuf, scroll_y: f32) {
        self.nav_history[self.nav_index].scroll_y = self.current_scroll_y;
        self.nav_history.truncate(self.nav_index + 1);
        self.nav_history.push(NavEntry {
            file_path,
            scroll_y,
        });
        self.nav_index = self.nav_history.len() - 1;
    }

    /// Restore the view to the entry at `nav_index`.
    fn restore_nav_entry(&mut self) -> Task<Message> {
        let entry = self.nav_history[self.nav_index].clone();
        let image_task = if entry.file_path != self.file_path {
            images::load_file(self, &entry.file_path)
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

    // -----------------------------------------------------------------------
    // -----------------------------------------------------------------------

    fn handle_link(&mut self, url: String) -> Task<Message> {
        if url.starts_with("http://") || url.starts_with("https://") {
            let _ = open::that(&url);
            return Task::none();
        }

        if let Some(anchor) = url.strip_prefix('#') {
            return self.navigate_to_anchor(anchor);
        }

        // Local file link
        let raw = url.strip_prefix("file://").unwrap_or(&url);
        let target = if Path::new(raw).is_absolute() {
            PathBuf::from(raw)
        } else {
            let base = self.file_path.parent().unwrap_or(Path::new("."));
            base.join(raw)
        };
        if target.exists() && target.is_file() {
            self.push_nav(target.clone(), 0.0);
            let image_task = images::load_file(self, &target);
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
    fn navigate_to_anchor(&mut self, anchor: &str) -> Task<Message> {
        if let Some(target_y) = self.compute_anchor_y(anchor) {
            self.push_nav(self.file_path.clone(), target_y);
            let offset = RelativeOffset {
                x: 0.0,
                y: target_y,
            };
            operation::snap_to(Id::new(SCROLLABLE_ID), offset)
        } else {
            Task::none()
        }
    }

    fn compute_anchor_y(&self, anchor: &str) -> Option<f32> {
        use std::collections::HashMap;

        let total_lines = self.raw_markdown.lines().count() as f32;
        if total_lines <= 0.0 {
            return None;
        }

        let target_anchor = anchor.to_lowercase();

        // Pass 1: exact slug match using GitHub-style slug generation
        let mut seen: HashMap<String, u32> = HashMap::new();

        for (i, line) in self.raw_markdown.lines().enumerate() {
            if let Some(heading_text) = md_helpers::extract_atx_heading(line) {
                let slug = md_helpers::github_slug(heading_text, &mut seen);
                let slug_bare = slug.strip_prefix('#').unwrap_or(&slug);
                if slug_bare == target_anchor {
                    let fraction = (i as f32) / total_lines;
                    return Some(nav::content_fraction_to_scroll_y(self, fraction));
                }
            }
        }

        // Pass 2: relaxed match
        let anchor_normalized = md_helpers::normalize_for_match(&target_anchor);
        if anchor_normalized.is_empty() {
            return None;
        }

        for (i, line) in self.raw_markdown.lines().enumerate() {
            if let Some(heading_text) = md_helpers::extract_atx_heading(line) {
                let heading_normalized =
                    md_helpers::normalize_for_match(&heading_text.replace('`', "").to_lowercase());
                if heading_normalized == anchor_normalized {
                    let fraction = (i as f32) / total_lines;
                    return Some(nav::content_fraction_to_scroll_y(self, fraction));
                }
            }
        }

        None
    }
    fn scroll_to_link(&self, idx: usize) -> Task<Message> {
        let total_lines = self.raw_markdown.lines().count() as f32;
        if total_lines <= 0.0 {
            return Task::none();
        }
        let line = self.links[idx].line as f32;
        let fraction = line / total_lines;
        let y = nav::content_fraction_to_scroll_y(self, fraction);
        let offset = RelativeOffset { x: 0.0, y };
        operation::snap_to(Id::new(SCROLLABLE_ID), offset)
    }
}
