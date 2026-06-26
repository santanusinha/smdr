//! Custom markdown::Viewer implementation for image and code block rendering.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use iced::widget::{
    column, container, image as image_widget, markdown, rich_text, scrollable, span, svg, text,
};
use iced::{Color, ContentFit, Element, Length, Renderer, Theme};

use super::state::{ImageData, MAX_IMAGE_WIDTH};
use super::styles::code_block_container_style;

pub(super) struct MdrViewer<'b> {
    pub(super) image_cache: &'b HashMap<String, ImageData>,
    pub(super) image_pending: &'b HashSet<String>,
    pub(super) image_failed: &'b HashSet<String>,
    pub(super) mermaid_cache: &'b HashMap<String, svg::Handle>,
    pub(super) mermaid_pending: &'b HashSet<String>,
    /// Active search query for in-text highlighting.  Empty string means no
    /// highlighting is applied.
    pub(super) search_query: &'b str,
}

impl<'a, 'b: 'a> markdown::Viewer<'a, markdown::Uri, Theme, Renderer> for MdrViewer<'b> {
    fn on_link_click(url: markdown::Uri) -> markdown::Uri {
        url
    }

    fn paragraph(
        &self,
        settings: markdown::Settings,
        text: &markdown::Text,
    ) -> Element<'a, markdown::Uri, Theme, Renderer> {
        let spans = text.spans(settings.style);
        if self.search_query.is_empty() {
            return rich_text(spans)
                .size(settings.text_size)
                .on_link_click(Self::on_link_click)
                .into();
        }
        let highlighted = apply_search_highlights(&spans, self.search_query);
        rich_text(highlighted)
            .size(settings.text_size)
            .on_link_click(Self::on_link_click)
            .into()
    }

    fn heading(
        &self,
        settings: markdown::Settings,
        level: &'a markdown::HeadingLevel,
        text: &'a markdown::Text,
        index: usize,
    ) -> Element<'a, markdown::Uri, Theme, Renderer> {
        let spans = text.spans(settings.style);
        if self.search_query.is_empty() {
            return markdown::heading(settings, level, text, index, Self::on_link_click);
        }
        let highlighted = apply_search_highlights(&spans, self.search_query);
        let size = heading_size(settings, level);
        rich_text(highlighted)
            .size(size)
            .on_link_click(Self::on_link_click)
            .into()
    }

    fn image(
        &self,
        settings: markdown::Settings,
        url: &'a markdown::Uri,
        _title: &'a str,
        alt: &markdown::Text,
    ) -> Element<'a, markdown::Uri, Theme, Renderer> {
        if let Some(img_data) = self.image_cache.get(url.as_str()) {
            match img_data {
                ImageData::Svg(handle) => container(
                    svg(handle.clone())
                        .content_fit(ContentFit::Contain)
                        .width(Length::Fill)
                        .height(Length::Shrink),
                )
                .max_width(MAX_IMAGE_WIDTH)
                .center_x(Length::Fill)
                .padding(settings.spacing.0)
                .into(),
                ImageData::Raster(handle) => container(
                    image_widget(handle.clone())
                        .content_fit(ContentFit::ScaleDown)
                        .width(Length::Shrink)
                        .height(Length::Shrink),
                )
                .max_width(MAX_IMAGE_WIDTH)
                .center_x(Length::Fill)
                .padding(settings.spacing.0)
                .into(),
            }
        } else if self.image_failed.contains(url.as_str()) {
            container(
                text("⚠ Failed to load image")
                    .size(13)
                    .color(Color::from_rgb(0.7, 0.3, 0.3)),
            )
            .padding(settings.spacing.0)
            .into()
        } else if self.image_pending.contains(url.as_str()) {
            container(
                text("⏳ Loading image…")
                    .size(13)
                    .color(Color::from_rgb(0.5, 0.5, 0.5)),
            )
            .padding(settings.spacing.0)
            .into()
        } else {
            // Fallback: show alt text
            container(rich_text(alt.spans(settings.style)).on_link_click(Self::on_link_click))
                .padding(settings.spacing.0)
                .into()
        }
    }

    fn code_block(
        &self,
        settings: markdown::Settings,
        language: Option<&'a str>,
        code: &'a str,
        lines: &'a [markdown::Text],
    ) -> Element<'a, markdown::Uri, Theme, Renderer> {
        // Mermaid diagram rendering (use cached handle to avoid re-decoding every frame)
        if language == Some("mermaid") {
            if let Some(handle) = self.mermaid_cache.get(code) {
                let svg_content = container(
                    svg(handle.clone())
                        .content_fit(ContentFit::ScaleDown)
                        .width(Length::Fill)
                        .height(Length::Shrink),
                )
                .max_width(MAX_IMAGE_WIDTH)
                .center_x(Length::Fill)
                .padding(settings.spacing.0)
                .style(code_block_container_style);

                let btn = iced::widget::button(svg_content)
                    .on_press(markdown::Uri::from(format!(
                        "smdr-mermaid:{}",
                        urlencoding::encode(code)
                    )))
                    .padding(0)
                    .style(iced::widget::button::text);

                return iced::widget::container(btn).into();
            } else if self.mermaid_pending.contains(code) {
                return container(
                    text("⏳ Rendering diagram…")
                        .size(13)
                        .color(Color::from_rgb(0.5, 0.5, 0.5)),
                )
                .padding(settings.spacing.0)
                .style(code_block_container_style)
                .into();
            }
        }

        container(
            scrollable(
                container(column(lines.iter().map(|line| {
                    rich_text(line.spans(settings.style))
                        .on_link_click(Self::on_link_click)
                        .font(settings.style.code_block_font)
                        .size(settings.code_size)
                        .into()
                })))
                .padding(settings.code_size),
            )
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::default()
                    .width(settings.code_size / 2)
                    .scroller_width(settings.code_size / 2),
            )),
        )
        .width(Length::Fill)
        .padding(settings.code_size / 4)
        .style(code_block_container_style)
        .into()
    }
}

