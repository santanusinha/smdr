//! smdr — Simple Markdown Reader

mod daemon;
mod ipc;
mod render;

use clap::Parser;
use smdr::annotate::OutputFormat;
use smdr::theme::ThemeArg;
use std::io::IsTerminal;
use std::path::PathBuf;

use render::ViewerConfig;

/// Output serializer for `--review`. Mirrors §6 of the design doc.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum ReviewFormat {
    /// Annotated markdown: whole doc + inline notes (self-contained).
    Md,
    /// Structured JSON envelope (for harnesses that branch on kind). DEFAULT.
    Json,
    /// Unified-diff review transport (sparse; base-in-context).
    Diff,
}

impl From<ReviewFormat> for OutputFormat {
    fn from(f: ReviewFormat) -> Self {
        match f {
            ReviewFormat::Md => OutputFormat::Md,
            ReviewFormat::Json => OutputFormat::Json,
            ReviewFormat::Diff => OutputFormat::Diff,
        }
    }
}

/// Simple Markdown Reader.
#[derive(Parser, Debug)]
#[command(name = "smdr", version, about)]
struct Cli {
    /// Path to the markdown file to view. Optional when reading from stdin pipe.
    /// Paths to the markdown files to view. Multiple files each open in their
    /// own tab. Optional when reading from stdin pipe.
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,

    /// Watch the file for changes and auto-reload.
    #[arg(short, long)]
    watch: bool,

    /// Color theme (default: system).
    #[arg(short, long, value_enum)]
    theme: Option<ThemeArg>,

    /// Disable network image fetching (use local files only).
    #[arg(long)]
    no_network: bool,

    /// List available themes and exit.
    #[arg(long)]
    list_themes: bool,

    /// Parse arguments and validate the file, then exit without opening a window.
    /// Used by the test suite to verify CLI parsing without launching a GUI.
    #[arg(long, hide = true)]
    dry_run: bool,

    /// Mark this file for review. Without --annotations-in, opens the normal
    /// GUI viewer. With --annotations-in, runs a headless one-shot turn:
    /// reads the annotations JSON, emits feedback to stdout (or --out), exits.
    #[arg(long)]
    review: bool,

    /// Path to a JSON file of annotations to ingest for the review turn.
    /// v1 stand-in for GUI authoring. Shape: a JSON array of Annotation, or a
    /// full ReviewEnvelope.
    #[arg(long, value_name = "PATH")]
    annotations_in: Option<PathBuf>,

    /// Where to write review output. Defaults to stdout.
    #[arg(long, value_name = "PATH")]
    out: Option<PathBuf>,

    /// Output format for review mode.
    #[arg(long, value_enum, default_value_t = ReviewFormat::Json)]
    format: ReviewFormat,
}

/// Validate that a path given to `--review` is a regular, accessible file.
///
/// Exits the process with a descriptive error on failure so both the headless
/// and interactive `--review` paths share identical validation logic.
fn require_review_file(file: &std::path::Path) {
    if !file.is_file() {
        eprintln!("Error: not a file: {}", file.display());
        std::process::exit(1);
    }
}

///
/// Reads the draft from `file`, the annotations from `annotations_in` (a JSON
/// array of `Annotation` OR a full `ReviewEnvelope`), renders the chosen
/// `format`, and writes to `--out` (or stdout). NEVER writes to `file`.
fn run_review(
    file: &std::path::Path,
    annotations_in: Option<&std::path::Path>,
    out: Option<&std::path::Path>,
    format: ReviewFormat,
) -> i32 {
    use smdr::annotate::{Annotation, ReviewEnvelope, render};

    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {e}", file.display());
            return 1;
        }
    };

    // Build the envelope from the annotations file (v1 stand-in for the GUI).
    let env: ReviewEnvelope = match annotations_in {
        Some(path) => {
            let text = match std::fs::read_to_string(path) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Error reading {}: {e}", path.display());
                    return 1;
                }
            };
            // Accept EITHER a bare array of Annotation OR a full envelope.
            match serde_json::from_str::<ReviewEnvelope>(&text) {
                Ok(env) => env,
                Err(_) => match serde_json::from_str::<Vec<Annotation>>(&text) {
                    Ok(comments) => ReviewEnvelope::new(file.to_string_lossy(), comments),
                    Err(e) => {
                        eprintln!("Error parsing annotations JSON {}: {e}", path.display());
                        return 1;
                    }
                },
            }
        }
        // No annotations supplied → empty (still valid: "no comments" turn).
        None => ReviewEnvelope::new(file.to_string_lossy(), Vec::new()),
    };
    // Single dispatcher shared with the interactive GUI submit so both honour
    // `--format` identically.
    let rendered = render(&source, &env, format.into());

    match out {
        Some(path) => {
            if let Err(e) = std::fs::write(path, rendered) {
                eprintln!("Error writing {}: {e}", path.display());
                return 1;
            }
        }
        None => print!("{rendered}"),
    }
    0
}

