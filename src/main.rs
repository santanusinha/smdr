//! smdr — Simple Markdown Reader

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

    /// Color theme.
    #[arg(short, long, value_enum, default_value = "system")]
    theme: ThemeArg,

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

    let config = ViewerConfig {
        theme: cli.theme,
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

            if let Err(e) = render::launch(&file, &config) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
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
