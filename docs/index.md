# mdr

A minimal desktop markdown reader built with Rust.

Renders markdown files in a native window with vim-style navigation, live file watching, and multiple themes.

![Screenshot placeholder](assets/screenshot.png)
<!-- TODO: Replace with actual screenshot -->

---

## Install

### From source

```sh
git clone https://github.com/user/mdr.git
cd mdr
cargo install --path .
```

### Pre-built binaries

See [Releases](https://github.com/user/mdr/releases).

---

## Usage

```
mdr [OPTIONS] [FILE]
```

Read from a file:

```sh
mdr README.md
```

Read from stdin:

```sh
cat README.md | mdr
```

Watch for changes (auto-reload):

```sh
mdr -w README.md
```

### Options

| Flag | Description |
|------|-------------|
| `-w`, `--watch` | Watch file for changes and auto-reload |
| `-t`, `--theme <THEME>` | Color theme (default: `system`) |
| `--no-network` | Disable network image fetching |
| `--list-themes` | List available themes and exit |

---

## Keymap

### Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `Ctrl-D` / `PgDn` | Page down |
| `Ctrl-U` / `PgUp` | Page up |
| `h` / `←` | Navigate back |
| `l` / `→` | Navigate forward |

### Links

| Key | Action |
|-----|--------|
| `Tab` | Focus next link |
| `Shift-Tab` | Focus previous link |
| `Enter` | Activate focused link |

### Search

| Key | Action |
|-----|--------|
| `/` or `?` | Open search |
| `Ctrl-F` | Open search |
| `n` | Next search hit |
| `p` | Previous search hit |
| `Esc` | Close search |

### UI

| Key | Action |
|-----|--------|
| `Ctrl-B` | Toggle sidebar (table of contents) |
| `Ctrl-T` | Cycle theme |
| `Esc` | Close overlay |

---

## Themes

mdr ships with 22 built-in themes. Use `--list-themes` to see all options.

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

![Themes placeholder](assets/themes.png)
<!-- TODO: Replace with actual screenshot showing theme variety -->

---

## License

MIT
