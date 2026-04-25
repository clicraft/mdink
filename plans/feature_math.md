# Feature: Hybrid LaTeX Math Rendering

> **Prerequisites:** Phase 4 (images) complete, Feature 7 (basic Unicode math) complete
> **Standards:** All code must follow [standards.md](standards.md)
> **New dependencies:** `ratex-parser`, `ratex-layout`, `ratex-svg` (LaTeX→SVG), `resvg` (SVG→pixels) — optional, feature-gated

**Goal:** Render `$inline$` and `$$display$$` math expressions with a hybrid strategy:
Unicode approximation for instant display, then asynchronous pixel rendering of **ALL**
formulas (both inline and display) via terminal graphics protocols. The page is refreshed
once — only after **every** formula has finished rendering.

---

## Design Summary

```
LaTeX formula encountered during parse
  │
  ├─ 1. Immediate: unicode_math() → StyledSpan  (zero delay, user sees content)
  │     Inline: pushed into current_spans inside paragraph
  │     Display: emitted as RenderedBlock::MathUnicode
  │
  ├─ 2. ALL formulas queued for async rendering (no complexity filter)
  │     Inline formulas tracked via Paragraph-embedded MathSpan markers
  │     Display formulas tracked via MathUnicode.raw_latex
  │
  └─ 3. When ALL formulas finish rendering:
        single re-parse + re-flatten → batch replace everything
        inline:  MathSpan text → in-line image segment
        display: MathUnicode block → MathImage block
```

**Key insight:** This mirrors the existing `ImagePending` → async fetch → cache → re-parse
pattern. The only differences: (a) the "source" is a LaTeX string, not a URL; (b) the
refresh fires only when the pending set becomes empty, not on each result.

---

## 0. Feature Switch

### CLI flag

```rust
/// Disable pixel rendering of LaTeX formulas. When set, all formulas
/// stay as Unicode text approximations and no background rendering occurs.
#[arg(long)]
pub no_math_images: bool,
```

**Default:** katex+resvg async rendering is **enabled**. The flag `--no-math-images` turns
it off.

### Config key

```json
{"math_images": false}
```

`math_images: false` disables pixel rendering (equivalent to `--no-math-images`).
`math_images: true` or key absent → rendering enabled (default).

### Env var

```bash
MDINK_NO_MATH_IMAGES=1
```

### Precedence

CLI `--no-math-images` > env `MDINK_NO_MATH_IMAGES` > config `math_images: false` > default (enabled).

### Behavior matrix

| Condition | Inline formula | Display formula |
|-----------|---------------|-----------------|
| Rendering disabled | Unicode (instant, permanent) | Unicode (instant, permanent) |
| Rendering enabled, no graphics protocol | Unicode (instant, permanent) | Unicode (instant, permanent) |
| Rendering enabled, Halfblocks only | Unicode (instant, permanent) | Unicode (instant, permanent) |
| Rendering enabled, has high-quality graphics (Sixel/Kitty/iTerm2) | Unicode instant → async pixel | Unicode instant → async pixel |
| All formulas finish rendering | Batch refresh: all → pixel images | Batch refresh: all → pixel images |
| Some formulas fail | Failed ones stay Unicode, rest → pixel | Failed ones stay Unicode, rest → pixel |

---

## 1. New IR Types

### `RenderedBlock` additions (`src/parser/mod.rs`)

```rust
/// Display math formula rendered as Unicode text (immediate, universal fallback).
/// Always produced first for display formulas. May be replaced by MathImage
/// after async rendering completes.
MathUnicode {
    content: Vec<StyledSpan>,
    raw_latex: String,     // original LaTeX source (cache key + async render input)
},

/// Display math formula rendered as a pixel image via terminal graphics protocol.
/// Produced after async rendering completes and cache is warm.
MathImage {
    protocol_index: usize,
    width_cells: u16,
    height_cells: u16,
    px_width: u32,
    px_height: u32,
    raw_latex: String,     // preserved for cache key on resize
},
```

### Inline math — no new `RenderedBlock` variant

Inline formulas are embedded inside paragraph `Vec<StyledSpan>`. The parser emits
a special `StyledSpan` with `url: None` and the `raw_latex` stored in a new field:

```rust
pub struct StyledSpan {
    pub text: String,
    pub style: Style,
    pub url: Option<String>,
    /// Non-empty when this span represents an inline math formula.
    /// Contains the original LaTeX source for async rendering cache key.
    /// Empty for all other spans.
    pub math_latex: String,
}
```

During re-parse, if `math_latex` is non-empty and the cache has a rendered image for it,
the layout replaces the span text with a placeholder and the inline image is positioned
at that location (see §6 — Inline Math Layout).

### `DocumentLine` additions (`src/layout/mod.rs`)

No new `DocumentLine` variants:
- `MathUnicode` flattens to `DocumentLine::Text` (same as Paragraph)
- `MathImage` flattens to `DocumentLine::ImageStart` + `ImageContinuation` (same as Image)

This minimizes renderer changes.

---

## 2. MathEngine (`src/math.rs`) — Leaf Module

A new **leaf module** (no imports from other mdink modules, same isolation as
`highlight.rs`, `images.rs`, `font_detect.rs`).

### Module structure

```rust
//! LaTeX math rendering engine.
//!
//! Leaf module — never imports from other mdink modules.
//! Provides:
//! - `unicode_math()` — LaTeX→Unicode text conversion (moved from parser)
//! - `MathEngine` — async LaTeX→pixel rendering with cache
//! - `MathRenderRequest` / `MathRenderResult` — channel message types

use std::collections::{HashMap, HashSet};
use image::DynamicImage;
```

### Key types

```rust
/// Request sent to the background math render thread.
pub struct MathRenderRequest {
    pub latex: String,
    pub display: bool,       // affects DPI/scaling
    /// Desired render width in terminal columns.
    pub width_cells: u16,
    /// Font cell size in pixels (width, height).
    pub font_size: (u16, u16),
}

/// Result sent back from the background math render thread.
pub enum MathRenderResult {
    Ok {
        latex: String,
        dyn_img: DynamicImage,
    },
    Err {
        latex: String,
        error: String,
    },
}

/// Rendering engine state. Caches rendered images and tracks pending/failed formulas.
pub struct MathEngine {
    /// Whether pixel rendering is enabled and available.
    enabled: bool,
    /// Cache: LaTeX source → rendered DynamicImage.
    /// Survives re-parse within the same document, cleared on document change.
    cache: HashMap<String, DynamicImage>,
    /// LaTeX strings currently being rendered by the background thread.
    pending: HashSet<String>,
    /// LaTeX strings that failed to render (not retried within same document).
    failed: HashSet<String>,
}
```

### MathEngine methods

