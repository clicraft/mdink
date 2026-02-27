//! Theme module: JSON-driven styling for all markdown elements.
//!
//! This is a **leaf module** — it never imports from other mdink modules.
//! All style-producing code in the pipeline (parser, layout, renderer) calls
//! helpers in this module instead of hardcoding colors and modifiers.

use std::fs;

use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;

// ── Theme structs ────────────────────────────────────────────────────────────

/// Top-level markdown theme. All fields have defaults so partial JSON works.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct MarkdownTheme {
    pub name: String,
    pub document: DocumentStyle,
    pub heading: Vec<HeadingStyle>,
    pub code_block: CodeBlockStyle,
    pub code_inline: InlineStyle,
    pub block_quote: BlockQuoteStyle,
    pub table: TableStyle,
    pub thematic_break: ThematicBreakStyle,
    pub list: ListStyle,
    pub emphasis: InlineStyle,
    pub strong: InlineStyle,
    pub strikethrough: InlineStyle,
    pub link: InlineStyle,
    pub image_alt: InlineStyle,
    pub status_bar: StatusBarStyle,
    pub syntect_theme: String,
}

/// Document-level style (background).
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct DocumentStyle {
    pub bg: Option<String>,
}

/// Per-heading-level style.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct HeadingStyle {
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub prefix: Option<String>,
}

/// Code block chrome style.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct CodeBlockStyle {
    pub bg: Option<String>,
    pub label_fg: Option<String>,
    pub label_bg: Option<String>,
    pub label_italic: bool,
}

/// Block quote style.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct BlockQuoteStyle {
    pub fg: Option<String>,
    pub border_fg: Option<String>,
    pub prefix: String,
    pub italic: bool,
    pub dim: bool,
}

impl Default for BlockQuoteStyle {
    fn default() -> Self {
        Self {
            fg: None,
            border_fg: None,
            prefix: "│ ".to_string(),
            italic: true,
            dim: true,
        }
    }
}

/// Reusable inline style (emphasis, strong, code_inline, link, etc.).
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct InlineStyle {
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub dim: bool,
}

/// Table style.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct TableStyle {
    pub header_fg: Option<String>,
    pub header_bold: bool,
    pub border_fg: Option<String>,
    pub border_dim: bool,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self {
            header_fg: None,
            header_bold: true,
            border_fg: None,
            border_dim: true,
        }
    }
}

/// Thematic break (horizontal rule) style.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ThematicBreakStyle {
    pub fg: Option<String>,
    #[serde(rename = "char")]
    pub char_: String,
    pub dim: bool,
}

impl Default for ThematicBreakStyle {
    fn default() -> Self {
        Self {
            fg: None,
            char_: "─".to_string(),
            dim: true,
        }
    }
}

/// List style.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ListStyle {
    pub bullet: Vec<String>,
    pub task_checked: String,
    pub task_unchecked: String,
    pub bullet_fg: Option<String>,
    pub number_fg: Option<String>,
    pub task_checked_fg: Option<String>,
    pub task_unchecked_fg: Option<String>,
    pub indent_size: u16,
}

impl Default for ListStyle {
    fn default() -> Self {
        Self {
            bullet: vec!["•".to_string(), "◦".to_string(), "▪".to_string()],
            task_checked: "☑".to_string(),
            task_unchecked: "☐".to_string(),
            bullet_fg: None,
            number_fg: None,
            task_checked_fg: None,
            task_unchecked_fg: None,
            indent_size: 2,
        }
    }
}

/// Status bar style.
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct StatusBarStyle {
    pub fg: Option<String>,
    pub bg: Option<String>,
    pub bold: bool,
}

impl Default for StatusBarStyle {
    fn default() -> Self {
        Self {
            fg: Some("black".to_string()),
            bg: Some("white".to_string()),
            bold: true,
        }
    }
}

// ── Default theme (dark) ─────────────────────────────────────────────────────

