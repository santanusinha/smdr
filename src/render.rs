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

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use iced::border;
use iced::event;
use iced::keyboard;
use iced::mouse;
use iced::widget::Id;
use iced::widget::operation::{self, AbsoluteOffset, RelativeOffset};
use iced::widget::{
    button, column, container, markdown, mouse_area, pick_list, rich_text, row, rule, scrollable,
    text, text_input,
};
use iced::{
    Background, Color, Element, Event, Length, Pixels, Renderer, Subscription, Task, Theme,
};

use mdr::watcher;

use crate::ThemeArg;

/// Configuration passed to [`launch`].
pub struct ViewerConfig {
    pub theme: ThemeArg,
    pub watch: bool,
    /// Reserved for future use (e.g. fetching remote images).
    #[allow(dead_code)]
    pub network_enabled: bool,
}

/// Pixels scrolled per j/k keypress.
const LINE_SCROLL: f32 = 40.0;

/// Scrollable widget ID for programmatic scrolling.
const SCROLLABLE_ID: &str = "mdr-content-scroll";

/// Text input widget ID for search bar focus.
const SEARCH_INPUT_ID: &str = "mdr-search-input";

/// Default sidebar width in pixels.
/// Default sidebar ratio (fraction of window width).
const DEFAULT_SIDEBAR_RATIO: f32 = 0.25;

/// Minimum sidebar ratio.
const MIN_SIDEBAR_RATIO: f32 = 0.15;

/// Maximum sidebar ratio.
const MAX_SIDEBAR_RATIO: f32 = 0.40;

/// Initial window width used before the first resize event.
const INITIAL_WINDOW_WIDTH: f32 = 960.0;

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
}

impl AppInit {
    fn build(self) -> (MdrApp, Task<Message>) {
        let links = extract_links(&self.markdown_src);
        let toc = extract_toc(&self.markdown_src);
        let content = markdown::Content::parse(&self.markdown_src);
        let app = MdrApp {
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
            sidebar_ratio: DEFAULT_SIDEBAR_RATIO,
            sidebar_dragging: false,
            window_width: INITIAL_WINDOW_WIDTH,
        };
        (app, Task::none())
    }
}

// ---------------------------------------------------------------------------
// Navigation history entry
// ---------------------------------------------------------------------------

/// A single entry in the browser-like navigation history.
///
/// Each entry records the file being viewed and the relative scroll position
/// (0.0 = top, 1.0 = bottom) at the time of navigation.
#[derive(Debug, Clone)]
struct NavEntry {
    file_path: PathBuf,
    scroll_y: f32,
}

// ---------------------------------------------------------------------------
// Document link (for Tab navigation)
// ---------------------------------------------------------------------------

/// A link found in the markdown document, used for keyboard-only navigation.
#[derive(Debug, Clone)]
struct DocumentLink {
    /// Source line number (0-based) where the link appears.
    line: usize,
    /// The link destination URL/path.
    url: String,
    /// Display text of the link.
    text: String,
}

// ---------------------------------------------------------------------------
// Table of contents entry
// ---------------------------------------------------------------------------

/// A heading extracted from the document for sidebar navigation.
#[derive(Debug, Clone)]
struct TocEntry {
    /// Heading level (1-6).
    level: u8,
    /// Display text of the heading.
    text: String,
    /// Line number (0-based) in the source.
    line: usize,
}

// ---------------------------------------------------------------------------
// Overlay state
// ---------------------------------------------------------------------------

