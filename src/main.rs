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
    /// Light theme.
    Light,
    /// Dark theme.
    Dark,
    /// Dracula — popular dark theme with vibrant colors.
    Dracula,
    /// Nord — arctic, north-bluish color palette.
    Nord,
    /// Solarized Light — Ethan Schoonover's light palette.
    SolarizedLight,
    /// Solarized Dark — Ethan Schoonover's dark palette.
    SolarizedDark,
    /// Gruvbox Light — retro groove light palette.
    GruvboxLight,
    /// Gruvbox Dark — retro groove dark palette.
    GruvboxDark,
    /// Catppuccin Latte — soothing pastel, light variant.
    CatppuccinLatte,
    /// Catppuccin Frappé — soothing pastel, mid-dark variant.
    CatppuccinFrappe,
    /// Catppuccin Macchiato — soothing pastel, darker variant.
    CatppuccinMacchiato,
    /// Catppuccin Mocha — soothing pastel, darkest variant.
    CatppuccinMocha,
    /// Tokyo Night — dark theme with purple/blue accent palette.
    TokyoNight,
    /// Tokyo Night Storm — slightly lighter Tokyo Night variant.
    TokyoNightStorm,
    /// Tokyo Night Light — light variant of Tokyo Night.
    TokyoNightLight,
    /// Kanagawa Wave — dark blue inspired by Katsushika Hokusai.
    KanagawaWave,
    /// Kanagawa Dragon — darker Kanagawa variant.
    KanagawaDragon,
    /// Kanagawa Lotus — light Kanagawa variant.
    KanagawaLotus,
    /// Moonfly — dark theme with emerald accents.
    Moonfly,
    /// Nightfly — dark theme with blue accents.
    Nightfly,
    /// Oxocarbon — IBM Carbon-inspired dark theme.
    Oxocarbon,
    /// Ferra — warm, muted dark theme.
    Ferra,
}

impl ThemeArg {
    /// All available theme variants.
    pub const ALL: &'static [Self] = &[
        Self::System,
        Self::Light,
        Self::Dark,
        Self::Dracula,
        Self::Nord,
        Self::SolarizedLight,
        Self::SolarizedDark,
        Self::GruvboxLight,
        Self::GruvboxDark,
        Self::CatppuccinLatte,
        Self::CatppuccinFrappe,
        Self::CatppuccinMacchiato,
        Self::CatppuccinMocha,
        Self::TokyoNight,
        Self::TokyoNightStorm,
        Self::TokyoNightLight,
        Self::KanagawaWave,
        Self::KanagawaDragon,
        Self::KanagawaLotus,
        Self::Moonfly,
        Self::Nightfly,
        Self::Oxocarbon,
        Self::Ferra,
    ];

    /// Convert to an `iced::Theme`.
    pub fn to_theme(self) -> iced::Theme {
        use iced::Theme;
        match self {
            Self::System => Theme::Light,
            Self::Light => Theme::Light,
            Self::Dark => Theme::Dark,
            Self::Dracula => Theme::Dracula,
            Self::Nord => Theme::Nord,
            Self::SolarizedLight => Theme::SolarizedLight,
            Self::SolarizedDark => Theme::SolarizedDark,
            Self::GruvboxLight => Theme::GruvboxLight,
            Self::GruvboxDark => Theme::GruvboxDark,
            Self::CatppuccinLatte => Theme::CatppuccinLatte,
            Self::CatppuccinFrappe => Theme::CatppuccinFrappe,
            Self::CatppuccinMacchiato => Theme::CatppuccinMacchiato,
            Self::CatppuccinMocha => Theme::CatppuccinMocha,
            Self::TokyoNight => Theme::TokyoNight,
            Self::TokyoNightStorm => Theme::TokyoNightStorm,
            Self::TokyoNightLight => Theme::TokyoNightLight,
            Self::KanagawaWave => Theme::KanagawaWave,
            Self::KanagawaDragon => Theme::KanagawaDragon,
            Self::KanagawaLotus => Theme::KanagawaLotus,
            Self::Moonfly => Theme::Moonfly,
            Self::Nightfly => Theme::Nightfly,
            Self::Oxocarbon => Theme::Oxocarbon,
            Self::Ferra => Theme::Ferra,
        }
    }

    /// CLI-friendly slug (kebab-case name).
    pub fn slug(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Dracula => "dracula",
            Self::Nord => "nord",
            Self::SolarizedLight => "solarized-light",
            Self::SolarizedDark => "solarized-dark",
            Self::GruvboxLight => "gruvbox-light",
            Self::GruvboxDark => "gruvbox-dark",
            Self::CatppuccinLatte => "catppuccin-latte",
            Self::CatppuccinFrappe => "catppuccin-frappe",
            Self::CatppuccinMacchiato => "catppuccin-macchiato",
            Self::CatppuccinMocha => "catppuccin-mocha",
            Self::TokyoNight => "tokyo-night",
            Self::TokyoNightStorm => "tokyo-night-storm",
            Self::TokyoNightLight => "tokyo-night-light",
            Self::KanagawaWave => "kanagawa-wave",
            Self::KanagawaDragon => "kanagawa-dragon",
            Self::KanagawaLotus => "kanagawa-lotus",
            Self::Moonfly => "moonfly",
            Self::Nightfly => "nightfly",
            Self::Oxocarbon => "oxocarbon",
            Self::Ferra => "ferra",
        }
    }

    /// Human-readable description for the theme.
    pub fn description(self) -> &'static str {
        match self {
            Self::System => "Follow OS dark/light preference",
            Self::Light => "Light theme",
            Self::Dark => "Dark theme",
            Self::Dracula => "Popular dark theme with vibrant colors",
            Self::Nord => "Arctic, north-bluish color palette",
            Self::SolarizedLight => "Ethan Schoonover's light palette",
            Self::SolarizedDark => "Ethan Schoonover's dark palette",
            Self::GruvboxLight => "Retro groove light palette",
            Self::GruvboxDark => "Retro groove dark palette",
            Self::CatppuccinLatte => "Soothing pastel, light variant",
            Self::CatppuccinFrappe => "Soothing pastel, mid-dark variant",
            Self::CatppuccinMacchiato => "Soothing pastel, darker variant",
            Self::CatppuccinMocha => "Soothing pastel, darkest variant",
            Self::TokyoNight => "Dark theme with purple/blue accents",
            Self::TokyoNightStorm => "Slightly lighter Tokyo Night",
            Self::TokyoNightLight => "Light variant of Tokyo Night",
            Self::KanagawaWave => "Dark blue inspired by Hokusai",
            Self::KanagawaDragon => "Darker Kanagawa variant",
            Self::KanagawaLotus => "Light Kanagawa variant",
            Self::Moonfly => "Dark theme with emerald accents",
            Self::Nightfly => "Dark theme with blue accents",
            Self::Oxocarbon => "IBM Carbon-inspired dark theme",
            Self::Ferra => "Warm, muted dark theme",
        }
    }
}

impl std::fmt::Display for ThemeArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
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
