# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

@.claude/rust-practices.md

## Commands

```bash
cargo test                          # run all tests
cargo test test_name                # run a single test by name (substring match)
cargo clippy -- -D warnings         # lint (must be clean before committing)
cargo run -- testdata/basic.md      # run the app (requires a real terminal)
cargo run -- testdata/font-slots.md # exercise all four font-slot rendering paths
```

## Architecture

mdink is a terminal markdown renderer (ratatui + pulldown-cmark + syntect). It uses a strict **unidirectional pipeline** — each stage is a pure function over the previous stage's output:

```
&str (markdown source)
  → parser::parse()      → Vec<RenderedBlock>         (semantic IR)
    → layout::flatten()  → PreRenderedDocument         (layout-resolved lines)
      → renderer::draw() → ratatui Frame               (pixels)
```

No stage imports from a later stage. The `Highlighter` from `highlight.rs` is passed into `parse()` as a parameter — it never touches layout or renderer.

### Module responsibilities

| Module | Input | Output | Key type |
|--------|-------|--------|----------|
| `parser.rs` | `&str` + `&Highlighter` | semantic blocks | `RenderedBlock` |
| `highlight.rs` | `&str` (code) + language + theme | colored spans | `Vec<Line<'static>>` |
| `layout.rs` | `&[RenderedBlock]` + width | display-ready lines | `PreRenderedDocument` |
| `renderer.rs` | `&App` | writes to frame | — |
| `app.rs` | keyboard events | scroll state mutation, link navigation state | `App` |
| `images.rs` | URLs + file paths | terminal graphics protocols + ASCII art | `ImageManager` |
| `math.rs` | LaTeX strings + render settings | Unicode text + pixel images | `MathEngine` |
| `font_detect.rs` | env vars + config files | TTF file paths | `ResolvedFonts` |
| `pdf.rs` | `&[DocumentLine]` + fonts | PDF file on disk | — |
| `logging.rs` | CLI + config + env vars | logger init | `LogConfig` |

### `RenderedBlock` — the IR

```rust
pub enum RenderedBlock {
    Heading { level: u8, content: Vec<StyledSpan> },
    Paragraph { content: Vec<StyledSpan> },
    CodeBlock { language: String, highlighted_lines: Vec<Line<'static>> },
    ThematicBreak,
    Spacer { lines: u16 },
    // Image variants:
    Image { protocol_index: usize, src_url: String, alt_text: String, width_cells: u16, height_cells: u16, px_width: u32, px_height: u32 },
    AsciiImage { lines: Vec<Line<'static>>, src_url: String, alt_text: String },
    ImageFallback { src_url: String, alt_text: String },
    ImagePending { url: String, alt_text: String },
    // Math variants:
    MathUnicode { content: Vec<StyledSpan>, raw_latex: String },
    MathImage { protocol_index: usize, width_cells: u16, height_cells: u16, px_width: u32, px_height: u32, raw_latex: String },
}
```

`StyledSpan` carries owned `text: String` + `style: Style` + `url: Option<String>` + `math_latex: String`. The `math_latex` field is non-empty for inline math spans (used by async rendering). Adding a new block type means adding a variant here, a match arm in `parser.rs`, a match arm in `layout.rs`, and a match arm in `renderer.rs` — no other files need changing.

### Image syntax support

Both markdown `![alt](url)` and HTML `<img src="url" alt="text">` images are supported. pulldown-cmark emits HTML `<img>` tags as `Event::Html`/`Event::InlineHtml` (not `Tag::Image`), so `parser.rs` extracts `src`/`alt` via `extract_attr()` and routes through `load_and_emit_image()`. This extraction happens both in `dispatch()` (inline HTML) and `on_skipping_event()` (HTML inside skipped blocks like `<div>`).

### Layout word-wrap algorithm

`layout.rs` cannot use ratatui's built-in wrapping because ratatui has no way to propagate per-character styles across line breaks. Instead:

1. Build a plain `String` from all spans + a parallel `Vec<Style>` indexed by byte offset.
2. Call `textwrap::wrap()` on the plain string.
3. Walk each wrapped line with a byte cursor; look up each character's style from the byte map; reconstruct `Span`s.

Hard breaks (`\n` embedded in `StyledSpan.text`) are handled by splitting the span and wrapping each half independently before merging.

### Font slot strategy

