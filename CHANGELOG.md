# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.0] - 2026-02-28

### Added
- **ASCII image rendering** — `--ascii-images` flag converts images to colored ASCII art using Unicode half-block characters, works on every terminal with no graphics protocol required
- **Images inside GFM tables** — image references in table cells are rendered inline (native protocol or ASCII art depending on mode)
- **Native-to-ASCII fallback chain** — when `--ascii-images` is set, images first attempt native protocol rendering and automatically fall back to ASCII art
- Font-metrics-based ASCII sizing — ASCII art images are proportionally scaled using terminal font dimensions instead of filling the full terminal width

### Changed
- `--ascii-images` is also configurable via `config.json` (`"ascii_images": true`)

## [0.1.3] - 2026-02-28

### Added
- **Outline panel** — press `o` to toggle a table-of-contents sidebar showing h1–h3 headings
  - Wide terminals (≥ 101 cols): persistent side panel with vertical border
  - Narrow terminals: dropdown overlay with bordered frame
  - `Tab` / `Shift+Tab` to navigate headings, `Enter` to jump, `Esc` to close
  - `<` / `>` to shrink/grow panel width (2% per press, range 10–33%)
  - Percentage-based width (`width_percent` in theme) adapts to terminal size
  - Visual polish: content padding, hanging indent for wrapped headings, blank-line separators
- Outline panel colors are fully theme-configurable (`outline` section in theme JSON)
- GitHub wiki with 10 reference pages (Installation, CLI Reference, Keybindings, Themes, Terminal Compatibility, Font Slot Strategy, Architecture, Contributing, Release Process)

### Changed
- README: added outline panel to features list and navigation table
- Wiki: updated Keybindings page with outline keys, Themes page with outline schema

## [0.1.2] - 2026-02-28

### Added
- Config file support (`~/.config/mdink/config.json`) for persistent settings
- Stdin support (`mdink -` or piping to mdink)
- `--no-color` flag and `NO_COLOR` environment variable support
- `--dump-theme` flag to export resolved theme as JSON
- Long `--help` text with usage examples, keybindings, and environment variables
- Exit codes: 65 (file too large), 66 (file not found/unreadable)
- Man page (`man mdink`) via xtask generation
- Shell completions for bash, zsh, and fish
- Homebrew formula template (`packaging/homebrew/mdink.rb`)
- AUR PKGBUILD template (`packaging/aur/PKGBUILD`)
- `exclude` in Cargo.toml for crates.io packaging
- This CHANGELOG

## [0.1.1] - 2026-02-28

### Added
- JSON theming system with 3 built-in themes (dark, light, dracula)
- `--style` flag and `MDINK_STYLE` environment variable
- `--list-themes` flag
- Post-deserialization theme sanitization
- Configurable list marker colors (bullet_fg, number_fg, task_checked_fg, task_unchecked_fg)

### Changed
- Reorganized src/ into directory modules
- All hardcoded colors/modifiers replaced with theme-driven styling

## [0.1.0] - 2026-02-28

### Added
- Terminal markdown rendering with vim-style navigation
- Syntax-highlighted code blocks (syntect, 200+ languages)
- Inline terminal images (Sixel, Kitty, iTerm2, halfblocks)
- GFM tables with auto-sizing columns
- Nested block quotes with dim/italic styling
- Ordered/unordered/task lists with Unicode bullets
- Font slot strategy (bold, italic, bold+italic mapped to markdown elements)
- PowerShell syntax highlighting
- File size guard (100 MB) and highlight size guard (512 KB)
- Release workflow for Linux/macOS/Windows
- Debian packaging metadata
