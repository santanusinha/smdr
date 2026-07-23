//! Core state types, constants, and message definitions for the smdr viewer.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use iced::Size;
use iced::widget::{image as image_widget, markdown, scrollable, svg};

use smdr::markdown::{DocumentLink, TocEntry};
use smdr::theme::ThemeArg;

/// Configuration passed to [`launch`](super::app::launch).
#[derive(Debug, Clone, Copy)]
pub struct ViewerConfig {
    pub theme: ThemeArg,
    /// `true` when the user explicitly passed `--theme` on the command line.
    /// When `false` the persisted theme (if any) takes precedence at startup.
    pub theme_explicit: bool,
    pub watch: bool,
    /// Allow fetching remote images over the network.
    pub network_enabled: bool,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Pixels scrolled per j/k keypress.
pub(super) const LINE_SCROLL: f32 = 40.0;

/// Maximum width for rendered images (pixels).
pub(super) const MAX_IMAGE_WIDTH: f32 = 800.0;

/// Scrollable widget ID for programmatic scrolling.
pub(super) const SCROLLABLE_ID: &str = "smdr-content-scroll";

/// Scrollable widget ID for the read-only source (comment) view.
pub(super) const SOURCE_SCROLLABLE_ID: &str = "smdr-source-scroll";

/// Text input widget ID for search bar focus.
pub(super) const SEARCH_INPUT_ID: &str = "smdr-search-input";

/// Text input widget ID for the line-comment composer.
pub(super) const COMMENT_INPUT_ID: &str = "smdr-comment-input";

/// Scrollable widget ID for sidebar programmatic scrolling.
pub(super) const SIDEBAR_SCROLLABLE_ID: &str = "smdr-sidebar-scroll";

/// Scrollable widget ID for mermaid modal scrolling.
pub(super) const MERMAID_SCROLLABLE_ID: &str = "smdr-mermaid-scroll";

/// Default sidebar ratio (fraction of window width).
pub(super) const DEFAULT_SIDEBAR_RATIO: f32 = 0.25;

/// Minimum sidebar ratio.
pub(super) const MIN_SIDEBAR_RATIO: f32 = 0.15;

/// Maximum sidebar ratio.
pub(super) const MAX_SIDEBAR_RATIO: f32 = 0.40;

/// Initial window width in pixels used before the first resize event.
pub(super) const INITIAL_WINDOW_WIDTH: f32 = 960.0;

// ---------------------------------------------------------------------------
// Image cache types
// ---------------------------------------------------------------------------

/// Cached image handle for display in the viewer.
///
/// Storing the iced `Handle` (rather than raw bytes) ensures the handle's
/// internal `Id` is generated **once** and reused across frames.  Both
/// `image::Handle::from_bytes` and `svg::Handle::from_memory` create a new
/// identifier on every call; if called inside `build_ui` every frame, iced's
/// image-raster cache never hits and re-decodes the image continuously —
/// pinning the CPU at 100% per image.
#[derive(Debug, Clone)]
pub(super) enum ImageData {
    Svg(svg::Handle),
    Raster(image_widget::Handle),
}

// ---------------------------------------------------------------------------
// Navigation history entry
// ---------------------------------------------------------------------------

/// A single entry in the browser-like navigation history.
///
/// Each entry records the file being viewed and the relative scroll position
/// (0.0 = top, 1.0 = bottom) at the time of navigation.
#[derive(Debug, Clone)]
pub(super) struct NavEntry {
    pub(super) file_path: PathBuf,
    pub(super) scroll_y: f32,
}

// ---------------------------------------------------------------------------
// Overlay state
// ---------------------------------------------------------------------------

/// Which overlay panel (if any) is currently displayed.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Overlay {
    None,
    Shortcuts,
    About,
    MermaidModal(svg::Handle, f32),
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(super) enum Message {
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
    SidebarToggleVisibility,
    SidebarToggleFocus,
    UnfocusSidebar,
    SidebarNext,
    SidebarPrev,
    SidebarActivate,
    SidebarDragStart,
    SidebarDragMove(f32),
    SidebarDragEnd,
    NavigateToHeading(usize),
    Tick,
    Scrolled(scrollable::Viewport),
    WindowResized(Size),
    ImageLoaded(String, Option<ImageData>),
    MermaidZoomIn,
    MermaidZoomOut,
    MermaidScrollBy(f32, f32),
    ScrollToTop,
    ScrollToBottom,
    JumpToLastPosition,
    /// Reload the current file from disk (Ctrl-R).
    ReloadFile,
    /// Copy the current document's raw markdown to the clipboard (Ctrl-C).
    CopyToClipboard,
    ExitApp,
    PendingKey(char),
    /// Mermaid diagram rendered to SVG: (source_code, svg_bytes).
    MermaidRendered(String, Option<Vec<u8>>),
    // --- Tab messages ---
    /// Open a file in a new tab.
    OpenInNewTab(PathBuf),
    /// Switch to the tab at the given visual index.
    SwitchTab(usize),
    /// Close the tab at the given visual index.
    CloseTab(usize),
    /// Switch to the next tab.
    NextTab,
    /// Switch to the previous tab.
    PrevTab,
    /// A file path was received over IPC from another instance; open it
    /// in a new tab.
    IpcFileReceived(PathBuf),
    // --- Comment / review mode (line-anchored comments) ---
    /// Toggle the read-only, line-numbered source view used for commenting.
    ToggleCommentMode,
    /// A gutter line number was clicked; open the composer for that 0-based line.
    GutterLineClicked(usize),
    /// The comment composer text changed.
    CommentDraftChanged(String),
    /// Confirm the current composer draft, attaching it to the target line.
    CommentSubmit,
    /// Discard the current composer draft without saving.
    CommentCancel,
    /// A raw `text_editor` action from the source view. Edit actions are
    /// ignored (read-only); selection/scroll/click actions are applied.
    SourceEditorAction(iced::widget::text_editor::Action),
}

// ---------------------------------------------------------------------------
// Line-anchored comment
// ---------------------------------------------------------------------------

/// A single comment anchored to a 0-based source line.
///
/// This is a deliberately minimal, in-memory stand-in. Persistence and the
/// richer `Kind`/envelope model live on the `feature/annotation-review-mode`
/// branch; when that merges, `line` maps 1:1 onto `annotate::Annotation.line`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LineComment {
    /// 0-based source line the comment is anchored to.
    pub(super) line: usize,
    /// Freeform comment body.
    pub(super) text: String,
}

