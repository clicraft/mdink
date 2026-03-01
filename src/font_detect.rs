//! Terminal font detection and resolution for PDF export.
//!
//! Leaf module: no mdink imports (same isolation as `highlight.rs`).
//!
//! Detects the terminal's configured font family from config files, resolves
//! font family names to TTF file paths via `fc-match`, and reads monospace
//! glyph metrics from TTF files for accurate PDF character width calculation.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Font family names as configured in the terminal.
#[derive(Debug)]
pub struct TerminalFonts {
    pub normal: String,
    pub bold: Option<String>,
    pub italic: Option<String>,
    pub bold_italic: Option<String>,
}

/// Resolved TTF file paths for each font variant.
#[derive(Debug)]
pub struct ResolvedFonts {
    pub regular: PathBuf,
    pub bold: Option<PathBuf>,
    pub italic: Option<PathBuf>,
    pub bold_italic: Option<PathBuf>,
}

/// Full pipeline: CLI override > config > terminal auto-detect > None.
pub fn detect_and_resolve(pdf_font: Option<&str>) -> Option<ResolvedFonts> {
    let fonts = match pdf_font {
        Some(family) => TerminalFonts {
            normal: family.to_string(),
            bold: None,
            italic: None,
            bold_italic: None,
        },
        None => detect_terminal_font()?,
    };
    resolve_font_family(&fonts)
}

/// Read monospace width ratio from a TTF file (advance_width / units_per_em).
///
/// Returns the ratio for U+0020 (space), which in monospace fonts equals
/// the advance width of every glyph. Typical values: 0.5–0.6.
pub fn monospace_width_ratio(path: &Path) -> Option<f32> {
    let data = std::fs::read(path).ok()?;
    let face = ttf_parser::Face::parse(&data, 0).ok()?;
    let units_per_em = face.units_per_em() as f32;
    if units_per_em == 0.0 {
        return None;
    }
    let glyph_id = face.glyph_index(' ')?;
    let advance = face.glyph_hor_advance(glyph_id)? as f32;
    Some(advance / units_per_em)
}

// ── Terminal detection ────────────────────────────────────────────────

/// Detects the terminal's font configuration by checking TERM_PROGRAM
/// and terminal-specific env vars.
fn detect_terminal_font() -> Option<TerminalFonts> {
    let term = std::env::var("TERM_PROGRAM").ok().unwrap_or_default();

    match term.as_str() {
        "kitty" => detect_kitty(),
        "Alacritty" | "alacritty" => detect_alacritty(),
        "WezTerm" => detect_wezterm(),
        "ghostty" => detect_ghostty(),
        _ => {
            // Fall back to terminal-specific env vars.
            if std::env::var("KITTY_PID").is_ok() {
                detect_kitty()
            } else if std::env::var("WEZTERM_PANE").is_ok() {
                detect_wezterm()
            } else if std::env::var("GHOSTTY_RESOURCES_DIR").is_ok() {
                detect_ghostty()
            } else {
                None
            }
        }
    }
}

fn detect_kitty() -> Option<TerminalFonts> {
    let path = dirs::config_dir()?.join("kitty").join("kitty.conf");
    let content = std::fs::read_to_string(path).ok()?;
    parse_kitty_content(&content)
}

fn detect_alacritty() -> Option<TerminalFonts> {
    let config_dir = dirs::config_dir()?;
    // Alacritty checks alacritty.toml first, then alacritty.yml (legacy).
    let path = config_dir.join("alacritty").join("alacritty.toml");
    let content = std::fs::read_to_string(path).ok()?;
    parse_alacritty_content(&content)
}

fn detect_wezterm() -> Option<TerminalFonts> {
    let path = dirs::config_dir()?.join("wezterm").join("wezterm.lua");
    let content = std::fs::read_to_string(path).ok()?;
    parse_wezterm_content(&content)
}

fn detect_ghostty() -> Option<TerminalFonts> {
    let path = dirs::config_dir()?.join("ghostty").join("config");
    let content = std::fs::read_to_string(path).ok()?;
    parse_ghostty_content(&content)
}

// ── Config parsers (take &str for testability) ────────────────────────