```rust
impl MathEngine {
    /// Creates a new MathEngine.
    /// - `user_enabled`: true unless user passed --no-math-images or config says off.
    /// - `graphics_available`: true if terminal supports Sixel/Kitty/iTerm2.
    /// If either is false, all formulas stay as Unicode text.
    pub fn new(user_enabled: bool, graphics_available: bool) -> Self;

    /// Returns true if async pixel rendering is active.
    pub fn enabled(&self) -> bool;

    /// Checks the cache for a previously rendered formula.
    pub fn get_cached(&self, latex: &str) -> Option<&DynamicImage>;

    /// Inserts a rendered image into the cache.
    pub fn insert_cache(&mut self, latex: String, dyn_img: DynamicImage);

    /// Marks a formula as pending render. Returns false if already pending or failed.
    pub fn mark_pending(&mut self, latex: &str) -> bool;

    /// Marks a formula as failed.
    pub fn mark_failed(&mut self, latex: &str);

    /// Marks a formula as resolved.
    pub fn mark_resolved(&mut self, latex: &str);

    /// Returns true if there are formulas still being rendered.
    pub fn has_pending(&self) -> bool;

    /// Clears protocols but keeps cache (same-document re-parse).
    pub fn clear_protocols(&mut self);

    /// Clears everything (new document).
    pub fn clear_all(&mut self);
}
```

### Rendering pipeline (background thread)

```
MathRenderRequest { latex, bg_color }
  → ratex-parser: LaTeX → AST
  → ratex-layout: AST → LayoutBox → DisplayList
  → ratex-svg: DisplayList → SVG string (embed_glyphs=true)
  → inject_svg_fill_color(): override SVG fill to contrast with bg_color
  → resvg: SVG → usvg Tree → tiny_skia::Pixmap (filled with bg_color, DPR 2.0)
  → premultiplied RGBA → standard RGBA → DynamicImage::ImageRgba8
  → MathRenderResult::Ok { latex, dyn_img }
```

**Terminal background detection:** `main.rs` queries the terminal's background
color via OSC 11 (`\x1b]11;?\x07`) before entering alternate screen. The detected
color is passed as `bg_color` in every `MathRenderRequest`. Falls back to black
if the terminal doesn't respond.

**SVG fill color injection:** ratex-svg generates paths with black fill by
default — invisible on dark terminals. `inject_svg_fill_color()` injects a
`<style>` element into the SVG that overrides all fill/stroke colors to a
contrasting foreground color computed via `contrasting_color(bg)` (white text
on dark backgrounds, black text on light backgrounds).

**Fallback if rendering unavailable:** `MathEngine::enabled()` returns `false`,
render thread is never spawned, all formulas stay as `MathUnicode` / inline spans.

---

## 3. Parser Changes (`src/parser/mod.rs`)

### Source pre-processing: `normalize_math_delimiters()`

pulldown-cmark's math extension requires the opening `$` to be immediately
followed by a non-whitespace character and the closing `$` to be immediately
preceded by a non-whitespace character. Some markdown authors write formulas
like `$ \hat{p}_i$` with a space after the opening `$`, which prevents
pulldown-cmark from recognizing the math expression.

`normalize_math_delimiters()` is called at the top of `parse()` and strips
leading whitespace after `$`/`$$` and trailing whitespace before the matching
closing delimiter. Guards against false positives:

- **Code blocks** (fenced ` ``` ` and inline ` ` `) are left untouched.
- **Escaped dollars** (`\$`) are left untouched.
- **Content between delimiters must not contain `$`** — prevents matching the
  wrong pair when `$` appears in prose like `$5, not $10`.
- **Closing `$` must not be followed by a digit** — prevents currency amounts
  like `$5` from being treated as math.
