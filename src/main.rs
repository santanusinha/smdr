//! smdr — Simple Markdown Reader

mod daemon;
mod ipc;
mod render;

use clap::Parser;
use smdr::theme::ThemeArg;
use std::io::IsTerminal;
use std::path::PathBuf;

use render::ViewerConfig;

/// Simple Markdown Reader.
#[derive(Parser, Debug)]
#[command(name = "smdr", version, about)]
struct Cli {
    /// Path to the markdown file to view. Optional when reading from stdin pipe.
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,

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
}

fn main() {
    let cli = Cli::parse();

    if cli.list_themes {
        println!("Available themes (pass with --theme <NAME>):");
        for t in ThemeArg::ALL {
            println!("  {:22} — {}", t.slug(), t.description());
        }
        return;
    }

    let theme = cli.theme.unwrap_or(ThemeArg::System);
    let config = ViewerConfig {
        theme,
        // true iff --theme / -t was explicitly provided on the command line.
        theme_explicit: cli.theme.is_some(),
        watch: cli.watch,
        network_enabled: !cli.no_network,
    };

    // --dry-run: used by the test suite to verify CLI parsing and file
    // validation without opening a GUI window.  Exit cleanly here.
    if cli.dry_run {
        return;
    }

    // Determine input source: file argument or stdin pipe
    let stdin_is_pipe = !std::io::stdin().is_terminal();

    match cli.file {
        Some(file) => {
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

            // Resolve to an absolute path so the receiving instance can open it
            // regardless of its own working directory (it may have been
            // daemonized with a different cwd).
            let abs_path = std::fs::canonicalize(&file).unwrap_or_else(|_| file.clone());
            let path_str = abs_path.to_string_lossy().into_owned();

            // If another instance is already running, hand off the path so it
            // opens as a new tab, then exit without launching a second window.
            if ipc::client_send(&path_str).is_ok() {
                return;
            }

            // No running instance — become the first one.  Detach from the
            // controlling terminal so the shell is not blocked, then launch the
            // GUI (which also runs the IPC server to receive future paths).
            //
            // SAFETY: `daemonize` forks; it must run before any threads (the
            // tokio runtime, iced's workers) are spawned.  We are still
            // single-threaded here.
            daemon::daemonize();

            if let Err(e) = render::launch(&abs_path, &config) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }

            // Best-effort cleanup of the IPC socket on exit.
            ipc::cleanup_socket();
        }
        None if stdin_is_pipe => {
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
        }
        None => {
            eprintln!("Error: no FILE argument and stdin is not a pipe");
            eprintln!("Usage: smdr <FILE> or pipe markdown to smdr");
            std::process::exit(1);
        }
    }
}