fn main() {
    // Re-exec'd daemon (macOS fork+exec path): already detached, go straight
    // to the GUI without daemonizing again.
    if daemon::is_daemonized() {
        run_viewer(std::env::args().skip(1).collect());
        return;
    }

    let cli = Cli::parse();

    if cli.list_themes {
        println!("Available themes (pass with --theme <NAME>):");
        for t in ThemeArg::ALL {
            println!("  {:22} — {}", t.slug(), t.description());
        }
        return;
    }

    // --review + --annotations-in: headless one-shot turn; emit feedback and
    // exit. Without --annotations-in the flag falls through to the GUI viewer
    // (where comments are authored interactively in the source-gutter view).
    if cli.review && cli.annotations_in.is_some() {
        let Some(file) = cli.files.first() else {
            eprintln!("Error: --review requires a FILE argument");
            std::process::exit(2);
        };
        require_review_file(file);
        if cli.dry_run {
            return;
        }
        let code = run_review(
            file,
            cli.annotations_in.as_deref(),
            cli.out.as_deref(),
            cli.format,
        );
        std::process::exit(code);
    }

    let theme = cli.theme.unwrap_or(ThemeArg::System);
    let config = ViewerConfig {
        theme,
        // true iff --theme / -t was explicitly provided on the command line.
        theme_explicit: cli.theme.is_some(),
        watch: cli.watch,
        network_enabled: !cli.no_network,
        // --review (without --annotations-in) enables interactive review in the
        // GUI: comments authored in the source-gutter view are submitted as an
        // envelope to --out (or stdout) on ReviewSubmit.
        review_mode: cli.review,
        review_out: cli.out.clone(),
        review_format: cli.format.into(),
        // A review window is a one-shot, self-contained process: it does NOT
        // run the IPC server (no tab hand-off, no shared socket). A normal
        // viewer does, so later invocations open as tabs.
        ipc_enabled: !cli.review,
        // Set to true just before the normal viewer daemonizes (below). A
        // daemonized process has stdout wired to /dev/null, so a review submit
        // must fall back to a timestamped temp file rather than vanishing.
        daemonized: false,
    };

    // Interactive review: open a SINGLE foreground window straight into review
    // mode. Unlike the normal viewer we deliberately do NOT daemonize — the
    // double-fork redirects stdout/stderr to /dev/null, which would silently
    // swallow the review output emitted on ReviewSubmit. Running in the
    // foreground keeps stdout wired to the caller's terminal/pipe so the
    // diffed (or --format) output is delivered on submit.
    if cli.review {
        let Some(file) = cli.files.first() else {
            eprintln!("Error: --review requires a FILE argument");
            std::process::exit(2);
        };
        require_review_file(file);
        if cli.dry_run {
            return;
        }
        let abs = std::fs::canonicalize(file).unwrap_or_else(|_| file.clone());
        if let Err(e) = render::launch(&[abs], &config) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return;
    }

    // Determine input source: file argument(s) or stdin pipe
    let stdin_is_pipe = !std::io::stdin().is_terminal();

    if cli.files.is_empty() {
        if stdin_is_pipe {
            use std::io::Read;
            let mut content = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut content) {
                eprintln!("Error reading stdin: {e}");
                std::process::exit(1);
            }
            if content.is_empty() {
                eprintln!("Error: no input received from stdin");
                std::process::exit(1);
            }
            if let Err(e) = render::launch_stdin(content, &config) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
            return;
        }
        eprintln!("Error: no FILE argument and stdin is not a pipe");
        eprintln!("Usage: smdr <FILE>... or pipe markdown to smdr");
        std::process::exit(1);
    }

    // Validate every file and resolve to absolute paths so the receiving
    // instance can open them regardless of its own working directory (it may
    // have been daemonized with a different cwd).
    let mut abs_paths: Vec<PathBuf> = Vec::with_capacity(cli.files.len());
    for file in &cli.files {
        if !file.exists() {
            eprintln!("Error: file not found: {}", file.display());
            std::process::exit(1);
        }
        if !file.is_file() {
            eprintln!("Error: not a file: {}", file.display());
            std::process::exit(1);
        }
        if let Some(ext) = file.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            if !matches!(ext_lower.as_str(), "md" | "markdown" | "mdown" | "mkd") {
                eprintln!(
                    "Warning: {} doesn't look like a markdown file (extension: .{})",
                    file.display(),
                    ext_lower
                );
            }
        }
        let abs = std::fs::canonicalize(file).unwrap_or_else(|_| file.clone());
        abs_paths.push(abs);
    }

    // --dry-run: used by the test suite to verify CLI parsing and file
    // validation without opening a GUI window.  Exit cleanly here, after
    // validation but before any window or IPC hand-off.
    if cli.dry_run {
        return;
    }

    let path_strs: Vec<String> = abs_paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    // If another instance is already running, hand off every path so each
    // opens as a new tab, then exit without launching a second window.
    if ipc::client_send(&path_strs).is_ok() {
        return;
    }

    run_viewer(cli_args_for_daemon(std::env::args().collect(), cli.files.len(), abs_paths));
}