// ---------------------------------------------------------------------------
// Saved tab state
// ---------------------------------------------------------------------------

/// Snapshot of the active document's state, saved when switching away from
/// a tab and restored when switching back.
///
/// Only the fields that are document-specific are saved; UI state like
/// `sidebar_open` and `active_theme` is shared across all tabs.
#[derive(Debug)]
pub(super) struct SavedTab {
    pub(super) label: String,
    pub(super) raw_markdown: String,
    pub(super) line_count: usize,
    pub(super) file_path: PathBuf,
    pub(super) watcher_rx: Option<Receiver<()>>,
    pub(super) nav_history: Vec<NavEntry>,
    pub(super) nav_index: usize,
    pub(super) current_scroll_y: f32,
    pub(super) links: Vec<DocumentLink>,
    pub(super) focused_link: Option<usize>,
    pub(super) search_mode: bool,
    pub(super) search_query: String,
    pub(super) search_query_lower: String,
    pub(super) search_hits: Vec<usize>,
    pub(super) current_hit: Option<usize>,
    pub(super) toc: Vec<TocEntry>,
    pub(super) sidebar_selected: Option<usize>,
    pub(super) image_cache: HashMap<String, ImageData>,
    pub(super) image_pending: HashSet<String>,
    pub(super) image_failed: HashSet<String>,
    pub(super) mermaid_cache: HashMap<String, svg::Handle>,
    pub(super) mermaid_pending: HashSet<String>,
    pub(super) base_dir: PathBuf,
    pub(super) pending_key: Option<char>,
    pub(super) last_scroll_y: f32,
}

