//! egui/eframe viewer — window creation and markdown rendering.
//!
//! Replaces the old wry/tao/WebKit stack with an immediate-mode egui UI
//! backed by `eframe` (wgpu renderer) and `egui_commonmark` for markdown.
//!
//! Responsibilities:
//! - Create a native OS window via `eframe`.
//! - Render the markdown document using `egui_commonmark`.
//! - Poll the file-watcher channel and hot-reload on changes (`--watch`).
//! - Intercept link clicks: open external URLs in the browser, navigate local
//!   links within the viewer, and scroll to in-document `#anchor` fragments.
//! - Provide a menu bar for theme selection.
//! - Support vim-style navigation keys (h/j/k/l, Ctrl-U/D, arrows, PageUp/PageDown).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use mdr::watcher;

use crate::ThemeArg;

/// Configuration passed to [`launch`].
pub struct ViewerConfig {
    pub theme: ThemeArg,
    pub watch: bool,
    pub network_enabled: bool,
}

/// Launches the viewer window and blocks until it is closed.
///
/// # Errors
/// Returns an error if the window or file reader cannot be created.
pub fn launch(file_path: &Path, config: &ViewerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let markdown = std::fs::read_to_string(file_path)?;

    let title = format!(
        "mdr — {}",
        file_path.file_name().unwrap_or_default().to_string_lossy()
    );

    let watcher_rx: Option<Receiver<()>> = if config.watch {
        match watcher::watch_file(file_path) {
            Ok((w, rx)) => {
                Box::leak(Box::new(w));
                Some(rx)
            }
            Err(e) => {
                eprintln!("Warning: could not set up file watcher: {e}");
                None
            }
        }
    } else {
        None
    };

    let app = MdrApp::new(markdown, file_path.to_path_buf(), watcher_rx, config.theme);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size([960.0, 720.0])
            // Ensure the OS draws native window decorations (title bar, borders).
            // On Wayland this also triggers proper CSD negotiation with the compositor.
            .with_decorations(true)
            // Wayland app-id / X11 WM_CLASS — used by the compositor to pick the
            // correct theme and to match .desktop files.
            .with_app_id("mdr"),
        ..Default::default()
    };

    let theme = config.theme;
    let network_enabled = config.network_enabled;

    eframe::run_native(
        &title,
        native_options,
        Box::new(move |cc| {
            // ── Fonts ──────────────────────────────────────────────────────
            setup_fonts(&cc.egui_ctx);

            // ── Colour scheme ──────────────────────────────────────────────
            apply_theme(&cc.egui_ctx, theme);

            // ── Image loaders ─────────────────────────────────────────────
            // When --no-network is set we skip the HTTP loader so that remote
            // image URLs are silently ignored rather than fetched.
            if network_enabled {
                egui_extras::install_image_loaders(&cc.egui_ctx);
            } else {
                let ctx = &cc.egui_ctx;
                use egui_extras::loaders::{
                    file_loader::FileLoader, image_loader::ImageCrateLoader, svg_loader::SvgLoader,
                };
                if !ctx.is_loader_installed(FileLoader::ID) {
                    ctx.add_bytes_loader(std::sync::Arc::new(FileLoader::default()));
                }
                if !ctx.is_loader_installed(ImageCrateLoader::ID) {
                    ctx.add_image_loader(std::sync::Arc::new(ImageCrateLoader::default()));
                }
                if !ctx.is_loader_installed(SvgLoader::ID) {
                    ctx.add_image_loader(std::sync::Arc::new(SvgLoader::default()));
                }
            }

            Ok(Box::new(app))
        }),
    )
    .map_err(|e| e.to_string().into())
}

// ---------------------------------------------------------------------------
// Theme application
// ---------------------------------------------------------------------------

/// Apply the requested theme to the egui context.
///
/// - `System` delegates to egui's OS-preference detection via
///   [`egui::ThemePreference::System`], which respects the desktop's
///   dark/light mode automatically.
/// - `Light` / `Dark` pin egui to the built-in palettes.
/// - `TokyoNight` / `SolarizedDark` set `ThemePreference::Dark` first
///   (so widgets use dark defaults), then override the panel/code colours
///   with the custom palette.
fn apply_theme(ctx: &egui::Context, theme: ThemeArg) {
    match theme {
        ThemeArg::System => {
            ctx.set_theme(egui::ThemePreference::System);
        }
        ThemeArg::Light => {
            ctx.set_theme(egui::ThemePreference::Light);
        }
        ThemeArg::Dark => {
            ctx.set_theme(egui::ThemePreference::Dark);
        }
        ThemeArg::TokyoNight => {
            ctx.set_theme(egui::ThemePreference::Dark);
            apply_custom_visuals(ctx, tokyo_night_visuals());
        }
        ThemeArg::SolarizedDark => {
            ctx.set_theme(egui::ThemePreference::Dark);
            apply_custom_visuals(ctx, solarized_dark_visuals());
        }
    }
}

