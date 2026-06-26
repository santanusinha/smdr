//! Theme definitions and conversion to iced themes.

use serde::{Deserialize, Serialize};

/// CLI-parseable theme selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

    /// Human-readable display name for the theme (used in UI pick-list).
    pub fn display_name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::Dracula => "Dracula",
            Self::Nord => "Nord",
            Self::SolarizedLight => "Solarized Light",
            Self::SolarizedDark => "Solarized Dark",
            Self::GruvboxLight => "Gruvbox Light",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::CatppuccinLatte => "Catppuccin Latte",
            Self::CatppuccinFrappe => "Catppuccin Frapp\u{e9}",
            Self::CatppuccinMacchiato => "Catppuccin Macchiato",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::TokyoNight => "Tokyo Night",
            Self::TokyoNightStorm => "Tokyo Night Storm",
            Self::TokyoNightLight => "Tokyo Night Light",
            Self::KanagawaWave => "Kanagawa Wave",
            Self::KanagawaDragon => "Kanagawa Dragon",
            Self::KanagawaLotus => "Kanagawa Lotus",
            Self::Moonfly => "Moonfly",
            Self::Nightfly => "Nightfly",
            Self::Oxocarbon => "Oxocarbon",
            Self::Ferra => "Ferra",
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
        f.write_str(self.display_name())
    }
}
