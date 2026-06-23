//! Tests for the ThemeArg type — conversion, enumeration, display.

use smdr::theme::ThemeArg;

#[test]
fn theme_arg_to_theme_conversion() {
    use iced::Theme;
    assert_eq!(ThemeArg::Light.to_theme(), Theme::Light);
    assert_eq!(ThemeArg::Dark.to_theme(), Theme::Dark);
    assert_eq!(ThemeArg::Dracula.to_theme(), Theme::Dracula);
    assert_eq!(ThemeArg::Nord.to_theme(), Theme::Nord);
    assert_eq!(ThemeArg::TokyoNight.to_theme(), Theme::TokyoNight);
    assert_eq!(ThemeArg::CatppuccinMocha.to_theme(), Theme::CatppuccinMocha);
    assert_eq!(ThemeArg::Ferra.to_theme(), Theme::Ferra);
}

#[test]
fn theme_arg_all_contains_all_variants() {
    assert_eq!(ThemeArg::ALL.len(), 23);
}

#[test]
fn theme_arg_display() {
    assert_eq!(ThemeArg::System.to_string(), "System");
    assert_eq!(ThemeArg::SolarizedLight.to_string(), "Solarized Light");
    assert_eq!(
        ThemeArg::CatppuccinFrappe.to_string(),
        "Catppuccin Frapp\u{e9}"
    );
}
