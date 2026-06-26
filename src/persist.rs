//! Persistent reader state — saved to `~/.state/smdr/state` in JSON format.
//!
//! The file is created (along with its parent directories) on the first save.
//! A missing or malformed file is silently ignored, falling back to defaults.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::theme::ThemeArg;

/// The subset of viewer state persisted across sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedState {
    /// Last theme selected by the user.
    pub theme: ThemeArg,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            theme: ThemeArg::System,
        }
    }
}

/// Returns the path to the state file: `~/.state/smdr/state`.
pub fn state_path() -> Option<PathBuf> {
    dirs_home().map(|home| home.join(".state").join("smdr").join("state"))
}

/// Load persisted state from disk.
///
/// Returns `None` if the home directory cannot be determined.
/// Returns `PersistedState::default()` if the file does not exist or cannot
/// be parsed — errors are silently swallowed so a corrupt file never prevents
/// the app from launching.
pub fn load() -> Option<PersistedState> {
    let path = state_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Save `state` to disk.
///
/// Creates `~/.state/smdr/` and any intermediate directories if they do not
/// exist.  Errors (e.g. read-only filesystem) are silently discarded so a
/// failed save never crashes the application.
pub fn save(state: &PersistedState) {
    let Some(path) = state_path() else { return };
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!(
            "smdr: could not create state directory {}: {e}",
            parent.display()
        );
        return;
    }
    match serde_json::to_string_pretty(state) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                eprintln!("smdr: could not write state file {}: {e}", path.display());
            }
        }
        Err(e) => {
            eprintln!("smdr: could not serialise state: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Returns the user's home directory, or `None` if it cannot be determined.
fn dirs_home() -> Option<PathBuf> {
    // Try $HOME first (POSIX-portable); fall back to the passwd entry on Unix.
    if let Some(home) = std::env::var_os("HOME") {
        return Some(PathBuf::from(home));
    }
    None
}