/// Which overlay panel (if any) is currently displayed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Overlay {
    None,
    Shortcuts,
    About,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Message {
    LinkClicked(markdown::Uri),
    ScrollBy(f32),
    HistoryBack,
    HistoryForward,
    FocusNextLink,
    FocusPrevLink,
    ActivateLink,
    SearchOpen,
    SearchClose,
    SearchInput(String),
    SearchSubmit,
    SearchNext,
    SearchPrev,
    ThemeChanged(ThemeArg),
    CycleTheme,
    ShowShortcuts,
    ShowAbout,
    CloseOverlay,
    ToggleSidebar,
    SidebarDragStart,
    SidebarDragMove(f32),
    SidebarDragEnd,
    NavigateToHeading(usize),
    Tick,
    Scrolled(scrollable::Viewport),
    WindowResized(iced::Size),
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct MdrApp {
    raw_markdown: String,
    content: markdown::Content,
    file_path: PathBuf,
    watcher_rx: Option<Receiver<()>>,
    active_theme: ThemeArg,
    title: String,
    /// Browser-like navigation history.
    nav_history: Vec<NavEntry>,
    /// Current position within `nav_history`.
    nav_index: usize,
    /// Live scroll position (relative y offset 0.0..=1.0), updated on every scroll event.
    current_scroll_y: f32,
    /// All links in the current document (for Tab navigation).
    links: Vec<DocumentLink>,
    /// Currently focused link index (Tab stop), or `None` if no link is focused.
    focused_link: Option<usize>,
    search_mode: bool,
    search_query: String,
    search_hits: Vec<usize>,
    current_hit: Option<usize>,
    overlay: Overlay,
    /// Table of contents (headings) for sidebar navigation.
    toc: Vec<TocEntry>,
    /// Whether the sidebar is visible.
    sidebar_open: bool,
    /// Current sidebar width as a ratio of window width (0.15..=0.40).
    sidebar_ratio: f32,
    /// Whether the user is actively dragging the sidebar resize handle.
    sidebar_dragging: bool,
    /// Current window width in pixels (updated on resize events).
    window_width: f32,
}

impl MdrApp {
    // -----------------------------------------------------------------------
    // Update
    // -----------------------------------------------------------------------

    fn update(&mut self, message: Message) -> Task<Message> {
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
                    self.scroll_to_current_hit()
                } else {
                    Task::none()
                }
            }
            Message::SearchOpen => {
                self.focused_link = None;
                self.search_mode = true;
                operation::focus(Id::new(SEARCH_INPUT_ID))
            }
            Message::SearchClose => {
                self.search_mode = false;
                self.search_query.clear();
                self.search_hits.clear();
                self.current_hit = None;
                Task::none()
            }
            Message::SearchInput(q) => {
                self.search_query = q;
                self.recompute_search_hits();
                self.scroll_to_current_hit()
            }
            Message::SearchSubmit => {
                self.recompute_search_hits();
                self.search_mode = false;
                self.scroll_to_current_hit()
            }
            Message::SearchNext => {
                if !self.search_hits.is_empty() {
                    let next = match self.current_hit {
                        Some(i) => (i + 1) % self.search_hits.len(),
                        None => 0,
                    };
                    self.current_hit = Some(next);
                }
                self.scroll_to_current_hit()
            }
            Message::SearchPrev => {
                if !self.search_hits.is_empty() {
                    let prev = match self.current_hit {
                        Some(0) => self.search_hits.len() - 1,
                        Some(i) => i - 1,
                        None => self.search_hits.len() - 1,
                    };
                    self.current_hit = Some(prev);
                }
                self.scroll_to_current_hit()
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
            Message::ToggleSidebar => {
                self.sidebar_open = !self.sidebar_open;
                Task::none()
            }
            Message::SidebarDragStart => {
                self.sidebar_dragging = true;
                Task::none()
            }
            Message::SidebarDragMove(x) => {
                if self.sidebar_dragging {
                    self.sidebar_ratio =
                        (x / self.window_width).clamp(MIN_SIDEBAR_RATIO, MAX_SIDEBAR_RATIO);
                }
                Task::none()
            }
            Message::SidebarDragEnd => {
                self.sidebar_dragging = false;
                Task::none()
            }
            Message::NavigateToHeading(idx) => {
                if let Some(entry) = self.toc.get(idx) {
                    let total_lines = self.raw_markdown.lines().count() as f32;
                    if total_lines > 0.0 {
                        let target_y = (entry.line as f32) / total_lines;
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
            Message::Tick => {
                self.poll_watcher();
                Task::none()
            }
            Message::Scrolled(viewport) => {
                self.current_scroll_y = viewport.relative_offset().y;
                Task::none()
            }
            Message::WindowResized(size) => {
                self.window_width = size.width;
                Task::none()
            }
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
        let viewer = MdrViewer;
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
            let sidebar = self.build_sidebar();

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

    /// Build the collapsible left sidebar showing document outline.
    fn build_sidebar(&self) -> Element<'_, Message> {
        let min_level = self.toc.iter().map(|e| e.level).min().unwrap_or(1);

        let mut items = column![].spacing(2).padding([8, 4]);

        for (idx, entry) in self.toc.iter().enumerate() {
            let indent = ((entry.level - min_level) as u16) * 12;
            let label = text(&entry.text).size(13);
            let btn = button(label)
                .on_press(Message::NavigateToHeading(idx))
                .padding([2, 4])
                .style(button::text);

            let left_pad = iced::Padding::ZERO.left((indent as f32) * 1.0);
            items = items.push(container(btn).padding(left_pad));
        }

        let header = row![
            text("Outline").size(13),
            container(
                button(text("✕").size(11))
                    .on_press(Message::ToggleSidebar)
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
            column![header, scrollable(items).height(Length::Fill)]
                .height(Length::Fill)
                .width(Length::Fixed(self.window_width * self.sidebar_ratio)),
        )
        .height(Length::Fill)
        .style(container::rounded_box)
        .into()
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
            .on_press(Message::ToggleSidebar)
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
        let shortcuts = [
            ("j / ↓", "Scroll down"),
            ("k / ↑", "Scroll up"),
            ("Ctrl-D / PgDn", "Page down"),
            ("Ctrl-U / PgUp", "Page up"),
            ("h / ←", "Navigate back"),
            ("l / →", "Navigate forward"),
            ("Tab", "Next link"),
            ("Shift-Tab", "Previous link"),
            ("Enter", "Activate link / next hit"),
            ("/ or ?", "Open search"),
            ("Ctrl-F", "Open search"),
            ("n", "Next search hit"),
            ("p", "Previous search hit"),
            ("Ctrl-B", "Toggle sidebar"),
            ("Ctrl-T", "Cycle theme"),
            ("Esc", "Close search / overlay"),
        ];

        let mut shortcut_rows = column![].spacing(4).padding(8);
        for (key, desc) in shortcuts {
            shortcut_rows = shortcut_rows.push(
                row![
                    container(text(key).size(12)).width(Length::Fixed(140.0)),
                    text(desc).size(12),
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

        container(column![header, shortcut_rows].spacing(8).padding(12))
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

        let keys = keyboard::listen()
            .with((search_mode, has_overlay))
            .filter_map(|((search_mode, has_overlay), event)| {
                let keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    text: _,
                    modified_key: _,
                    physical_key: _,
                    location: _,
                    repeat: _,
                } = event
                else {
                    return None;
                };

                // Escape always closes overlay or search
                if matches!(&key, keyboard::Key::Named(keyboard::key::Named::Escape)) {
                    if has_overlay {
                        return Some(Message::CloseOverlay);
                    }
                    if search_mode {
                        return Some(Message::SearchClose);
                    }
                    return None;
                }

                if search_mode {
                    return None;
                }

                if has_overlay {
                    return None;
                }

                match &key {
                    keyboard::Key::Named(named) => match named {
                        keyboard::key::Named::ArrowDown => Some(Message::ScrollBy(LINE_SCROLL)),
                        keyboard::key::Named::ArrowUp => Some(Message::ScrollBy(-LINE_SCROLL)),
                        keyboard::key::Named::ArrowLeft => Some(Message::HistoryBack),
                        keyboard::key::Named::ArrowRight => Some(Message::HistoryForward),
                        keyboard::key::Named::PageDown => Some(Message::ScrollBy(360.0)),
                        keyboard::key::Named::PageUp => Some(Message::ScrollBy(-360.0)),
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
                    keyboard::Key::Character(c) => {
                        let s = c.as_str();
                        if modifiers.control() {
                            match s {
                                "d" => Some(Message::ScrollBy(360.0)),
                                "u" => Some(Message::ScrollBy(-360.0)),
                                "f" => Some(Message::SearchOpen),
                                "b" => Some(Message::ToggleSidebar),
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
                                "/" | "?" => Some(Message::SearchOpen),
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
        if entry.file_path != self.file_path {
            self.load_file(&entry.file_path);
        }
        let offset = RelativeOffset {
            x: 0.0,
            y: entry.scroll_y,
        };
        operation::snap_to(Id::new(SCROLLABLE_ID), offset)
    }

    // -----------------------------------------------------------------------
    // Link helpers
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
            self.load_file(&target);
            operation::snap_to(Id::new(SCROLLABLE_ID), RelativeOffset { x: 0.0, y: 0.0 })
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

    /// Compute the relative scroll y-position for a given anchor.
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
            if let Some(heading_text) = extract_atx_heading(line) {
                let slug = github_slug(heading_text, &mut seen);
                let slug_bare = slug.strip_prefix('#').unwrap_or(&slug);
                if slug_bare == target_anchor {
                    return Some((i as f32) / total_lines);
                }
            }
        }

        // Pass 2: relaxed match
        let anchor_normalized = normalize_for_match(&target_anchor);
        if anchor_normalized.is_empty() {
            return None;
        }

        for (i, line) in self.raw_markdown.lines().enumerate() {
            if let Some(heading_text) = extract_atx_heading(line) {
                let heading_normalized =
                    normalize_for_match(&heading_text.replace('`', "").to_lowercase());
                if heading_normalized == anchor_normalized {
                    return Some((i as f32) / total_lines);
                }
            }
        }

        None
    }

    /// Scroll the view to the link at the given index.
    fn scroll_to_link(&self, idx: usize) -> Task<Message> {
        let total_lines = self.raw_markdown.lines().count() as f32;
        if total_lines <= 0.0 {
            return Task::none();
        }
        let line = self.links[idx].line as f32;
        let ratio = line / total_lines;
        let offset = RelativeOffset { x: 0.0, y: ratio };
        operation::snap_to(Id::new(SCROLLABLE_ID), offset)
    }

    // -----------------------------------------------------------------------
    // File loading
    // -----------------------------------------------------------------------

    fn load_file(&mut self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(src) => {
                self.links = extract_links(&src);
                self.toc = extract_toc(&src);
                self.focused_link = None;
                self.raw_markdown = src;
                self.content = markdown::Content::parse(&self.raw_markdown);
                self.file_path = path.to_path_buf();
                self.title = format!(
                    "mdr — {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                self.search_hits.clear();
                self.current_hit = None;
            }
            Err(e) => eprintln!("Warning: could not read '{}': {e}", path.display()),
        }
    }

    fn poll_watcher(&mut self) {
        let Some(ref rx) = self.watcher_rx else {
            return;
        };
        if rx.try_recv().is_ok() {
            while rx.try_recv().is_ok() {}
            match std::fs::read_to_string(&self.file_path) {
                Ok(new_content) => {
                    self.links = extract_links(&new_content);
                    self.toc = extract_toc(&new_content);
                    self.focused_link = None;
                    self.raw_markdown = new_content;
                    self.content = markdown::Content::parse(&self.raw_markdown);
                }
                Err(e) => eprintln!("Warning: could not reload file: {e}"),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Search helpers
    // -----------------------------------------------------------------------

    fn recompute_search_hits(&mut self) {
        self.search_hits.clear();
        self.current_hit = None;

        if self.search_query.is_empty() {
            return;
        }

        let query_lower = self.search_query.to_lowercase();

        for (i, line) in self.raw_markdown.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                self.search_hits.push(i);
            }
        }

        if !self.search_hits.is_empty() {
            self.current_hit = Some(0);
        }
    }

    fn scroll_to_current_hit(&self) -> Task<Message> {
        let Some(hit_idx) = self.current_hit else {
            return Task::none();
        };
        let Some(&line_num) = self.search_hits.get(hit_idx) else {
            return Task::none();
        };

        let total_lines = self.raw_markdown.lines().count() as f32;
        if total_lines <= 0.0 {
            return Task::none();
        }

        let ratio = (line_num as f32) / total_lines;
        let offset = RelativeOffset { x: 0.0, y: ratio };
        operation::snap_to(Id::new(SCROLLABLE_ID), offset)
    }
}

// ---------------------------------------------------------------------------
// Custom Viewer for theme-adaptive code block styling
// ---------------------------------------------------------------------------

/// Custom markdown viewer that overrides code block rendering to use a
/// theme-adaptive background instead of the default hardcoded dark style.
struct MdrViewer;

impl<'a> markdown::Viewer<'a, markdown::Uri, Theme, Renderer> for MdrViewer {
    fn on_link_click(url: markdown::Uri) -> markdown::Uri {
        url
    }

    fn code_block(
        &self,
        settings: markdown::Settings,
        _language: Option<&'a str>,
        _code: &'a str,
        lines: &'a [markdown::Text],
    ) -> Element<'a, markdown::Uri, Theme, Renderer> {
        container(
            scrollable(
                container(column(lines.iter().map(|line| {
                    rich_text(line.spans(settings.style))
                        .on_link_click(Self::on_link_click)
                        .font(settings.style.code_block_font)
                        .size(settings.code_size)
                        .into()
                })))
                .padding(settings.code_size),
            )
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::default()
                    .width(settings.code_size / 2)
                    .scroller_width(settings.code_size / 2),
            )),
        )
        .width(Length::Fill)
        .padding(settings.code_size / 4)
        .style(code_block_container_style)
        .into()
    }
}

/// Theme-adaptive container style for fenced code blocks.
///
/// On light themes uses a warm gray background with high-contrast dark text;
/// on dark themes uses a slightly elevated surface with light text.
fn code_block_container_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    if palette.is_dark {
        // Dark themes: slightly lighter than page background, light text
        container::Style {
            background: Some(Background::Color(Color::from_rgb(0.14, 0.15, 0.18))),
            text_color: Some(Color::from_rgb(0.87, 0.89, 0.93)),
            border: border::rounded(6),
            ..container::Style::default()
        }
    } else {
        // Light themes: distinct cool-gray background, dark text for readability
        container::Style {
            background: Some(Background::Color(Color::from_rgb(0.95, 0.96, 0.97))),
            text_color: Some(Color::from_rgb(0.13, 0.14, 0.16)),
            border: border::rounded(6),
            ..container::Style::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Link extraction (using pulldown-cmark)
// ---------------------------------------------------------------------------
/// Extract all links from the markdown source with their line positions.
fn extract_links(source: &str) -> Vec<DocumentLink> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    let mut links = Vec::new();
    let parser = Parser::new_ext(source, Options::all());

    // Track byte offset → line number mapping
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter_map(|(i, b)| if b == b'\n' { Some(i + 1) } else { None }),
        )
        .collect();

    let byte_offset_to_line = |offset: usize| -> usize {
        line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1)
    };

    let mut in_link: Option<(String, usize)> = None; // (url, line)
    let mut link_text = String::new();

    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                let line = byte_offset_to_line(range.start);
                in_link = Some((dest_url.to_string(), line));
                link_text.clear();
            }
            Event::End(TagEnd::Link) => {
                if let Some((url, line)) = in_link.take() {
                    let display = if link_text.is_empty() {
                        url.clone()
                    } else {
                        link_text.clone()
                    };
                    links.push(DocumentLink {
                        line,
                        url,
                        text: display,
                    });
                }
                link_text.clear();
            }
            Event::Text(t) if in_link.is_some() => {
                link_text.push_str(&t);
            }
            Event::Code(c) if in_link.is_some() => {
                link_text.push('`');
                link_text.push_str(&c);
                link_text.push('`');
            }
            _ => {}
        }
    }

    links
}

// ---------------------------------------------------------------------------
// Table of contents extraction
// ---------------------------------------------------------------------------

/// Extract all headings from the markdown source for the sidebar outline.
fn extract_toc(source: &str) -> Vec<TocEntry> {
    let mut entries = Vec::new();

    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim_start_matches('#');
        let hash_count = line.len() - trimmed.len();
        if hash_count == 0 || hash_count > 6 {
            continue;
        }
        // Must have a space after the hashes
        if !trimmed.starts_with(' ') {
            continue;
        }
        let heading_text = trimmed[1..].trim_end().trim_end_matches('#').trim();
        if heading_text.is_empty() {
            continue;
        }
        // Strip backticks for display
        let display_text = heading_text.replace('`', "");
        entries.push(TocEntry {
            level: hash_count as u8,
            text: display_text,
            line: line_num,
        });
    }

    entries
}

// ---------------------------------------------------------------------------
// Heading / slug helpers
// ---------------------------------------------------------------------------

/// Generate a GitHub-style anchor slug from heading text.
///
/// Rules (matching GitHub.com behaviour):
/// 1. Strip inline-code backticks (keep their content).
/// 2. Lowercase.
/// 3. Remove everything that is not ASCII alphanumeric, space, hyphen,
///    or underscore.
/// 4. Replace spaces with hyphens (consecutive hyphens are preserved).
/// 5. Deduplicate: second occurrence gets suffix `-1`, third gets `-2`, etc.
fn github_slug(text: &str, seen: &mut std::collections::HashMap<String, u32>) -> String {
    let base: String = text
        .replace('`', "")
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect();

    let count = seen.entry(base.clone()).or_insert(0);
    let slug = if *count == 0 {
        base.clone()
    } else {
        format!("{base}-{}", *count - 1)
    };
    *count += 1;
    format!("#{slug}")
}

/// Extract the heading text from an ATX heading line, or `None` if the line
/// is not a valid ATX heading.
fn extract_atx_heading(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches('#');
    let hashes = line.len() - trimmed.len();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = trimmed.strip_prefix(' ')?;
    let text = rest.trim_end().trim_end_matches('#').trim_end_matches(' ');
    Some(if text.is_empty() { rest.trim() } else { text })
}

/// Strips everything except ASCII alphanumeric characters for fuzzy anchor comparison.
fn normalize_for_match(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn slug_simple_heading() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug("Severity Legend", &mut seen),
            "#severity-legend"
        );
    }

    #[test]
    fn slug_apostrophe_removed() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug("What's Done Well", &mut seen),
            "#whats-done-well"
        );
    }

    #[test]
    fn slug_preserves_consecutive_hyphens() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug("Part I — Rust Idioms & Anti-patterns", &mut seen),
            "#part-i--rust-idioms--anti-patterns"
        );
    }

    #[test]
    fn slug_strips_backticks_keeps_content() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug("`run_async` spawns a runtime", &mut seen),
            "#run_async-spawns-a-runtime"
        );
    }

    #[test]
    fn slug_preserves_underscores() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug("`let _ =` silences errors", &mut seen),
            "#let-_--silences-errors"
        );
    }

    #[test]
    fn slug_emoji_and_special_chars_removed() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug(
                "I-1 \u{1f534} `HeapError::Success` — success state inside an error enum",
                &mut seen
            ),
            "#i-1--heaperrorsuccess--success-state-inside-an-error-enum"
        );
    }

    #[test]
    fn slug_deduplication() {
        let mut seen = HashMap::new();
        assert_eq!(github_slug("Heading", &mut seen), "#heading");
        assert_eq!(github_slug("Heading", &mut seen), "#heading-0");
        assert_eq!(github_slug("Heading", &mut seen), "#heading-1");
    }

    #[test]
    fn extract_atx_heading_basic() {
        assert_eq!(extract_atx_heading("## Hello World"), Some("Hello World"));
        assert_eq!(extract_atx_heading("### Foo"), Some("Foo"));
        assert_eq!(extract_atx_heading("Not a heading"), None);
        assert_eq!(extract_atx_heading("####### Too many"), None);
    }

    #[test]
    fn extract_atx_heading_trailing_hashes() {
        assert_eq!(extract_atx_heading("## Title ##"), Some("Title"));
    }

    #[test]
    fn normalize_strips_non_alphanumeric() {
        assert_eq!(normalize_for_match("severity-legend"), "severitylegend");
        assert_eq!(
            normalize_for_match("i-1--heaperror-success--success-state-inside-an-error-enum"),
            "i1heaperrorsuccesssuccessstateinsideanerrorenum"
        );
    }

    #[test]
    fn anchor_match_relaxed_handles_nonstandard_slug() {
        let heading =
            "I-1 \u{1f534} `HeapError::Success` \u{2014} success state inside an error enum";
        let anchor = "i-1--heaperror-success--success-state-inside-an-error-enum";

        let anchor_normalized = normalize_for_match(&anchor.to_lowercase());
        let heading_normalized = normalize_for_match(&heading.replace('`', "").to_lowercase());
        assert_eq!(anchor_normalized, heading_normalized);
    }

    #[test]
    fn extract_links_finds_inline_links() {
        let md = "Hello [world](https://example.com) and [foo](./bar.md)";
        let links = extract_links(md);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].text, "world");
        assert_eq!(links[0].url, "https://example.com");
        assert_eq!(links[1].text, "foo");
        assert_eq!(links[1].url, "./bar.md");
    }

    #[test]
    fn extract_links_finds_anchor_links() {
        let md = "- [Section One](#section-one)\n- [Section Two](#section-two)\n";
        let links = extract_links(md);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].url, "#section-one");
        assert_eq!(links[1].url, "#section-two");
        assert_eq!(links[0].line, 0);
        assert_eq!(links[1].line, 1);
    }

    #[test]
    fn extract_links_with_code_in_text() {
        let md = "See [`Config`](./config.md) for details.";
        let links = extract_links(md);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "`Config`");
        assert_eq!(links[0].url, "./config.md");
    }

    #[test]
    fn theme_arg_to_theme_conversion() {
        assert_eq!(ThemeArg::Light.to_theme(), Theme::Light);
        assert_eq!(ThemeArg::Dark.to_theme(), Theme::Dark);
        assert_eq!(ThemeArg::Dracula.to_theme(), Theme::Dracula);
        assert_eq!(ThemeArg::Nord.to_theme(), Theme::Nord);
        assert_eq!(ThemeArg::TokyoNight.to_theme(), Theme::TokyoNight);
        assert_eq!(ThemeArg::CatppuccinMocha.to_theme(), Theme::CatppuccinMocha);
        assert_eq!(ThemeArg::Ferra.to_theme(), Theme::Ferra);
    }

    #[test]
    fn theme_arg_all_contains_all_variants() {
        assert_eq!(ThemeArg::ALL.len(), 23);
    }

    #[test]
    fn theme_arg_display() {
        assert_eq!(ThemeArg::System.to_string(), "System");
        assert_eq!(ThemeArg::SolarizedLight.to_string(), "Solarized Light");
        assert_eq!(
            ThemeArg::CatppuccinFrappe.to_string(),
            "Catppuccin Frapp\u{e9}"
        );
    }

    #[test]
    fn extract_toc_basic() {
        let md = "# Title\n\nSome text\n\n## Section One\n\nContent\n\n### Subsection\n\n## Section Two\n";
        let toc = extract_toc(md);
        assert_eq!(toc.len(), 4);
        assert_eq!(toc[0].level, 1);
        assert_eq!(toc[0].text, "Title");
        assert_eq!(toc[0].line, 0);
        assert_eq!(toc[1].level, 2);
        assert_eq!(toc[1].text, "Section One");
        assert_eq!(toc[1].line, 4);
        assert_eq!(toc[2].level, 3);
        assert_eq!(toc[2].text, "Subsection");
        assert_eq!(toc[2].line, 8);
        assert_eq!(toc[3].level, 2);
        assert_eq!(toc[3].text, "Section Two");
        assert_eq!(toc[3].line, 10);
    }

    #[test]
    fn extract_toc_strips_backticks() {
        let md = "## `Config` options\n";
        let toc = extract_toc(md);
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].text, "Config options");
    }

    #[test]
    fn extract_toc_skips_non_headings() {
        let md = "Not a heading\n#nospace\n####### Too many\n## Valid\n";
        let toc = extract_toc(md);
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].text, "Valid");
        assert_eq!(toc[0].line, 3);
    }

    #[test]
    fn extract_toc_trailing_hashes() {
        let md = "## Title ##\n";
        let toc = extract_toc(md);
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].text, "Title");
    }
}