/// Overlay custom `Visuals` fields on top of the dark defaults.
/// Using `set_visuals` here is intentional: the custom themes ARE fixed
/// palettes, not OS-adaptive, so we want to pin their colours.
fn apply_custom_visuals(ctx: &egui::Context, visuals: egui::Visuals) {
    ctx.set_visuals(visuals);
}

/// Tokyo Night colour palette.
///
/// Reference: <https://github.com/enkia/tokyo-night-vscode-theme>
///
/// Key hex values:
/// - Background  `#1a1b26`  (night.background)
/// - Surface     `#16161e`  (night.black)
/// - Foreground  `#a9b1d6`  (night.foreground)
/// - Blue        `#7aa2f7`
/// - Purple      `#bb9af7`
/// - Cyan        `#7dcfff`
/// - Red         `#f7768e`
/// - Yellow      `#e0af68`
fn tokyo_night_visuals() -> egui::Visuals {
    use egui::Color32;

    let bg = Color32::from_rgb(0x1a, 0x1b, 0x26);
    let surface = Color32::from_rgb(0x16, 0x16, 0x1e);
    let fg = Color32::from_rgb(0xa9, 0xb1, 0xd6);
    let blue = Color32::from_rgb(0x7a, 0xa2, 0xf7);
    let purple = Color32::from_rgb(0xbb, 0x9a, 0xf7);
    let subtle = Color32::from_rgb(0x24, 0x28, 0x3b); // selection/hover bg

    let mut v = egui::Visuals::dark();
    v.panel_fill = bg;
    v.window_fill = bg;
    v.extreme_bg_color = surface;
    v.code_bg_color = surface;
    v.faint_bg_color = subtle;
    v.hyperlink_color = blue;
    v.selection.bg_fill = purple.gamma_multiply(0.35);
    v.selection.stroke = egui::Stroke::new(1.0, purple);

    // Widget colours — inactive, hovered, active states.
    v.widgets.noninteractive.bg_fill = subtle;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, fg);
    v.widgets.inactive.bg_fill = subtle;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, fg);
    v.widgets.hovered.bg_fill = purple.gamma_multiply(0.25);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, blue);
    v.widgets.active.bg_fill = purple.gamma_multiply(0.45);
    v.widgets.active.fg_stroke = egui::Stroke::new(2.0, blue);

    v
}

/// Solarized Dark colour palette.
///
/// Reference: <https://ethanschoonover.com/solarized/>
///
/// Key hex values:
/// - base03  `#002b36`  (darkest bg)
/// - base02  `#073642`  (bg highlight)
/// - base01  `#586e75`  (comments / secondary fg)
/// - base0   `#839496`  (body text)
/// - yellow  `#b58900`
/// - cyan    `#2aa198`
/// - blue    `#268bd2`
/// - violet  `#6c71c4`
fn solarized_dark_visuals() -> egui::Visuals {
    use egui::Color32;

    let base03 = Color32::from_rgb(0x00, 0x2b, 0x36);
    let base02 = Color32::from_rgb(0x07, 0x36, 0x42);
    let base0 = Color32::from_rgb(0x83, 0x94, 0x96);
    let yellow = Color32::from_rgb(0xb5, 0x89, 0x00);
    let cyan = Color32::from_rgb(0x2a, 0xa1, 0x98);
    let blue = Color32::from_rgb(0x26, 0x8b, 0xd2);
    let violet = Color32::from_rgb(0x6c, 0x71, 0xc4);

    let mut v = egui::Visuals::dark();
    v.panel_fill = base03;
    v.window_fill = base03;
    v.extreme_bg_color = base02;
    v.code_bg_color = base02;
    v.faint_bg_color = base02;
    v.hyperlink_color = cyan;
    v.selection.bg_fill = blue.gamma_multiply(0.35);
    v.selection.stroke = egui::Stroke::new(1.0, yellow);

    v.widgets.noninteractive.bg_fill = base02;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, base0);
    v.widgets.inactive.bg_fill = base02;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, base0);
    v.widgets.hovered.bg_fill = violet.gamma_multiply(0.25);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, cyan);
    v.widgets.active.bg_fill = violet.gamma_multiply(0.45);
    v.widgets.active.fg_stroke = egui::Stroke::new(2.0, blue);

    v
}

