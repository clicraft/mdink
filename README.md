# mdink

<p align="center">
  <img src="assets/mdink-logo.png" alt="mdink" width="747">
</p>

A terminal markdown renderer with syntax highlighting and inline images. Inspired by [glow](https://github.com/charmbracelet/glow), built in Rust on [ratatui](https://ratatui.rs).

```
mdink README.md
```

**[Full documentation →](https://github.com/mdink-rs/mdink/wiki)**

## Features

- **Headings** (h1–h6) with distinct colors and font-slot modifiers
- **Inline formatting** — bold, italic, bold+italic, strikethrough, inline code
- **Syntax-highlighted code blocks** — 40+ languages via syntect (base16-ocean theme)
- **Lists** — unordered, ordered, task lists, up to 4 levels deep
- **Block quotes** — nested, with full inline formatting
- **Tables** — column alignment, CJK-aware width calculation
- **Horizontal rules**
- **Outline panel** — toggle a table-of-contents sidebar (`o`) with heading navigation, jump-to-heading, and resizable width
- **Terminal images** — Sixel, Kitty, iTerm2, and half-block fallback; remote HTTP/HTTPS images load asynchronously with caching; degrades gracefully to alt text
- **ASCII image rendering** — `--ascii-images` converts images to colored ASCII art using Unicode half-blocks; works on *every* terminal, no graphics protocol needed
- **Images in tables** — embed images inside GFM table cells with automatic sizing
- **Responsive layout** — re-flows at the correct width on every terminal resize

## Installation

### From crates.io

```bash
cargo install mdink
```

### Pre-built binaries

Pre-built binaries (Linux x86\_64/aarch64, macOS x86\_64/aarch64, Windows x86\_64) and a Debian `.deb` package are available on the [releases page](https://github.com/mdink-rs/mdink/releases). See the [Installation wiki page](https://github.com/mdink-rs/mdink/wiki/Installation) for full instructions including shell completions and the man page.

## Usage

```
mdink <FILE>
mdink --ascii-images <FILE>   # render images as colored ASCII art
mdink --no-images <FILE>      # disable image rendering entirely
```

The `--ascii-images` flag renders images as colored Unicode half-block art — useful on terminals without graphics protocol support (plain SSH sessions, older terminals, screen/tmux). The `--no-images` flag disables all image rendering for faster startup.

## Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down one line |
| `k` / `↑` | Scroll up one line |
| `d` / `PgDn` | Scroll down half a page |
| `u` / `PgUp` | Scroll up half a page |
| `g` / `Home` | Jump to top |
| `G` / `End` | Jump to bottom |
| `o` | Toggle outline panel |
| `Tab` / `Shift+Tab` | Navigate outline headings |
| `Enter` | Jump to selected heading (outline) or follow link (link mode) |
| `<` / `>` | Shrink / grow outline panel |
| `l` | Enter link navigation mode |
| `i` | Enter image navigation mode |
| `Backspace` | Go back to previous document |
| `q` / `Esc` / `Ctrl+C` | Quit |

## Link navigation

Press `l` to enter link mode. All hyperlinks in the document are collected and can be cycled with `Tab` / `Shift+Tab`. The selected link's URL is shown in the status bar. Press `Enter` to follow the link:

- **Local `.md` files** are loaded and rendered inside mdink
- **Remote `.md` URLs** (http/https) are fetched and rendered inside mdink
- **Other URLs** are opened with your system browser

Press `Backspace` at any time to return to the previous document. Navigation history is preserved across multiple link follows.

## Image navigation

Press `i` to enter image mode. All images in the document (local files, remote URLs, ASCII art, fallback placeholders) are collected and can be cycled with `Tab` / `Shift+Tab`. The selected image's URL is shown in the status bar. Press `Enter` to open the image:

- **Remote URLs** are opened in your system browser
- **Local files** are opened with the system's default image viewer

Press `Esc` to exit image mode.

## Image support

mdink renders images using the best available method for your terminal:

| Terminal | Protocol |
|----------|----------|
| Kitty | Kitty graphics protocol |
| WezTerm | Kitty graphics protocol |
| iTerm2 (macOS) | iTerm2 inline images |
| Alacritty ≥ 0.13 | Sixel |
| Most others | Half-block fallback (Unicode block elements) |

### ASCII image mode

For terminals with no graphics protocol at all (plain SSH, screen, tmux, older terminals), use `--ascii-images` to render images as colored ASCII art using Unicode half-block characters (`▀▄█`). The ASCII renderer uses terminal font metrics to scale images proportionally. This mode also works for images embedded inside GFM table cells.

```
mdink --ascii-images README.md
```

You can make this the default by adding `"ascii_images": true` to `~/.config/mdink/config.json`.

On unsupported terminals without `--ascii-images`, images fall back to their alt text. Use `--no-images` to skip image rendering entirely.

Remote images (`![alt](https://example.com/img.png)`) are fetched asynchronously in a background thread and cached in memory. They appear as `[loading: alt text]` placeholders while downloading, then render once ready. Cached images survive terminal resize and refresh without re-downloading.

## Minimum Rust version

**1.86.0** (edition 2024)

## License

MIT
