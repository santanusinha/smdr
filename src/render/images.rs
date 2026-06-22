//! Image caching, async loading, and mermaid diagram rendering.

use std::path::{Path, PathBuf};

use iced::Task;
use iced::widget::markdown;

use mdr::markdown as md_helpers;

use super::state::{ImageData, MdrApp, Message};

// ---------------------------------------------------------------------------
// File loading (also triggers image loading)
// ---------------------------------------------------------------------------

/// Load a markdown file, parse it, and spawn image loading tasks.
pub(super) fn load_file(app: &mut MdrApp, path: &Path) -> Task<Message> {
    match std::fs::read_to_string(path) {
        Ok(src) => {
            app.links = md_helpers::extract_links(&src);
            app.toc = md_helpers::extract_toc(&src);
            app.focused_link = None;
            app.raw_markdown = src;
            app.content = markdown::Content::parse(&app.raw_markdown);
            app.file_path = path.to_path_buf();
            app.base_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            app.title = format!(
                "mdr — {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            );
            app.search_hits.clear();
            app.current_hit = None;
            spawn_image_loads(app)
        }
        Err(e) => {
            eprintln!("Warning: could not read '{}': {e}", path.display());
            Task::none()
        }
    }
}

// ---------------------------------------------------------------------------
// Image loading
// ---------------------------------------------------------------------------

/// Spawn async image loading tasks for all images in the current content.
pub(super) fn spawn_image_loads(app: &mut MdrApp) -> Task<Message> {
    // Pre-render mermaid diagrams into the cache
    prerender_mermaid(app);

    let image_urls: Vec<String> = app
        .content
        .images()
        .iter()
        .filter(|u| {
            let s = u.as_str();
            !app.image_cache.contains_key(s) && !app.image_failed.contains(s)
        })
        .map(|u| u.as_str().to_owned())
        .collect();

    if image_urls.is_empty() {
        return Task::none();
    }

    // Mark all URLs as pending
    for url in &image_urls {
        app.image_pending.insert(url.clone());
    }

    let base_dir = app.base_dir.clone();
    let network_enabled = app.network_enabled;

    Task::batch(image_urls.into_iter().map(move |url| {
        let base = base_dir.clone();
        let net = network_enabled;
        Task::perform(
            async move { load_image_async(&url, &base, net).await },
            |(url, data)| Message::ImageLoaded(url, data),
        )
    }))
}

/// Pre-render all mermaid code blocks and cache their SVG output.
pub(super) fn prerender_mermaid(app: &mut MdrApp) {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

    let parser = Parser::new_ext(&app.raw_markdown, Options::all());
    let mut in_mermaid = false;
    let mut code_buf = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                if lang.as_ref() == "mermaid" =>
            {
                in_mermaid = true;
                code_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) if in_mermaid => {
                in_mermaid = false;
                if !app.mermaid_cache.contains_key(&code_buf)
                    && let Ok(svg_str) = mermaid_rs_renderer::render(&code_buf)
                {
                    app.mermaid_cache
                        .insert(code_buf.clone(), svg_str.into_bytes());
                }
            }
            Event::Text(t) if in_mermaid => {
                code_buf.push_str(&t);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Async image loading helpers
// ---------------------------------------------------------------------------

/// Load an image from a local file path or network URL.
///
/// Returns `(url, Some(ImageData))` on success, `(url, None)` on failure.
pub(super) async fn load_image_async(
    url: &str,
    base_dir: &Path,
    network_enabled: bool,
) -> (String, Option<ImageData>) {
    let result = load_image_inner(url, base_dir, network_enabled).await;
    (url.to_owned(), result)
}

async fn load_image_inner(url: &str, base_dir: &Path, network_enabled: bool) -> Option<ImageData> {
    let is_remote = url.starts_with("http://") || url.starts_with("https://");

    let bytes = if is_remote {
        if !network_enabled {
            return None;
        }
        reqwest::get(url).await.ok()?.bytes().await.ok()?.to_vec()
    } else {
        // Local file: resolve relative to base_dir
        let path = if Path::new(url).is_absolute() {
            PathBuf::from(url)
        } else {
            base_dir.join(url)
        };
        std::fs::read(&path).ok()?
    };

    // Detect SVG by content or extension
    let is_svg = url.ends_with(".svg")
        || bytes.starts_with(b"<?xml")
        || bytes.starts_with(b"<svg")
        || bytes.windows(4).take(256).any(|w| w == b"<svg");

    if is_svg {
        Some(ImageData::Svg(bytes))
    } else {
        Some(ImageData::Raster(bytes))
    }
}
// ---------------------------------------------------------------------------
// File watcher polling
// ---------------------------------------------------------------------------

/// Poll the file-watcher channel; reload the file if it has changed.
pub(super) fn poll_watcher(app: &mut MdrApp) -> Task<Message> {
    let Some(ref rx) = app.watcher_rx else {
        return Task::none();
    };
    if rx.try_recv().is_ok() {
        while rx.try_recv().is_ok() {}
        match std::fs::read_to_string(&app.file_path) {
            Ok(new_content) => {
                app.links = md_helpers::extract_links(&new_content);
                app.toc = md_helpers::extract_toc(&new_content);
                app.focused_link = None;
                app.raw_markdown = new_content;
                app.content = markdown::Content::parse(&app.raw_markdown);
                return spawn_image_loads(app);
            }
            Err(e) => eprintln!("Warning: could not reload file: {e}"),
        }
    }
    Task::none()
}