- **Trailing whitespace heuristic:** when the closing `$` is preceded by
  whitespace, the trimmed content must pass `looks_like_math()` (contains `\`,
  `^`, or `_{`). This handles `$ x^2 $` (both leading+trailing whitespace)
  while rejecting `$ 5, but $x^2$` (prose between delimiters).

**UTF-8 safety:** the function iterates over proper Unicode characters
(`src[i..].chars().next()`) in the default output path, not over raw bytes.
This preserves multi-byte characters (CJK, emoji, etc.) correctly.

### Move `unicode_math()` to `src/math.rs`

The `unicode_math()` function and its helpers (`collect_braced_or_single`,
`superscript_char`, `subscript_char`, `latex_command_to_unicode`) move from
`parser/mod.rs` to `math.rs`. Parser imports `math::unicode_math`.

### Updated event handlers

```rust
// Before (current code):
Event::InlineMath(text) => self.push_inline_math(&text),
Event::DisplayMath(text) => self.push_display_math(&text),

// After:
Event::InlineMath(text) => self.push_math(&text, false),
Event::DisplayMath(text) => self.push_math(&text, true),
```

### Unified handler

```rust
fn push_math(&mut self, text: &str, display: bool) {
    let raw_latex = text.to_string();

    // Step 1: Always produce Unicode approximation (immediate, zero delay).
    let converted = math::unicode_math(text);

    if display {
        // Display math: emit as standalone block.
        // Check cache first — on re-parse with warm cache, upgrade to MathImage.
        if let Some(dyn_img) = self.math.get_cached(&raw_latex) {
            let dyn_img = dyn_img.clone();
            match self.images.load_image_from_memory(dyn_img) {
                Ok((idx, w, h, pw, ph)) => {
                    self.emit_block(RenderedBlock::MathImage {
                        protocol_index: idx,
                        width_cells: w,
                        height_cells: h,
                        px_width: pw,
                        px_height: ph,
                        raw_latex,
                    });
                    return;
                }
                Err(_) => { /* fall through to Unicode */ }
            }
        }
        let style = theme::inline_style(&self.theme.math_display);
        let content = vec![StyledSpan {
            text: format!("$${converted}$$"),
            style,
            url: None,
            math_latex: String::new(), // display math tracked via block, not span
        }];
        self.emit_block(RenderedBlock::MathUnicode { content, raw_latex });
    } else {
        // Inline math: push into current paragraph spans.
        // Store raw_latex in the span for async rendering tracking.
        let style = theme::inline_style(&self.theme.math_inline);
        self.current_spans.push(StyledSpan {
            text: format!("${converted}$"),
            style,
            url: None,
            math_latex: raw_latex, // non-empty → signals "this is an inline formula"
        });
    }
}
```

### `StyledSpan` change

```rust
pub struct StyledSpan {
    pub text: String,
    pub style: Style,
    pub url: Option<String>,
    /// Non-empty for inline math spans. Contains the original LaTeX source.
    /// Used by queue_pending_math_renders() to find inline formulas.
    pub math_latex: String,
}
```

All existing `StyledSpan` construction sites must add `math_latex: String::new()`.

### Parser signature change

```rust
// Current:
pub fn parse(source: &str, highlighter: &Highlighter, images: &mut ImageManager, theme: &MarkdownTheme) -> Vec<RenderedBlock>

// After:
pub fn parse(source: &str, highlighter: &Highlighter, images: &mut ImageManager, math: &mut MathEngine, theme: &MarkdownTheme) -> Vec<RenderedBlock>
```

---

## 4. Main Event Loop Changes (`src/main.rs`)

### New channels

```rust
// Math render thread (mirrors image fetch thread pattern)
let (math_tx, math_rx) = mpsc::channel::<MathRenderRequest>();
let (math_result_tx, math_result_rx) = mpsc::channel::<MathRenderResult>();

if math.enabled() {
    std::thread::spawn(move || {
        while let Ok(req) = math_rx.recv() {
            let result = match render_latex_to_image(
                &req.latex, req.display, req.width_cells, req.font_size,
            ) {
                Ok(dyn_img) => MathRenderResult::Ok { latex: req.latex, dyn_img },
                Err(e) => MathRenderResult::Err { latex: req.latex, error: e.to_string() },
            };
            if math_result_tx.send(result).is_err() {
                break;
            }
        }
    });
}
```

### Queue function — ALL formulas, no filter

```rust
/// Walks the `RenderedBlock` tree and sends ALL formulas (inline + display)
/// to the background render thread. No complexity heuristic — every formula
/// is queued. Skips formulas already cached, pending, or failed.
fn queue_pending_math_renders(
    blocks: &[RenderedBlock],
    tx: &mpsc::Sender<MathRenderRequest>,
    math: &mut MathEngine,
    cols: u16,
    font_size: (u16, u16),
) {
    if !math.enabled() {
        return;
    }
    for block in blocks {
        match block {
            // Display math: queued via MathUnicode.raw_latex
            RenderedBlock::MathUnicode { raw_latex, .. } => {
                if math.get_cached(raw_latex).is_some() { continue; }
                if math.mark_pending(raw_latex) {
                    let _ = tx.send(MathRenderRequest {
                        latex: raw_latex.clone(),
                        display: true,
                        width_cells: cols,
                        font_size,
                    });
                }
            }
            // Already rendered — skip
            RenderedBlock::MathImage { .. } => {}

            // Inline math: queued via StyledSpan.math_latex in Paragraphs and Headings
            RenderedBlock::Paragraph { content } => {
                for span in content {
                    if span.math_latex.is_empty() { continue; }
                    if math.get_cached(&span.math_latex).is_some() { continue; }
                    if math.mark_pending(&span.math_latex) {
                        let _ = tx.send(MathRenderRequest {
                            latex: span.math_latex.clone(),
                            display: false,
                            width_cells: cols,
                            font_size,
                        });
                    }
                }
            }
            RenderedBlock::Heading { content, .. } => {
                for span in content {
                    if span.math_latex.is_empty() { continue; }
                    if math.get_cached(&span.math_latex).is_some() { continue; }
                    if math.mark_pending(&span.math_latex) {
                        let _ = tx.send(MathRenderRequest {
                            latex: span.math_latex.clone(),
                            display: false,
                            width_cells: cols,
                            font_size,
                        });
                    }
                }
            }

            // Recurse into containers
            RenderedBlock::List { items, .. } => {
                for item in items {
                    // Scan item.content for inline math spans (formulas in list items).
                    for span in &item.content {
                        if span.math_latex.is_empty() { continue; }
                        if math.get_cached(&span.math_latex).is_some() { continue; }
                        if math.mark_pending(&span.math_latex) {
                            let _ = tx.send(MathRenderRequest {
                                latex: span.math_latex.clone(),
                                display: false,
                                width_cells: cols,
                                font_size,
                            });
                        }
                    }
                    queue_pending_math_renders(&item.children, tx, math, cols, font_size);
                }
            }
            RenderedBlock::BlockQuote { children } => {
                queue_pending_math_renders(children, tx, math, cols, font_size);
            }
            RenderedBlock::Table { headers, rows, .. } => {
                for cell in headers {
                    if let TableCell::Block(b) = cell {
                        queue_pending_math_renders(std::slice::from_ref(b), tx, math, cols, font_size);
                    }
                }
                for row in rows {
                    for cell in row {
                        if let TableCell::Block(b) = cell {
                            queue_pending_math_renders(std::slice::from_ref(b), tx, math, cols, font_size);
                        }
                    }
                }
            }

            // Leaf blocks with no math content
            RenderedBlock::CodeBlock { .. }
            | RenderedBlock::ThematicBreak
            | RenderedBlock::Spacer { .. }
            | RenderedBlock::Image { .. }
            | RenderedBlock::AsciiImage { .. }
            | RenderedBlock::ImageFallback { .. } => {}
        }
    }
}
```

### Drain math results — wait for ALL to complete

```rust
// Drain completed math renders into cache. Do NOT re-parse yet.
loop {
    match math_result_rx.try_recv() {
        Ok(MathRenderResult::Ok { latex, dyn_img }) => {
            math.mark_resolved(&latex);
            math.insert_cache(latex, dyn_img);
        }
        Ok(MathRenderResult::Err { latex, .. }) => {
            math.mark_failed(&latex);
        }
        Err(mpsc::TryRecvError::Empty) => break,
        Err(mpsc::TryRecvError::Disconnected) => break,
    }
}

// Only refresh when ALL formulas are done (pending set is empty).
if !math.has_pending() && math.cache_touched() {
    image_manager.clear_protocols();
    *blocks = parser::parse(source, highlighter, image_manager, math, &app.theme);
    queue_pending_fetches(blocks, &fetch_tx, image_manager);
    queue_pending_math_renders(blocks, &math_tx, math, cols, font_size);
    let w = compute_content_width(cols, app);
    app.document = layout::flatten(blocks, w, &app.theme);
}
```

**Critical difference from image fetch:** Image fetch re-parses immediately on each
result (`images_arrived = true`). Math rendering **waits until `pending` is empty**
before refreshing. This avoids partial updates and visual flicker.

### How `cache_touched()` works

`MathEngine` tracks a `cache_dirtied: bool` flag:
- Set `true` when `insert_cache()` or `mark_failed()` is called
- Set `false` after the batch re-parse fires
- The `if !math.has_pending() && math.cache_touched()` guard ensures:
  - No refresh when nothing changed
  - No refresh while formulas are still rendering
  - Exactly one refresh when the last formula finishes

### Batch replacement: the re-parse trick

When all math results are in:
1. `MathEngine` cache is warm (contains ALL rendered formulas)
2. Re-parse the entire document
3. Parser encounters `$$...$$` → checks cache → emits `MathImage` for hits
4. Parser encounters `$...$` → checks cache → replaces span text with rendered content
5. Failed formulas: cache miss → stays as `MathUnicode` / inline Unicode span
6. Re-flatten the document with the upgraded blocks

---

## 5. Inline Math → True Inline Image Embedding

The final implementation embeds inline math images **within** text lines (not on
separate lines). This gives a natural reading experience where inline formulas
appear inline, just like in a browser-rendered page.

### Architecture

```
"$E=mc^2$" → StyledSpan { text: "NBSP×3", math_image: Some{protocol_index, width_cells} }
           → wrap_styled_spans() → (Vec<Line>, Vec<Vec<InlineImageMeta>>)
           → DocumentLine::Text + InlineImageEntry{line_index, protocol_index, col_offset, width}
           → renderer: draw text, then overlay StatefulImage at (x+col_offset, y, width, 1)
```

### InlineMathImage struct (parser)

```rust
pub struct InlineMathImage {
    pub protocol_index: usize,
    pub width_cells: u16,
    pub px_width: u32,
    pub px_height: u32,
}
```

Added to `StyledSpan`:
```rust
pub struct StyledSpan {
    pub text: String,
    pub style: Style,
    pub url: Option<String>,
    pub math_latex: String,
    pub math_image: Option<InlineMathImage>,  // Some when re-parse with warm cache
}
```

### Parser behavior on re-parse (cache hit)

```rust
fn push_inline_math(&mut self, text: &str) {
    if let Some(dyn_img) = self.math.get_cached(&raw_latex) {
        match self.images.load_image_from_memory(dyn_img) {
            Ok((idx, w, _h, pw, ph)) => {
                // NBSP characters: non-breaking, count equals width_cells
                self.current_spans.push(StyledSpan {
                    text: "\u{00A0}".repeat(w.max(1) as usize),
                    style, url: None,
                    math_latex: raw_latex,
                    math_image: Some(InlineMathImage { protocol_index: idx, width_cells: w, px_width: pw, px_height: ph }),
                });
                return;
            }
            Err(_) => { /* fall through to Unicode */ }
        }
    }
    // Unicode fallback (cache miss or load failure)
    let converted = crate::math::unicode_math(text);
    self.current_spans.push(StyledSpan {
        text: format!("${converted}$"), style, url: None,
        math_latex: raw_latex, math_image: None,
    });
}
```

**Why NBSP:** Regular spaces would be split by textwrap at word boundaries,
breaking the placeholder. NBSP (`\u{00A0}`) is treated as non-breaking by
textwrap's UnicodeBreakProperties word separator, keeping the entire span
together during word wrapping.

### Inline image metadata in layout

```rust
// Internal to layout — per-line metadata
struct InlineImageMeta {
    protocol_index: usize,
    col_offset: u16,
    width: u16,
}

// Public — part of PreRenderedDocument
pub struct InlineImageEntry {
    pub line_index: usize,
    pub protocol_index: usize,
    pub col_offset: u16,
    pub width: u16,
}
```

`wrap_styled_spans()` returns `(Vec<Line>, Vec<Vec<InlineImageMeta>>)` — a tuple
of wrapped lines and per-line inline image metadata. The `collect_inline_image_metas()`
function walks the byte range of each wrapped line, computing `col_offset` from
display widths.

`flatten()` collects `InlineImageEntry` records with absolute line indices.

### Renderer overlay

In the `DocumentLine::Text` match arm, after rendering the paragraph widget,
the renderer overlays inline math images:

```rust
for entry in &app.document.inline_images {
    if entry.line_index == line_idx {
        let img_rect = Rect {
            x: content_area.x + entry.col_offset,
            y, width: entry.width, height: 1,
        };
        if img_rect.x + img_rect.width <= content_area.x + content_area.width {
            let protocol = images.get_protocol(entry.protocol_index);
            let widget = StatefulImage::default();
            frame.render_stateful_widget(widget, img_rect, protocol);
        }
    }
}
```

### Inline math in list items

List items carry inline content in `ListItem.content: Vec<StyledSpan>`. Three
places in the pipeline must handle inline math metadata from list items:

1. **`queue_pending_math_renders()`** — scans `item.content` for `math_latex`
   spans (in addition to recursing into `item.children`)
2. **`flatten_list()`** — propagates `InlineImageMeta` from `wrap_styled_spans()`,
   shifting `col_offset` by the prefix width (bullet/number + indent)
3. **`flatten()`** — collects `InlineImageEntry` records from `RenderedBlock::List`
   blocks (not just the `_` catch-all arm)

### Inline math image scaling

`render_latex_to_image()` resizes inline formulas (`display=false`) to exactly
`cell_h` pixels height via `image::imageops::resize()`. Display formulas
(`display=true`) are not resized.

### Graceful degradation

- If `load_image_from_memory` fails → falls through to Unicode text (no breakage)
- If the renderer cannot draw the inline image → NBSPs are invisible (no visual regression)
- Failed formulas stay as Unicode permanently (not retried)

---

## 6. Layout Changes (`src/layout/mod.rs`)

### Flattening

```rust
// Display math
RenderedBlock::MathUnicode { content, .. } => {
    // Same as Paragraph — wrap and emit Text lines
    let (wrapped, metas) = wrap_styled_spans(content, width);
    // ... convert to DocumentLine::Text, propagate metas
}

// Math image (display)
RenderedBlock::MathImage { protocol_index, height_cells, .. } => {
    // Same as Image — emit ImageStart + ImageContinuation
}

// Inline math in paragraphs/headings: handled by wrap_styled_spans() returning
// (Vec<Line>, Vec<Vec<InlineImageMeta>>). The metas are threaded through
// flatten_block_with_links() → flatten() → PreRenderedDocument.inline_images.
```

### Inline image metadata propagation

`wrap_styled_spans()` returns `(Vec<Line>, Vec<Vec<InlineImageMeta>>)`. Callers:

| Function | Inline metas handling |
|----------|----------------------|
| `flatten_plain_block()` | Returns metas from `wrap_styled_spans()` |
| `flatten_block_with_links()` | 3-tuple `(lines, links, metas)` |
| `flatten_list()` | **Critical fix:** propagates metas from `item.content`, shifts `col_offset` by prefix width |
| `flatten()` | Collects `InlineImageEntry` with absolute line indices for Paragraph/Heading **and List** blocks |

### List item inline image handling

`flatten_list()` is the most complex case because list items prepend bullet/number
prefixes that shift content right. The prefix width varies between first line and
continuation lines:

- First line: `indent + marker + space` (e.g., `"  • "`)
- Continuation: same width as spaces (e.g., `"    "`)

`col_offset` values from `wrap_styled_spans()` (relative to content start) are
shifted by the prefix width:

```rust
let shifted = line_metas.into_iter().map(|m| InlineImageMeta {
    col_offset: m.col_offset + prefix_width as u16,
    ..m
}).collect();
```

```rust
RenderedBlock::MathUnicode { content, .. } => {
    // Same as Paragraph — wrap and emit Text lines
    let wrapped = wrap_styled_spans(content, width);
    // ... convert to DocumentLine::Text
}

RenderedBlock::MathImage { protocol_index, height_cells, .. } => {
    // Same as Image — emit ImageStart + ImageContinuation
    let mut lines = Vec::with_capacity(*height_cells as usize);
    lines.push(DocumentLine::ImageStart {
        protocol_index: *protocol_index,
        height: *height_cells,
    });
    for _ in 1..*height_cells {
        lines.push(DocumentLine::ImageContinuation);
    }
    lines
}
```

### Link/image extraction updates

`extract_block_links()` and `extract_block_images()` must add exhaustive match
arms for `MathUnicode` and `MathImage` (return empty — math blocks have no
navigable links or images).

### Resize handling

On resize, `MathEngine.clear_protocols()` is called (keeps cache), then
re-parse + re-flatten. The cache is warm, so `MathImage` blocks get new
protocol indices with correct dimensions for the new terminal width.

---

## 7. Renderer Changes (`src/renderer.rs`)

### Display math — no changes

`MathUnicode` becomes `DocumentLine::Text` (already handled). `MathImage`
becomes `DocumentLine::ImageStart` + `ImageContinuation` (already handled).

### Inline math image overlay

In the `DocumentLine::Text` match arm, after rendering the paragraph widget,
the renderer overlays inline math images at their recorded column positions:

```rust
for entry in &app.document.inline_images {
    if entry.line_index == line_idx {
        let img_rect = Rect {
            x: content_area.x + entry.col_offset,
            y, width: entry.width, height: 1,
        };
        if img_rect.x + img_rect.width <= content_area.x + content_area.width {
            let protocol = images.get_protocol(entry.protocol_index);
            let widget = StatefulImage::default();
            frame.render_stateful_widget(widget, img_rect, protocol);
        }
    }
}
```

The bounds check prevents rendering images that overflow the right edge.

---

## 8. Exhaustive Match Sites

Every `match` on `RenderedBlock` must be updated. Locations:

| File | Function | Add arms for |
|------|----------|-------------|
| `src/parser/mod.rs` | `extract_block_links()` | `MathUnicode`, `MathImage` |
| `src/parser/mod.rs` | `extract_block_images()` | `MathUnicode`, `MathImage` |
| `src/layout/mod.rs` | `flatten()` | `MathUnicode`, `MathImage` |
| `src/main.rs` | `queue_pending_fetches()` | `MathUnicode`, `MathImage` |
| `src/main.rs` | `queue_pending_math_renders()` (new) | all variants |

**Rule:** No `_ =>` catch-all on `RenderedBlock`. Every new variant must cause
a compile error that forces updating all match sites.
(See [standards.md §8.3](standards.md))

---

## 9. Dependency Strategy

### Feature-gated dependencies in `Cargo.toml`

```toml
[dependencies]
# ... existing deps ...
ratex-parser = "0.1.2"
ratex-layout = "0.1.2"
ratex-types = "0.1.2"
ratex-svg = { version = "0.1.2", features = ["standalone"] }
ratex-katex-fonts = "0.1.2"
resvg = { version = "0.47", default-features = false, features = ["text", "system-fonts"] }
```

The `text` and `system-fonts` features enable:
- `usvg/text` — fontdb, rustybuzz, ttf-parser for SVG `<text>` element rendering
- `usvg/system-fonts` — fontdb/fs, fontdb/fontconfig for loading system fonts

All rendering dependencies are unconditional (no feature gate). Every build
includes the full ratex-svg + resvg rendering pipeline.

### Rendering pipeline (when feature enabled)

```
LaTeX string
  → ratex_parser::parse()            → Vec<ParseNode>
  → ratex_layout::layout()           → LayoutBox
  → ratex_layout::to_display_list()  → DisplayList
  → ratex_svg::render_to_svg()       → SVG string
       ├─ No CJK: embed_glyphs=true  → SVG <path> elements (self-contained)
       └─ Has CJK: embed_glyphs=false → SVG <text> elements (needs fontdb)
  → inject_svg_fill_color()          → override fill to contrast with terminal bg
  → resvg::usvg::Tree::from_str()    → usvg Tree
       ├─ No CJK: default options
       └─ Has CJK: build_cjk_aware_svg_options() with system + KaTeX fonts
  → resvg::render()                  → tiny_skia::Pixmap (filled with terminal bg, DPR 2.0)
  → premultiplied RGBA → standard RGBA → DynamicImage::ImageRgba8
  → (if inline) resize to cell_h     → height = 1 terminal row
```

KaTeX TTF fonts are embedded via `ratex-katex-fonts` and extracted to a temp
directory (`$TMPDIR/mdink-katex-fonts/`) on first render call. The `standalone`
feature in ratex-svg uses `ab_glyph` + `ratex-font` to convert TTF glyphs into
SVG `<path>` elements — producing fully self-contained SVGs that resvg renders
without any font dependency.

### Terminal background color detection

Terminal graphics protocols (Sixel/Kitty) don't handle transparent pixels — they
render as black, causing black boxes on light terminals. To solve this:

1. **OSC 11 query** — `main.rs` sends `\x1b]11;?\x07` before entering alternate
   screen. Most modern terminals (WezTerm, Kitty, iTerm2, Alacritty) respond with
   their background color in `rgb:RRRR/GGGG/BBBB` format.
2. **Fallback** — if the terminal doesn't respond (200ms timeout), default to black.
3. **Pixmap fill** — the render thread fills the pixmap with the detected bg_color
   before compositing the SVG, so transparent areas match the terminal background.

### SVG fill color injection

ratex-svg generates glyph `<path>` elements with black fill (`#000000`) by default.
On dark terminals, black glyphs are invisible. `inject_svg_fill_color()` solves this:

1. Computes a contrasting foreground color via `contrasting_color(bg)` using perceived
   luminance: `> 0.5 → black text`, else `white text`.
2. Injects `<style>path, text, ... { fill: COLOR !important; stroke: COLOR !important; }</style>`
   right after the opening `<svg>` tag.

### Quality improvements over previous ratex-render pipeline

- **Terminal-matched background** — pixmap filled with detected terminal bg color via OSC 11
- **Contrasting formula color** — SVG fill overridden to white on dark themes, black on light themes
- **DPR 2.0 scaling** — resvg renders at 2x resolution for crisp output on high-DPI terminals
- **Resolution-independent glyphs** — `<path>` outlines scale to any size without pixelation
- **No white/black rectangles** — background matches terminal exactly; no jarring color mismatch

### CJK font support

KaTeX fonts contain no CJK glyphs. Formulas with `\text{中文}` would render with
missing characters when using `embed_glyphs=true`. The fix uses a hybrid approach:

1. **CJK detection:** `is_cjk()` checks for CJK Unified Ideographs, Hiragana,
   Katakana, Hangul, Fullwidth Forms, and CJK Symbols ranges.

2. **SVG generation:** When the formula contains CJK, `embed_glyphs` is set to
   `false`. This makes ratex-svg emit SVG `<text>` elements instead of embedded
   glyph paths. CJK characters are preserved as text content in the SVG.

3. **Font database:** `build_cjk_aware_svg_options()` creates a resvg fontdb with:
   - System fonts via `db.load_system_fonts()` (provides CJK fonts)
   - KaTeX fonts from the temp directory (for math symbol resolution)

4. **Font fallback:** When resvg encounters a CJK character in a `<text
   font-family="KaTeX_Main">` element, it tries KaTeX_Main first (no CJK glyph),
   then falls back to system CJK fonts automatically. This produces correctly
   rendered Chinese text alongside math symbols.

5. **Unicode math fallback:** `unicode_math()` handles `\text{中文}` correctly —
   Chinese characters pass through unchanged. `\frac` and formatting commands
   recursively process their brace arguments.

For pure-math formulas (no CJK), `embed_glyphs=true` is still used — self-contained
SVG paths with no font dependency, matching the original behavior.

### Unicode math enhancements

Several LaTeX commands are handled beyond basic symbol mapping:

| Command | Behavior |
|---------|----------|
| `\text{...}`, `\mathrm{...}`, etc. | Consumes brace content, recursively processes via `unicode_math()` |
| `\frac{a}{b}` | Consumes two brace arguments, outputs `numerator/denominator` with recursive processing |
| Unicode minus `−` (U+2212) | Maps to subscript `₋` or superscript `⁻` when inside `_{...}` or `^{...}` |
| `∣` (U+2223), `−` (U+2212) | Pass through correctly in formulas (these appear in Chinese math textbooks) |

`MathEngine::new()` checks both user preference and graphics availability:

```rust
impl MathEngine {
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
}
```

**Halfblocks fallback:** `graphics_available` is `true` only for high-quality
protocols (Sixel, Kitty, iTerm2). Halfblocks — which renders at only 2 vertical
pixels per cell — is treated as unavailable for math, so formulas fall back to
sharper Unicode text instead of blurry halfblock images. This decision is made
in `main.rs`:

```rust
let math_has_high_quality = picker.as_ref().is_some_and(|p| {
    p.protocol_type() != ProtocolType::Halfblocks
});
let mut math_engine = math::MathEngine::new(math_user_enabled, math_has_high_quality);
```

---

## 10. File Changes Summary

| File | Action | Changes |
|------|--------|---------|
| `src/math/mod.rs` | **New** | `unicode_math()` (moved from parser), `MathEngine`, `MathRenderRequest/Result`, `render_latex_to_image()`, `is_cjk()`, `build_cjk_aware_svg_options()`, inline image resize |
| `src/math/tests.rs` | **New** | Unit tests for unicode_math, MathEngine, rendering (including CJK) |
| `src/parser/mod.rs` | Modified | Remove unicode_math, import from math, add `MathUnicode`/`MathImage` variants, `InlineMathImage` struct, update `push_inline_math()` (NBSP placeholder), `push_display_math()`, add `math_latex` and `math_image` to `StyledSpan`, `normalize_math_delimiters()` pre-processing, `looks_like_math()` heuristic |
| `src/parser/tests.rs` | Modified | Update parse() call sites, add `math_latex`/`math_image` fields to span construction, normalize_math tests (leading/trailing whitespace, CJK preservation, false-positive guards) |
| `src/layout/mod.rs` | Modified | `InlineImageMeta`, `InlineImageEntry` structs, `wrap_styled_spans()` returns 2-tuple with metas, `collect_inline_image_metas()`, `flatten_list()` propagates metas with prefix offset, `flatten()` collects entries for List blocks |
| `src/layout/tests.rs` | Modified | Update all span construction with `math_latex`/`math_image` fields |
| `src/renderer.rs` | Modified | Inline math image overlay in `DocumentLine::Text` arm via `StatefulImage` |
| `src/main.rs` | Modified | MathEngine init, math render thread, drain loop, `queue_pending_math_renders()` (including `ListItem.content` scan), batch refresh, OSC 11 bg detection, bg_color pass-through |
| `src/app/mod.rs` | Modified | `inline_images: Vec<InlineImageEntry>` in `PreRenderedDocument` |
| `src/app/tests.rs` | Modified | `inline_images: Vec::new()` in all `PreRenderedDocument` constructions |
| `src/cli.rs` | Modified | Add `--no-math-images` flag |
| `src/config.rs` | Modified | Add `math_images` config key |
| `Cargo.toml` | Modified | Add optional deps with `resvg = { features = ["text", "system-fonts"] }` |
| `CLAUDE.md` | Modified | Inline math invariants (placeholder, rendering, col_offset, tracking) |
| `plans/feature_math.md` | Modified | Updated to reflect actual implementation |
| `plans/overview.md` | Modified | Updated feature_math.md description |

---

## 11. Implementation Order

### Step 1: Extract math module (zero-behavior-change refactor)

1. Create `src/math/mod.rs` with `pub fn unicode_math()` (moved from parser)
2. Create `src/math/tests.rs` (move existing tests from parser/tests.rs)
3. Update `parser/mod.rs` to import `crate::math::unicode_math`
4. Run `cargo test`, `cargo clippy -- -D warnings` — must be clean

### Step 2: Add `math_latex` field to `StyledSpan`

1. Add `pub math_latex: String` to `StyledSpan`
2. Update ALL `StyledSpan { .. }` construction sites with `math_latex: String::new()`
3. Inline math sets `math_latex: raw_latex`
4. Run tests — behavior unchanged (new field is always empty except inline math)

### Step 3: Add new `RenderedBlock` variants

1. Add `MathUnicode`, `MathImage` to `RenderedBlock` enum
2. Update ALL exhaustive match sites (parser, layout, main)
3. Display math now emits `MathUnicode` instead of `Paragraph`
4. Run tests — display math tests updated to match new variant

### Step 4: Thread `MathEngine` through pipeline

1. Create `MathEngine` struct (cache + pending/failed tracking, no rendering yet)
2. Add `math: &mut MathEngine` param to `parser::parse()`
3. In parser: display math checks `math.get_cached()` → emits `MathImage` or `MathUnicode`
4. Wire through `main.rs`
5. Run tests — behavior unchanged (MathEngine cache is empty)

### Step 5: Background render thread + batch refresh

1. Add `MathRenderRequest/Result` types
2. Spawn render thread in `main.rs`
3. Add `queue_pending_math_renders()` — queues ALL formulas (inline via `math_latex`, display via `MathUnicode`)
4. Add drain loop: collects results into cache, does NOT re-parse until `has_pending() == false`
5. Batch refresh: `!math.has_pending() && math.cache_touched()` → re-parse + re-flatten
6. Test with mock renderer

### Step 6: Inline math image rendering

1. Parser re-parse: inline cache hit → emit `MathImage` as separate block (simplified approach)
2. Layout: `MathImage` from inline math is smaller but uses same `ImageStart`/`ImageContinuation`
3. Run tests — verify inline formulas upgrade correctly

### Step 7: Real rendering backend

1. Add `ratex-svg` (with `standalone` feature) + `resvg` as optional dependencies
2. Implement `render_latex_to_image()` in math.rs using SVG pipeline
3. Pipeline: ratex-parser → ratex-layout → ratex-svg → inject fill color → resvg → Pixmap → DynamicImage
4. Terminal bg detection via OSC 11, pixmap filled with detected bg_color
5. SVG fill color injection via `<style>` element for contrasting formula color
6. DPR 2.0 scaling, premultiplied→straight alpha conversion
7. Integration test with real LaTeX formulas

### Step 8: CLI and config integration

1. Add `--no-math-images` flag to `cli.rs`
2. Add `math_images` config key to `config.rs`
3. Add `MDINK_NO_MATH_IMAGES` env var
4. Update precedence chain in `main.rs`

### Step 9: Test data and comprehensive tests

1. Expand `testdata/math.md`
2. Add unit tests for all new code
3. Integration tests for the full pipeline
4. Verify resize behavior
5. Verify `--no-math-images` flag behavior

---

## 12. Test Data Specification

### `testdata/math.md` (expanded)

```markdown
# Math Rendering Tests

## Inline formulas

The identity $E = mc^2$ changed physics.
Variables $\alpha$, $\beta$, and $\gamma$ are Greek letters.
Sum notation $\sum_{i=0}^{n} x_i$.
A fraction $\frac{a}{b}$ inline.
Matrix $\begin{pmatrix} a & b \\ c & d \end{pmatrix}$ inline.

## Display formulas

$$E = mc^2$$

$$\alpha + \beta = \gamma$$

## Complex display formulas

### Fractions

$$\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}$$

$$\frac{\partial f}{\partial x} = \lim_{h \to 0} \frac{f(x+h) - f(x)}{h}$$

### Matrices

$$\begin{pmatrix} a & b \\ c & d \end{pmatrix}$$

$$\begin{bmatrix} 1 & 2 & 3 \\ 4 & 5 & 6 \\ 7 & 8 & 9 \end{bmatrix}$$

### Aligned equations

$$\begin{aligned}
\nabla \cdot \mathbf{E} &= \frac{\rho}{\epsilon_0} \\
\nabla \cdot \mathbf{B} &= 0 \\
\nabla \times \mathbf{E} &= -\frac{\partial \mathbf{B}}{\partial t} \\
\nabla \times \mathbf{B} &= \mu_0 \mathbf{J} + \mu_0 \epsilon_0 \frac{\partial \mathbf{E}}{\partial t}
\end{aligned}$$

### Cases

$$f(x) = \begin{cases} x^2 & \text{if } x \geq 0 \\ -x^2 & \text{if } x < 0 \end{cases}$$

### Limits and integrals

$$\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}$$

$$\lim_{n \to \infty} \left(1 + \frac{1}{n}\right)^n = e$$

## Mixed: inline + display in same paragraph

The energy $E = mc^2$ leads to:

$$E = \frac{mc^2}{\sqrt{1 - \frac{v^2}{c^2}}}$$

where $v$ is velocity and $c$ is the speed of light.

## Edge cases

Just text: $\text{hello world}$
Unrecognized command: $\foobar$
Escaped braces: $\{x\}$
Multiple inline: $a^2 + b^2 = c^2$ and $e^{i\pi} + 1 = 0$

## Math inside containers

- List item with $x^2$ inline
- Another item with $\frac{1}{2}$

> Blockquote with $\alpha + \beta$ inline

| Column 1 | Column 2 |
|----------|----------|
| $x^2$    | $y^2$    |
```

---

## 13. Test Cases

### Unit tests — `src/math/tests.rs`

```rust
// ── unicode_math tests (moved from parser) ──

#[test] fn test_unicode_math_greek_letters()
#[test] fn test_unicode_math_operators()
#[test] fn test_unicode_math_superscript()
#[test] fn test_unicode_math_subscript()
#[test] fn test_unicode_math_arrows()
#[test] fn test_unicode_math_unrecognized_passthrough()
#[test] fn test_unicode_math_escaped_chars()
#[test] fn test_unicode_math_empty_input()
#[test] fn test_unicode_math_frac_passthrough()   // \frac → /
#[test] fn test_unicode_math_combined()            // x^2 + \alpha_0

// ── MathEngine tests ──

#[test] fn test_math_engine_disabled_when_no_graphics()
#[test] fn test_math_engine_cache_insert_and_get()
#[test] fn test_math_engine_pending_dedup()
#[test] fn test_math_engine_failed_not_retried()
#[test] fn test_math_engine_has_pending_true_while_rendering()
#[test] fn test_math_engine_has_pending_false_when_all_done()
#[test] fn test_math_engine_clear_protocols_keeps_cache()
#[test] fn test_math_engine_clear_all_resets_everything()
#[test] fn test_math_engine_cache_dirtied_flag()
```

### Unit tests — `src/parser/tests.rs`

```rust
#[test] fn test_parser_inline_math_produces_span_with_math_latex()
#[test] fn test_parser_display_math_produces_math_unicode_block()
#[test] fn test_parser_display_math_cached_produces_math_image_block()
#[test] fn test_parser_inline_math_cached_emits_math_image_block()
#[test] fn test_parser_inline_math_uncached_emits_unicode_span()
#[test] fn test_parser_math_in_list()
#[test] fn test_parser_math_in_blockquote()
#[test] fn test_parser_math_in_table()
#[test] fn test_parser_multiple_inline_math_in_paragraph()
#[test] fn test_parser_mixed_inline_and_display()
```

### Unit tests — `src/layout/tests.rs`

```rust
#[test] fn test_layout_math_unicode_wraps_like_paragraph()
#[test] fn test_layout_math_image_emits_image_start_continuation()
#[test] fn test_layout_math_unicode_width_clamp()
```

### Integration tests

```rust
#[test] fn test_render_math_testdata_no_panic()
#[test] fn test_math_resize_reparse_uses_cache()
#[test] fn test_math_all_queued_and_batch_replaced()
#[test] fn test_math_inline_and_display_both_rendered()
#[test] fn test_math_no_math_images_flag_stays_unicode()
#[test] fn test_math_failed_formulas_stay_unicode()
```

---

## 14. Invariants to Preserve

- **MathEngine is a leaf module:** `math.rs` never imports from other mdink modules.
  Same isolation as `highlight.rs`, `images.rs`, `font_detect.rs`.
- **Exhaustive match on RenderedBlock:** No `_ =>` catch-all. New variants
  `MathUnicode` and `MathImage` must be handled in every match.
- **ALL formulas queued:** No complexity heuristic. Every formula (inline + display)
  is sent to the render thread.
- **Batch refresh:** Re-parse fires only when `pending` is empty AND cache was touched.
  No partial updates. No flicker.
- **Inline math upgrade path:** Inline formulas start as Unicode text in paragraph spans.
  After batch render, they are re-emitted as `MathImage` blocks (simplified: on their
  own line, not embedded in text flow).
- **Cache survives re-parse:** `clear_protocols()` keeps the math cache.
  `clear_all()` clears everything (called on document change).
- **Pending/failed dedup:** Formulas are sent to the render thread only once.
  Failed formulas are tracked and not retried within the same document.
- **Default ON:** katex+resvg rendering is enabled by default. `--no-math-images`
  or `math_images: false` disables it.
- **Zero overhead when disabled:** `--no-math-images` → no MathEngine init,
  no render thread, no WASM runtime. All formulas are instant Unicode.
- **Size guard:** LaTeX input to the render engine bounded at 10KB per formula.
- **Width clamp:** `layout.rs` clamps width to >= 1 for MathUnicode wrapping.
- **Terminal restore invariant preserved:** TERMINAL_ACTIVE flag untouched by math changes.
- **StyledSpan backward compatible:** New `math_latex: String` field is `String::new()`
  for all non-math spans. All existing construction sites updated.
- **Math delimiter normalization:** `normalize_math_delimiters()` runs before
  pulldown-cmark parsing. It preserves multi-byte UTF-8 characters (uses
  `src[i..].chars().next()`, not `b[i] as char`). Code blocks and inline code
  are skipped. Content between delimiters must not contain `$`. Closing `$`
  must not be followed by a digit.
- **Terminal bg detection:** OSC 11 query runs once at startup before alternate screen.
  Detected color is passed to all `MathRenderRequest` instances. If detection fails,
  falls back to (0,0,0) (black — safe default for dark terminals). Result is logged
  at `info` level via `log::info!`.
- **Diagnostics logging:** All key decisions in the terminal→math pipeline are logged
  at `info` level: terminal identity, picker result (protocol + font size), protocol
  quality classification, bg color detection, and MathEngine enable/disable state.
  Invisible at default log level (`Warn`) — visible with `--log-level info`.
- **Halfblocks math fallback:** When `ProtocolType::Halfblocks` is detected,
  `graphics_available` is set to `false` for MathEngine. Halfblocks renders images at
  only 2 vertical pixels per cell — too coarse for legible formulas. Unicode text is
  sharper. Regular images (ImageManager) are unaffected.
- **Formula color contrast:** `contrasting_color(bg)` chooses white or black text
  based on perceived luminance. `inject_svg_fill_color()` ensures SVG glyphs are
  always visible against the terminal background.

---

## Definition of Done

- [x] `src/math/mod.rs` created as leaf module with `unicode_math()` (moved from parser)
- [x] `src/math/tests.rs` with all unicode_math tests moved and passing
- [x] `StyledSpan` has `math_latex: String` field, all construction sites updated
- [x] `StyledSpan` has `math_image: Option<InlineMathImage>` field for inline image metadata
- [x] `InlineMathImage` struct with `protocol_index`, `width_cells`, `px_width`, `px_height`
- [x] `RenderedBlock::MathUnicode` and `RenderedBlock::MathImage` variants added
- [x] All exhaustive match sites updated (parser, layout, main) — no `_ =>` catch-all
- [x] `MathEngine` struct with cache, pending/failed tracking, `has_pending()`, `cache_touched()`
- [x] Background math render thread spawned in main.rs (only when enabled)
- [x] `queue_pending_math_renders()` queues ALL formulas (inline via `math_latex`, display via `MathUnicode`)
- [x] `queue_pending_math_renders()` scans `ListItem.content` for inline math (not just `children`)
- [x] Drain loop collects results; re-parse fires ONLY when `!has_pending() && cache_touched()`
- [x] Parser checks math cache on re-parse → emits `MathImage` for display formulas, inline `StyledSpan` with `InlineMathImage` for inline formulas
- [x] Inline math uses NBSP (`\u{00A0}`) placeholder text, count equals `width_cells`
- [x] Layout handles `MathUnicode` (Text lines) and `MathImage` (ImageStart/Continuation)
- [x] `wrap_styled_spans()` returns `(Vec<Line>, Vec<Vec<InlineImageMeta>>)` with inline image positions
- [x] `InlineImageEntry` with `(line_index, protocol_index, col_offset, width)` in `PreRenderedDocument`
- [x] `flatten_list()` propagates inline image metas, shifts `col_offset` by prefix width
- [x] `flatten()` collects `InlineImageEntry` for Paragraph/Heading AND List blocks
- [x] Renderer overlays `StatefulImage` at `(content_area.x + col_offset, y, width, 1)` for inline math
- [x] Inline math images scaled to 1 terminal row height via `image::imageops::resize()`
- [x] `--no-math-images` CLI flag, `MDINK_NO_MATH_IMAGES` env var, `math_images` config key
- [x] Default behavior: async rendering enabled
- [x] `testdata/math.md` expanded with inline, display, and container formulas
- [x] All new tests pass (~494 total)
- [x] `cargo clippy -- -D warnings` clean (with and without feature)
- [x] Phase 1–4 features still work (no regressions)
- [x] MathEngine is a leaf module (verified: no mdink-internal imports)
- [x] Unconditional dependencies (ratex-parser, ratex-layout, ratex-types, ratex-svg, ratex-katex-fonts, resvg) — no feature gate
- [x] Zero overhead when --no-math-images is set — no MathEngine init, no render thread
- [x] Real rendering backend: ratex-parser → ratex-layout → ratex-svg → resvg pipeline (feature-gated)
- [x] KaTeX fonts embedded via ratex-katex-fonts, extracted to temp dir on first use
- [x] Transparent background (SVG default, no white fill)
- [x] DPR 2.0 scaling for crisp output on high-DPI terminals
- [x] Self-contained SVG with embedded glyph paths (ratex-svg standalone + embed_glyphs: true)
- [x] Premultiplied RGBA → standard RGBA conversion for image crate compatibility
- [x] Terminal background color detection via OSC 11 escape sequence (fallback to black)
- [x] Pixmap filled with detected terminal_bg_color before SVG compositing
- [x] SVG fill color injection: contrasting_color() computes white/black fg from bg luminance
- [x] inject_svg_fill_color() overrides SVG path fill via injected <style> element
- [x] MathRenderRequest includes bg_color field, threaded through all render queue sites
- [x] Works correctly on both dark and light terminal themes (WezTerm, Kitty, etc.)
- [x] **CJK font support:** resvg configured with `text` + `system-fonts` features
- [x] **CJK font fallback:** `build_cjk_aware_svg_options()` loads KaTeX + system fonts into fontdb
- [x] **CJK SVG generation:** `embed_glyphs=false` for CJK formulas → `<text>` elements
- [x] **CJK formula rendering:** system CJK fonts render Chinese characters via resvg font fallback
- [x] **Unicode math `\text{}`:** recursively processes brace content, handles Chinese text
- [x] **Unicode math `\frac{}{}`:** recursively processes numerator and denominator
- [x] **Unicode minus (U+2212):** maps to subscript `₋` and superscript `⁻`
- [x] **Inline math in lists:** formulas inside list items render correctly as inline images
- [x] **Inline math in list items:** col_offset shifted by bullet/number prefix width
- [x] **Math delimiter normalization:** `normalize_math_delimiters()` strips whitespace after `$`/`$$` and before closing delimiters
- [x] **UTF-8 safe pre-processing:** uses `chars()` iteration, preserves CJK and multi-byte characters
- [x] **False-positive guards:** code blocks skipped, content must not contain `$`, closing `$` not followed by digit, trailing-ws requires `looks_like_math()`
- [x] **CJK preservation tests:** `test_normalize_math_preserves_cjk_text`, `test_normalize_math_preserves_cjk_only`
