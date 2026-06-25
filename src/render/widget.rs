//! Custom markdown::Viewer implementation for image and code block rendering.

use std::collections::{HashMap, HashSet};

use iced::widget::{
    column, container, image as image_widget, markdown, rich_text, scrollable, svg, text,
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
}

impl<'a, 'b: 'a> markdown::Viewer<'a, markdown::Uri, Theme, Renderer> for MdrViewer<'b> {
    fn on_link_click(url: markdown::Uri) -> markdown::Uri {
        url
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
                return container(
                    svg(handle.clone())
                        .content_fit(ContentFit::ScaleDown)
                        .width(Length::Fill)
                        .height(Length::Shrink),
                )
                .max_width(MAX_IMAGE_WIDTH)
                .center_x(Length::Fill)
                .padding(settings.spacing.0)
                .style(code_block_container_style)
                .into();
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