Modern terminals (WezTerm, Kitty, Alacritty) allow a different font per ANSI modifier combo. mdink deliberately maps markdown elements to slots:

| Slot | Modifier | Elements |
|------|----------|----------|
| Normal | none | body text |
| Bold | `BOLD` | h1–h3, `**strong**` |
| Italic | `ITALIC` | `*emphasis*`, links |
| Bold+Italic | `BOLD\|ITALIC` | h4–h6, `` `inline code` `` |

Code block comments are forced to `ITALIC` via a color-matching heuristic: `resolve_comment_color()` reads the `comment` scope's color from the syntect theme once, then any token whose foreground matches that color gets `ITALIC` added.

### PDF export and font embedding

`pdf.rs` exports the document as a text-mode PDF using `printpdf` 0.7. Font handling has two critical workarounds:

**1. printpdf font descriptor bug.** printpdf 0.7 sets `Flags 32` (Nonsymbolic) and `ItalicAngle 0` for ALL embedded fonts, even italic variants. PDF viewers use these descriptors to decide whether to use the embedded font or substitute a system font. Without `FixedPitch` (bit 0) and `Italic` (bit 6), viewers like Adobe Reader substitute a proportional serif font for italic slots. `fix_font_descriptors()` post-processes the raw PDF bytes to patch:
- F0/F1 (Regular/Bold): `Flags 33` (FixedPitch + Nonsymbolic)
- F2/F3 (Italic/BoldItalic): `Flags 97` (FixedPitch + Nonsymbolic + Italic)

These are same-length byte replacements ("32"→"33"/"97"), so no PDF structure offsets change.

