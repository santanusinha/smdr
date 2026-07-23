//! Auto-saved review drafts — persisted to a temp file keyed by file path.
//!
//! While a reviewer authors line-anchored comments in review mode, every change
//! is mirrored to a per-document JSON file under the system temp dir. If the
//! window is closed WITHOUT submitting, reopening the same file in review mode
//! restores the in-progress comments, so no work is lost. A successful
//! `ReviewSubmit` clears the draft (the turn is complete).
//!
//! The draft file name is a stable hash of the document's (canonical) path, so
//! the same document always maps to the same draft regardless of the working
//! directory it was opened from.

use std::path::{Path, PathBuf};

use crate::annotate::Annotation;

/// Subdirectory (under the system temp dir) that holds all review drafts.
const DRAFT_DIR: &str = "smdr-drafts";

/// FNV-1a 64-bit hash of a path's textual form, hex-encoded.
///
/// Used to derive a stable, collision-resistant-enough draft filename from a
/// document path. FNV-1a is chosen over [`std::hash::DefaultHasher`] because the
/// latter's output is not guaranteed stable across Rust releases — a draft must
/// still be found again after the user upgrades their toolchain.
fn hash_path(path: &Path) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for b in path.to_string_lossy().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(PRIME);
    }
    format!("{h:016x}")
}

/// Path to the draft file for `file`: `<tmp>/smdr-drafts/<hash>.json`.
///
/// `file` is canonicalized first so `./a.md`, `a.md`, and an absolute path all
/// resolve to the same draft. Falls back to the given path if canonicalization
/// fails (e.g. the file was deleted).
pub fn draft_path(file: &Path) -> PathBuf {
    let canonical = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    std::env::temp_dir()
        .join(DRAFT_DIR)
        .join(format!("{}.json", hash_path(&canonical)))
}

/// Derive a timestamped output path for a submitted review turn under the temp
/// dir: `<tmp>/<stem>-<unix_secs>.<ext>`.
///
/// Used when a review is submitted from a DAEMONIZED viewer whose stdout was
/// redirected to `/dev/null` — writing the output to a discoverable file keeps
/// it from vanishing. The stem comes from the document's file name (falling
/// back to `review` for stdin/unnamed docs); `ext` matches the chosen output
/// format (json/md/diff). The timestamp is whole seconds since the Unix epoch,
/// so concurrent submits of the same file are unlikely to collide.
pub fn review_output_path(file: &Path, ext: &str) -> PathBuf {
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "review".to_string());
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("{stem}-{secs}.{ext}"))
}

/// Load a previously auto-saved draft for `file`.
///
/// Returns an empty vector if no draft exists or the file is unreadable/corrupt
/// — a bad draft never blocks opening the document.
pub fn load(file: &Path) -> Vec<Annotation> {
    let path = draft_path(file);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

/// Auto-save the in-progress `comments` for `file`.
///
/// An empty list clears the draft (nothing left to restore). Creates the draft
/// directory on first save. Errors are logged but never propagated, so a failed
/// save can't interrupt authoring.
pub fn save(file: &Path, comments: &[Annotation]) {
    if comments.is_empty() {
        clear(file);
        return;
    }
    let path = draft_path(file);
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!(
            "smdr: could not create draft directory {}: {e}",
            parent.display()
        );
        return;
    }
    match serde_json::to_string_pretty(comments) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                eprintln!("smdr: could not write draft {}: {e}", path.display());
            }
        }
        Err(e) => eprintln!("smdr: could not serialise draft: {e}"),
    }
}

/// Remove the draft for `file` (e.g. after a successful submit). A missing file
/// is not an error.
pub fn clear(file: &Path) {
    let path = draft_path(file);
    if let Err(e) = std::fs::remove_file(&path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!("smdr: could not remove draft {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ann(line: usize, comment: &str) -> Annotation {
        Annotation {
            line,
            comment: comment.to_string(),
        }
    }

    #[test]
    fn hash_is_stable_and_path_dependent() {
        let a = Path::new("/tmp/smdr-test/one.md");
        let b = Path::new("/tmp/smdr-test/two.md");
        assert_eq!(hash_path(a), hash_path(a), "hash must be deterministic");
        assert_ne!(hash_path(a), hash_path(b), "distinct paths differ");
        // 16 hex chars (64-bit).
        assert_eq!(hash_path(a).len(), 16);
    }

    #[test]
    fn save_load_clear_roundtrip() {
        // Use a unique fake path so the derived draft file is test-private.
        let dir = std::env::temp_dir().join("smdr-draft-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(format!("doc-{}.md", std::process::id()));
        std::fs::write(&file, "# hi\n").unwrap();

        // No draft yet.
        clear(&file);
        assert!(load(&file).is_empty());

        // Save, then load restores identical comments.
        let comments = vec![ann(0, "heading"), ann(4, "tighten")];
        save(&file, &comments);
        assert_eq!(load(&file), comments);

        // Saving an empty list clears the draft.
        save(&file, &[]);
        assert!(load(&file).is_empty());

        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn review_output_path_has_stem_timestamp_and_ext() {
        let file = Path::new("/home/user/docs/report.md");
        let p = review_output_path(file, "json");
        let name = p.file_name().unwrap().to_string_lossy();
        // <stem>-<digits>.<ext>
        assert!(name.starts_with("report-"), "stem prefix: {name}");
        assert!(name.ends_with(".json"), "ext suffix: {name}");
        let ts = name.trim_start_matches("report-").trim_end_matches(".json");
        assert!(
            !ts.is_empty() && ts.chars().all(|c| c.is_ascii_digit()),
            "timestamp is all digits: {ts}"
        );
        // Lands under the system temp dir.
        assert!(p.starts_with(std::env::temp_dir()));
    }

    #[test]
    fn review_output_path_falls_back_to_review_stem() {
        // A path with no usable file stem falls back to "review".
        let p = review_output_path(Path::new("/"), "diff");
        let name = p.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("review-"), "fallback stem: {name}");
        assert!(name.starts_with("review-"), "fallback stem: {name}");
        assert!(name.ends_with(".diff"));
    }
}
