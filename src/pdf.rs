//! PDF export: text-mode PDF that resembles terminal markdown output.
//!
//! Uses `printpdf` with built-in Courier fonts by default. When external
//! fonts are provided (via `ResolvedFonts`), embeds TTF files so the PDF
//! visually matches the terminal's configured font.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use printpdf::{BuiltinFont, IndirectFontRef, Mm, PdfDocument, PdfDocumentReference};
use ratatui::style::Modifier;

use crate::font_detect::ResolvedFonts;
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
pub fn usable_columns_for_ratio(ratio: f32) -> u16 {
    (USABLE_WIDTH_MM / char_width_mm(ratio)) as u16
}

/// Four font references used during PDF rendering.
struct FontSet {
    regular: IndirectFontRef,
    bold: IndirectFontRef,
    italic: IndirectFontRef,
    bold_italic: IndirectFontRef,
    char_width: f32,
}

/// Loads external TTF fonts into the PDF document.
///
/// Missing variants (bold, italic, bold_italic) fall back to the regular font.
/// Returns `None` if the regular font cannot be loaded.
fn load_external_fonts(
    doc: &PdfDocumentReference,
    fonts: &ResolvedFonts,
    ratio: f32,
) -> Option<FontSet> {
    let regular = doc
        .add_external_font(&mut File::open(&fonts.regular).ok()?)
        .ok()?;

    let bold = fonts
        .bold
        .as_ref()
        .and_then(|p| File::open(p).ok())
        .and_then(|mut f| doc.add_external_font(&mut f).ok())
        .unwrap_or_else(|| regular.clone());

    let italic = fonts
        .italic
        .as_ref()
        .and_then(|p| File::open(p).ok())
        .and_then(|mut f| doc.add_external_font(&mut f).ok())
        .unwrap_or_else(|| regular.clone());

    let bold_italic = fonts
        .bold_italic
        .as_ref()
        .and_then(|p| File::open(p).ok())
        .and_then(|mut f| doc.add_external_font(&mut f).ok())
        .unwrap_or_else(|| regular.clone());

    Some(FontSet {
        regular,
        bold,
        italic,
        bold_italic,
        char_width: char_width_mm(ratio),
    })
}

/// Loads built-in Courier fonts as the fallback font set.
fn load_courier_fonts(doc: &PdfDocumentReference) -> Result<FontSet, PdfError> {
    Ok(FontSet {
        regular: doc.add_builtin_font(BuiltinFont::Courier)?,
        bold: doc.add_builtin_font(BuiltinFont::CourierBold)?,
        italic: doc.add_builtin_font(BuiltinFont::CourierOblique)?,
        bold_italic: doc.add_builtin_font(BuiltinFont::CourierBoldOblique)?,
        char_width: char_width_mm(COURIER_EM_RATIO),
    })
}

/// Exports the document lines as a text-mode PDF file.
///
/// When `fonts` is `Some`, embeds the specified TTF files and uses `ratio`
/// for character width calculation. Falls back to Courier if font loading fails.
pub fn export_pdf(
    lines: &[DocumentLine],
    path: &Path,
    fonts: Option<&ResolvedFonts>,
    ratio: f32,
) -> Result<(), PdfError> {
    let (doc, page_idx, layer_idx) =
        PdfDocument::new("mdink export", Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), "Layer 1");

    let font_set = match fonts.and_then(|f| load_external_fonts(&doc, f, ratio)) {
        Some(fs) => fs,
        None => load_courier_fonts(&doc)?,
    };

    let cols = usable_columns_for_ratio(font_set.char_width / (FONT_SIZE_PT * PT_TO_MM));

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
                    let text = span.content.to_string();
                    if text.is_empty() {
                        continue;
                    }

                    let mods = span.style.add_modifier;
                    let is_bold = mods.contains(Modifier::BOLD);
                    let is_italic = mods.contains(Modifier::ITALIC);

                    let font = match (is_bold, is_italic) {
                        (true, true) => &font_set.bold_italic,
                        (true, false) => &font_set.bold,
                        (false, true) => &font_set.italic,
                        (false, false) => &font_set.regular,
                    };

                    current_layer.use_text(&text, FONT_SIZE_PT, Mm(x), Mm(y), font);

                    let char_count = text.chars().count();
                    x += char_count as f32 * font_set.char_width;
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

    let file = File::create(path).map_err(|e| PdfError::Io(e.to_string()))?;
    doc.save(&mut BufWriter::new(file))
        .map_err(|e| PdfError::Pdf(e.to_string()))?;

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
