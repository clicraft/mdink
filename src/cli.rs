//! CLI argument definition for mdink.
//!
//! This module is intentionally free of non-clap dependencies so that
//! the Phase 7 xtask can import it via `#[path]` for man page and
//! shell completion generation.

use clap::Parser;

/// Terminal markdown renderer.
#[derive(Parser)]
#[command(
    name = "mdink",
    version,
    about = "Terminal markdown renderer",
    long_about = "\
Terminal markdown renderer with syntax highlighting and image support.

USAGE EXAMPLES:
  mdink README.md                  Render a markdown file
  mdink -s dracula README.md       Render with the Dracula theme
  cat notes.md | mdink -           Read from stdin
  mdink --dump-theme > my.json     Export current theme as JSON
  mdink -s my.json README.md       Use a custom theme file
  mdink https://example.com/README.md  Render from URL
  mdink --print README.md             Print to stdout (no TUI)

KEYBINDINGS:
  j / Down / Scroll Down    Scroll down
  k / Up / Scroll Up        Scroll up
  d / Page Down              Page down
  u / Page Up                Page up
  g / Home                   Go to top
  G / End                    Go to bottom
  o                          Toggle outline panel
  f                          Open file browser
  r                          Refresh / re-render
  t                          Cycle theme (dark/light/dracula)
  q / Esc                    Quit

THEMES:
  Built-in: dark (default), light, dracula
  Custom:   place .json files in ~/.config/mdink/themes/
  Export:   mdink --dump-theme > mytheme.json

ENVIRONMENT VARIABLES:
  MDINK_STYLE    Theme name or path (overridden by --style)
  NO_COLOR       Disable all colors when set (any value)

CONFIG FILE:
  ~/.config/mdink/config.json
  {\"style\": \"dracula\", \"no_images\": false, \"ascii_images\": false, \"no_color\": false}
  CLI flags and env vars take precedence over config values."
)]
pub struct Cli {
    /// Markdown file or URL to render (use "-" for stdin).
    #[arg(required_unless_present_any = ["list_themes", "dump_theme"])]
    pub file: Option<String>,

    /// Disable image rendering (show alt text instead).
    #[arg(long)]
    pub no_images: bool,

    /// Force ASCII art for images (useful when the terminal falsely claims graphics support).
    #[arg(long)]
    pub ascii_images: bool,

    /// Theme: dark, light, dracula, or path to JSON file.
    #[arg(short = 's', long = "style")]
    pub style: Option<String>,

    /// List available built-in themes and exit.
    #[arg(long)]
    pub list_themes: bool,

    /// Disable colored output.
    #[arg(long)]
    pub no_color: bool,

    /// Dump the resolved theme as JSON and exit.
    #[arg(long)]
    pub dump_theme: bool,

    /// Print rendered output to stdout (no TUI) and exit.
    #[arg(long)]
    pub print: bool,

    /// Font family for PDF export (overrides terminal auto-detection).
    #[arg(long = "pdf-font")]
    pub pdf_font: Option<String>,

    /// Export to PDF and exit (no TUI). Output path defaults to <input>.pdf.
    #[arg(long)]
    pub pdf: bool,
}
