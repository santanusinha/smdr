//! Application lifecycle — launch, initialization, and trait wiring.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use iced::Task;
use iced::widget::markdown;
#[cfg(target_os = "linux")]
use iced::window::settings::PlatformSpecific;
use iced::{Size, window};

use smdr::markdown::{self as md_helpers};
use smdr::persist;
use smdr::theme::ThemeArg;
use smdr::watcher;

#[cfg(target_os = "linux")]
use super::desktop;
use super::images;
use super::state::{
    DEFAULT_SIDEBAR_RATIO, INITIAL_WINDOW_WIDTH, MdrApp, Message, NavEntry, Overlay, ViewerConfig,
};
use super::update;
use super::view;

/// PNG icon bytes embedded at compile time.
const ICON_PNG: &[u8] = include_bytes!("../../assets/icon_256.png");

/// Builds the window icon from the embedded PNG bytes.
/// Returns `None` and prints a warning if the icon cannot be decoded.
fn build_window_icon() -> Option<window::Icon> {
    match window::icon::from_file_data(ICON_PNG, None) {
        Ok(icon) => Some(icon),
        Err(e) => {
            eprintln!("Warning: could not load window icon: {e}");
            None
        }
    }
}

/// Launches the viewer window and blocks until it is closed.
///
/// `file_paths` must be non-empty. The first path becomes the primary
/// document; any remaining paths open as additional tabs once the window is up.
///
/// # Errors
/// Returns an error if the window cannot be created.
pub fn launch(
    file_paths: &[std::path::PathBuf],
    config: &ViewerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Install icon + .desktop file into ~/.local/share on first run (Linux only).
    #[cfg(target_os = "linux")]
    desktop::ensure_xdg_assets();

    let file_path = file_paths
        .first()
        .ok_or("launch requires at least one file path")?;

    // Remaining files open as additional tabs after the primary loads.
    let extra_tabs: Vec<PathBuf> = file_paths.iter().skip(1).cloned().collect();

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
        "smdr — {}",
        file_path.file_name().unwrap_or_default().to_string_lossy()
    );

    let theme_arg = config.theme;
    let file_path = file_path.to_path_buf();

    // Apply persisted theme unless the user explicitly chose one on the CLI.
    let theme_arg = if config.theme_explicit {
        theme_arg
    } else {
        persist::load().map(|s| s.theme).unwrap_or(theme_arg)
    };

    let app_state = AppInit {
        markdown_src,
        file_path,
        watcher_rx,
        theme: theme_arg,
        title,
        network_enabled: config.network_enabled,
        extra_tabs,
        review_mode: config.review_mode,
        review_out: config.review_out.clone(),
        review_format: config.review_format,
        ipc_enabled: config.ipc_enabled,
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
        update::handle_message,
        view::build_ui,
    )
    .subscription(view::build_subscription)
    .theme(|app: &MdrApp| app.active_theme.to_theme())
    .title(|app: &MdrApp| app.title.clone())
    .window(window::Settings {
        size: Size::new(960.0, 720.0),
        icon: build_window_icon(),
        #[cfg(target_os = "linux")]
        platform_specific: PlatformSpecific {
            application_id: String::from("smdr"),
            ..PlatformSpecific::default()
        },
        ..window::Settings::default()
    })
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
    // Install icon + .desktop file into ~/.local/share on first run (Linux only).
    #[cfg(target_os = "linux")]
    desktop::ensure_xdg_assets();

    let title = String::from("smdr — stdin");
    let file_path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("stdin");

    let app_state = AppInit {
        markdown_src: content,
        file_path,
        watcher_rx: None,
        // Apply persisted theme unless the user explicitly chose one on the CLI.
        theme: if config.theme_explicit {
            config.theme
        } else {
            persist::load().map(|s| s.theme).unwrap_or(config.theme)
        },
        title,
        network_enabled: config.network_enabled,
        extra_tabs: Vec::new(),
        review_mode: config.review_mode,
        review_out: config.review_out.clone(),
        review_format: config.review_format,
        ipc_enabled: config.ipc_enabled,
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
        update::handle_message,
        view::build_ui,
    )
    .subscription(view::build_subscription)
    .theme(|app: &MdrApp| app.active_theme.to_theme())
    .title(|app: &MdrApp| app.title.clone())
    .window(window::Settings {
        size: Size::new(960.0, 720.0),
        icon: build_window_icon(),
        #[cfg(target_os = "linux")]
        platform_specific: PlatformSpecific {
            application_id: String::from("smdr"),
            ..PlatformSpecific::default()
        },
        ..window::Settings::default()
    })
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
    /// Additional files to open as tabs once the primary document is loaded.
    extra_tabs: Vec<PathBuf>,
    /// `true` when launched with `--review`; enables the submit affordance.
    review_mode: bool,
    /// Where a completed review turn is written; `None` means stdout.
    review_out: Option<PathBuf>,
    /// Output serializer for a submitted review turn (mirrors `--format`).
    review_format: smdr::annotate::OutputFormat,
    /// Whether the single-instance IPC server runs for this app.
    ipc_enabled: bool,
}

impl AppInit {
    fn build(self) -> (MdrApp, Task<Message>) {
        let links = md_helpers::extract_links(&self.markdown_src);
        let toc = md_helpers::extract_toc(&self.markdown_src);
        let content = markdown::Content::parse(&self.markdown_src);
        let source_content = iced::widget::text_editor::Content::with_text(&self.markdown_src);
        let base_dir = self
            .file_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let network_enabled = self.network_enabled;
        let extra_tabs = self.extra_tabs;

        // In review mode, restore any auto-saved draft for this file (computed
        // before `self.file_path` is moved into the struct below).
        let restored_comments = if self.review_mode {
            smdr::draft::load(&self.file_path)
        } else {
            Vec::new()
        };

        let mut app = MdrApp {
            raw_markdown: self.markdown_src,
            line_count: 0, // will be set via the helper below
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
            search_query_lower: String::new(),
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
            mermaid_pending: HashSet::new(),
            network_enabled,
            base_dir: base_dir.clone(),
            pending_key: None,
            last_scroll_y: 0.0,
            content_height: 0.0,
            viewport_height: 0.0,
            tabs: Vec::new(),
            active_tab: 0,
            comment_mode: self.review_mode,
            source_content,
            comment_target_line: None,
            comment_draft: String::new(),
            // In review mode, restore any auto-saved draft for this file so a
            // reviewer who closed the window without submitting picks up where
            // they left off. Normal viewing starts with no comments.
            comments: restored_comments,
            review_mode: self.review_mode,
            review_out: self.review_out,
            review_format: self.review_format,
            ipc_enabled: self.ipc_enabled,
        };
        app.line_count = app.raw_markdown.lines().count();

        // Mermaid diagrams are rendered asynchronously by spawn_image_loads.

        // Mark image URLs as pending and spawn loading tasks.
        let mut task = images::spawn_image_loads(&mut app);

        // Open any additional files (from a multi-file command line) as tabs.
        // Each emits an `OpenInNewTab` message, processed in order after boot,
        // so files appear as tabs left-to-right in the order given.  Opening a
        // tab makes it active, so a final `SwitchTab(0)` returns focus to the
        // first (primary) document.
        if !extra_tabs.is_empty() {
            let mut tab_tasks: Vec<Task<Message>> = extra_tabs
                .into_iter()
                .map(|path| Task::done(Message::OpenInNewTab(path)))
                .collect();
            tab_tasks.push(Task::done(Message::SwitchTab(0)));
            task = task.chain(Task::batch(tab_tasks));
        }

        (app, task)
    }
}
