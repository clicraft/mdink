//! LaTeX math rendering engine.
//!
//! This is a **leaf module** — it never imports from other mdink modules.
//! Provides:
//! - `unicode_math()` — LaTeX→Unicode text conversion (moved from parser)
//! - `MathEngine` — async LaTeX→pixel rendering with cache and tracking
//! - `MathRenderRequest` / `MathRenderResult` — channel message types

use std::collections::{HashMap, HashSet};

use image::DynamicImage;

// ── LaTeX-to-Unicode conversion ──────────────────────────────────────────────

/// Best-effort conversion of LaTeX math commands to Unicode characters.
///
/// Handles Greek letters, common operators, arrows, and limited
/// superscript/subscript notation. Unrecognized commands pass through as-is.
pub fn unicode_math(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Collect the command name (alphabetic characters after \).
            let mut cmd = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphabetic() {
                    cmd.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if cmd.is_empty() {
                // Escaped non-alpha character (e.g. \{ \} \#): pass through.
                if let Some(&next) = chars.peek() {
                    result.push(next);
                    chars.next();
                } else {
                    result.push('\\');
                }
            } else {
                // Check for commands that consume brace arguments.
                match cmd.as_str() {
                    // Formatting wrappers: consume {content}, recursively process, output.
                    "text" | "mathrm" | "mathbf" | "mathit" | "mathsf" | "mathtt"
                    | "textbf" | "textit" | "textrm" => {
                        let content = collect_braced_or_single(&mut chars);
                        result.push_str(&unicode_math(&content));
                    }
                    // Fraction: consume {numerator}{denominator}, recursively process, output num/den.
                    "frac" => {
                        let numerator = collect_braced_or_single(&mut chars);
                        let denominator = collect_braced_or_single(&mut chars);
                        result.push_str(&unicode_math(&numerator));
                        result.push('/');
                        result.push_str(&unicode_math(&denominator));
                    }
                    _ => {
                        if let Some(replacement) = latex_command_to_unicode(&cmd) {
                            result.push_str(replacement);
                        } else {
                            // Unrecognized command: pass through as-is.
                            result.push('\\');
                            result.push_str(&cmd);
                        }
                    }
                }
            }
        } else if ch == '^' {
            // Superscript: ^{...} or ^x
            let content = collect_braced_or_single(&mut chars);
            for c in content.chars() {
                result.push(superscript_char(c));
            }
        } else if ch == '_' {
            // Subscript: _{...} or _x
            let content = collect_braced_or_single(&mut chars);
            for c in content.chars() {
                result.push(subscript_char(c));
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Collects content from `{...}` braces or a single character.
fn collect_braced_or_single(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    if chars.peek() == Some(&'{') {
        chars.next(); // consume '{'
        let mut content = String::new();
        let mut depth = 1u32;
        for c in chars.by_ref() {
            if c == '{' {
                depth += 1;
                content.push(c);
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                content.push(c);
            } else {
                content.push(c);
            }
        }
        content
    } else if let Some(&c) = chars.peek() {
        chars.next();
        c.to_string()
    } else {
        String::new()
    }
}

/// Returns true for CJK characters that KaTeX fonts cannot render.
/// These characters are handled correctly by the Unicode text fallback.
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Unified Ideographs Extension A
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility Ideographs
        | '\u{3000}'..='\u{303F}' // CJK Symbols and Punctuation
        | '\u{3040}'..='\u{309F}' // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{AC00}'..='\u{D7AF}' // Hangul Syllables
        | '\u{FF00}'..='\u{FFEF}' // Fullwidth Forms
    )
}

/// Maps a character to its Unicode superscript equivalent, if one exists.
/// Falls back to the original character for unmapped codepoints.
fn superscript_char(c: char) -> char {
    match c {
        '0' => '\u{2070}', '1' => '\u{00B9}', '2' => '\u{00B2}', '3' => '\u{00B3}',
        '4' => '\u{2074}', '5' => '\u{2075}', '6' => '\u{2076}', '7' => '\u{2077}',
        '8' => '\u{2078}', '9' => '\u{2079}', '+' => '\u{207A}', '-' => '\u{207B}',
        '=' => '\u{207C}', '(' => '\u{207D}', ')' => '\u{207E}', 'n' => '\u{207F}',
        'i' => '\u{2071}',
        '\u{2212}' => '\u{207B}', // Unicode minus sign → superscript minus
        _ => c,
    }
}

/// Maps a character to its Unicode subscript equivalent, if one exists.
/// Falls back to the original character for unmapped codepoints.
fn subscript_char(c: char) -> char {
    match c {
        '0' => '\u{2080}', '1' => '\u{2081}', '2' => '\u{2082}', '3' => '\u{2083}',
        '4' => '\u{2084}', '5' => '\u{2085}', '6' => '\u{2086}', '7' => '\u{2087}',
        '8' => '\u{2088}', '9' => '\u{2089}', '+' => '\u{208A}', '-' => '\u{208B}',
        '=' => '\u{208C}', '(' => '\u{208D}', ')' => '\u{208E}',
        'a' => '\u{2090}', 'e' => '\u{2091}', 'o' => '\u{2092}', 'x' => '\u{2093}',
        'h' => '\u{2095}', 'k' => '\u{2096}', 'l' => '\u{2097}', 'm' => '\u{2098}',
        'n' => '\u{2099}', 'p' => '\u{209A}', 's' => '\u{209B}', 't' => '\u{209C}',
        '\u{2212}' => '\u{208B}', // Unicode minus sign → subscript minus
        _ => c,
    }
}

/// Maps a LaTeX command name to its Unicode replacement.
fn latex_command_to_unicode(cmd: &str) -> Option<&'static str> {
    Some(match cmd {
        // Greek lowercase
        "alpha" => "\u{03B1}",
        "beta" => "\u{03B2}",
        "gamma" => "\u{03B3}",
        "delta" => "\u{03B4}",
        "epsilon" => "\u{03B5}",
        "zeta" => "\u{03B6}",
        "eta" => "\u{03B7}",
        "theta" => "\u{03B8}",
        "iota" => "\u{03B9}",
        "kappa" => "\u{03BA}",
        "lambda" => "\u{03BB}",
        "mu" => "\u{03BC}",
        "nu" => "\u{03BD}",
        "xi" => "\u{03BE}",
        "pi" => "\u{03C0}",
        "rho" => "\u{03C1}",
        "sigma" => "\u{03C3}",
        "tau" => "\u{03C4}",
        "upsilon" => "\u{03C5}",
        "phi" => "\u{03C6}",
        "chi" => "\u{03C7}",
        "psi" => "\u{03C8}",
        "omega" => "\u{03C9}",
        // Greek uppercase
        "Gamma" => "\u{0393}",
        "Delta" => "\u{0394}",
        "Theta" => "\u{0398}",
        "Lambda" => "\u{039B}",
        "Xi" => "\u{039E}",
        "Pi" => "\u{03A0}",
        "Sigma" => "\u{03A3}",
        "Phi" => "\u{03A6}",
        "Psi" => "\u{03A8}",
        "Omega" => "\u{03A9}",
        // Operators
        "times" => "\u{00D7}",
        "div" => "\u{00F7}",
        "pm" => "\u{00B1}",
        "mp" => "\u{2213}",
        "cdot" => "\u{00B7}",
        "leq" | "le" => "\u{2264}",
        "geq" | "ge" => "\u{2265}",
        "neq" | "ne" => "\u{2260}",
        "approx" => "\u{2248}",
        "equiv" => "\u{2261}",
        "subset" => "\u{2282}",
        "supset" => "\u{2283}",
        "subseteq" => "\u{2286}",
        "supseteq" => "\u{2287}",
        "in" => "\u{2208}",
        "notin" => "\u{2209}",
        "cup" => "\u{222A}",
        "cap" => "\u{2229}",
        "land" | "wedge" => "\u{2227}",
        "lor" | "vee" => "\u{2228}",
        "neg" | "lnot" => "\u{00AC}",
        "forall" => "\u{2200}",
        "exists" => "\u{2203}",
        "nabla" => "\u{2207}",
        "partial" => "\u{2202}",
        "infty" => "\u{221E}",
        "emptyset" => "\u{2205}",
        // Big operators
        "sum" => "\u{03A3}",
        "prod" => "\u{03A0}",
        "int" => "\u{222B}",
        "iint" => "\u{222C}",
        "iiint" => "\u{222D}",
        "oint" => "\u{222E}",
        "sqrt" => "\u{221A}",
        // Arrows
        "to" | "rightarrow" => "\u{2192}",
        "leftarrow" => "\u{2190}",
        "leftrightarrow" => "\u{2194}",
        "Rightarrow" => "\u{21D2}",
        "Leftarrow" => "\u{21D0}",
        "Leftrightarrow" | "iff" => "\u{21D4}",
        "uparrow" => "\u{2191}",
        "downarrow" => "\u{2193}",
        "mapsto" => "\u{21A6}",
        // Miscellaneous
        "ldots" | "dots" | "cdots" => "\u{2026}",
        "prime" => "\u{2032}",
        "circ" => "\u{2218}",
        "bullet" => "\u{2022}",
        "star" => "\u{22C6}",
        "dagger" => "\u{2020}",
        "ddagger" => "\u{2021}",
        // Spacing commands: collapse to a space.
        "quad" | "qquad" => " ",
        // Formatting wrappers and frac are handled in unicode_math() directly
        // (they consume brace arguments). These are only reached for unrecognized
        // commands via the fallback, so they stay here for documentation clarity.
        "left" | "right" | "bigl" | "bigr" | "Big" | "big" => "",
        _ => return None,
    })
}