impl Default for MarkdownTheme {
    fn default() -> Self {
        Self {
            name: "dark".to_string(),
            document: DocumentStyle::default(),
            heading: vec![
                HeadingStyle { fg: Some("light_cyan".to_string()), bold: true, ..Default::default() },
                HeadingStyle { fg: Some("green".to_string()), bold: true, ..Default::default() },
                HeadingStyle { fg: Some("yellow".to_string()), bold: true, ..Default::default() },
                HeadingStyle { fg: Some("white".to_string()), bold: true, italic: true, ..Default::default() },
                HeadingStyle { fg: Some("white".to_string()), bold: true, italic: true, ..Default::default() },
                HeadingStyle { fg: Some("white".to_string()), bold: true, italic: true, ..Default::default() },
            ],
            code_block: CodeBlockStyle {
                bg: Some("235".to_string()),
                label_fg: Some("245".to_string()),
                label_bg: Some("235".to_string()),
                label_italic: true,
            },
            code_inline: InlineStyle {
                fg: Some("252".to_string()),
                bg: Some("236".to_string()),
                bold: true,
                italic: true,
                ..Default::default()
            },
            block_quote: BlockQuoteStyle::default(),
            table: TableStyle::default(),
            thematic_break: ThematicBreakStyle::default(),
            list: ListStyle::default(),
            emphasis: InlineStyle { italic: true, ..Default::default() },
            strong: InlineStyle { bold: true, ..Default::default() },
            strikethrough: InlineStyle { strikethrough: true, ..Default::default() },
            link: InlineStyle { italic: true, ..Default::default() },
            image_alt: InlineStyle { dim: true, ..Default::default() },
            status_bar: StatusBarStyle::default(),
            syntect_theme: "base16-ocean.dark".to_string(),
        }
    }
}

// ── Post-deserialization sanitization ────────────────────────────────────────

impl MarkdownTheme {
    /// Ensures all theme values are safe for the rest of the pipeline.
    ///
    /// Call this after every `serde_json::from_str()`. It fixes degenerate
    /// values that would cause invisible rendering or index panics:
    /// - Pads `heading` to exactly 6 entries (repeating the last, or using defaults).
    /// - Replaces empty `thematic_break.char_` with `"─"`.
    /// - Replaces empty `block_quote.prefix` with `"│ "`.
    /// - Replaces empty `list.bullet` with `["•"]`.
    /// - Clamps `list.indent_size` of 0 to 2.
    pub fn sanitize(&mut self) {
        // Heading: pad to 6 entries. If user provides 3, entries 4-6 repeat the last.
        if self.heading.is_empty() {
            self.heading = MarkdownTheme::default().heading;
        }
        while self.heading.len() < 6 {
            self.heading.push(self.heading.last().unwrap().clone());
        }
        self.heading.truncate(6);

        // Thematic break: empty char renders invisible rules.
        if self.thematic_break.char_.is_empty() {
            self.thematic_break.char_ = "─".to_string();
        }

        // Block quote: empty prefix loses visual border.
        if self.block_quote.prefix.is_empty() {
            self.block_quote.prefix = "│ ".to_string();
        }

        // List bullets: empty vec falls back to hardcoded "•" anyway,
        // but sanitize it explicitly for clarity.
        if self.list.bullet.is_empty() {
            self.list.bullet = vec!["•".to_string()];
        }

        // Zero indent makes nested lists visually flat.
        if self.list.indent_size == 0 {
            self.list.indent_size = 2;
        }
    }
}

// ── Color parsing ────────────────────────────────────────────────────────────

/// Parses a color string into a ratatui `Color`.
///
/// Supports:
/// - 6-digit hex: `"#ff5500"` or `"ff5500"` → `Color::Rgb`
/// - Indexed: `"99"` → `Color::Indexed(99)` (0–255)
/// - Named: `"red"`, `"light_cyan"`, etc. → `Color::Red`, `Color::LightCyan`
/// - Empty / invalid → `None` (fail-safe, never panics)
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Try hex (with or without #)
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some(Color::Rgb(r, g, b));
    }

    // Try indexed (0–255)
    if let Ok(idx) = s.parse::<u8>() {
        return Some(Color::Indexed(idx));
    }

    // Try named colors
    match s.to_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "dark_gray" | "darkgray" => Some(Color::DarkGray),
        "light_red" | "lightred" => Some(Color::LightRed),
        "light_green" | "lightgreen" => Some(Color::LightGreen),
        "light_yellow" | "lightyellow" => Some(Color::LightYellow),
        "light_blue" | "lightblue" => Some(Color::LightBlue),
        "light_magenta" | "lightmagenta" => Some(Color::LightMagenta),
        "light_cyan" | "lightcyan" => Some(Color::LightCyan),
        "gray" => Some(Color::Gray),
        _ => None,
    }
}