impl MdrApp {
    /// Snapshot the current document state into a `SavedTab`.
    pub(super) fn save_current_tab(&mut self) -> SavedTab {
        SavedTab {
            label: self
                .file_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "stdin".to_string()),
            raw_markdown: self.raw_markdown.clone(),
            line_count: self.line_count,
            file_path: self.file_path.clone(),
            watcher_rx: self.watcher_rx.take(),
            nav_history: self.nav_history.clone(),
            nav_index: self.nav_index,
            current_scroll_y: self.current_scroll_y,
            links: self.links.clone(),
            focused_link: self.focused_link,
            search_mode: self.search_mode,
            search_query: self.search_query.clone(),
            search_query_lower: self.search_query_lower.clone(),
            search_hits: self.search_hits.clone(),
            current_hit: self.current_hit,
            toc: self.toc.clone(),
            sidebar_selected: self.sidebar_selected,
            image_cache: self.image_cache.clone(),
            image_pending: self.image_pending.clone(),
            image_failed: self.image_failed.clone(),
            mermaid_cache: self.mermaid_cache.clone(),
            mermaid_pending: self.mermaid_pending.clone(),
            base_dir: self.base_dir.clone(),
            pending_key: self.pending_key,
            last_scroll_y: self.last_scroll_y,
        }
    }

    /// Restore document state from a `SavedTab`.
    pub(super) fn restore_tab(&mut self, tab: SavedTab) {
        self.content = markdown::Content::parse(&tab.raw_markdown);
        self.source_content = iced::widget::text_editor::Content::with_text(&tab.raw_markdown);
        // Reset the composer for the incoming document; comment mode itself
        // (the view toggle) is shared UI state and intentionally preserved.
        self.comment_target_line = None;
        self.comment_draft.clear();
        self.comments.clear();
        self.raw_markdown = tab.raw_markdown;
        self.line_count = tab.line_count;
        self.file_path = tab.file_path;
        self.watcher_rx = tab.watcher_rx;
        self.nav_history = tab.nav_history;
        self.nav_index = tab.nav_index;
        self.current_scroll_y = tab.current_scroll_y;
        self.links = tab.links;
        self.focused_link = tab.focused_link;
        self.search_mode = tab.search_mode;
        self.search_query = tab.search_query;
        self.search_query_lower = tab.search_query_lower;
        self.search_hits = tab.search_hits;
        self.current_hit = tab.current_hit;
        self.toc = tab.toc;
        self.sidebar_selected = tab.sidebar_selected;
        self.image_cache = tab.image_cache;
        self.image_pending = tab.image_pending;
        self.image_failed = tab.image_failed;
        self.mermaid_cache = tab.mermaid_cache;
        self.mermaid_pending = tab.mermaid_pending;
        self.base_dir = tab.base_dir;
        self.pending_key = tab.pending_key;
        self.last_scroll_y = tab.last_scroll_y;
        let label = self
            .file_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "stdin".to_string());
        self.title = format!("smdr — {label}");
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub(super) struct MdrApp {
    pub(super) raw_markdown: String,
    /// Number of lines in `raw_markdown`, kept in sync on every assignment.
    /// Replaces 5 independent O(N) `raw_markdown.lines().count()` calls.
    pub(super) line_count: usize,
    pub(super) content: markdown::Content,
    pub(super) file_path: PathBuf,
    pub(super) watcher_rx: Option<Receiver<()>>,
    pub(super) active_theme: ThemeArg,
    pub(super) title: String,
    /// Browser-like navigation history.
    pub(super) nav_history: Vec<NavEntry>,
    /// Current position within `nav_history`.
    pub(super) nav_index: usize,
    /// Live scroll position (relative y offset 0.0..=1.0), updated on every scroll event.
    pub(super) current_scroll_y: f32,
    /// All links in the current document (for Tab navigation).
    pub(super) links: Vec<DocumentLink>,
    /// Currently focused link index (Tab stop), or `None` if no link is focused.
    pub(super) focused_link: Option<usize>,
    pub(super) search_mode: bool,
    pub(super) search_query: String,
    /// Lowercase version of `search_query`, kept in sync on every mutation.
    /// Avoids recomputing `to_lowercase()` on every keystroke in the search loop.
    pub(super) search_query_lower: String,
    pub(super) search_hits: Vec<usize>,
    pub(super) current_hit: Option<usize>,
    pub(super) overlay: Overlay,
    /// Table of contents (headings) for sidebar navigation.
    pub(super) toc: Vec<TocEntry>,
    /// Whether the sidebar is visible.
    pub(super) sidebar_open: bool,
    /// Whether keyboard focus is in the sidebar (outline navigation).
    pub(super) sidebar_focused: bool,
    /// Currently selected heading index in the sidebar.
    pub(super) sidebar_selected: Option<usize>,
    /// Current sidebar width as a ratio of window width (0.15..=0.40).
    pub(super) sidebar_ratio: f32,
    /// Whether the user is actively dragging the sidebar resize handle.
    pub(super) sidebar_dragging: bool,
    /// Current window width in pixels (updated on resize events).
    pub(super) window_width: f32,
    /// Cached images keyed by URL.
    pub(super) image_cache: HashMap<String, ImageData>,
    /// URLs that are currently being loaded.
    pub(super) image_pending: HashSet<String>,
    /// URLs that failed to load.
    pub(super) image_failed: HashSet<String>,
    /// Cached mermaid diagram handles keyed by source code.
    pub(super) mermaid_cache: HashMap<String, svg::Handle>,
    /// Mermaid source codes currently being rendered.
    pub(super) mermaid_pending: HashSet<String>,
    /// Whether network fetching is enabled.
    pub(super) network_enabled: bool,
    /// Base directory for resolving relative image paths.
    pub(super) base_dir: PathBuf,
    /// Pending key for multi-key sequences (gg, GG, qq, ZZ, ``).
    pub(super) pending_key: Option<char>,
    /// Last scroll position before a jump (for `` to return).
    pub(super) last_scroll_y: f32,
    /// Total content height in pixels (updated on scroll events).
    pub(super) content_height: f32,
    /// Visible viewport height in pixels (updated on scroll events).
    pub(super) viewport_height: f32,
    // --- Tab state ---
    /// Saved background tabs.
    pub(super) tabs: Vec<SavedTab>,
    /// Index of the active tab (0 = first document).
    pub(super) active_tab: usize,
    // --- Comment / review mode ---
    /// Whether the read-only, line-numbered source view is active.
    pub(super) comment_mode: bool,
    /// Read-only editor content mirroring `raw_markdown`, rebuilt on load.
    /// Wrapped so the line-oriented `text_editor` can render/select it.
    pub(super) source_content: iced::widget::text_editor::Content,
    /// Line the composer is currently open for (0-based), or `None`.
    pub(super) comment_target_line: Option<usize>,
    /// Current composer draft text.
    pub(super) comment_draft: String,
    /// All line-anchored comments authored this session (not yet persisted).
    pub(super) comments: Vec<LineComment>,
}
