//! Terminal font detection and resolution for PDF export.
//!
//! Leaf module: no mdink imports (same isolation as `highlight.rs`).
//!
//! Detects the terminal's configured font family from config files, resolves
//! font family names to TTF file paths via `fc-match`, and reads monospace
//! glyph metrics from TTF files for accurate PDF character width calculation.
//!
//! ## Font resolution cascade
//!
//! ```text
//! --pdf-font "Family"  →  all 4 slots use that family
//!         ↓ (not set)
//! TERM_PROGRAM / env vars  →  probe matching terminal's config only
//!         ↓ (no match)
//! JetBrains Mono  →  default (WezTerm's built-in font)
//!         ↓ (not installed)
//! Courier  →  built-in PDF font (last resort, in pdf.rs)
//! ```
//!
//! ## Important: only probe the running terminal
//!
//! `detect_terminal_font()` must NOT blindly probe all config files on disk.
//! On WSL, config files from multiple terminals coexist (e.g. an Alacritty
//! config with Iosevka/Victor Mono alongside WezTerm running JetBrains Mono).
//! Probing by env var (`TERM_PROGRAM`, `WEZTERM_PANE`, `KITTY_PID`, etc.)
//! ensures we only read the config of the terminal that is actually running.

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

/// WezTerm's default font, used as the final fallback when no terminal
/// config is detected. Most modern terminals ship JetBrains Mono or have
/// it installed, making it a better PDF default than Courier.
const DEFAULT_FONT_FAMILY: &str = "JetBrains Mono";

/// Full pipeline: CLI override > config > terminal auto-detect > JetBrains Mono.
///
/// The default fallback explicitly names JetBrains Mono for all four slots
/// so that `fc-match` resolves Regular, Bold, Italic, and BoldItalic variants
/// — matching WezTerm's built-in default font exactly.
pub fn detect_and_resolve(pdf_font: Option<&str>) -> Option<ResolvedFonts> {
    let fonts = match pdf_font {
        Some(family) => TerminalFonts {
            normal: family.to_string(),
            bold: Some(family.to_string()),
            italic: Some(family.to_string()),
            bold_italic: Some(family.to_string()),
        },
        None => detect_terminal_font().unwrap_or_else(default_fonts),
    };
    resolve_font_family(&fonts)
}

/// Returns `TerminalFonts` with JetBrains Mono for all four slots.
fn default_fonts() -> TerminalFonts {
    TerminalFonts {
        normal: DEFAULT_FONT_FAMILY.to_string(),
        bold: Some(DEFAULT_FONT_FAMILY.to_string()),
        italic: Some(DEFAULT_FONT_FAMILY.to_string()),
        bold_italic: Some(DEFAULT_FONT_FAMILY.to_string()),
    }
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
///
/// Only probes config files for the terminal that is actually running.
/// Blindly probing all config files would pick up stale configs from
/// terminals that are installed but not in use (e.g. Alacritty config
/// found on WSL while WezTerm is the active terminal).
fn detect_terminal_font() -> Option<TerminalFonts> {
    let term = std::env::var("TERM_PROGRAM").ok().unwrap_or_default();

    // Direct match on TERM_PROGRAM.
    let result = match term.as_str() {
        "kitty" => detect_kitty(),
        "Alacritty" | "alacritty" => detect_alacritty(),
        "WezTerm" => detect_wezterm(),
        "ghostty" => detect_ghostty(),
        _ => None,
    };
    if result.is_some() {
        return result;
    }

    // Fall back to terminal-specific env vars.
    if std::env::var("KITTY_PID").is_ok() {
        if let Some(f) = detect_kitty() {
            return Some(f);
        }
    }
    if std::env::var("WEZTERM_PANE").is_ok() {
        if let Some(f) = detect_wezterm() {
            return Some(f);
        }
    }
    if std::env::var("GHOSTTY_RESOURCES_DIR").is_ok() {
        if let Some(f) = detect_ghostty() {
            return Some(f);
        }
    }
    if std::env::var("ALACRITTY_WINDOW_ID").is_ok() || std::env::var("COLORTERM").ok().as_deref() == Some("alacritty") {
        if let Some(f) = detect_alacritty() {
            return Some(f);
        }
    }
    if std::env::var("WT_SESSION").is_ok() {
        if let Some(f) = detect_windows_terminal() {
            return Some(f);
        }
    }

    // No terminal identified — caller falls back to default_fonts().
    None
}

fn detect_kitty() -> Option<TerminalFonts> {
    let path = dirs::config_dir()?.join("kitty").join("kitty.conf");
    let content = std::fs::read_to_string(path).ok()?;
    parse_kitty_content(&content)
}

fn detect_alacritty() -> Option<TerminalFonts> {
    // Linux/macOS path.
    let config_dir = dirs::config_dir()?;
    let path = config_dir.join("alacritty").join("alacritty.toml");
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Some(fonts) = parse_alacritty_content(&content) {
            return Some(fonts);
        }
    }
    // WSL: check Windows-side %APPDATA%/alacritty/alacritty.toml.
    if let Some(appdata) = wsl_appdata_dir() {
        let path = appdata.join("alacritty").join("alacritty.toml");
        if let Ok(content) = std::fs::read_to_string(&path) {
            return parse_alacritty_content(&content);
        }
    }
    None
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

/// Detects font from Windows Terminal settings.json (WSL only).
fn detect_windows_terminal() -> Option<TerminalFonts> {
    let localappdata = wsl_localappdata_dir()?;
    // Windows Terminal stores settings in a package directory.
    let packages = localappdata.join("Packages");
    let entries = std::fs::read_dir(&packages).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("Microsoft.WindowsTerminal") {
            let settings = entry
                .path()
                .join("LocalState")
                .join("settings.json");
            if let Ok(content) = std::fs::read_to_string(&settings) {
                if let Some(fonts) = parse_windows_terminal_content(&content) {
                    return Some(fonts);
                }
            }
        }
    }
    None
}

