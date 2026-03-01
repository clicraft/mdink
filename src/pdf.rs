//! PDF export: text-mode PDF that resembles terminal markdown output.
//!
//! Embeds JetBrains Mono (Regular/Bold/Italic/BoldItalic) by default,
//! matching WezTerm's built-in font. When the terminal's configured font
//! is detected (or overridden via `--pdf-font`), uses those TTF files
//! instead. Falls back to built-in Courier only if no TTF can be loaded.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use printpdf::{BuiltinFont, IndirectFontRef, Mm, PdfDocument, PdfDocumentReference};
use ratatui::style::Modifier;

use crate::font_detect::{self, ResolvedFonts};
use crate::layout::DocumentLine;

/// Font size in points for the monospace output.
const FONT_SIZE_PT: f32 = 9.0;

/// Line height in mm.
const LINE_HEIGHT_MM: f32 = 4.0;

/// Page dimensions: A4 portrait.
const PAGE_WIDTH_MM: f32 = 210.0;
const PAGE_HEIGHT_MM: f32 = 297.0;

/// Standard A4 margins in mm (25mm on all sides).
const MARGIN_LEFT_MM: f32 = 25.0;
const MARGIN_RIGHT_MM: f32 = 25.0;
const MARGIN_TOP_MM: f32 = 20.0;
const MARGIN_BOTTOM_MM: f32 = 20.0;

/// Points-to-mm conversion factor (1pt = 25.4/72 mm).
const PT_TO_MM: f32 = 25.4 / 72.0;

/// Courier's em-width ratio (600/1000).
const COURIER_EM_RATIO: f32 = 0.6;

/// Usable width in mm (page minus left and right margins).
const USABLE_WIDTH_MM: f32 = PAGE_WIDTH_MM - MARGIN_LEFT_MM - MARGIN_RIGHT_MM;

/// Character width in mm for a given em-width ratio at the configured font size.
fn char_width_mm(ratio: f32) -> f32 {
    ratio * FONT_SIZE_PT * PT_TO_MM
}

/// Number of monospace columns that fit within the printable area for a given ratio.
fn usable_columns_for_ratio(ratio: f32) -> u16 {
    (USABLE_WIDTH_MM / char_width_mm(ratio)) as u16
}

/// Number of monospace columns for PDF layout, based on the regular font's width.
///
/// Uses the regular font's glyph metrics when available, falling back to Courier's
/// 0.6 ratio. The regular font determines column count because body text dominates
/// the layout and `textwrap` wraps by character count.
pub fn usable_columns(fonts: Option<&ResolvedFonts>) -> u16 {
    let ratio = fonts
        .and_then(|f| font_detect::monospace_width_ratio(&f.regular))
        .unwrap_or(COURIER_EM_RATIO);
    usable_columns_for_ratio(ratio)
}

/// Four font references used during PDF rendering, each with its own char width.
struct FontSet {
    regular: IndirectFontRef,
    bold: IndirectFontRef,
    italic: IndirectFontRef,
    bold_italic: IndirectFontRef,
    char_width_regular: f32,
    char_width_bold: f32,
    char_width_italic: f32,
    char_width_bold_italic: f32,
}

/// Loads external TTF fonts into the PDF document.
///
/// Computes per-slot character widths from each font file's actual glyph metrics.
/// Missing variants fall back to the regular font (both ref and width).
/// Returns `None` if the regular font cannot be loaded.
fn load_external_fonts(
    doc: &PdfDocumentReference,
    fonts: &ResolvedFonts,
) -> Option<FontSet> {
    let regular = doc
        .add_external_font(&mut File::open(&fonts.regular).ok()?)
        .ok()?;

    let regular_ratio = font_detect::monospace_width_ratio(&fonts.regular)
        .unwrap_or(COURIER_EM_RATIO);

    let (bold, bold_ratio) = load_variant(doc, &fonts.bold, &regular, regular_ratio);
    let (italic, italic_ratio) = load_variant(doc, &fonts.italic, &regular, regular_ratio);
    let (bold_italic, bi_ratio) = load_variant(doc, &fonts.bold_italic, &regular, regular_ratio);

    Some(FontSet {
        regular,
        bold,
        italic,
        bold_italic,
        char_width_regular: char_width_mm(regular_ratio),
        char_width_bold: char_width_mm(bold_ratio),
        char_width_italic: char_width_mm(italic_ratio),
        char_width_bold_italic: char_width_mm(bi_ratio),
    })
}