// ---------------------------------------------------------------------------
// Font setup
// ---------------------------------------------------------------------------

/// Name used to register the JetBrainsMonoNL Nerd Font inside egui's font registry.
const JBMONO_FONT_NAME: &str = "JetBrainsMonoNL-NF";

/// NotoEmoji v2 (outline/glyf format, ~60% emoji-block coverage).
///
/// egui's bundled `NotoEmoji-Regular` is v1.05 (408 KB, ~37% coverage) — it
/// silently drops many common emoji such as 🦀 U+1F980.  We replace it by
/// inserting our own bytes under the same font-registry key so the existing
/// fallback lists in every `FontFamily` keep working without change.
///
/// **Why not `NotoColorEmoji.ttf`?**  That font stores glyphs as CBDT colour
/// bitmaps.  epaint's rasteriser (skrifa + vello_cpu) only follows outline
/// (`glyf`/`CFF`) code paths; CBDT and COLR tables are ignored.  Colour emoji
/// rendering is therefore a known egui limitation upstream of this crate.
///
/// The font file lives in `assets/fonts/` and is baked into the binary at
/// compile time, so it is always available regardless of the working directory.
static NOTO_EMOJI_V2: &[u8] = include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf");

/// Attempt to load a TTF file from disk; returns `None` and logs a warning on failure.
fn load_font_bytes(path: &std::path::Path) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            eprintln!("Warning: could not load font {}: {e}", path.display());
            None
        }
    }
}

/// Configures the font stack:
///
/// - Replaces egui's bundled NotoEmoji v1.05 with the v2 outline build
///   (baked in via [`NOTO_EMOJI_V2`]) which covers ~60% of the emoji block
///   including most commonly-used symbols (🦀 😀 🔥 ✨ etc.).
/// - Attempts to load **JetBrainsMonoNL Nerd Font** (Regular) from
///   `~/.local/share/fonts/` and registers it as the primary face for both
///   the `Monospace` and `Proportional` egui font families.
/// - NotoEmoji v2 stays as the last fallback in every family so codepoints
///   not present in the primary font fall through correctly.
/// - Heading sizes use a tight scale relative to body so visual hierarchy is
///   clear without the jump feeling jarring.
fn setup_fonts(ctx: &egui::Context) {
    use egui::{FontFamily, FontId, TextStyle};

    // ── 1. Font data ────────────────────────────────────────────────────────
    let mut fonts = egui::FontDefinitions::default();

    // Replace egui's bundled NotoEmoji v1.05 with our v2 build.
    // Inserting under the same key overwrites the Arc<FontData> that
    // FontDefinitions::default() already placed there, so all existing
    // family fallback lists that reference "NotoEmoji-Regular" now point
    // at the better font automatically.
    fonts.font_data.insert(
        "NotoEmoji-Regular".to_owned(),
        egui::FontData::from_static(NOTO_EMOJI_V2)
            .tweak(egui::FontTweak {
                scale: 0.81, // same scale as the epaint default — keeps glyph metrics comparable
                ..Default::default()
            })
            .into(),
    );

    // Resolve ~/.local/share/fonts at runtime so the path works for any user.
    let font_path = std::env::var("HOME").ok().map(|home| {
        std::path::PathBuf::from(home)
            .join(".local/share/fonts")
            .join("JetBrainsMonoNLNerdFont-Regular.ttf")
    });

    let loaded_jbmono = font_path
        .as_deref()
        .and_then(load_font_bytes)
        .map(egui::FontData::from_owned);

    if let Some(font_data) = loaded_jbmono {
        // Register the Nerd Font under our stable internal name.
        fonts
            .font_data
            .insert(JBMONO_FONT_NAME.to_owned(), font_data.into());

        // Place it first in both Proportional and Monospace families so all
        // text — body, headings, code spans alike — renders in JBMono NF.
        // NotoEmoji v2 remains as a fallback for codepoints the Nerd Font
        // does not cover.
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            let list = fonts.families.entry(family).or_default();
            list.insert(0, JBMONO_FONT_NAME.to_owned());
        }
    } else {
        // Fall back to egui's bundled Hack; NotoEmoji v2 is still in place.
        eprintln!("Info: JetBrainsMonoNL Nerd Font not found, falling back to bundled Hack.");
    }

    // Ensure NotoEmoji v2 is the last fallback in every family so
    // Unicode emoji codepoints get a glyph even if the primary font lacks them.
    for list in fonts.families.values_mut() {
        // Remove from wherever it is now (default puts it 2nd), then push to back.
        if let Some(idx) = list.iter().position(|s| s == "NotoEmoji-Regular") {
            list.remove(idx);
        }
        list.push("NotoEmoji-Regular".to_owned());
    }

    ctx.set_fonts(fonts);

    // ── 2. Text styles ──────────────────────────────────────────────────────
    // Body and code spans use Monospace (= JBMono NF when loaded).
    // Headings use Proportional so egui_commonmark can apply its own bold
    // rendering; sizes are kept close to body for a calm visual rhythm.
    //
    // Scale: Body 15 → H3 16.5 → H2 18.5 → H1 (Heading) 21
    let prop = FontFamily::Proportional;
    let mono = FontFamily::Monospace;
    let mut style = (*ctx.global_style()).clone();
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, mono.clone())),
        (TextStyle::Body, FontId::new(15.0, prop.clone())),
        (TextStyle::Button, FontId::new(15.0, prop.clone())),
        (TextStyle::Heading, FontId::new(21.0, prop.clone())),
        (TextStyle::Monospace, FontId::new(14.0, mono.clone())),
        // egui_commonmark checks for Name("Heading2") / Name("Heading3") and
        // uses them when present, giving per-level sizing.
        (
            TextStyle::Name("Heading2".into()),
            FontId::new(18.5, prop.clone()),
        ),
        (TextStyle::Name("Heading3".into()), FontId::new(16.5, prop)),
    ]
    .into();
    ctx.set_global_style(style);
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

