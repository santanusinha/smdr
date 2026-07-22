# smdr

**Simple Markdown Reader** — a fast, native markdown viewer built with Rust and [iced](https://github.com/iced-rs/iced).

Renders markdown files in a native window with vim-style navigation, live file watching, and 22 built-in themes.

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
| `Esc` | Close overlay / unfocus sidebar |

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