// ── MathEngine ───────────────────────────────────────────────────────────────

/// Request sent to the background math render thread.
pub struct MathRenderRequest {
    pub latex: String,
    pub display: bool,
    /// Desired render width in terminal columns.
    pub width_cells: u16,
    /// Font cell size in pixels (width, height).
    pub font_size: (u16, u16),
    /// Terminal background color (RGB) to composite onto transparent SVG output.
    pub bg_color: (u8, u8, u8),
}

/// Result sent back from the background math render thread.
pub enum MathRenderResult {
    /// Successfully rendered LaTeX to pixel image.
    Ok {
        latex: String,
        dyn_img: DynamicImage,
    },
    /// Rendering failed (invalid LaTeX, WASM error, etc.).
    #[allow(dead_code)]
    Err {
        latex: String,
        error: String,
    },
}

/// Owns the formula cache and tracks pending/failed state for async rendering.
///
/// Created once at startup. When `enabled` is false (user disabled via
/// `--no-math-images`, or no graphics protocol available), all formulas
/// stay as Unicode text and no background rendering occurs.
pub struct MathEngine {
    /// Whether async pixel rendering is active.
    enabled: bool,
    /// Cache: LaTeX source → rendered DynamicImage.
    /// Survives re-parse within the same document, cleared on document change.
    cache: HashMap<String, DynamicImage>,
    /// LaTeX strings currently being rendered by the background thread.
    pending: HashSet<String>,
    /// LaTeX strings that failed to render (not retried within same document).
    failed: HashSet<String>,
    /// Set true when cache is modified, cleared after batch re-parse fires.
    cache_dirtied: bool,
}