fn parse_kitty_content(content: &str) -> Option<TerminalFonts> {
    let mut normal = None;
    let mut bold = None;
    let mut italic = None;
    let mut bold_italic = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        // Format: "key  value" (whitespace separated).
        let Some((key, value)) = split_kitty_line(line) else {
            continue;
        };
        let value = if value == "auto" { None } else { Some(value.to_string()) };

        match key {
            "font_family" => normal = value,
            "bold_font" => bold = value,
            "italic_font" => italic = value,
            "bold_italic_font" => bold_italic = value,
            _ => continue,
        }
    }

    Some(TerminalFonts {
        normal: normal?,
        bold,
        italic,
        bold_italic,
    })
}

/// Splits "key  value with spaces" into ("key", "value with spaces").
fn split_kitty_line(line: &str) -> Option<(&str, &str)> {
    let mut iter = line.splitn(2, |c: char| c.is_whitespace());
    let key = iter.next()?;
    let value = iter.next()?.trim();
    if value.is_empty() {
        return None;
    }
    Some((key, value))
}

fn parse_alacritty_content(content: &str) -> Option<TerminalFonts> {
    let table: toml::Value = toml::from_str(content).ok()?;
    let font = table.get("font")?;

    let normal = font
        .get("normal")
        .and_then(|n| n.get("family"))
        .and_then(|f| f.as_str())
        .map(String::from)?;

    let bold = font
        .get("bold")
        .and_then(|b| b.get("family"))
        .and_then(|f| f.as_str())
        .map(String::from);

    let italic = font
        .get("italic")
        .and_then(|i| i.get("family"))
        .and_then(|f| f.as_str())
        .map(String::from);

    let bold_italic = font
        .get("bold_italic")
        .and_then(|bi| bi.get("family"))
        .and_then(|f| f.as_str())
        .map(String::from);

    Some(TerminalFonts {
        normal,
        bold,
        italic,
        bold_italic,
    })
}

fn parse_wezterm_content(content: &str) -> Option<TerminalFonts> {
    // WezTerm configs are Lua, so we use regex-style matching.
    // Look for: wezterm.font("Family Name") or wezterm.font('Family Name')
    // Also handles: wezterm.font { family = "Family Name" }
    let normal = extract_wezterm_font(content)?;

    Some(TerminalFonts {
        normal,
        bold: None,
        italic: None,
        bold_italic: None,
    })
}

/// Extracts the font family from a WezTerm Lua config.
fn extract_wezterm_font(content: &str) -> Option<String> {
    // Pattern 1: wezterm.font("Family Name")  or  wezterm.font('Family Name')
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("font") {
            if let Some(family) = extract_quoted_after(rest) {
                return Some(family);
            }
        }
        // Also check for config.font = wezterm.font(...)
        if line.contains("wezterm.font") {
            if let Some(pos) = line.find("wezterm.font") {
                let after = &line[pos + "wezterm.font".len()..];
                if let Some(family) = extract_quoted_after(after) {
                    return Some(family);
                }
            }
        }
    }
    None
}

/// Extracts a quoted string after optional whitespace and a `(` or `{`.
fn extract_quoted_after(s: &str) -> Option<String> {
    let s = s.trim_start();
    // Skip past ( or {
    let s = s.strip_prefix('(').or_else(|| s.strip_prefix('{'))?;
    let s = s.trim_start();
    // Handle family = "..." inside braces
    let s = if let Some(rest) = s.strip_prefix("family") {
        let rest = rest.trim_start();
        rest.strip_prefix('=')?.trim_start()
    } else {
        s
    };
    // Extract quoted value
    let (quote, rest) = if let Some(r) = s.strip_prefix('"') {
        ('"', r)
    } else if let Some(r) = s.strip_prefix('\'') {
        ('\'', r)
    } else {
        return None;
    };
    let end = rest.find(quote)?;
    let family = rest[..end].to_string();
    if family.is_empty() {
        return None;
    }
    Some(family)
}

