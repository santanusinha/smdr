//! CLI integration tests using `assert_cmd`.
//!
//! These tests invoke the compiled `mdr` binary via [`assert_cmd::Command`]
//! and verify exit codes and output messages without launching a real GUI.
//!
//! Any test that passes a valid file and wants to verify CLI flag parsing uses
//! `--dry-run` (a hidden flag) so the binary exits cleanly after argument
//! validation, before `render::launch` is ever called.  That way no display
//! connection is required and no windows are opened during `cargo test`.
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn mdr() -> Command {
    Command::cargo_bin("mdr").expect("mdr binary")
}

/// Creates a temp dir and a minimal markdown file inside it.
/// Returns (TempDir guard, path-string of the file).
fn temp_md_file(content: &str) -> (TempDir, String) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("test.md");
    fs::write(&path, content).expect("write md");
    let path_str = path.to_string_lossy().into_owned();
    (dir, path_str)
}

/// Convenience: run mdr with `--dry-run` + extra args + the file path.
/// `--dry-run` is a hidden flag that exits cleanly after arg/file validation,
/// before `render::launch` is called, so no window is ever opened.
fn mdr_dry(extra_args: &[&str], path: &str) -> std::process::Output {
    mdr()
        .arg("--dry-run")
        .args(extra_args)
        .arg(path)
        .output()
        .expect("run")
}

// ---------------------------------------------------------------------------
// --help / --version
// ---------------------------------------------------------------------------

#[test]
fn test_help_flag_exits_zero() {
    mdr().arg("--help").assert().success();
}

#[test]
fn test_help_output_contains_usage() {
    mdr()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("mdr").or(predicate::str::contains("Usage")));
}

#[test]
fn test_version_flag_exits_zero() {
    mdr().arg("--version").assert().success();
}

#[test]
fn test_version_output_contains_version_number() {
    mdr()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.1.0").or(predicate::str::contains("mdr")));
}

// ---------------------------------------------------------------------------
// Missing / wrong arguments
// ---------------------------------------------------------------------------

#[test]
fn test_no_args_exits_nonzero() {
    mdr().assert().failure();
}

#[test]
fn test_no_args_prints_error_or_usage() {
    mdr()
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

#[test]
fn test_unknown_flag_exits_nonzero() {
    mdr().arg("--frobulate").assert().failure();
}

// ---------------------------------------------------------------------------
// File validation errors (exit before GUI)
// ---------------------------------------------------------------------------

#[test]
fn test_nonexistent_file_exits_nonzero() {
    mdr()
        .arg("/tmp/mdr_test_definitely_does_not_exist_xyzzy.md")
        .assert()
        .failure();
}

#[test]
fn test_nonexistent_file_prints_error_to_stderr() {
    mdr()
        .arg("/tmp/mdr_test_definitely_does_not_exist_xyzzy.md")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Error")
                .or(predicate::str::contains("error"))
                .or(predicate::str::contains("not found")),
        );
}

#[test]
fn test_directory_as_file_exits_nonzero() {
    let dir = TempDir::new().expect("tempdir");
    mdr()
        .arg(dir.path().to_str().expect("path"))
        .assert()
        .failure();
}