impl MathEngine {
    /// Creates a new MathEngine.
    ///
    /// - `user_enabled`: true unless user passed `--no-math-images` or config says off.
    /// - `graphics_available`: true if terminal supports a graphics protocol.
    ///
    /// If either is false, `enabled()` returns false.
    pub fn new(user_enabled: bool, graphics_available: bool) -> Self {
        let enabled = user_enabled && graphics_available;
        log::info!(
            "MathEngine: user_enabled={}, graphics_available={}, enabled={}",
            user_enabled, graphics_available, enabled
        );
        Self {
            enabled,
            cache: HashMap::new(),
            pending: HashSet::new(),
            failed: HashSet::new(),
            cache_dirtied: false,
        }
    }

    /// Returns true if async pixel rendering is active.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Toggles async pixel rendering at runtime.
    ///
    /// After toggling ON, callers should re-parse the document and re-queue
    /// pending math renders. After toggling OFF, re-parse produces Unicode text.
    /// The existing cache is preserved — previously rendered images remain available.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Checks the cache for a previously rendered formula.
    pub fn get_cached(&self, latex: &str) -> Option<&DynamicImage> {
        self.cache.get(latex)
    }

    /// Inserts a rendered image into the cache.
    pub fn insert_cache(&mut self, latex: String, dyn_img: DynamicImage) {
        log::info!("cached rendered formula ({}x{} px): {:.40}…", dyn_img.width(), dyn_img.height(), latex);
        self.cache.insert(latex, dyn_img);
        self.cache_dirtied = true;
    }