/// Loads a single font variant, returning the font ref and its width ratio.
/// Falls back to the regular font ref and ratio if the variant is missing.
fn load_variant(
    doc: &PdfDocumentReference,
    path: &Option<PathBuf>,
    fallback: &IndirectFontRef,
    fallback_ratio: f32,
) -> (IndirectFontRef, f32) {
    match path {
        Some(p) => {
            let font_ref = File::open(p)
                .ok()
                .and_then(|mut f| doc.add_external_font(&mut f).ok());
            match font_ref {
                Some(r) => {
                    let ratio = font_detect::monospace_width_ratio(p)
                        .unwrap_or(fallback_ratio);
                    (r, ratio)
                }
                None => (fallback.clone(), fallback_ratio),
            }
        }
        None => (fallback.clone(), fallback_ratio),
    }
}

/// Loads built-in Courier fonts as the fallback font set.
///
/// All Courier variants share the same 0.6 em-width ratio.
fn load_courier_fonts(doc: &PdfDocumentReference) -> Result<FontSet, PdfError> {
    let cw = char_width_mm(COURIER_EM_RATIO);
    Ok(FontSet {
        regular: doc.add_builtin_font(BuiltinFont::Courier)?,
        bold: doc.add_builtin_font(BuiltinFont::CourierBold)?,
        italic: doc.add_builtin_font(BuiltinFont::CourierOblique)?,
        bold_italic: doc.add_builtin_font(BuiltinFont::CourierBoldOblique)?,
        char_width_regular: cw,
        char_width_bold: cw,
        char_width_italic: cw,
        char_width_bold_italic: cw,
    })
}

