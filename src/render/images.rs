//! Image caching, async loading, and mermaid diagram rendering.

use std::path::{Path, PathBuf};

use iced::Task;
use iced::widget::{image as image_widget, markdown, svg};

use smdr::markdown as md_helpers;

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
                "smdr — {}",
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
    // Pre-render mermaid diagrams into the cache (async, off UI thread)
    let mermaid_task = spawn_mermaid_loads(app);

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

    if image_urls.is_empty() && mermaid_task.is_none() {
        return Task::none();
    }

    // Mark all URLs as pending
    for url in &image_urls {
        app.image_pending.insert(url.clone());
    }

    let base_dir = app.base_dir.clone();
    let network_enabled = app.network_enabled;

    let image_tasks: Vec<Task<Message>> = image_urls
        .into_iter()
        .map(move |url| {
            let base = base_dir.clone();
            let net = network_enabled;
            Task::perform(
                async move { load_image_async(&url, &base, net).await },
                |(url, data)| Message::ImageLoaded(url, data),
            )
        })
        .collect();

    let mut all_tasks: Vec<Task<Message>> = image_tasks;
    if let Some(mt) = mermaid_task {
        all_tasks.push(mt);
    }
    Task::batch(all_tasks)
}

/// Maximum pixmap dimension (per side) to prevent memory blowup.
const MAX_PIXMAP_DIM: u32 = 4096;

/// Collect all mermaid code blocks from the markdown and spawn async
/// rasterization tasks for those not yet cached.
///
/// Rasterization runs on a blocking thread to avoid freezing the UI.
pub(super) fn spawn_mermaid_loads(app: &mut MdrApp) -> Option<Task<Message>> {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

    let parser = Parser::new_ext(&app.raw_markdown, Options::all());
    let mut in_mermaid = false;
    let mut code_buf = String::new();
    let mut to_render: Vec<String> = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) if &*lang == "mermaid" => {
                in_mermaid = true;
                code_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) if in_mermaid => {
                in_mermaid = false;
                if !app.mermaid_cache.contains_key(&code_buf)
                    && !app.mermaid_pending.contains(&code_buf)
                {
                    app.mermaid_pending.insert(code_buf.clone());
                    to_render.push(code_buf.clone());
                }
            }
            Event::Text(t) if in_mermaid => {
                code_buf.push_str(&t);
            }
            _ => {}
        }
    }

    if to_render.is_empty() {
        return None;
    }

    let task = Task::batch(to_render.into_iter().map(|code| {
        let code_clone = code.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || rasterize_mermaid(&code))
                    .await
                    .map_err(|_| ())
                    .and_then(|inner| inner)
            },
            move |result| Message::MermaidRendered(code_clone, result.ok()),
        )
    }));

    Some(task)
}

/// Render a mermaid diagram source to PNG bytes.
///
/// 1. Generate SVG via `mermaid_rs_renderer::render`.
/// 2. Parse the SVG with `usvg`.
/// 3. Rasterize to a `tiny_skia::Pixmap` at 2× scale (capped).
/// 4. Encode as PNG.
///
/// # Safety bounds
/// Pixmap dimensions are capped at [`MAX_PIXMAP_DIM`]×[`MAX_PIXMAP_DIM`] to
/// prevent memory exhaustion on very large diagrams.
fn rasterize_mermaid(code: &str) -> Result<Vec<u8>, ()> {
    let svg_str = mermaid_rs_renderer::render(code).map_err(|_| ())?;

    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts();
    let options = usvg::Options {
        fontdb: std::sync::Arc::new(fontdb),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(svg_str.as_bytes(), &options).map_err(|_| ())?;

    let tree_size = tree.size();
    let scale = 2.0_f32;
    let mut px_w = (tree_size.width() * scale).ceil() as u32;
    let mut px_h = (tree_size.height() * scale).ceil() as u32;

    // Cap dimensions to prevent memory blowup; adjust scale if needed.
    let actual_scale = if px_w > MAX_PIXMAP_DIM || px_h > MAX_PIXMAP_DIM {
        let max_dim = px_w.max(px_h) as f32;
        let adjusted = (MAX_PIXMAP_DIM as f32 / max_dim) * scale;
        px_w = (tree_size.width() * adjusted).ceil() as u32;
        px_h = (tree_size.height() * adjusted).ceil() as u32;
        adjusted
    } else {
        scale
    };

    // Guard against zero-size pixmaps.
    if px_w == 0 || px_h == 0 {
        return Err(());
    }

    let mut pixmap = tiny_skia::Pixmap::new(px_w, px_h).ok_or(())?;
    let transform = tiny_skia::Transform::from_scale(actual_scale, actual_scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap.encode_png().map_err(|_| ())
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

    // Create the iced Handle **once** here so its internal Id is stable across
    // frames.  Calling from_bytes / from_memory inside build_ui every frame
    // generates a fresh Id each time, defeating iced's decode cache and
    // re-decoding the image on every frame (CPU 100%).
    if is_svg {
        Some(ImageData::Svg(svg::Handle::from_memory(bytes)))
    } else {
        Some(ImageData::Raster(image_widget::Handle::from_bytes(bytes)))
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
