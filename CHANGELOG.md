# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