#[test]
fn test_directory_as_file_prints_error_to_stderr() {
    let dir = TempDir::new().expect("tempdir");
    mdr()
        .arg(dir.path().to_str().expect("path"))
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

// ---------------------------------------------------------------------------
// Theme argument validation
// ---------------------------------------------------------------------------

#[test]
fn test_invalid_theme_exits_nonzero() {
    let (_dir, path) = temp_md_file("# hi");
    mdr()
        .args(["--dry-run", "--theme", "rainbow", &path])
        .assert()
        .failure();
}

#[test]
fn test_invalid_theme_prints_error() {
    let (_dir, path) = temp_md_file("# hi");
    mdr()
        .args(["--dry-run", "--theme", "rainbow", &path])
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

// ---------------------------------------------------------------------------
// Non-markdown extension warning
// Passes --dry-run so the binary exits before touching the display.
// ---------------------------------------------------------------------------

#[test]
fn test_txt_file_does_not_cause_clap_error() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("notes.txt");
    fs::write(&path, "hello").expect("write");
    let output = mdr()
        .args(["--dry-run", path.to_str().expect("path")])
        .output()
        .expect("run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("unrecognized"),
        "clap should not reject a .txt file path, got stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// Flag combinations — all use --dry-run so no window is opened
// ---------------------------------------------------------------------------

#[test]
fn test_watch_flag_is_accepted_by_clap() {
    let (_dir, path) = temp_md_file("# hi");
    let out = mdr_dry(&["--watch"], &path);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success()
            && !stderr.contains("unexpected argument")
            && !stderr.contains("unrecognized"),
        "clap should accept --watch, got: {stderr}"
    );
}

#[test]
fn test_no_network_flag_is_accepted_by_clap() {
    let (_dir, path) = temp_md_file("# hi");
    let out = mdr_dry(&["--no-network"], &path);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success()
            && !stderr.contains("unexpected argument")
            && !stderr.contains("unrecognized"),
        "clap should accept --no-network, got: {stderr}"
    );
}

#[test]
fn test_dark_theme_is_accepted_by_clap() {
    let (_dir, path) = temp_md_file("# hi");
    let out = mdr_dry(&["--theme", "dark"], &path);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success()
            && !stderr.contains("unexpected argument")
            && !stderr.contains("invalid value")
            && !stderr.contains("unrecognized"),
        "clap should accept --theme dark, got: {stderr}"
    );
}

#[test]
fn test_light_theme_is_accepted_by_clap() {
    let (_dir, path) = temp_md_file("# hi");
    let out = mdr_dry(&["--theme", "light"], &path);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success()
            && !stderr.contains("unexpected argument")
            && !stderr.contains("invalid value")
            && !stderr.contains("unrecognized"),
        "clap should accept --theme light, got: {stderr}"
    );
}

#[test]
fn test_system_theme_is_accepted_by_clap() {
    let (_dir, path) = temp_md_file("# hi");
    let out = mdr_dry(&["--theme", "system"], &path);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success()
            && !stderr.contains("unexpected argument")
            && !stderr.contains("invalid value")
            && !stderr.contains("unrecognized"),
        "clap should accept --theme system, got: {stderr}"
    );
}

#[test]
fn test_tokyo_night_theme_is_accepted_by_clap() {
    let (_dir, path) = temp_md_file("# hi");
    let out = mdr_dry(&["--theme", "tokyo-night"], &path);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success()
            && !stderr.contains("unexpected argument")
            && !stderr.contains("invalid value")
            && !stderr.contains("unrecognized"),
        "clap should accept --theme tokyo-night, got: {stderr}"
    );
}

#[test]
fn test_solarized_dark_theme_is_accepted_by_clap() {
    let (_dir, path) = temp_md_file("# hi");
    let out = mdr_dry(&["--theme", "solarized-dark"], &path);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success()
            && !stderr.contains("unexpected argument")
            && !stderr.contains("invalid value")
            && !stderr.contains("unrecognized"),
        "clap should accept --theme solarized-dark, got: {stderr}"
    );
}

#[test]
fn test_all_flags_together_accepted_by_clap() {
    let (_dir, path) = temp_md_file("# hi");
    let out = mdr_dry(&["--watch", "--theme", "dark", "--no-network"], &path);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success()
            && !stderr.contains("unexpected argument")
            && !stderr.contains("invalid value")
            && !stderr.contains("unrecognized"),
        "clap should accept all flags combined, got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// --dry-run: verify it exits zero and never reaches render::launch
// ---------------------------------------------------------------------------

#[test]
fn test_dry_run_exits_zero() {
    let (_dir, path) = temp_md_file("# hi");
    mdr().args(["--dry-run", &path]).assert().success();
}

#[test]
fn test_dry_run_produces_no_output() {
    let (_dir, path) = temp_md_file("# hi");
    mdr()
        .args(["--dry-run", &path])
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::is_empty());
}

// ---------------------------------------------------------------------------
// --list-themes
// ---------------------------------------------------------------------------

#[test]
fn test_list_themes_exits_zero() {
    mdr().arg("--list-themes").assert().success();
}

#[test]
fn test_list_themes_output_contains_known_themes() {
    mdr().arg("--list-themes").assert().success().stdout(
        predicate::str::contains("system")
            .and(predicate::str::contains("dark"))
            .and(predicate::str::contains("light"))
            .and(predicate::str::contains("tokyo-night"))
            .and(predicate::str::contains("solarized-dark")),
    );
}