/// Pixels scrolled per j / k / Arrow keypress.
const LINE_SCROLL: f32 = 40.0;

struct MdrApp {
    /// Current markdown source text.
    markdown: String,
    /// Path to the file on disk (for hot-reload).
    file_path: PathBuf,
    /// Parsed/cached state used by egui_commonmark across frames.
    cache: CommonMarkCache,
    /// Channel to receive file-change notifications.
    watcher_rx: Option<Receiver<()>>,
    /// Current vertical scroll position in logical pixels.
    ///
    /// Driven by vim-motion keys; kept in sync with the ScrollArea output so
    /// that mouse-wheel / drag scrolling is also reflected here.
    scroll_offset: f32,
    /// Currently active theme (can be changed at runtime via the menu bar).
    active_theme: ThemeArg,
    /// Markdown split at ATX heading boundaries.
    ///
    /// Each entry is `(slug, chunk)` where `slug` is the GitHub-style anchor
    /// for that section's heading and `chunk` is the markdown text for that
    /// section (heading line + body until the next heading).  The first entry
    /// has an empty slug when there is preamble content before the first heading.
    heading_sections: Vec<(String, String)>,
    /// Content-space Y coordinate (in logical pixels) of each heading anchor.
    ///
    /// Populated during the first rendered frame and updated every frame so
    /// that it stays accurate after font/layout changes.
    heading_y_positions: HashMap<String, f32>,
}

impl MdrApp {
    fn new(
        markdown: String,
        file_path: PathBuf,
        watcher_rx: Option<Receiver<()>>,
        theme: ThemeArg,
    ) -> Self {
        let heading_sections = parse_heading_sections(&markdown);
        let mut cache = CommonMarkCache::default();
        register_anchor_hooks(&mut cache, &heading_sections);
        Self {
            markdown,
            file_path,
            cache,
            watcher_rx,
            scroll_offset: 0.0,
            active_theme: theme,
            heading_sections,
            heading_y_positions: HashMap::new(),
        }
    }

    /// Load a new markdown document, replacing the current one.
    ///
    /// Resets the cache, re-parses headings, and re-registers anchor hooks.
    fn load_document(&mut self, content: String, path: PathBuf) {
        self.heading_sections = parse_heading_sections(&content);
        self.markdown = content;
        self.file_path = path;
        self.cache = CommonMarkCache::default();
        register_anchor_hooks(&mut self.cache, &self.heading_sections);
        self.scroll_offset = 0.0;
        self.heading_y_positions.clear();
    }