fn parse_ghostty_content(content: &str) -> Option<TerminalFonts> {
    let mut normal = None;
    let mut bold = None;
    let mut italic = None;
    let mut bold_italic = None;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        match key {
            "font-family" => normal = Some(value.to_string()),
            "font-family-bold" => bold = Some(value.to_string()),
            "font-family-italic" => italic = Some(value.to_string()),
            "font-family-bold-italic" => bold_italic = Some(value.to_string()),
            _ => continue,
        }
    }

    Some(TerminalFonts {
        normal: normal?,
        bold,
        italic,
        bold_italic,
    })
}

// ── Font resolution via fc-match ──────────────────────────────────────

/// Resolves a `TerminalFonts` to TTF file paths using `fc-match`.
fn resolve_font_family(fonts: &TerminalFonts) -> Option<ResolvedFonts> {
    let regular = fc_match_font(&fonts.normal, false, false)?;

    let bold = match &fonts.bold {
        Some(family) => fc_match_font(family, false, false),
        None => fc_match_font(&fonts.normal, true, false),
    };

    let italic = match &fonts.italic {
        Some(family) => fc_match_font(family, false, false),
        None => fc_match_font(&fonts.normal, false, true),
    };

    let bold_italic = match &fonts.bold_italic {
        Some(family) => fc_match_font(family, false, false),
        None => fc_match_font(&fonts.normal, true, true),
    };

    Some(ResolvedFonts {
        regular,
        bold,
        italic,
        bold_italic,
    })
}

