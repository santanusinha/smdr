# smdr

Simple Markdown Reader — a fast, native markdown viewer built with Rust.

Renders markdown files in a native window with vim-style navigation, live file watching, and multiple themes.

![Screenshot](assets/screenshot.png)

---

## Install

### From source

```sh
git clone https://github.com/santanusinha/smdr.git
cd smdr
cargo install --path .
```

### Pre-built binaries

See [Releases](https://github.com/santanusinha/smdr/releases).

---

## Usage

```
smdr [OPTIONS] [FILE]
```

Read from a file:

```sh
smdr README.md
```

Read from stdin:

```sh
cat README.md | smdr
```

!!! tip "Live file watching"
    Pass `-w` / `--watch` and smdr will monitor the file for changes and
    automatically reload the view whenever you save — ideal for editing
    workflows, documentation previews, and note-taking.

    ```sh
    smdr -w README.md
    ```

    Already viewing a file without `-w`? Press **`Ctrl-R`** at any time
    to manually reload from disk.

### Options

| Flag | Description |
|------|-------------|
| `-w`, `--watch` | Watch file for changes and auto-reload |
| `-t`, `--theme <THEME>` | Color theme (default: `system`) |
| `--no-network` | Disable network image fetching (use local files only) |
| `--list-themes` | List available themes and exit |

!!! tip "stdin support"
    smdr automatically detects piped input — no flag needed:
    ```sh
    cat README.md | smdr
    man git | smdr
    ```

---

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
| ` `` ` | Jump to last position |
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

### Mermaid diagrams

Click any Mermaid diagram to open it in a full-screen modal.

| Key | Action |
|-----|--------|
| `Ctrl-=` / `Ctrl-+` | Zoom in |
| `Ctrl--` | Zoom out |
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `h` / `←` | Scroll left |
| `l` / `→` | Scroll right |
| `Esc` | Close modal |

---

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


---

## License

MIT
