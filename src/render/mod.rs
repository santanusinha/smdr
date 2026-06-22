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

use iced::widget::Id;
use iced::widget::markdown;
use iced::widget::operation::{self, AbsoluteOffset, RelativeOffset};
use iced::{Element, Subscription, Task};

use mdr::markdown::{self as md_helpers};
use mdr::theme::ThemeArg;
use mdr::watcher;

mod images;
mod navigation;
mod search;
mod sidebar;
mod state;
mod styles;
mod view;
mod widget;

pub use state::ViewerConfig;
use state::{
    DEFAULT_SIDEBAR_RATIO, INITIAL_WINDOW_WIDTH, MdrApp, Message, NavEntry, Overlay, SCROLLABLE_ID,
};

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
            Message::LinkClicked(url) => navigation::handle_link(self, url),
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
                navigation::restore_nav_entry(self)
            }
            Message::HistoryForward => {
                if self.nav_index + 1 >= self.nav_history.len() {
                    return Task::none();
                }
                self.focused_link = None;
                self.nav_history[self.nav_index].scroll_y = self.current_scroll_y;
                self.nav_index += 1;
                navigation::restore_nav_entry(self)
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
                navigation::scroll_to_link(self, next)
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
                navigation::scroll_to_link(self, prev)
            }
            Message::ActivateLink => {
                if self.focused_link.is_some() {
                    let idx = self.focused_link.unwrap();
                    let url = self.links[idx].url.clone();
                    self.focused_link = None;
                    navigation::handle_link(self, url)
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
            Message::NavigateToHeading(idx) => navigation::navigate_to_heading(self, idx),
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
    // View — delegated to render::view module
    // -----------------------------------------------------------------------
    fn view(&self) -> Element<'_, Message> {
        view::build_ui(self)
    }

    fn subscription(&self) -> Subscription<Message> {
        view::build_subscription(self)
    }
}