// ---------------------------------------------------------------------------
// Search highlighting helpers
// ---------------------------------------------------------------------------

/// Background colour used for search-match highlights.
const HIGHLIGHT_BG: Color = Color {
    r: 1.0,
    g: 0.85,
    b: 0.0,
    a: 0.55,
};

/// Foreground colour override for highlighted text (dark for legibility).
const HIGHLIGHT_FG: Color = Color {
    r: 0.10,
    g: 0.10,
    b: 0.10,
    a: 1.0,
};

/// Walk the existing `spans` and split each one at every case-insensitive
/// occurrence of `query`, wrapping matches in a yellow highlight.
///
/// Returns a `Vec` of owned `'static` spans ready for `rich_text(…)`.
fn apply_search_highlights(
    spans: &Arc<[iced::widget::text::Span<'static, markdown::Uri>]>,
    query: &str,
) -> Vec<iced::widget::text::Span<'static, markdown::Uri>> {
    let query_lower = query.to_lowercase();
    let mut result = Vec::new();

    for original in spans.iter() {
        let text_str: &str = &original.text;
        if text_str.is_empty() {
            result.push(clone_span(original, Cow::Borrowed("")));
            continue;
        }

        let text_lower = text_str.to_lowercase();
        let mut cursor = 0usize;

        while let Some(rel) = text_lower[cursor..].find(&query_lower) {
            let match_start = cursor + rel;
            let match_end = match_start + query.len();

            // Emit text before the match (if any)
            if match_start > cursor {
                let before: String = text_str[cursor..match_start].to_owned();
                result.push(clone_span(original, Cow::Owned(before)));
            }

            // Emit the highlighted match
            let matched: String = text_str[match_start..match_end].to_owned();
            let mut hi_span = clone_span(original, Cow::Owned(matched));
            hi_span.color = Some(HIGHLIGHT_FG);
            hi_span = hi_span.background(iced::Background::Color(HIGHLIGHT_BG));
            hi_span = hi_span.border(iced::border::rounded(2));
            result.push(hi_span);

            cursor = match_end;
        }

        // Emit remaining text after the last match
        if cursor < text_str.len() {
            let tail: String = text_str[cursor..].to_owned();
            result.push(clone_span(original, Cow::Owned(tail)));
        }
    }

    result
}

/// Clone a span, replacing its text fragment.
fn clone_span(
    src: &iced::widget::text::Span<'static, markdown::Uri>,
    text: Cow<'static, str>,
) -> iced::widget::text::Span<'static, markdown::Uri> {
    let mut sp = span(text)
        .font_maybe(src.font)
        .color_maybe(src.color)
        .link_maybe(src.link.clone())
        .underline(src.underline)
        .strikethrough(src.strikethrough)
        .background_maybe(src.highlight.map(|h| h.background))
        .border_maybe(src.highlight.map(|h| h.border))
        .padding(src.padding);
    if let Some(s) = src.size {
        sp = sp.size(s);
    }
    sp
}

/// Return the appropriate text size for a heading level, mirroring the
/// default sizes that `markdown::heading` uses.
fn heading_size(settings: markdown::Settings, level: &markdown::HeadingLevel) -> iced::Pixels {
    use markdown::HeadingLevel;
    match level {
        HeadingLevel::H1 => settings.h1_size,
        HeadingLevel::H2 => settings.h2_size,
        HeadingLevel::H3 => settings.h3_size,
        HeadingLevel::H4 => settings.h4_size,
        HeadingLevel::H5 => settings.h5_size,
        HeadingLevel::H6 => settings.h6_size,
    }
}