/// Strips OSC 8 hyperlink escape sequences from text.
///
/// Layout embeds `ESC ] 8 ; ; URL ST text ESC ] 8 ; ; ST` for terminal
/// clickable links. PDF rendering must strip these — they are invisible
/// in terminals but `printpdf` would render them as literal characters.
fn strip_osc8(s: &str) -> String {
    if !s.contains("\x1b]8") {
        return s.to_string();
    }
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Check for `]8;`
            if chars.peek() == Some(&']') {
                chars.next(); // consume `]`
                if chars.peek() == Some(&'8') {
                    chars.next(); // consume `8`
                    // Skip until ST (String Terminator: ESC \ or BEL).
                    for c2 in chars.by_ref() {
                        if c2 == '\x1b' {
                            // ESC \ is the ST — consume the backslash and stop.
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                        if c2 == '\x07' {
                            break; // BEL as ST
                        }
                    }
                    continue;
                }
                // Not OSC 8 — emit what we consumed.
                result.push('\x1b');
                result.push(']');
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Fixes font descriptors that `printpdf` 0.7 generates incorrectly.
///
/// printpdf sets `Flags 32` (Nonsymbolic only) and `ItalicAngle 0` for ALL
/// embedded fonts, regardless of their actual style. PDF viewers use these
/// descriptors to decide whether to use the embedded font program or substitute
/// a system font. Missing `FixedPitch` (bit 0) and `Italic` (bit 6) flags
/// cause viewers like Adobe Reader and Edge to ignore the embedded TTF and
/// substitute a proportional serif font for italic/bold-italic variants.
///
/// This function patches the raw PDF bytes in-place to set correct Flags:
///   F0 (Regular):     Flags 33  (FixedPitch + Nonsymbolic)
///   F1 (Bold):        Flags 33  (FixedPitch + Nonsymbolic)
///   F2 (Italic):      Flags 97  (FixedPitch + Nonsymbolic + Italic)
///   F3 (Bold+Italic): Flags 97  (FixedPitch + Nonsymbolic + Italic)
fn fix_font_descriptors(buf: &mut [u8]) {
    // Font descriptors appear in order F0, F1, F2, F3.
    // Each contains "/FontName/Fn" where n is 0-3.
    // We patch:
    //   "Flags 32" → "Flags 33" (Regular/Bold: add FixedPitch)
    //   "Flags 32" → "Flags 97" (Italic/BoldItalic: add FixedPitch + Italic)
    //
    // Both "33" and "97" are 2-digit, same byte-length as "32" — safe in-place.

    let flags_pattern = b"Flags 32";
    let font_name_pattern = b"/FontName/F";

    let mut pos = 0;
    while let Some(desc_start) = find_bytes(buf, b"/Type/FontDescriptor", pos) {
        // Find which font this descriptor belongs to.
        let search_end = (desc_start + 500).min(buf.len());
        let region = &buf[desc_start..search_end];

        let font_index = if let Some(fn_off) = find_bytes(region, font_name_pattern, 0) {
            let digit_pos = fn_off + font_name_pattern.len();
            if digit_pos < region.len() {
                region[digit_pos] - b'0'
            } else {
                pos = desc_start + 1;
                continue;
            }
        } else {
            pos = desc_start + 1;
            continue;
        };

        // Find "Flags 32" within this descriptor region.
        if let Some(flags_off) = find_bytes(region, flags_pattern, 0) {
            let abs_flags = desc_start + flags_off + 6; // offset of "32" in the buffer
            let new_flags: &[u8; 2] = match font_index {
                0 | 1 => b"33", // Regular, Bold: FixedPitch + Nonsymbolic (1 + 32)
                _ => b"97",     // Italic, Bold+Italic: FixedPitch + Nonsymbolic + Italic (1 + 32 + 64)
            };
            buf[abs_flags] = new_flags[0];
            buf[abs_flags + 1] = new_flags[1];
        }

        pos = desc_start + 1;
    }
}

/// Finds the first occurrence of `needle` in `haystack` starting at `offset`.
fn find_bytes(haystack: &[u8], needle: &[u8], offset: usize) -> Option<usize> {
    if needle.is_empty() || offset + needle.len() > haystack.len() {
        return None;
    }
    haystack[offset..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + offset)
}

/// Exports the document lines as a text-mode PDF file.
///
/// When `fonts` is `Some`, embeds the specified TTF files and computes
/// per-slot character widths from each font's glyph metrics.
/// Falls back to built-in Courier if font loading fails.
pub fn export_pdf(
    lines: &[DocumentLine],
    path: &Path,
    fonts: Option<&ResolvedFonts>,
) -> Result<(), PdfError> {
    let (doc, page_idx, layer_idx) =
        PdfDocument::new("mdink export", Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), "Layer 1");

    let font_set = match fonts.and_then(|f| load_external_fonts(&doc, f)) {
        Some(fs) => fs,
        None => load_courier_fonts(&doc)?,
    };

    let cols = usable_columns_for_ratio(
        font_set.char_width_regular / (FONT_SIZE_PT * PT_TO_MM),
    );

    let mut current_layer = doc.get_page(page_idx).get_layer(layer_idx);
    let mut y = PAGE_HEIGHT_MM - MARGIN_TOP_MM;

    let rule_text = "\u{2500}".repeat(cols as usize);

    for line in lines {
        if y < MARGIN_BOTTOM_MM {
            let (new_page, new_layer) =
                doc.add_page(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), "Layer 1");
            current_layer = doc.get_page(new_page).get_layer(new_layer);
            y = PAGE_HEIGHT_MM - MARGIN_TOP_MM;
        }

        match line {
            DocumentLine::Text(l) | DocumentLine::Code(l) | DocumentLine::AsciiArt(l) => {
                let mut x = MARGIN_LEFT_MM;
                for span in &l.spans {
                    let raw = span.content.to_string();
                    let text = strip_osc8(&raw);
                    if text.is_empty() {
                        continue;
                    }

                    let mods = span.style.add_modifier;
                    let is_bold = mods.contains(Modifier::BOLD);
                    let is_italic = mods.contains(Modifier::ITALIC);

                    let (font, cw) = match (is_bold, is_italic) {
                        (true, true) => (&font_set.bold_italic, font_set.char_width_bold_italic),
                        (true, false) => (&font_set.bold, font_set.char_width_bold),
                        (false, true) => (&font_set.italic, font_set.char_width_italic),
                        (false, false) => (&font_set.regular, font_set.char_width_regular),
                    };

                    current_layer.use_text(&text, FONT_SIZE_PT, Mm(x), Mm(y), font);

                    let char_count = text.chars().count();
                    x += char_count as f32 * cw;
                }
                y -= LINE_HEIGHT_MM;
            }
            DocumentLine::Empty => {
                y -= LINE_HEIGHT_MM;
            }
            DocumentLine::Rule => {
                current_layer.use_text(
                    &rule_text,
                    FONT_SIZE_PT,
                    Mm(MARGIN_LEFT_MM),
                    Mm(y),
                    &font_set.regular,
                );
                y -= LINE_HEIGHT_MM;
            }
            DocumentLine::ImageStart { .. } | DocumentLine::ImageContinuation => {}
        }
    }

    let mut buf = Vec::new();
    doc.save(&mut BufWriter::new(&mut buf))
        .map_err(|e| PdfError::Pdf(e.to_string()))?;

    // printpdf 0.7 sets Flags=32 and ItalicAngle=0 for ALL embedded fonts,
    // even italic variants. This causes PDF viewers to substitute system fonts
    // instead of using the embedded TTF. Fix the descriptors in-place.
    if fonts.is_some() {
        fix_font_descriptors(&mut buf);
    }

    std::fs::write(path, &buf).map_err(|e| PdfError::Io(e.to_string()))?;

    Ok(())
}

/// Errors that can occur during PDF export.
#[derive(Debug)]
pub enum PdfError {
    Io(String),
    Pdf(String),
}

impl std::fmt::Display for PdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfError::Io(msg) => write!(f, "I/O error: {msg}"),
            PdfError::Pdf(msg) => write!(f, "PDF error: {msg}"),
        }
    }
}

