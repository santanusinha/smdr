//! Self-installation of XDG desktop integration assets on Linux.
//!
//! When `ensure_xdg_assets` is called, it writes the application icon and
//! `.desktop` file into the user's local XDG directories
//! (`~/.local/share/icons` and `~/.local/share/applications`) so that
//! GNOME / KDE / any XDG-compliant desktop shows the correct icon and name
//! in the app-switcher, taskbar, and file manager — without requiring a
//! separate installer script.
//!
//! Files are written only when missing or when the embedded version stamp
//! differs from what is on disk, so repeated launches are cheap (a single
//! file-existence check each).

const APP_ID: &str = "smdr";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Icon PNG bytes embedded at compile time (256 × 256 RGBA).
const ICON_PNG: &[u8] = include_bytes!("../../assets/icon_256.png");

/// `.desktop` file content.  The `X-Version` key is used as a staleness
/// stamp: if the on-disk file has a different version we rewrite everything.
fn desktop_file_contents() -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=smdr\n\
         GenericName=Markdown Viewer\n\
         Comment=Simple Markdown Reader — fast native markdown viewer with vim-style navigation\n\
         Exec=smdr %f\n\
         Icon=smdr\n\
         Terminal=false\n\
         Categories=Utility;TextEditor;Viewer;\n\
         MimeType=text/markdown;text/x-markdown;\n\
         Keywords=markdown;md;viewer;reader;\n\
         StartupWMClass=smdr\n\
         StartupNotify=true\n\
         X-Version={VERSION}\n"
    )
}

/// Ensures the XDG icon and `.desktop` file are present and up-to-date.
///
/// Silently skips any step that fails — broken desktop integration must never
/// prevent the application from launching.
pub fn ensure_xdg_assets() {
    if let Some(home) = home_dir() {
        ensure_icon(&home);
        ensure_desktop_file(&home);
    }
}

fn home_dir() -> Option<std::path::PathBuf> {
    // Prefer XDG_DATA_HOME base; fall back to ~/.local/share.
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

fn xdg_data_home(home: &std::path::Path) -> std::path::PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"))
}

fn ensure_icon(home: &std::path::Path) {
    let icon_path = xdg_data_home(home)
        .join("icons/hicolor/256x256/apps")
        .join(format!("{APP_ID}.png"));

    if icon_path.exists() {
        return;
    }

    if let Some(parent) = icon_path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }

    let _ = std::fs::write(&icon_path, ICON_PNG);

    // Best-effort icon cache refresh — not available everywhere.
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .args(["-f", "-t"])
        .arg(
            xdg_data_home(home)
                .join("icons/hicolor")
                .to_string_lossy()
                .as_ref(),
        )
        .output();
}

fn ensure_desktop_file(home: &std::path::Path) {
    let desktop_path = xdg_data_home(home)
        .join("applications")
        .join(format!("{APP_ID}.desktop"));

    // Rewrite if missing or if the version stamp is outdated.
    if desktop_path.exists() && !needs_update(&desktop_path) {
        return;
    }

    if let Some(parent) = desktop_path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }

    let _ = std::fs::write(&desktop_path, desktop_file_contents());

    // Refresh the desktop database so the entry is immediately visible.
    let _ = std::process::Command::new("update-desktop-database")
        .arg(
            xdg_data_home(home)
                .join("applications")
                .to_string_lossy()
                .as_ref(),
        )
        .output();
}

/// Returns `true` when the on-disk `.desktop` file's `X-Version` line differs
/// from the current binary's version.
fn needs_update(path: &std::path::Path) -> bool {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return true;
    };
    let stamp = format!("X-Version={VERSION}");
    !contents.lines().any(|l| l == stamp)
}
