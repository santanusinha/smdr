//! File-change watcher for live-reload.
//!
//! Uses the `notify` crate to monitor the directory containing the markdown
//! file.  When a modification is detected a unit value is sent through a
//! channel.  The main event loop can poll the receiver with `try_recv()` and
//! trigger a reload whenever a message arrives.

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};

/// Starts watching the directory that contains `file_path` for changes.
///
/// The parent directory (not just the file itself) is watched because many
/// editors write changes atomically by creating a temporary file and renaming
/// it, which would not trigger a file-level watch.
///
/// # Returns
/// A `(Watcher, Receiver<()>)` pair.  The `Watcher` **must be kept alive** for
/// as long as watching is desired — dropping it stops the watch.  Poll the
/// `Receiver` with `try_recv()` to check for changes.
///
/// # Errors
/// Returns an error if the underlying OS watcher cannot be created or if the
/// path cannot be watched.
pub fn watch_file(
    file_path: &Path,
) -> Result<(RecommendedWatcher, Receiver<()>), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel::<()>();

    let mut watcher = RecommendedWatcher::new(
        move |result: notify::Result<notify::Event>| {
            if let Ok(event) = result
                && (event.kind.is_modify() || event.kind.is_create())
            {
                // Ignore send errors: the receiver might have been dropped
                // (e.g., during shutdown) and that is fine.
                let _ = tx.send(());
            }
        },
        Config::default(),
    )?;

    let watch_dir = file_path.parent().unwrap_or(file_path);
    watcher.watch(watch_dir, RecursiveMode::NonRecursive)?;

    Ok((watcher, rx))
}