impl std::error::Error for PdfError {}

impl From<printpdf::Error> for PdfError {
    fn from(e: printpdf::Error) -> Self {
        PdfError::Pdf(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_detect;
    use ratatui::style::Style;
    use ratatui::text::{Line, Span};
    use std::process::Command;

    /// Build minimal document lines exercising all 4 font slots.
    fn four_slot_lines() -> Vec<DocumentLine> {
        vec![
            // Normal slot
            DocumentLine::Text(Line::from(Span::styled(
                "normal text",
                Style::default(),
            ))),
            // Bold slot
            DocumentLine::Text(Line::from(Span::styled(
                "bold text",
                Style::default().add_modifier(Modifier::BOLD),
            ))),
            // Italic slot
            DocumentLine::Text(Line::from(Span::styled(
                "italic text",
                Style::default().add_modifier(Modifier::ITALIC),
            ))),
            // Bold+Italic slot
            DocumentLine::Text(Line::from(Span::styled(
                "bold italic text",
                Style::default().add_modifier(Modifier::BOLD | Modifier::ITALIC),
            ))),
        ]
    }

    #[test]
    fn test_export_pdf_courier_fallback() {
        let lines = four_slot_lines();
        let path = std::env::temp_dir().join("mdink_test_courier.pdf");
        let result = export_pdf(&lines, &path, None);
        assert!(result.is_ok(), "Courier fallback export failed: {result:?}");

        let bytes = std::fs::read(&path).unwrap();
        let content = String::from_utf8_lossy(&bytes);
        assert!(content.contains("Courier"), "PDF should contain Courier fonts");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_pdf_with_embedded_fonts() {
        // Skip if fc-match unavailable.
        if Command::new("fc-match").arg("--version").output().is_err() {
            return;
        }

        let resolved = font_detect::detect_and_resolve(Some("DejaVu Sans Mono"));
        let Some(resolved) = resolved else {
            return; // font not installed, skip
        };

        let lines = four_slot_lines();
        let path = std::env::temp_dir().join("mdink_test_dejavu.pdf");
        let result = export_pdf(&lines, &path, Some(&resolved));
        assert!(result.is_ok(), "Embedded font export failed: {result:?}");

        let bytes = std::fs::read(&path).unwrap();
        let content = String::from_utf8_lossy(&bytes);
        // Embedded TTF fonts should NOT contain Courier references.
        assert!(
            !content.contains("BaseFont/Courier"),
            "PDF should not use Courier when external fonts are provided"
        );
        // Should contain TrueType font references.
        assert!(
            content.contains("DejaVuSansMono"),
            "PDF should contain DejaVu Sans Mono font name"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_export_pdf_four_distinct_fonts() {
        // Tests PDF export with 4 different font families (one per slot).
        if Command::new("fc-match").arg("--version").output().is_err() {
            return;
        }

        let fonts = font_detect::TerminalFonts {
            normal: "JetBrains Mono".to_string(),
            bold: Some("Iosevka".to_string()),
            italic: Some("Victor Mono".to_string()),
            bold_italic: Some("Fira Code".to_string()),
        };
        let resolved = font_detect::resolve_font_family_pub(&fonts);
        let Some(resolved) = resolved else {
            return; // fonts not installed, skip
        };

        let lines = four_slot_lines();
        let path = std::env::temp_dir().join("mdink_test_four_fonts.pdf");
        let result = export_pdf(&lines, &path, Some(&resolved));
        assert!(result.is_ok(), "Four-font export failed: {result:?}");

        let bytes = std::fs::read(&path).unwrap();
        let content = String::from_utf8_lossy(&bytes);
        // Should NOT contain Courier.
        assert!(
            !content.contains("Courier"),
            "PDF should not use Courier when external fonts are provided"
        );
        // printpdf names embedded fonts F0–F3; verify all 4 are present.
        for i in 0..4 {
            let name = format!("BaseFont/F{i}");
            assert!(content.contains(&name), "PDF should contain {name}");
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_strip_osc8_no_sequences() {
        assert_eq!(strip_osc8("plain text"), "plain text");
    }

    #[test]
    fn test_strip_osc8_link() {
        let input = "\x1b]8;;https://example.com\x1b\\hyperlinks\x1b]8;;\x1b\\";
        assert_eq!(strip_osc8(input), "hyperlinks");
    }

    #[test]
    fn test_strip_osc8_mixed() {
        let input = "before \x1b]8;;https://x.com\x1b\\link\x1b]8;;\x1b\\ after";
        assert_eq!(strip_osc8(input), "before link after");
    }

    #[test]
    fn test_strip_osc8_empty_result() {
        // An OSC 8 open+close with no visible text.
        let input = "\x1b]8;;https://x.com\x1b\\\x1b]8;;\x1b\\";
        assert_eq!(strip_osc8(input), "");
    }

    #[test]
    fn test_strip_osc8_multiple_links() {
        // Multiple OSC 8 links in one string.
        let input = "\x1b]8;;https://github.com\x1b\\GitHub\x1b]8;;\x1b\\ and \x1b]8;;https://crates.io\x1b\\crates.io\x1b]8;;\x1b\\ and \x1b]8;;https://docs.rs\x1b\\docs.rs\x1b]8;;\x1b\\";
        assert_eq!(strip_osc8(input), "GitHub and crates.io and docs.rs");
    }

    #[test]
    fn test_strip_osc8_bel_terminated() {
        // Some terminals use BEL (0x07) as String Terminator instead of ESC\.
        let input = "\x1b]8;;https://example.com\x07link text\x1b]8;;\x07";
        assert_eq!(strip_osc8(input), "link text");
    }

    #[test]
    fn test_strip_osc8_bold_inside_link() {
        // Link containing bold text: open-link, bold-text, close-link.
        let input = "\x1b]8;;https://example.com/release\x1b\\Important\x1b]8;;\x1b\\\x1b]8;;https://example.com/release\x1b\\ release notes\x1b]8;;\x1b\\";
        assert_eq!(strip_osc8(input), "Important release notes");
    }

    #[test]
    fn test_strip_osc8_bare_url_as_text() {
        let input = "\x1b]8;;https://example.com\x1b\\https://example.com\x1b]8;;\x1b\\";
        assert_eq!(strip_osc8(input), "https://example.com");
    }

    #[test]
    fn test_strip_osc8_preserves_non_osc_escapes() {
        // Non-OSC-8 escape sequences should be preserved (not stripped).
        let input = "text\x1b[31mred\x1b[0m";
        assert_eq!(strip_osc8(input), "text\x1b[31mred\x1b[0m");
    }
}
