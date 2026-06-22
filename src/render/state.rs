//! Core state types, constants, and message definitions for the mdr viewer.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use iced::Size;
use iced::widget::{markdown, scrollable};

use mdr::markdown::{DocumentLink, TocEntry};
use mdr::theme::ThemeArg;

/// Configuration passed to [`launch`](super::app::launch).
#[derive(Debug, Clone, Copy)]
pub struct ViewerConfig {
    pub theme: ThemeArg,
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
pub(super) const SCROLLABLE_ID: &str = "mdr-content-scroll";

/// Text input widget ID for search bar focus.
pub(super) const SEARCH_INPUT_ID: &str = "mdr-search-input";

/// Scrollable widget ID for sidebar programmatic scrolling.
pub(super) const SIDEBAR_SCROLLABLE_ID: &str = "mdr-sidebar-scroll";

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

/// Cached image data for display in the viewer.
#[derive(Debug, Clone)]
pub(super) enum ImageData {
    Svg(Vec<u8>),
    Raster(Vec<u8>),
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Overlay {
    None,
    Shortcuts,
    About,
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
    ScrollToTop,
    ScrollToBottom,
    JumpToLastPosition,
    ExitApp,
    PendingKey(char),
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub(super) struct MdrApp {
    pub(super) raw_markdown: String,
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
    /// Cached mermaid diagram SVGs keyed by source code.
    pub(super) mermaid_cache: HashMap<String, Vec<u8>>,
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
}