    /// Drain the watcher channel and reload the file if a change arrived.
    fn poll_watcher(&mut self) {
        let Some(ref rx) = self.watcher_rx else {
            return;
        };
        if rx.try_recv().is_ok() {
            // Drain duplicates.
            while rx.try_recv().is_ok() {}
            match std::fs::read_to_string(&self.file_path) {
                Ok(new_content) => {
                    let path = self.file_path.clone();
                    self.load_document(new_content, path);
                }
                Err(e) => eprintln!("Warning: could not reload file: {e}"),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// eframe::App implementation
// ---------------------------------------------------------------------------
impl eframe::App for MdrApp {
    /// The wgpu surface clear color — must match the active theme's window fill
    /// so that the dark default (12, 12, 12) eframe uses does not bleed through
    /// the transparent root [`egui::Ui`] that [`Self::ui`] receives.
    ///
    /// Without this override, `--theme light` produces white egui widgets but
    /// a near-black background because eframe's default `clear_color` ignores
    /// the `visuals` argument it receives.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.window_fill().to_normalized_gamma_f32()
    }

    /// Called each frame for non-UI work: poll the watcher and schedule
    /// the next repaint so hot-reload feels responsive.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_watcher();
        // Re-check every 500 ms even if the OS didn't push a watcher event
        // (handles editors that write via rename/replace).
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    /// Called each frame to paint the UI.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // ── Menu bar ──────────────────────────────────────────────────────
        egui::Panel::top("menubar").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Theme", |ui| {
                    const THEMES: &[(ThemeArg, &str)] = &[
                        (ThemeArg::System, "System"),
                        (ThemeArg::Light, "Light"),
                        (ThemeArg::Dark, "Dark"),
                        (ThemeArg::TokyoNight, "Tokyo Night"),
                        (ThemeArg::SolarizedDark, "Solarized Dark"),
                    ];
                    for &(theme, label) in THEMES {
                        if ui
                            .add(egui::Button::selectable(self.active_theme == theme, label))
                            .clicked()
                        {
                            self.active_theme = theme;
                            apply_theme(ui.ctx(), theme);
                            ui.close();
                        }
                    }
                });
            });
        });

        // ── Vim-motion keyboard navigation ────────────────────────────────
        let half_page = ui
            .ctx()
            .input(|i| i.viewport().inner_rect.map_or(360.0, |r| r.height() / 2.0));

        ui.ctx().input_mut(|i| {
            // j / l / ArrowDown — scroll down one line
            if i.consume_key(egui::Modifiers::NONE, egui::Key::J)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::L)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                self.scroll_offset += LINE_SCROLL;
            }
            // k / h / ArrowUp — scroll up one line
            if i.consume_key(egui::Modifiers::NONE, egui::Key::K)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::H)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            {
                self.scroll_offset -= LINE_SCROLL;
            }
            // Ctrl-D / PageDown — scroll down half a page
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::D)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::PageDown)
            {
                self.scroll_offset += half_page;
            }
            // Ctrl-U / PageUp — scroll up half a page
            if i.consume_key(egui::Modifiers::CTRL, egui::Key::U)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::PageUp)
            {
                self.scroll_offset -= half_page;
            }
        });

        self.scroll_offset = self.scroll_offset.max(0.0);

        // ── Markdown content ──────────────────────────────────────────────
        //
        // The document is split at ATX-heading boundaries so we can record
        // the content-space Y of each section before rendering it.  If an
        // anchor link was clicked this frame we update `scroll_offset` to
        // jump to that heading.
        let mut clicked_anchor: Option<String> = None;

        let scroll_output = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .vertical_scroll_offset(self.scroll_offset)
            .show(ui, |ui| {
                ui.set_max_width(860.0);

                // Y of the very first widget inside the ScrollArea (content space).
                // Because egui positions content at inner_rect.min - state.offset,
                // all widget positions inside this closure are already in content
                // coordinates — no manual scroll_offset addition is needed.
                let origin_y = ui.next_widget_position().y;

                for (index, (slug, chunk)) in self.heading_sections.iter().enumerate() {
                    // Record content-space Y before rendering this section.
                    let screen_y_before = ui.next_widget_position().y;
                    let content_y = screen_y_before - origin_y;
                    if !slug.is_empty() {
                        self.heading_y_positions.insert(slug.clone(), content_y);
                    }

                    // Each chunk gets its own ID namespace so that widgets
                    // rendered by CommonMarkViewer::show() don't clash across
                    // chunks (egui would warn "multiple uses of the same ID").
                    ui.push_id(index, |ui| {
                        CommonMarkViewer::new().show_alt_text_on_hover(true).show(
                            ui,
                            &mut self.cache,
                            chunk,
                        );

                        // Peek at link hooks after this chunk's render, before the
                        // next show() call resets them via prepare_show().
                        for (hook_slug, &activated) in self.cache.link_hooks() {
                            if activated {
                                clicked_anchor = Some(hook_slug.clone());
                            }
                        }
                    });
                }
            });

        // Keep our stored offset in sync with whatever the ScrollArea settled
        // on (mouse-wheel, drag, and clamping to content bounds all adjust it).
        self.scroll_offset = scroll_output.state.offset.y;

        // If a heading anchor was clicked, jump to it.
        if let Some(ref slug) = clicked_anchor
            && let Some(&target_y) = self.heading_y_positions.get(slug)
        {
            self.scroll_offset = target_y;
        }

        // ── Link handling ─────────────────────────────────────────────────
        // Drain every OutputCommand that egui_commonmark queued this frame.
        //
        // Routing logic:
        //   • http:// / https:// → open in the system browser.
        //   • Local paths (relative or file://) → resolve against the current
        //     file's directory, load the new markdown, reset state.
        //   • Fragment-only (#anchor) → handled via link_hooks above;
        //     drop to prevent egui-winit from forwarding them to the browser.
        //   • Any other OpenUrl (mailto:, unknown scheme) → drop silently.
        //   • Non-OpenUrl commands (CopyText, CopyImage) → re-queue so egui
        //     can process them normally.
        let commands: Vec<_> = ui.ctx().output_mut(|o| std::mem::take(&mut o.commands));
        for cmd in commands {
            match cmd {
                egui::OutputCommand::OpenUrl(ref open_url)
                    if open_url.url.starts_with("http://")
                        || open_url.url.starts_with("https://") =>
                {
                    let _ = open::that(&open_url.url);
                }
                egui::OutputCommand::OpenUrl(ref open_url) if !open_url.url.starts_with('#') => {
                    // Resolve the target path relative to the directory that
                    // contains the currently-displayed file.
                    let raw = open_url
                        .url
                        .strip_prefix("file://")
                        .unwrap_or(&open_url.url);
                    let target = if std::path::Path::new(raw).is_absolute() {
                        PathBuf::from(raw)
                    } else {
                        let base = self.file_path.parent().unwrap_or(std::path::Path::new("."));
                        base.join(raw)
                    };
                    match std::fs::read_to_string(&target) {
                        Ok(content) => {
                            self.load_document(content, target);
                        }
                        Err(e) => eprintln!("Warning: could not open '{}': {e}", target.display()),
                    }
                }
                // Fragment anchors (#section) are handled via link_hooks above;
                // drop to prevent egui-winit from forwarding them to the browser.
                egui::OutputCommand::OpenUrl(_) => {}
                // Re-queue non-URL commands (clipboard, etc.) for normal processing.
                other => {
                    ui.ctx().output_mut(|o| o.commands.push(other));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Anchor / heading helpers
// ---------------------------------------------------------------------------

/// Split `markdown` at ATX-heading boundaries.
///
/// Returns a list of `(slug, chunk)` pairs:
/// - The first entry may have an empty slug when there is preamble content
///   before the first heading.
/// - Every subsequent entry has a GitHub-style slug derived from the heading
///   text and carries the heading line plus its body text.
///
/// Only ATX headings (lines starting with one to six `#` characters followed
/// by a space) are detected.  Setext-style headings are ignored.
fn parse_heading_sections(markdown: &str) -> Vec<(String, String)> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current_chunk = String::new();
    let mut current_slug = String::new();
    let mut slug_counts: HashMap<String, u32> = HashMap::new();

    for line in markdown.lines() {
        if let Some(heading_text) = extract_atx_heading(line) {
            // Flush the previous section.
            if !current_chunk.is_empty() || !current_slug.is_empty() {
                sections.push((current_slug.clone(), current_chunk.clone()));
            }
            current_slug = github_slug(heading_text, &mut slug_counts);
            current_chunk = format!("{line}\n");
        } else {
            current_chunk.push_str(line);
            current_chunk.push('\n');
        }
    }
    // Push the final section.
    if !current_chunk.is_empty() {
        sections.push((current_slug, current_chunk));
    }
    sections
}

/// Extract the heading text from an ATX heading line, or `None` if the line
/// is not an ATX heading.
///
/// ATX heading: 1–6 `#` characters, a single space, then the heading text.
/// Optional trailing ` ###...` is stripped per the CommonMark spec.
fn extract_atx_heading(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches('#');
    let hashes = line.len() - trimmed.len();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = trimmed.strip_prefix(' ')?;
    // Strip optional closing sequence: trailing whitespace + `#`s + whitespace.
    let text = rest.trim_end().trim_end_matches('#').trim_end_matches(' ');
    // An empty heading text is valid but produces a slug of "".
    // We still return Some so the section boundary is recorded.
    Some(if text.is_empty() { rest.trim() } else { text })
}

/// Generate a GitHub-style anchor slug from heading text.
///
/// Rules (matching GitHub.com behaviour):
/// 1. Strip inline-code backticks (keep their content).
/// 2. Lowercase.
/// 3. Remove everything that is not ASCII alphanumeric, space, hyphen,
///    or underscore.
/// 4. Replace spaces with hyphens (consecutive hyphens are preserved).
/// 5. Deduplicate: second occurrence gets suffix `-1`, third gets `-2`, etc.
fn github_slug(text: &str, seen: &mut HashMap<String, u32>) -> String {
    let base: String = text
        .replace('`', "")
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect();

    let count = seen.entry(base.clone()).or_insert(0);
    let slug = if *count == 0 {
        base.clone()
    } else {
        format!("{base}-{}", *count - 1)
    };
    *count += 1;
    // Prepend '#' to match the format egui_commonmark uses for fragment links.
    format!("#{slug}")
}

/// Register all non-empty anchor slugs from `sections` as link hooks in `cache`.
///
/// This must be called once after loading a document and after resetting the
/// cache so that fragment links in the rendered markdown are intercepted by
/// egui_commonmark rather than emitted as `OutputCommand::OpenUrl`.
fn register_anchor_hooks(cache: &mut CommonMarkCache, sections: &[(String, String)]) {
    for (slug, _) in sections {
        if !slug.is_empty() {
            cache.add_link_hook(slug.as_str());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_simple_heading() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug("Severity Legend", &mut seen),
            "#severity-legend"
        );
    }

    #[test]
    fn slug_apostrophe_removed() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug("What's Done Well", &mut seen),
            "#whats-done-well"
        );
    }

    #[test]
    fn slug_preserves_consecutive_hyphens() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug("Part I — Rust Idioms & Anti-patterns", &mut seen),
            "#part-i--rust-idioms--anti-patterns"
        );
    }

    #[test]
    fn slug_strips_backticks_keeps_content() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug("`run_async` spawns a runtime", &mut seen),
            "#run_async-spawns-a-runtime"
        );
    }

    #[test]
    fn slug_preserves_underscores() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug("`let _ =` silences errors", &mut seen),
            "#let-_--silences-errors"
        );
    }

    #[test]
    fn slug_emoji_and_special_chars_removed() {
        let mut seen = HashMap::new();
        assert_eq!(
            github_slug(
                "I-1 \u{1f534} `HeapError::Success` — success state inside an error enum",
                &mut seen
            ),
            "#i-1--heaperrorsuccess--success-state-inside-an-error-enum"
        );
    }

    #[test]
    fn slug_deduplication() {
        let mut seen = HashMap::new();
        assert_eq!(github_slug("Heading", &mut seen), "#heading");
        assert_eq!(github_slug("Heading", &mut seen), "#heading-0");
        assert_eq!(github_slug("Heading", &mut seen), "#heading-1");
    }

    #[test]
    fn extract_atx_heading_basic() {
        assert_eq!(extract_atx_heading("## Hello World"), Some("Hello World"));
        assert_eq!(extract_atx_heading("### Foo"), Some("Foo"));
        assert_eq!(extract_atx_heading("Not a heading"), None);
        assert_eq!(extract_atx_heading("####### Too many"), None);
    }

    #[test]
    fn extract_atx_heading_trailing_hashes() {
        assert_eq!(extract_atx_heading("## Title ##"), Some("Title"));
    }
}