// ── WSL interop helpers ───────────────────────────────────────────────

/// Returns the Windows %APPDATA% directory as a WSL path.
fn wsl_appdata_dir() -> Option<PathBuf> {
    wsl_env_dir("APPDATA")
}

/// Returns the Windows %LOCALAPPDATA% directory as a WSL path.
fn wsl_localappdata_dir() -> Option<PathBuf> {
    wsl_env_dir("LOCALAPPDATA")
}

/// Reads a Windows environment variable via cmd.exe and converts to a WSL path.
fn wsl_env_dir(var: &str) -> Option<PathBuf> {
    let output = Command::new("cmd.exe")
        .args(["/C", &format!("echo %{var}%")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let win_path = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();
    if win_path.is_empty() || win_path.contains('%') {
        return None;
    }
    let wsl_output = Command::new("wslpath")
        .arg(&win_path)
        .output()
        .ok()?;
    if !wsl_output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&wsl_output.stdout)
        .trim()
        .to_string();
    if path.is_empty() {
        return None;
    }
    Some(PathBuf::from(path))
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

fn parse_windows_terminal_content(content: &str) -> Option<TerminalFonts> {
    // Windows Terminal settings.json has:
    //   profiles.defaults.font.face = "FontName"
    //   profiles.list[].font.face = "FontName"
    // We check defaults first, then the active profile.
    let val: serde_json::Value = serde_json::from_str(content).ok()?;
    let profiles = val.get("profiles")?;

    // Check defaults.
    let face = profiles
        .get("defaults")
        .and_then(|d| d.get("font"))
        .and_then(|f| f.get("face"))
        .and_then(|f| f.as_str());

    // If no default font, check each profile in the list.
    let face = face.or_else(|| {
        profiles
            .get("list")
            .and_then(|list| list.as_array())
            .and_then(|arr| {
                arr.iter()
                    .filter_map(|p| p.get("font")?.get("face")?.as_str())
                    .next()
            })
    })?;

    Some(TerminalFonts {
        normal: face.to_string(),
        bold: None,
        italic: None,
        bold_italic: None,
    })
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

/// Public wrapper for tests that need to resolve specific TerminalFonts directly.
#[cfg(test)]
pub fn resolve_font_family_pub(fonts: &TerminalFonts) -> Option<ResolvedFonts> {
    resolve_font_family(fonts)
}

/// Resolves a `TerminalFonts` to TTF file paths using `fc-match`.
fn resolve_font_family(fonts: &TerminalFonts) -> Option<ResolvedFonts> {
    let regular = fc_match_font(&fonts.normal, false, false)?;

    let bold = match &fonts.bold {
        Some(family) => fc_match_font(family, true, false),
        None => fc_match_font(&fonts.normal, true, false),
    };

    let italic = match &fonts.italic {
        Some(family) => fc_match_font(family, false, true),
        None => fc_match_font(&fonts.normal, false, true),
    };

    let bold_italic = match &fonts.bold_italic {
        Some(family) => fc_match_font(family, true, true),
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
    fn test_parse_windows_terminal_defaults_font() {
        let content = r#"{
            "profiles": {
                "defaults": {
                    "font": { "face": "Cascadia Code", "size": 12 }
                },
                "list": []
            }
        }"#;
        let fonts = parse_windows_terminal_content(content).unwrap();
        assert_eq!(fonts.normal, "Cascadia Code");
    }

    #[test]
    fn test_parse_windows_terminal_profile_font() {
        let content = r#"{
            "profiles": {
                "defaults": {},
                "list": [
                    { "name": "PowerShell", "font": { "face": "JetBrains Mono" } },
                    { "name": "CMD" }
                ]
            }
        }"#;
        let fonts = parse_windows_terminal_content(content).unwrap();
        assert_eq!(fonts.normal, "JetBrains Mono");
    }

    #[test]
    fn test_parse_windows_terminal_no_font() {
        let content = r#"{
            "profiles": {
                "defaults": {},
                "list": [{ "name": "PowerShell" }]
            }
        }"#;
        assert!(parse_windows_terminal_content(content).is_none());
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

    #[test]
    fn test_resolve_produces_four_distinct_paths() {
        // Skip if fc-match unavailable.
        if Command::new("fc-match").arg("--version").output().is_err() {
            return;
        }

        let fonts = TerminalFonts {
            normal: "DejaVu Sans Mono".to_string(),
            bold: None,
            italic: None,
            bold_italic: None,
        };
        let Some(resolved) = resolve_font_family(&fonts) else {
            return; // font not installed, skip
        };

        // All four paths should be present and distinct.
        let bold = resolved.bold.as_ref().expect("bold should resolve");
        let italic = resolved.italic.as_ref().expect("italic should resolve");
        let bi = resolved.bold_italic.as_ref().expect("bold_italic should resolve");

        assert_ne!(&resolved.regular, bold, "regular and bold should differ");
        assert_ne!(&resolved.regular, italic, "regular and italic should differ");
        assert_ne!(&resolved.regular, bi, "regular and bold_italic should differ");
        assert_ne!(bold, italic, "bold and italic should differ");
    }

    #[test]
    fn test_resolve_with_per_slot_families() {
        // Simulates a terminal config with different families per slot
        // (like the font slot strategy: JetBrains for normal, different for bold).
        if Command::new("fc-match").arg("--version").output().is_err() {
            return;
        }

        let fonts = TerminalFonts {
            normal: "DejaVu Sans Mono".to_string(),
            bold: Some("DejaVu Sans".to_string()),  // non-mono, different family
            italic: None,
            bold_italic: None,
        };
        let Some(resolved) = resolve_font_family(&fonts) else {
            return;
        };

        let bold = resolved.bold.as_ref().expect("bold should resolve");
        // When bold has an explicit family, fc_match_font requests weight=bold
        // from that family — matching what terminals like Alacritty do.
        assert_ne!(&resolved.regular, bold, "different family should give different path");
    }

    #[test]
    fn test_resolve_four_slot_fonts() {
        // Tests the exact 4-font config from the font slot strategy:
        // normal=JetBrains Mono, bold=Iosevka, italic=Victor Mono, bold_italic=Fira Code.
        if Command::new("fc-match").arg("--version").output().is_err() {
            return;
        }

        let fonts = TerminalFonts {
            normal: "JetBrains Mono".to_string(),
            bold: Some("Iosevka".to_string()),
            italic: Some("Victor Mono".to_string()),
            bold_italic: Some("Fira Code".to_string()),
        };
        let Some(resolved) = resolve_font_family(&fonts) else {
            return; // fonts not installed, skip
        };

        // fc-match always returns *some* file (a fallback like DejaVu) when a
        // family is absent. If the regular slot didn't resolve to JetBrains Mono,
        // the test fonts aren't installed on this machine/CI runner — skip rather
        // than asserting against fallback fonts.
        let reg_name = resolved
            .regular
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        if !reg_name.to_lowercase().contains("jetbrains") {
            eprintln!("skipping test_resolve_four_slot_fonts: JetBrains Mono not installed (got {reg_name})");
            return;
        }

        let bold = resolved.bold.as_ref().expect("bold should resolve");
        let italic = resolved.italic.as_ref().expect("italic should resolve");
        let bi = resolved.bold_italic.as_ref().expect("bold_italic should resolve");

        // All four should be distinct font files.
        let paths = [&resolved.regular, bold, italic, bi];
        for (i, a) in paths.iter().enumerate() {
            for (j, b) in paths.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "slot {i} and {j} should have different font files");
                }
            }
        }
    }

    #[test]
    fn test_parse_alacritty_four_slot_fonts() {
        let content = r#"
[terminal.shell]
program = "wsl.exe"

[font.normal]
family = "JetBrains Mono"

[font.bold]
family = "Iosevka"

[font.italic]
family = "Victor Mono"

[font.bold_italic]
family = "Fira Code"
"#;
        let fonts = parse_alacritty_content(content).unwrap();
        assert_eq!(fonts.normal, "JetBrains Mono");
        assert_eq!(fonts.bold.as_deref(), Some("Iosevka"));
        assert_eq!(fonts.italic.as_deref(), Some("Victor Mono"));
        assert_eq!(fonts.bold_italic.as_deref(), Some("Fira Code"));
    }
}
