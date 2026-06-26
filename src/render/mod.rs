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

mod app;
#[cfg(target_os = "linux")]
mod desktop;
mod images;
mod navigation;
mod search;
mod sidebar;
mod state;
mod styles;
mod update;
mod view;
mod widget;

pub use app::{launch, launch_stdin};
pub use state::ViewerConfig;
