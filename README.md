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
- **Terminal images** — Sixel, Kitty, iTerm2, and half-block fallback; degrades gracefully to alt text
- **Responsive layout** — re-flows at the correct width on every terminal resize

## Installation

### From source

```bash
cargo install --path .
```

### Pre-built binaries

Pre-built binaries (Linux x86\_64/aarch64, macOS x86\_64/aarch64, Windows x86\_64) and a Debian `.deb` package are available on the [releases page](https://github.com/mdink-rs/mdink/releases). See the [Installation wiki page](https://github.com/mdink-rs/mdink/wiki/Installation) for full instructions including shell completions and the man page.

## Usage

```
mdink <FILE>
mdink --no-images <FILE>   # disable image rendering
```

The `--no-images` flag is useful on terminals that do not support any graphics protocol or when you want faster startup.

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
| `Enter` | Jump to selected heading |
| `<` / `>` | Shrink / grow outline panel |
| `q` / `Esc` / `Ctrl+C` | Quit |

## Image support

Inline images require a terminal that supports at least one graphics protocol:

| Terminal | Protocol |
|----------|----------|
| Kitty | Kitty graphics protocol |
| WezTerm | Kitty graphics protocol |
| iTerm2 (macOS) | iTerm2 inline images |
| Alacritty ≥ 0.13 | Sixel |
| Most others | Half-block fallback (Unicode block elements) |

On unsupported terminals, images fall back to their alt text. Use `--no-images` to skip the protocol detection query entirely.

## Minimum Rust version

**1.86.0** (edition 2024)

## License

MIT
