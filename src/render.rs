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
//! - `/` or `?` to search, `n`/`p` to cycle through matches.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use iced::keyboard;
use iced::widget::Id;
use iced::widget::operation::{self, AbsoluteOffset, RelativeOffset};
use iced::widget::{column, container, markdown, row, scrollable, text, text_input};
use iced::{Element, Length, Subscription, Task, Theme};

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
    .theme(|app: &MdrApp| app.theme())
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
            search_mode: false,
            search_query: String::new(),
            search_hits: Vec::new(),
            current_hit: None,
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
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Message {
    LinkClicked(markdown::Uri),
    ScrollBy(f32),
    HistoryBack,
    HistoryForward,
    SearchOpen,
    SearchClose,
    SearchInput(String),
    SearchSubmit,
    SearchNext,
    SearchPrev,
    Tick,
    Scrolled(scrollable::Viewport),
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
    search_mode: bool,
    search_query: String,
    search_hits: Vec<usize>,
    current_hit: Option<usize>,
}

impl MdrApp {
    fn theme(&self) -> Theme {
        match self.active_theme {
            ThemeArg::System => Theme::Light,
            ThemeArg::Light => Theme::Light,
            ThemeArg::Dark => Theme::Dark,
            ThemeArg::TokyoNight => Theme::TokyoNight,
            ThemeArg::SolarizedDark => Theme::SolarizedDark,
        }
    }

    // -----------------------------------------------------------------------
    // Update
    // -----------------------------------------------------------------------

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LinkClicked(url) => self.handle_link(url),
            Message::ScrollBy(delta) => {
                operation::scroll_by(Id::new(SCROLLABLE_ID), AbsoluteOffset { x: 0.0, y: delta })
            }
            Message::HistoryBack => {
                if self.nav_index == 0 {
                    return Task::none();
                }
                // Save current scroll position in current entry before leaving.
                self.nav_history[self.nav_index].scroll_y = self.current_scroll_y;
                self.nav_index -= 1;
                self.restore_nav_entry()
            }
            Message::HistoryForward => {
                if self.nav_index + 1 >= self.nav_history.len() {
                    return Task::none();
                }
                // Save current scroll position in current entry before leaving.
                self.nav_history[self.nav_index].scroll_y = self.current_scroll_y;
                self.nav_index += 1;
                self.restore_nav_entry()
            }
            Message::SearchOpen => {
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
            Message::Tick => {
                self.poll_watcher();
                Task::none()
            }
            Message::Scrolled(viewport) => {
                self.current_scroll_y = viewport.relative_offset().y;
                Task::none()
            }
        }
    }

    // -----------------------------------------------------------------------
    // View
    // -----------------------------------------------------------------------
    fn view(&self) -> Element<'_, Message> {
        let settings = markdown::Settings::from(&self.theme());
        let md_view: Element<Message> =
            markdown::view(self.content.items(), settings).map(Message::LinkClicked);

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

        if self.search_mode {
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

            let search_bar = container(
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
            .width(Length::Fill);

            column![search_bar, content_area].into()
        } else {
            content_area.into()
        }
    }

    // -----------------------------------------------------------------------
    // Subscription
    // -----------------------------------------------------------------------

    fn subscription(&self) -> Subscription<Message> {
        let search_mode = self.search_mode;

        let keys = keyboard::listen()
            .with(search_mode)
            .filter_map(|(search_mode, event)| {
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

                if search_mode {
                    match &key {
                        keyboard::Key::Named(keyboard::key::Named::Escape) => {
                            Some(Message::SearchClose)
                        }
                        _ => None,
                    }
                } else {
                    match &key {
                        keyboard::Key::Named(named) => match named {
                            keyboard::key::Named::ArrowDown => Some(Message::ScrollBy(LINE_SCROLL)),
                            keyboard::key::Named::ArrowUp => Some(Message::ScrollBy(-LINE_SCROLL)),
                            keyboard::key::Named::ArrowLeft => Some(Message::HistoryBack),
                            keyboard::key::Named::ArrowRight => Some(Message::HistoryForward),
                            keyboard::key::Named::PageDown => Some(Message::ScrollBy(360.0)),
                            keyboard::key::Named::PageUp => Some(Message::ScrollBy(-360.0)),
                            keyboard::key::Named::Escape => Some(Message::SearchClose),
                            _ => None,
                        },
                        keyboard::Key::Character(c) => {
                            let s = c.as_str();
                            if modifiers.control() {
                                match s {
                                    "d" => Some(Message::ScrollBy(360.0)),
                                    "u" => Some(Message::ScrollBy(-360.0)),
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
                }
            });

        let ticker =
            iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::Tick);

        Subscription::batch([keys, ticker])
    }

    // -----------------------------------------------------------------------
    // Navigation history helpers
    // -----------------------------------------------------------------------

    /// Push a new navigation entry, truncating any forward history.
    ///
    /// Before pushing, saves the current scroll position into the current entry.
    fn push_nav(&mut self, file_path: PathBuf, scroll_y: f32) {
        // Update the current entry with our live scroll position.
        self.nav_history[self.nav_index].scroll_y = self.current_scroll_y;
        // Discard any forward history (browser semantics).
        self.nav_history.truncate(self.nav_index + 1);
        // Push the new destination.
        self.nav_history.push(NavEntry {
            file_path,
            scroll_y,
        });
        self.nav_index = self.nav_history.len() - 1;
    }

    /// Restore the view to the entry at `nav_index`.
    ///
    /// Loads the file if it differs from the current file, then snaps the
    /// scrollable to the recorded scroll position.
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
    // Link / file helpers
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
    ///
    /// Returns `None` if the anchor cannot be matched to any heading.
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

        // Pass 2: relaxed match — strip everything except ascii-alphanumeric
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

    fn load_file(&mut self, path: &Path) {
        match std::fs::read_to_string(path) {
            Ok(src) => {
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
                    self.raw_markdown = new_content;
                    self.content = markdown::Content::parse(&self.raw_markdown);
                }
                Err(e) => eprintln!("Warning: could not reload file: {e}"),
            }
        }
    }

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
}
