# Changelog

All notable changes to **smdr** are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.1.3] — 2026-07-22

### Added
- **Tabbed browsing** — documents now open as tabs within a single window.
  A tab bar (with per-tab close buttons) appears whenever more than one
  document is open. Switch tabs with `Ctrl-Tab` / `Ctrl-Shift-Tab` or the
  vim-style `gt` / `gT`; close the current tab with `Ctrl-W`.
- **Multiple files on the command line** — `smdr a.md b.md c.md` opens each
  file in its own tab; the first file is the primary document and the rest
  load as tabs at startup.
- **Single-window hand-off** — running `smdr <file>` while a window is already
  open sends the path to the running instance (over a per-user Unix socket)
  so it opens as a new tab instead of spawning a second window. The launcher
  process detaches from the terminal, so the shell is never blocked.
- **Duplicate-file de-duplication** — opening a file that is already open
  switches to its existing tab and reloads it from disk rather than creating
  a duplicate tab.

---

## [0.1.2] — 2026-06-26

### Added
- **Window icon** — file icon with `md` label (white background, black border)
  embedded directly in the binary; shown in the X11 title bar.
- **Wayland app_id** — `platform_specific.application_id = "smdr"` set so
  GNOME Shell (and other XDG-compliant compositors) can match the window to
  its desktop entry for the alt-tab switcher and taskbar.
- **Self-installing desktop integration** (Linux) — on first launch the binary
  writes `~/.local/share/icons/hicolor/256x256/apps/smdr.png` and
  `~/.local/share/applications/smdr.desktop`; rewrites them automatically on
  version upgrade via an embedded `X-Version` stamp; failures are silently
  ignored so broken permissions never prevent the app from starting. No
  installer script required.

### Docs
- GitHub Pages workflow, Mermaid keymap section, stdin usage tip added to
  the documentation site.
- **Homebrew formula** — `HomebrewFormula/smdr.rb` added to the repo;
  install with
  `brew tap santanusinha/smdr https://github.com/santanusinha/smdr && brew install smdr`.

---

## [0.1.1] — 2026-06-26

### Added
- **Theme persistence** — last-selected theme is saved to
  `~/.local/state/smdr/state` and restored on next launch.
- **In-text search highlighting** — matched search terms are highlighted
  inline in the rendered document.
- **Mermaid full-screen modal** — click any Mermaid diagram to open it in a
  full-screen overlay with zoom (`+` / `-`) and keyboard scrolling
  (`j`/`k`/`h`/`l` and arrow keys).

### Fixed / Performance
- Fixed Mermaid CPU blowout: `iced` image handles are now cached; diagrams are
  rasterised asynchronously.
- Simplified Mermaid rendering pipeline to native SVG with a loading-state
  placeholder.
- Cached `line_count` and lowercased search query to avoid repeated
  recomputation on every keystroke.

### CI
- Added trusted publishing to crates.io via GitHub Actions OIDC.

---

## [0.1.0] — 2026-06-24

Initial public release on [crates.io](https://crates.io/crates/smdr).

### Added
- Native markdown viewer built with [iced](https://github.com/iced-rs/iced)
  and `pulldown-cmark`.
- **22 built-in themes** with a live theme picker in the status bar; theme
  shortcut key (`t`) cycles through all themes.
- **Vim-style navigation** — `j`/`k`, `Ctrl-U`/`D`, `gg`/`G`, `PageUp`/`PageDown`.
- **Browser-like history** — `h`/`←` back, `l`/`→` forward through visited
  links and positions.
- **Collapsible, resizable sidebar** showing the document outline (headings);
  fully keyboard-navigable (`Tab`/`Shift-Tab`, `j`/`k`, `Enter`).
- **In-document search** — `/` or `Ctrl-F` to open, `n`/`p` to cycle hits.
- **Tab/Shift-Tab link cycling** with `Enter` to activate.
- **Multi-key vim sequences** — `gg`, `Ctrl-U/D`, and other compound bindings.
- **Live file watching** — `--watch` flag auto-reloads on save.
- **stdin pipe support** — `cat file.md | smdr` works as expected.
- **`Ctrl-R` reload** and **`Ctrl-C` clipboard copy** shortcuts.
- **Mermaid diagram rendering** — inline diagrams rendered as SVG with async
  rasterisation and loading/error placeholders.
- **Remote image fetching** — network images loaded asynchronously;
  `--no-network` flag disables this.
- **Permanent status bar** with theme picker, keymap overlay, and about panel.
- Theme-adaptive code block and inline-code styling.
- File name shown in the window title bar.
- Consistent, proportional heading and code font sizes.

### CI
- GitHub Actions pipelines for Linux (x86-64), macOS (arm64), and Windows
  (x86-64) release builds.
- Multi-platform release artefacts published automatically on tag push.