/// Applies a color to a style via the given setter, if the color string is present and valid.
fn apply_color(style: Style, color: &Option<String>, setter: fn(Style, Color) -> Style) -> Style {
    match color.as_deref().and_then(parse_color) {
        Some(c) => setter(style, c),
        None => style,
    }
}

// ── Style conversion helpers ─────────────────────────────────────────────────

/// Converts a `HeadingStyle` to a ratatui `Style`.
pub fn heading_style(h: &HeadingStyle) -> Style {
    let mut style = Style::default();
    style = apply_color(style, &h.fg, Style::fg);
    style = apply_color(style, &h.bg, Style::bg);
    let mut modifier = Modifier::empty();
    if h.bold { modifier |= Modifier::BOLD; }
    if h.italic { modifier |= Modifier::ITALIC; }
    if h.underline { modifier |= Modifier::UNDERLINED; }
    style.add_modifier(modifier)
}

/// Converts an `InlineStyle` to a ratatui `Style`.
pub fn inline_style(s: &InlineStyle) -> Style {
    let mut style = Style::default();
    style = apply_color(style, &s.fg, Style::fg);
    style = apply_color(style, &s.bg, Style::bg);
    let mut modifier = Modifier::empty();
    if s.bold { modifier |= Modifier::BOLD; }
    if s.italic { modifier |= Modifier::ITALIC; }
    if s.underline { modifier |= Modifier::UNDERLINED; }
    if s.strikethrough { modifier |= Modifier::CROSSED_OUT; }
    if s.dim { modifier |= Modifier::DIM; }
    style.add_modifier(modifier)
}

/// Extracts the code block background color.
pub fn code_block_bg(cb: &CodeBlockStyle) -> Option<Color> {
    cb.bg.as_deref().and_then(parse_color)
}

/// Converts code block label fields to a ratatui `Style`.
pub fn code_label_style(cb: &CodeBlockStyle) -> Style {
    let mut style = Style::default();
    style = apply_color(style, &cb.label_fg, Style::fg);
    style = apply_color(style, &cb.label_bg, Style::bg);
    if cb.label_italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    style
}

/// Style for block quote content text (merged into child spans).
pub fn quote_content_style(bq: &BlockQuoteStyle) -> Style {
    let mut style = Style::default();
    style = apply_color(style, &bq.fg, Style::fg);
    let mut modifier = Modifier::empty();
    if bq.italic { modifier |= Modifier::ITALIC; }
    if bq.dim { modifier |= Modifier::DIM; }
    style.add_modifier(modifier)
}

/// Style for the block quote `│ ` prefix.
pub fn quote_prefix_style(bq: &BlockQuoteStyle) -> Style {
    let mut style = Style::default();
    style = apply_color(style, &bq.border_fg, Style::fg);
    let mut modifier = Modifier::empty();
    if bq.italic { modifier |= Modifier::ITALIC; }
    if bq.dim { modifier |= Modifier::DIM; }
    style.add_modifier(modifier)
}