/// Original argv (program name + user args) used to re-exec a fresh smdr
/// image on macOS so AppKit initializes in a non-forked process. Trailing
/// file arguments are replaced with their canonicalized absolute paths so
/// the re-exec'd process resolves them regardless of its own cwd.
fn cli_args_for_daemon(mut args: Vec<String>, file_count: usize, abs_paths: Vec<PathBuf>) -> Vec<String> {
    if file_count > 0 && args.len() >= file_count {
        let keep = args.len() - file_count;
        args.truncate(keep);
        args.extend(abs_paths.iter().map(|p| p.to_string_lossy().into_owned()));
    }
    args
}

/// Launch the GUI as the first (daemonized) instance.
fn run_viewer(args: Vec<String>) {
    // Parse the (possibly re-exec'd) arguments for viewer-relevant flags.
    let cli = match Cli::try_parse_from(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(2);
        }
    };

    let abs_paths: Vec<PathBuf> = cli
        .files
        .iter()
        .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()))
        .collect();

    let theme = cli.theme.unwrap_or(ThemeArg::System);
    let config = ViewerConfig {
        theme,
        theme_explicit: cli.theme.is_some(),
        watch: cli.watch,
        network_enabled: !cli.no_network,
        review_mode: cli.review,
        review_out: cli.out.clone(),
        review_format: cli.format.into(),
        ipc_enabled: !cli.review,
        // Set below when daemonized. A daemonized process has stdout wired to
        // /dev/null, so a review submit must fall back to a timestamped temp
        // file rather than vanishing.
        daemonized: daemon::is_daemonized(),
    };

    // If another instance is already running, hand off every path so each
    // opens as a new tab, then exit without launching a second window.
    let path_strs: Vec<String> = abs_paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if config.ipc_enabled && ipc::client_send(&path_strs).is_ok() {
        return;
    }

    // No running instance — become the first one.  Detach from the controlling
    // terminal so the shell is not blocked, then launch the GUI (which also
    // runs the IPC server to receive future paths).  The first file is the
    // primary document; any remaining files open as additional tabs at startup.
    //
    // SAFETY: `daemonize` forks; it must run before any threads (the tokio
    // runtime, iced's workers) are spawned.  We are still single-threaded here.
    if daemon::daemonize(&args) {
        // This invocation was the parent/intermediate: the daemon continues
        // via re-exec. Nothing to do here.
        return;
    }

    // Daemon (or foreground fallback) — launch the GUI.
    if let Err(e) = render::launch(&abs_paths, &config) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    // Best-effort cleanup of the IPC socket on exit.
    ipc::cleanup_socket();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn daemon_args_replace_relative_files_with_absolute() {
        let args = vec![
            "smdr".to_string(),
            "-w".to_string(),
            "a.md".to_string(),
            "b.md".to_string(),
        ];
        let abs = vec![PathBuf::from("/tmp/x/a.md"), PathBuf::from("/tmp/x/b.md")];
        let out = cli_args_for_daemon(args, 2, abs);
        assert_eq!(out, vec!["smdr", "-w", "/tmp/x/a.md", "/tmp/x/b.md"]);
    }

    #[test]
    fn daemon_args_passthrough_when_no_files() {
        let args = vec!["smdr".to_string(), "--theme".to_string(), "dark".to_string()];
        let out = cli_args_for_daemon(args.clone(), 0, vec![]);
        assert_eq!(out, args);
    }

    #[test]
    fn daemon_args_survive_arg_count_mismatch() {
        // Fewer argv entries than parsed files (cannot happen via clap, but
        // must not panic if it ever does).
        let args = vec!["smdr".to_string()];
        let out = cli_args_for_daemon(args, 3, vec![PathBuf::from("/tmp/a.md")]);
        assert_eq!(out, vec!["smdr"]);
    }
}