/// Runs `fc-match` to find the TTF file for a font pattern.
///
/// Rejects non-TrueType fonts (printpdf 0.7 requires TrueType).
fn fc_match_font(family: &str, bold: bool, italic: bool) -> Option<PathBuf> {
    let mut pattern = family.to_string();
    if bold {
        pattern.push_str(":weight=bold");
    }
    if italic {
        pattern.push_str(":slant=italic");
    }

    let output = Command::new("fc-match")
        .arg("-f")
        .arg("%{file}\n%{fontformat}\n")
        .arg(&pattern)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let file = lines.next()?.trim();
    let format = lines.next().map(|s| s.trim().to_lowercase());

    // Reject non-TrueType: printpdf 0.7 can only embed TrueType reliably.
    match format.as_deref() {
        Some("truetype" | "cff") => {}
        _ => return None,
    }

    let path = PathBuf::from(file);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kitty_basic() {
        let content = "\
# kitty.conf
font_family      JetBrains Mono
bold_font        auto
italic_font      auto
bold_italic_font auto
font_size 12.0
";
        let fonts = parse_kitty_content(content).unwrap();
        assert_eq!(fonts.normal, "JetBrains Mono");
        assert!(fonts.bold.is_none());
        assert!(fonts.italic.is_none());
        assert!(fonts.bold_italic.is_none());
    }

    #[test]
    fn test_parse_kitty_explicit_variants() {
        let content = "\
font_family      Fira Code
bold_font        Fira Code Bold
italic_font      Fira Code Italic
bold_italic_font Fira Code Bold Italic
";
        let fonts = parse_kitty_content(content).unwrap();
        assert_eq!(fonts.normal, "Fira Code");
        assert_eq!(fonts.bold.as_deref(), Some("Fira Code Bold"));
        assert_eq!(fonts.italic.as_deref(), Some("Fira Code Italic"));
        assert_eq!(fonts.bold_italic.as_deref(), Some("Fira Code Bold Italic"));
    }

    #[test]
    fn test_parse_kitty_no_font_returns_none() {
        let content = "# only comments\nfont_size 14\n";
        assert!(parse_kitty_content(content).is_none());
    }

    #[test]
    fn test_parse_alacritty_basic() {
        let content = r#"
[font]
size = 12.0

[font.normal]
family = "JetBrains Mono"
style = "Regular"
"#;
        let fonts = parse_alacritty_content(content).unwrap();
        assert_eq!(fonts.normal, "JetBrains Mono");
        assert!(fonts.bold.is_none());
    }

    #[test]
    fn test_parse_alacritty_all_variants() {
        let content = r#"
[font.normal]
family = "Fira Code"

[font.bold]
family = "Fira Code"

[font.italic]
family = "Fira Code"

[font.bold_italic]
family = "Fira Code"
"#;
        let fonts = parse_alacritty_content(content).unwrap();
        assert_eq!(fonts.normal, "Fira Code");
        assert_eq!(fonts.bold.as_deref(), Some("Fira Code"));
    }

    #[test]
    fn test_parse_alacritty_no_font_section() {
        let content = r#"
[window]
opacity = 0.9
"#;
        assert!(parse_alacritty_content(content).is_none());
    }

    #[test]
    fn test_parse_ghostty_basic() {
        let content = "\
# Ghostty config
font-family = JetBrains Mono
font-size = 14
";
        let fonts = parse_ghostty_content(content).unwrap();
        assert_eq!(fonts.normal, "JetBrains Mono");
        assert!(fonts.bold.is_none());
    }

    #[test]
    fn test_parse_ghostty_all_variants() {
        let content = "\
font-family = Iosevka
font-family-bold = Iosevka Bold
font-family-italic = Iosevka Italic
font-family-bold-italic = Iosevka Bold Italic
";
        let fonts = parse_ghostty_content(content).unwrap();
        assert_eq!(fonts.normal, "Iosevka");
        assert_eq!(fonts.bold.as_deref(), Some("Iosevka Bold"));
        assert_eq!(fonts.italic.as_deref(), Some("Iosevka Italic"));
        assert_eq!(fonts.bold_italic.as_deref(), Some("Iosevka Bold Italic"));
    }

    #[test]
    fn test_parse_ghostty_no_font_returns_none() {
        let content = "font-size = 14\ntheme = dracula\n";
        assert!(parse_ghostty_content(content).is_none());
    }

    #[test]
    fn test_parse_wezterm_function_call() {
        let content = r#"
local wezterm = require 'wezterm'
local config = {}
config.font = wezterm.font("JetBrains Mono")
return config
"#;
        let fonts = parse_wezterm_content(content).unwrap();
        assert_eq!(fonts.normal, "JetBrains Mono");
    }

    #[test]
    fn test_parse_wezterm_single_quotes() {
        let content = "config.font = wezterm.font('Fira Code')\n";
        let fonts = parse_wezterm_content(content).unwrap();
        assert_eq!(fonts.normal, "Fira Code");
    }

    #[test]
    fn test_parse_wezterm_brace_syntax() {
        let content = r#"config.font = wezterm.font { family = "Iosevka" }"#;
        let fonts = parse_wezterm_content(content).unwrap();
        assert_eq!(fonts.normal, "Iosevka");
    }

    #[test]
    fn test_parse_wezterm_no_font() {
        let content = "config.color_scheme = 'Dracula'\n";
        assert!(parse_wezterm_content(content).is_none());
    }

    #[test]
    fn test_extract_quoted_after_parens() {
        assert_eq!(
            extract_quoted_after("(\"Hello\")"),
            Some("Hello".to_string())
        );
        assert_eq!(
            extract_quoted_after("('World')"),
            Some("World".to_string())
        );
    }

    #[test]
    fn test_extract_quoted_after_braces() {
        assert_eq!(
            extract_quoted_after("{ family = \"Test\" }"),
            Some("Test".to_string())
        );
    }

    #[test]
    fn test_extract_quoted_after_empty() {
        assert!(extract_quoted_after("(\"\")").is_none());
        assert!(extract_quoted_after("no_paren").is_none());
    }

    #[test]
    fn test_monospace_width_ratio_system_font() {
        // Use fc-match to find any monospace TTF on the system.
        let output = Command::new("fc-match")
            .arg("-f")
            .arg("%{file}\n%{fontformat}\n")
            .arg("monospace")
            .output();
        let output = match output {
            Ok(o) if o.status.success() => o,
            _ => return, // skip if fc-match unavailable
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut lines = stdout.lines();
        let file = lines.next().unwrap_or("").trim();
        let format = lines.next().unwrap_or("").trim().to_lowercase();
        if format != "truetype" && format != "cff" {
            return; // skip non-TrueType
        }

        let ratio = monospace_width_ratio(Path::new(file));
        let ratio = ratio.expect("should parse system monospace font");
        assert!(
            (0.3..=0.8).contains(&ratio),
            "ratio {ratio} outside expected 0.3–0.8 range"
        );
    }

    #[test]
    fn test_monospace_width_ratio_nonexistent_file() {
        assert!(monospace_width_ratio(Path::new("/nonexistent.ttf")).is_none());
    }
}
