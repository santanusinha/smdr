//! mdr — Minimal Desktop Markdown Reader

mod render;

use clap::Parser;
use std::path::PathBuf;

use render::ViewerConfig;

/// A minimal desktop markdown reader.
#[derive(Parser, Debug)]
#[command(name = "mdr", version, about)]
struct Cli {
    /// Path to the markdown file to view. Not required when --list-themes is used.
    #[arg(value_name = "FILE", required_unless_present = "list_themes")]
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

/// CLI-parseable theme selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ThemeArg {
    /// Follow the operating-system dark/light preference (default).
    System,
    /// Force light theme.
    Light,
    /// Force dark theme.
    Dark,
    /// Tokyo Night — dark theme with purple/blue accent palette.
    TokyoNight,
    /// Solarized Dark — Ethan Schoonover's dark palette.
    SolarizedDark,
}

impl ThemeArg {
    /// Returns the canonical kebab-case name used on the CLI (same as clap's ValueEnum slug).
    fn slug(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::TokyoNight => "tokyo-night",
            Self::SolarizedDark => "solarized-dark",
        }
    }

    /// Human-readable description for --list-themes.
    fn description(self) -> &'static str {
        match self {
            Self::System => "Follow OS dark/light preference",
            Self::Light => "Light theme",
            Self::Dark => "Dark theme",
            Self::TokyoNight => "Tokyo Night — dark, purple/blue accents",
            Self::SolarizedDark => "Solarized Dark — base03 bg, yellow/cyan accents",
        }
    }
}

fn main() {
    let cli = Cli::parse();

    if cli.list_themes {
        println!("Available themes (pass with --theme <NAME>):");
        for t in [
            ThemeArg::System,
            ThemeArg::Light,
            ThemeArg::Dark,
            ThemeArg::TokyoNight,
            ThemeArg::SolarizedDark,
        ] {
            println!("  {:15} — {}", t.slug(), t.description());
        }
        return;
    }

    // FILE is required_unless_present = "list_themes", so unwrap is safe here.
    let file = cli
        .file
        .expect("FILE is required when --list-themes is not set");

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

    if let Err(e) = render::launch(&file, &config) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