/// Style for table header cells.
pub fn table_header_style(t: &TableStyle) -> Style {
    let mut style = Style::default();
    style = apply_color(style, &t.header_fg, Style::fg);
    if t.header_bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

/// Style for table border/separator.
pub fn table_border_style(t: &TableStyle) -> Style {
    let mut style = Style::default();
    style = apply_color(style, &t.border_fg, Style::fg);
    if t.border_dim {
        style = style.add_modifier(Modifier::DIM);
    }
    style
}

/// Style for thematic breaks (horizontal rules).
pub fn rule_style(tb: &ThematicBreakStyle) -> Style {
    let mut style = Style::default();
    style = apply_color(style, &tb.fg, Style::fg);
    if tb.dim {
        style = style.add_modifier(Modifier::DIM);
    }
    style
}

/// Style for list bullet markers.
pub fn list_bullet_style(ls: &ListStyle) -> Style {
    apply_color(Style::default(), &ls.bullet_fg, Style::fg)
}

/// Style for list number markers (ordered lists).
pub fn list_number_style(ls: &ListStyle) -> Style {
    apply_color(Style::default(), &ls.number_fg, Style::fg)
}

/// Style for checked task markers.
pub fn list_task_checked_style(ls: &ListStyle) -> Style {
    apply_color(Style::default(), &ls.task_checked_fg, Style::fg)
}

/// Style for unchecked task markers.
pub fn list_task_unchecked_style(ls: &ListStyle) -> Style {
    apply_color(Style::default(), &ls.task_unchecked_fg, Style::fg)
}

/// Style for the status bar.
pub fn status_bar_style(sb: &StatusBarStyle) -> Style {
    let mut style = Style::default();
    style = apply_color(style, &sb.fg, Style::fg);
    style = apply_color(style, &sb.bg, Style::bg);
    if sb.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

// ── Theme loading ────────────────────────────────────────────────────────────

/// Errors that can occur when loading a theme.
#[derive(Debug)]
pub enum ThemeError {
    NotFound { name: String },
    ParseError { source: serde_json::Error },
    IoError { path: String, source: std::io::Error },
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeError::NotFound { name } => {
                write!(f, "theme not found: '{name}'. Use --list-themes to see available themes.")
            }
            ThemeError::ParseError { source } => {
                write!(f, "theme JSON parse error: {source}")
            }
            ThemeError::IoError { path, source } => {
                write!(f, "cannot read theme file '{path}': {source}")
            }
        }
    }
}

impl std::error::Error for ThemeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ThemeError::ParseError { source } => Some(source),
            ThemeError::IoError { source, .. } => Some(source),
            ThemeError::NotFound { .. } => None,
        }
    }
}

/// Loads a theme by name or file path.
///
/// Resolution order:
/// 1. Built-in name (`"dark"`, `"light"`, `"dracula"`) → embedded JSON
/// 2. Path exists as file → read and parse
/// 3. `~/.config/mdink/themes/{name}.json` → read and parse
/// 4. `Err(ThemeError::NotFound)`
pub fn load_theme(name_or_path: &str) -> Result<MarkdownTheme, ThemeError> {
    // Helper: deserialize and sanitize.
    let parse_and_sanitize = |json: &str| -> Result<MarkdownTheme, ThemeError> {
        let mut theme: MarkdownTheme =
            serde_json::from_str(json).map_err(|e| ThemeError::ParseError { source: e })?;
        theme.sanitize();
        Ok(theme)
    };

    // 1. Built-in themes
    match name_or_path {
        "dark" => return parse_and_sanitize(include_str!("dark.json")),
        "light" => return parse_and_sanitize(include_str!("light.json")),
        "dracula" => return parse_and_sanitize(include_str!("dracula.json")),
        _ => {}
    }

    // 2. Direct file path
    let path = std::path::Path::new(name_or_path);
    if path.is_file() {
        let content = fs::read_to_string(path)
            .map_err(|e| ThemeError::IoError { path: name_or_path.to_string(), source: e })?;
        return parse_and_sanitize(&content);
    }

    // 3. Config directory
    if let Some(config_dir) = dirs::config_dir() {
        let theme_path = config_dir
            .join("mdink")
            .join("themes")
            .join(format!("{name_or_path}.json"));
        if theme_path.is_file() {
            let content = fs::read_to_string(&theme_path)
                .map_err(|e| ThemeError::IoError { path: theme_path.display().to_string(), source: e })?;
            return parse_and_sanitize(&content);
        }
    }

    // 4. Not found
    Err(ThemeError::NotFound { name: name_or_path.to_string() })
}

/// Returns the default theme (dark) without reading any files.
pub fn default_theme() -> MarkdownTheme {
    MarkdownTheme::default()
}

#[cfg(test)]
mod tests;