**2. Terminal font detection must not blindly probe all configs.** On WSL, config files from multiple terminals coexist. The old code probed every config file on disk as a "last resort", which picked up an Alacritty config with Iosevka/Victor Mono while WezTerm was the running terminal. `detect_terminal_font()` now only probes configs when an env var confirms the terminal is running (`TERM_PROGRAM`, `WEZTERM_PANE`, `KITTY_PID`, `ALACRITTY_WINDOW_ID`, `WT_SESSION`, etc.). When none match, it falls back to JetBrains Mono (WezTerm's default).

**Font cascade:** `--pdf-font` > terminal config > JetBrains Mono > Courier (built-in).

### Invariants to preserve

- **Highlight size guard:** `highlight.rs` rejects code blocks > 512 KB (Oniguruma can OOM on large inputs).
- **File size guard:** `main.rs` rejects files > 100 MB before terminal init.
- **Width clamp:** `layout.rs` clamps width to ≥ 1; `textwrap` has undefined behavior at width 0.
- **Style stack:** `parser.rs` pushes a `Style` for each inline format open tag and pops it on the matching close tag. All pop sites have `debug_assert!(!style_stack.is_empty())`.
- **Terminal restore:** `TERMINAL_ACTIVE` flag in `main.rs` ensures the panic hook only restores the terminal if it was successfully initialized. Never remove this flag.
- **Leaf module:** `highlight.rs` never imports from other mdink modules. syntect types must not leak into parser, layout, or renderer.
- **Leaf module:** `font_detect.rs` never imports from other mdink modules. Same isolation as `highlight.rs`.
- **PDF font descriptors:** `fix_font_descriptors()` must run after `doc.save()` to patch printpdf's broken Flags. If printpdf is upgraded, verify whether this workaround is still needed.
- **Terminal detection:** `detect_terminal_font()` must only probe config files for terminals confirmed by env vars. Never blindly probe all configs — on WSL, stale configs from other terminals cause wrong font embedding.
- **Navigation history:** `nav_history` entries are pushed before loading a new document. If loading fails, the entry is popped immediately to avoid orphan entries. `back_requested` is only set when `nav_history` is non-empty.
- **Link mode:** `extract_block_links()` uses exhaustive match on `RenderedBlock` — no `_ =>` catch-all. When new block types are added, the match must be updated.
- **Image mode:** `extract_block_images()` uses exhaustive match on `RenderedBlock` — no `_ =>` catch-all. When new block types are added, the match must be updated.
- **Remote image cache:** `cache` entries in `ImageManager` survive re-parse (same document) but are cleared on document change. `clear_protocols()` keeps cache; `clear_all()` clears everything (protocols + cache + pending/failed tracking).
- **Remote image fetch default:** Remote images are NOT fetched by default. The `--fetch-remote-images` CLI flag (or `MDINK_FETCH_REMOTE` env var, or `fetch_remote_images` config key) must be set to enable background fetching. When disabled, remote URLs produce `ImageFallback` immediately. Cache is checked before the flag — already-downloaded images are used regardless.
- **Failed URL fallback:** When a remote fetch fails (timeout, network error, unsupported format), the URL is added to `failed_urls`. On subsequent re-parses, `load_and_emit_image()` checks `is_failed_url()` and emits `ImageFallback` instead of `ImagePending`, preventing `[loading: ...]` from persisting indefinitely. The failed URL is displayed as `[image: alt_text (src_url)]`.
- **Remote markdown fetch default:** Remote `.md` links are NOT fetched by default. The `--fetch-remote-markdown` CLI flag (or `MDINK_FETCH_REMOTE_MD` env var, or `fetch_remote_markdown` config key) must be set. When disabled, remote `.md` links open in the system browser instead.
- **Image mode toggle:** Pressing `i` while in image mode exits it (toggle behavior). Pressing Enter to open an image keeps image mode active.
- **Fetch thread dedup:** URLs are sent to the fetch thread only once (tracked by `pending_urls` HashSet). Failed URLs are tracked by `failed_urls` and not retried within the same document. On re-parse, `is_failed_url()` causes the parser to emit `ImageFallback` instead of `ImagePending` for failed URLs — this prevents `[loading: ...]` from persisting indefinitely. `set_fetch_remote(true)` clears `failed_urls` so the user can retry by toggling `I` off then on.
- **Leaf module:** `images.rs` never imports from other mdink modules. Same isolation as `highlight.rs` and `font_detect.rs`.
- **Leaf module:** `math.rs` never imports from other mdink modules. Same isolation as `highlight.rs`, `images.rs`, and `font_detect.rs`.
- **Math rendering default:** Pixel rendering of LaTeX formulas is enabled by default. `--no-math-images` (or `MDINK_NO_MATH_IMAGES`, or `math_images: false` in config) disables it. When disabled, all formulas stay as Unicode text and no background rendering occurs.
- **Math batch refresh:** Re-parse after math rendering fires only when `!math_engine.has_pending() && math_engine.cache_touched()`. No partial updates. No flicker.
- **Math fetch dedup:** Formulas are sent to the render thread only once (tracked by `pending` HashSet). Failed formulas are tracked by `failed` HashSet and not retried within the same document.
- **Math cache lifecycle:** `MathEngine` cache survives re-parse (same document) but is cleared on document change (`clear_all()`). `clear_protocols()` keeps cache.
- **Math size guard:** LaTeX input to `render_latex_to_image()` is bounded at 10 KB per formula.
- **Inline math placeholder:** `StyledSpan` with `math_image: Some(...)` uses NBSP (`\u{00A0}`) characters for placeholder text. The NBSP count equals `width_cells`. textwrap treats NBSP as non-breaking, keeping the placeholder together during word wrapping.
- **Inline math rendering:** `render_latex_to_image()` resizes inline formulas (`display=false`) to exactly `cell_h` pixels height via `image::imageops::resize()`. Display formulas (`display=true`) are not resized.
- **Inline math col_offset:** `collect_inline_image_metas()` in `layout.rs` computes `col_offset` by walking the byte range and counting display widths. The renderer uses this to position `StatefulImage` at `(content_area.x + col_offset, y, width, 1)`.
- **Inline math tracking:** Inline formulas carry `math_latex` field in `StyledSpan`. On re-parse with warm cache, inline math is emitted as a `StyledSpan` with `math_image: Some(InlineMathImage)` — the span's text is NBSP characters matching `width_cells`. The layout engine tracks column offsets via `InlineImageEntry` and the renderer overlays `StatefulImage` at precise `(col_offset, y, width, 1)` positions within text lines.
- **Math delimiter normalization:** `normalize_math_delimiters()` runs at the top of `parse()` before pulldown-cmark. Strips whitespace inside `$...$` and `$$...$$` so that `$ \hat{p}_i$` is recognized as math. Uses proper UTF-8 character iteration (`chars()`) — never `b[i] as char` — to preserve CJK and multi-byte text. Guards: code blocks/inline code skipped, content must not contain `$`, closing `$` not followed by digit, trailing-whitespace closing `$` requires `looks_like_math()`.

### Link navigation

`layout::flatten()` collects `LinkEntry` structs (line_index + url) from `RenderedBlock` via `extract_block_links()`. Links map to the first line of their containing block. The `App` holds link mode state (`link_mode`, `link_selected`) and navigation history (`nav_history: Vec<NavHistoryEntry>`).

### Image navigation

`layout::flatten()` also collects `ImageEntry` structs (line_index + url) from `RenderedBlock` via `extract_block_images()`. Every image variant (`Image`, `AsciiImage`, `ImageFallback`, `ImagePending`) carries a `src_url` field (or `url` for `ImagePending`) used for image mode. The `App` holds image mode state (`image_mode`, `image_selected`).

Key handling priorities:
0. File browser
1. Search input mode
2. Link mode (Tab/Shift+Tab/Enter/Esc)
2.5. Image mode (Tab/Shift+Tab/Enter/Esc)
3. Outline mode
4. Search results
5. Normal keys (j/k/l/i/Backspace)

### Resize handling

On terminal resize, `main.rs` re-parses and re-flattens the document. The image cache is warm (no network I/O), so this is fast — it recomputes `width_cells`/`height_cells` for the new terminal width. `blocks` (the `Vec<RenderedBlock>`) is kept alive in `main.rs` for this purpose. Layout is stateless and idempotent — calling it again is always safe. The math cache is also warm — `MathImage` blocks get new protocol indices with correct dimensions.

### Math rendering

LaTeX formulas (`$inline$` and `$$display$$`) are handled by `math.rs`. The pipeline:

1. **Parse time:** All formulas immediately produce Unicode text via `unicode_math()`. Display math emits `MathUnicode` blocks; inline math produces `StyledSpan` with `math_latex` field set.
2. **Async rendering:** When enabled (default), ALL formulas are queued to a background render thread via `queue_pending_math_renders()`. The thread calls `render_latex_to_image()` which uses the RaTeX pipeline (ratex-parser → ratex-layout → ratex-svg → resvg) unconditionally.
3. **Batch refresh:** Results drain into `MathEngine` cache. Re-parse fires only when ALL formulas are done (`!has_pending() && cache_touched()`). On re-parse, cached formulas emit `MathImage` blocks; failed ones stay as Unicode.

Precedence: `--no-math-images` > `MDINK_NO_MATH_IMAGES` > `math_images: false` in config > default (enabled).

### Logging

mdink uses the `log` facade crate with `env_logger` backend. Logging is **zero-cost when disabled** (log macros compile to no-ops when no logger is initialized).

**Two-phase initialization** (respects TUI constraint):

1. **Phase A** (before `ratatui::init()`): logger writes to stderr.
2. **Phase B** (after `TERMINAL_ACTIVE = true`): logger writes to file or is suppressed. When no `--log-file` is specified and logging was explicitly requested, logs go to `${XDG_CACHE_HOME:-$HOME/.cache}/mdink/mdink.log`.

Precedence for log level: `--log-level` > `MDINK_LOG_LEVEL` > config `log_level` > `RUST_LOG` > default (Warn).
Precedence for log file: `--log-file` > `MDINK_LOG_FILE` > config `log_file` > auto-detected cache dir.

**Log level map:**
- `error!` — Fatal I/O before TUI init (file not found, too large)
- `warn!` — Recoverable failures (image fetch timeout, bad config, math render fail)
- `info!` — Milestone events (file loaded, terminal init, cache hit)
- `debug!` — Diagnostic detail (state transitions, re-parse triggers)
- `trace!` — Reserved for future high-frequency profiling

**Leaf module compatibility:** All leaf modules (`images.rs`, `math.rs`, `highlight.rs`, `font_detect.rs`) use `log` macros directly — it's an external crate, so no mdink import is needed.

**Invariant:** Never use `eprintln!()` during TUI operation. Use `log::warn!()` or `log::error!()` instead — Phase B ensures these go to a file, not stderr.

## Planned phases

The roadmap is tracked in `plans/overview.md`. Phase 3 adds lists/tables/blockquotes (new `RenderedBlock` variants). Phase 5 adds theming — the most invasive change, threading a `&Theme` through all style-producing functions. New work should avoid hardcoding colors or styles that will need to be theme-configurable.
