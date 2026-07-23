# smdr

**Simple Markdown Reader** — a fast, native markdown viewer built with Rust and [iced](https://github.com/iced-rs/iced).

Renders markdown files in a native window with vim-style navigation, live file watching, 22 built-in themes, and an interactive **review mode** for annotating documents with inline comments.

## Install

### From crates.io

```sh
cargo install smdr
```

### Homebrew (macOS / Linux)

```sh
brew tap santanusinha/smdr https://github.com/santanusinha/smdr
brew install smdr
```

### From source

```sh
git clone https://github.com/santanusinha/smdr.git
cd smdr
cargo install --path .
```

## Usage

```
smdr [OPTIONS] [FILE]...
```

Read from a file:

```sh
smdr README.md
```

Open several files at once — each opens in its own tab:

```sh
smdr README.md CHANGELOG.md docs/guide.md
```

Read from stdin:

```sh
cat README.md | smdr
```

Watch for changes (auto-reload):

```sh
smdr -w README.md
```

### Tabs

When more than one document is open, a tab bar appears at the top. Opening
further files — either by passing multiple paths on one command line or by
running `smdr <file>` again while a window is already open — adds them as tabs
in the existing window rather than spawning new windows. Re-opening a file that
is already open switches to its tab and reloads it from disk instead of
creating a duplicate.

| Key | Action |
|-----|--------|
| `Ctrl-Tab` / `gt` | Next tab |
| `Ctrl-Shift-Tab` / `gT` | Previous tab |
| `Ctrl-W` | Close current tab |

### Options

| Flag | Description |
|------|-------------|
| `-w`, `--watch` | Watch file for changes and auto-reload |
| `-t`, `--theme <THEME>` | Color theme (default: `system`) |
| `--no-network` | Disable network image fetching |
| `--list-themes` | List available themes and exit |
| `--review` | Open file in review mode (see [Review mode](#review-mode)) |
| `--annotations-in <PATH>` | Path to an annotations JSON file (enables headless review) |
| `--out <PATH>` | Write review output to `<PATH>` instead of stdout |
| `--format <FORMAT>` | Output format: `json` (default), `md`, or `diff` |

## Review mode

smdr includes an interactive review mode that lets you annotate a Markdown
document with inline comments and export the result in several formats.

### Interactive review

Open a file in the review UI:

```sh
smdr --review README.md
```

The source text is displayed with a gutter on the left. Click any gutter line
number to open the comment composer for that line; press `Esc` to cancel
without saving. When you are done, click **Submit review** (or press `Ctrl-Enter`)
to emit the annotated output and exit.

**Gutter key bindings**

| Key / Action | Behaviour |
|---|---|
| Click gutter line number | Open comment composer for that line |
| `c` | Toggle between rendered-source view and raw-comment-overlay view |
| `Esc` (in composer) | Cancel the current comment without saving |
| **Submit review** button | Emit output and exit |

**Draft auto-save** — unsaved comments are automatically persisted to
`$TMPDIR/smdr-drafts/` so they survive an accidental window close. Drafts are
restored the next time you open the same file with `--review` and are cleared
automatically when you submit.

### Headless (one-shot) review

If you already have an annotations file — for example one produced by another
tool — you can run smdr non-interactively:

```sh
smdr --review --annotations-in annotations.json README.md
```

smdr reads the annotations, merges them with the source, and writes the result
to stdout (or `--out <file>`) then exits immediately without opening a window.

### Output formats

| Value | Description |
|-------|-------------|
| `json` | Structured JSON — schema-tagged envelope with one object per annotation (default) |
| `md` | Annotated Markdown — source with `<!-- smdr: … -->` comment blocks inserted after each annotated line |
| `diff` | Unified-diff transport — insertion-only diff with 6 lines of context per hunk, suitable for patch workflows |

#### JSON envelope schema

The `json` format emits a single `ReviewEnvelope` object. The same schema is
expected by `--annotations-in` when running headless:

```json
{
  "schema": "smdr.review/v1",
  "file": "README.md",
  "comments": [
    { "line": 0, "comment": "Title looks good." },
    { "line": 7, "comment": "Add a quickstart example here." }
  ]
}
```

| Field | Type | Notes |
|-------|------|-------|
| `schema` | string | Must be `"smdr.review/v1"` — bump on breaking changes |
| `file` | string | Path to the file that was reviewed (informational) |
| `comments[].line` | integer | **0-based** source line the comment is anchored to |
| `comments[].comment` | string | Freeform review note |

```sh
# Export as annotated Markdown
smdr --review --annotations-in a.json --format md --out review.md README.md

# Export as a unified diff
smdr --review --annotations-in a.json --format diff README.md | patch -p0
```

> **Note:** Draft files are stored in `$TMPDIR/smdr-drafts/` indefinitely until
> you submit or manually delete them. A future release will add automatic
> expiry of stale drafts.

## Keymap

### Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `Ctrl-D` / `PgDn` / `Space` | Page down |
| `Ctrl-U` / `PgUp` | Page up |
| `gg` / `Home` | Scroll to top |
| `GG` / `End` | Scroll to bottom |
| `` ` `` | Jump to last position |
| `h` / `←` | Navigate back |
| `l` / `→` | Navigate forward |

### Links

| Key | Action |
|-----|--------|
| `Tab` | Focus next link |
| `Shift-Tab` | Focus previous link |
| `Enter` | Activate focused link (or next search hit) |

### Search

| Key | Action |
|-----|--------|
| `/` | Open search |
| `Ctrl-F` | Open search |
| `n` | Next search hit |
| `p` | Previous search hit |
| `Esc` | Close search |

### Sidebar (table of contents)

| Key | Action |
|-----|--------|
| `Ctrl-B` | Toggle sidebar visibility |
| `o` | Focus / unfocus sidebar |
| `j` / `↓` | Next heading (when sidebar focused) |
| `k` / `↑` | Previous heading (when sidebar focused) |
| `Enter` | Jump to heading (when sidebar focused) |

### File & clipboard

| Key | Action |
|-----|--------|
| `Ctrl-R` | Reload file from disk |
| `Ctrl-C` | Copy document to clipboard |

### UI & app

| Key | Action |
|-----|--------|
| `Ctrl-T` | Cycle theme |
| `?` | Show keyboard shortcuts |
| `qq` / `ZZ` | Exit |
| `Esc` | Close overlay / unfocus sidebar / cancel comment composer |

### Review mode

| Key / Action | Behaviour |
|---|---|
| Click gutter line | Open comment composer |
| `c` | Toggle source / comment-overlay view |
| `Esc` (in composer) | Cancel comment without saving |
| `Ctrl-Enter` | Submit review and exit |

## Themes

smdr ships with 22 built-in themes. Use `--list-themes` to see all options.

| Theme | Style |
|-------|-------|
| `system` | Follows OS dark/light preference |
| `light` | Light |
| `dark` | Dark |
| `dracula` | Dark, vibrant |
| `nord` | Arctic blue |
| `solarized-light` | Warm light |
| `solarized-dark` | Warm dark |
| `gruvbox-light` | Retro light |
| `gruvbox-dark` | Retro dark |
| `catppuccin-latte` | Pastel light |
| `catppuccin-frappe` | Pastel mid-dark |
| `catppuccin-macchiato` | Pastel dark |
| `catppuccin-mocha` | Pastel darkest |
| `tokyo-night` | Purple/blue dark |
| `tokyo-night-storm` | Lighter variant |
| `tokyo-night-light` | Light variant |
| `kanagawa-wave` | Blue dark |
| `kanagawa-dragon` | Darker variant |
| `kanagawa-lotus` | Light variant |
| `moonfly` | Emerald dark |
| `nightfly` | Blue dark |
| `oxocarbon` | IBM Carbon dark |
| `ferra` | Warm muted dark |

## License

MIT