    /// Marks a formula as pending render. Returns false if already pending or failed.
    pub fn mark_pending(&mut self, latex: &str) -> bool {
        if self.pending.contains(latex) || self.failed.contains(latex) {
            return false;
        }
        self.pending.insert(latex.to_string())
    }

    /// Marks a formula as failed (not retried within same document).
    pub fn mark_failed(&mut self, latex: &str) {
        log::warn!("LaTeX render failed: {:.60}", latex);
        self.pending.remove(latex);
        self.failed.insert(latex.to_string());
        self.cache_dirtied = true;
    }

    /// Marks a formula as resolved (successfully rendered).
    pub fn mark_resolved(&mut self, latex: &str) {
        self.pending.remove(latex);
    }

    /// Returns true if there are formulas still being rendered.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Returns the number of formulas currently pending.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns the number of formulas that failed rendering.
    pub fn failed_count(&self) -> usize {
        self.failed.len()
    }

    /// Returns the number of cached rendered formulas.
    pub fn cache_count(&self) -> usize {
        self.cache.len()
    }

    /// Returns true if the cache was modified since the last batch re-parse.
    pub fn cache_touched(&self) -> bool {
        self.cache_dirtied
    }

    /// Resets the cache-dirtied flag after a batch re-parse fires.
    pub fn clear_cache_touched(&mut self) {
        self.cache_dirtied = false;
    }

    /// Clears protocol-related state but keeps the image cache.
    /// Called before re-parse of the same document (e.g. after images arrive).
    pub fn clear_protocols(&mut self) {
        log::debug!("MathEngine::clear_protocols: cache={}, pending={}, failed={}",
            self.cache.len(), self.pending.len(), self.failed.len());
        // Cache is kept — re-parse will use it.
        // pending/failed are also kept — they track per-URL state within this document.
    }

    /// Clears everything (protocols, cache, pending/failed tracking).
    /// Called when loading a new document.
    pub fn clear_all(&mut self) {
        self.cache.clear();
        self.pending.clear();
        self.failed.clear();
        self.cache_dirtied = false;
    }
}

// ── Rendering backend ────────────────────────────────────────────────────────

/// Maximum LaTeX source size sent to the renderer (10 KB per formula).
const MAX_LATEX_BYTES: usize = 10 * 1024;

/// Renders a LaTeX formula to a pixel image.
///
/// Pipeline:
///   LaTeX → ratex-parser → AST → ratex-layout → DisplayList → ratex-svg → SVG → resvg → Pixmap → DynamicImage
pub fn render_latex_to_image(
    latex: &str,
    display: bool,
    _width_cells: u16,
    font_size: (u16, u16),
    bg_color: (u8, u8, u8),
) -> Result<DynamicImage, Box<dyn std::error::Error + Send + Sync>> {
    if latex.len() > MAX_LATEX_BYTES {
        log::warn!("rejecting LaTeX formula: too large ({} bytes)", latex.len());
        return Err(format!(
            "LaTeX formula too large ({} bytes; limit is {MAX_LATEX_BYTES})",
            latex.len()
        ).into());
    }

    let has_cjk = latex.chars().any(is_cjk);
    render_latex_ratex(latex, display, font_size, bg_color, has_cjk)
}

// ── Real rendering implementation ────────────────────────────────────────────

