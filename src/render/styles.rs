//! Theme-adaptive style helpers for the markdown viewer.

use iced::border;
use iced::widget::container;
use iced::{Background, Color, Theme};

/// Theme-adaptive container style for fenced code blocks.
///
/// On light themes uses a warm gray background with high-contrast dark text;
/// on dark themes uses a slightly elevated surface with light text.
pub fn code_block_container_style(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    if palette.is_dark {
        // Dark themes: slightly lighter than page background, light text
        container::Style {
            background: Some(Background::Color(Color::from_rgb(0.14, 0.15, 0.18))),
            text_color: Some(Color::from_rgb(0.87, 0.89, 0.93)),
            border: border::rounded(6),
            ..container::Style::default()
        }
    } else {
        // Light themes: distinct cool-gray background, dark text for readability
        container::Style {
            background: Some(Background::Color(Color::from_rgb(0.95, 0.96, 0.97))),
            text_color: Some(Color::from_rgb(0.13, 0.14, 0.16)),
            border: border::rounded(6),
            ..container::Style::default()
        }
    }
}