fn render_latex_ratex(
    latex: &str,
    display: bool,
    font_size: (u16, u16),
    bg_color: (u8, u8, u8),
    has_cjk: bool,
) -> Result<DynamicImage, Box<dyn std::error::Error + Send + Sync>> {
    use std::sync::OnceLock;

    // Ensure fonts are extracted to a temp directory (once).
    static FONT_DIR: OnceLock<String> = OnceLock::new();
    let font_dir = FONT_DIR.get_or_init(extract_katex_fonts);

    // Step 1: Parse LaTeX → AST.
    let ast = ratex_parser::parse(latex)
        .map_err(|e| {
            log::warn!("LaTeX parse error: {e}");
            format!("LaTeX parse error: {e}")
        })?;

    // Step 2: Layout → LayoutBox.
    let lbox = ratex_layout::layout(&ast, &ratex_layout::LayoutOptions::default());

    // Step 3: LayoutBox → DisplayList.
    let display_list = ratex_layout::to_display_list(&lbox);

    // Step 4: DisplayList → SVG string.
    // For CJK formulas, use embed_glyphs=false so ratex-svg emits <text>
    // elements.  resvg's font fallback will use system CJK fonts for Chinese
    // characters that aren't in KaTeX fonts.
    // For pure-math formulas, embed_glyphs=true gives self-contained paths.
    let svg_opts = ratex_svg::SvgOptions {
        font_size: 40.0,
        padding: 10.0,
        stroke_width: 1.0,
        embed_glyphs: !has_cjk,
        font_dir: font_dir.clone(),
    };
    let mut svg_str = ratex_svg::render_to_svg(&display_list, &svg_opts);

    // Step 4b: Override SVG fill color to contrast with terminal background.
    let fg = contrasting_color(bg_color);
    inject_svg_fill_color(&mut svg_str, fg);

    // Step 5: Parse SVG → usvg Tree.
    // Always use a fontdb with KaTeX + system fonts. Even when embed_glyphs=true,
    // some glyphs may still be emitted as <text> elements, so usvg needs the fonts
    // to avoid noisy fallback warnings.
    let svg_opts_usvg = get_svg_options(font_dir);
    let tree = resvg::usvg::Tree::from_str(&svg_str, svg_opts_usvg)
        .map_err(|e| format!("SVG parse error: {e}"))?;

    // Step 6: DPR-scaled render dimensions for crisp output.
    let svg_size = tree.size();
    let dpr = 2.0_f32;
    let width = (svg_size.width() * dpr) as u32;
    let height = (svg_size.height() * dpr) as u32;

    // Step 7: Render SVG → Pixmap filled with terminal background color.
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width.max(1), height.max(1))
        .ok_or("failed to create pixmap")?;
    let (r, g, b) = bg_color;
    pixmap.fill(resvg::tiny_skia::Color::from_rgba8(r, g, b, 255));
    let transform = resvg::tiny_skia::Transform::from_scale(dpr, dpr);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Step 8: Premultiplied RGBA (tiny-skia) → standard RGBA (image crate).
    // Background is opaque so alpha is always 255 for most pixels; conversion
    // mainly affects anti-aliased glyph edges where SVG was composited.
    let mut rgba = Vec::with_capacity(pixmap.data().len());
    for px in pixmap.data().chunks_exact(4) {
        let a = px[3] as u32;
        if a == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            rgba.push(((px[0] as u32 * 255 + a / 2) / a) as u8);
            rgba.push(((px[1] as u32 * 255 + a / 2) / a) as u8);
            rgba.push(((px[2] as u32 * 255 + a / 2) / a) as u8);
            rgba.push(px[3]);
        }
    }

    // Step 9: → DynamicImage.
    let img = image::RgbaImage::from_raw(width.max(1), height.max(1), rgba)
        .ok_or("failed to create image from RGBA data")?;
    let img = image::DynamicImage::ImageRgba8(img);

    // Step 10: Scale inline formulas to exactly 1 terminal row height.
    // Display formulas (display=true) are rendered at their natural size.
    // Inline formulas (display=false) are resized to match the terminal's
    // cell height so they fit within a single text line.
    if !display {
        let (_cell_w, cell_h) = font_size;
        let target_h = cell_h as u32;
        if target_h > 0 && img.height() > target_h {
            let scale = target_h as f32 / img.height() as f32;
            let target_w = (img.width() as f32 * scale).ceil() as u32;
            let resized = image::imageops::resize(
                &img.to_rgba8(),
                target_w.max(1),
                target_h,
                image::imageops::FilterType::Triangle,
            );
            return Ok(image::DynamicImage::ImageRgba8(resized));
        }
    }

    log::debug!("rendered LaTeX ({}x{} px): {:.40}…", img.width(), img.height(), latex);
    Ok(img)
}

/// Returns a cached reference to usvg Options with KaTeX + system fonts loaded.
///
/// Uses `OnceLock` so the fontdb is built only once per process. Shared across
/// all formula renders regardless of CJK content.
fn get_svg_options(font_dir: &str) -> &'static resvg::usvg::Options<'static> {
    use std::sync::OnceLock;
    static SVG_OPTS: OnceLock<resvg::usvg::Options<'static>> = OnceLock::new();
    SVG_OPTS.get_or_init(|| build_svg_font_options(font_dir))
}

/// Builds resvg/usvg Options with a fontdb that includes both KaTeX fonts
/// (for math symbols) and system fonts (for CJK character fallback).
///
/// This allows resvg to render `<text>` elements that mix Latin/math glyphs
/// from KaTeX with CJK glyphs from system fonts via font fallback.
fn build_svg_font_options(katex_font_dir: &str) -> resvg::usvg::Options<'static> {
    use std::sync::Arc;

    let mut db = resvg::usvg::fontdb::Database::new();

    // Load system fonts (provides CJK fonts on systems that have them).
    db.load_system_fonts();

    // Load KaTeX fonts so resvg can resolve font-family="KaTeX_Main" etc.
    if let Ok(entries) = std::fs::read_dir(katex_font_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "ttf") {
                let _ = db.load_font_file(&path);
            }
        }
    }

    resvg::usvg::Options {
        fontdb: Arc::new(db),
        ..resvg::usvg::Options::default()
    }
}

/// Returns a foreground color that contrasts with the given background.
/// Uses perceived luminance to pick white or black text.
fn contrasting_color(bg: (u8, u8, u8)) -> (u8, u8, u8) {
    let luminance = (0.299 * bg.0 as f32 + 0.587 * bg.1 as f32 + 0.114 * bg.2 as f32) / 255.0;
    if luminance > 0.5 {
        (0, 0, 0) // dark text on light background
    } else {
        (255, 255, 255) // light text on dark background
    }
}

/// Injects a CSS style into the SVG to override all fill colors.
/// The SVG from ratex-svg uses black fill by default — this re-colors
/// all path/text elements to the given foreground color.
fn inject_svg_fill_color(svg: &mut String, fg: (u8, u8, u8)) {
    let hex_fg = format!("#{:02x}{:02x}{:02x}", fg.0, fg.1, fg.2);
    let style = format!(
        "<style>path, text, rect, line, circle, ellipse, polyline, polygon {{ fill: {} !important; stroke: {} !important; }}</style>",
        hex_fg, hex_fg
    );
    // Insert the style element right after the opening <svg> tag.
    if let Some(pos) = svg.find('>').map(|p| p + 1) {
        svg.insert_str(pos, &style);
    }
}

/// KaTeX TTF filenames required by ratex-svg standalone mode.
const KATEX_FONT_FILES: &[&str] = &[
    "KaTeX_Main-Regular.ttf",
    "KaTeX_Main-Bold.ttf",
    "KaTeX_Main-Italic.ttf",
    "KaTeX_Main-BoldItalic.ttf",
    "KaTeX_Math-Italic.ttf",
    "KaTeX_Math-BoldItalic.ttf",
    "KaTeX_AMS-Regular.ttf",
    "KaTeX_Caligraphic-Regular.ttf",
    "KaTeX_Fraktur-Regular.ttf",
    "KaTeX_Fraktur-Bold.ttf",
    "KaTeX_SansSerif-Regular.ttf",
    "KaTeX_SansSerif-Bold.ttf",
    "KaTeX_SansSerif-Italic.ttf",
    "KaTeX_Script-Regular.ttf",
    "KaTeX_Typewriter-Regular.ttf",
    "KaTeX_Size1-Regular.ttf",
    "KaTeX_Size2-Regular.ttf",
    "KaTeX_Size3-Regular.ttf",
    "KaTeX_Size4-Regular.ttf",
];

/// Extracts embedded KaTeX fonts to a temp directory and returns its path.
/// Called once via `OnceLock`.
fn extract_katex_fonts() -> String {
    use std::fs;

    let dir = std::env::temp_dir().join("mdink-katex-fonts");
    let _ = fs::create_dir_all(&dir);

    for &filename in KATEX_FONT_FILES {
        if let Some(bytes) = ratex_katex_fonts::ttf_bytes(filename) {
            let path = dir.join(filename);
            // Write if missing or different size (handles updates).
            let should_write = match fs::metadata(&path) {
                Ok(meta) => meta.len() != bytes.len() as u64,
                Err(_) => true,
            };
            if should_write {
                let _ = fs::write(&path, bytes.as_ref());
            }
        }
    }

    dir.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests;
