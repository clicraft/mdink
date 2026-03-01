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
| `app.rs` | keyboard events | scroll state mutation | `App` |
| `font_detect.rs` | env vars + config files | TTF file paths | `ResolvedFonts` |
| `pdf.rs` | `&[DocumentLine]` + fonts | PDF file on disk | — |

### `RenderedBlock` — the IR

```rust
pub enum RenderedBlock {
    Heading { level: u8, content: Vec<StyledSpan> },
    Paragraph { content: Vec<StyledSpan> },
    CodeBlock { language: String, highlighted_lines: Vec<Line<'static>> },
    ThematicBreak,
    Spacer { lines: u16 },
}
```

`StyledSpan` carries owned `text: String` + `style: Style`. Adding a new block type means adding a variant here, a match arm in `parser.rs`, a match arm in `layout.rs`, and a match arm in `renderer.rs` — no other files need changing.

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

### Resize handling

On terminal resize, `main.rs` re-calls `layout::flatten(&blocks, new_width)` and stores the new `PreRenderedDocument` in `App`. `blocks` (the `Vec<RenderedBlock>`) is kept alive in `main.rs` for this purpose. Layout is stateless and idempotent — calling it again is always safe.

## Planned phases

The roadmap is tracked in `plans/overview.md`. Phase 3 adds lists/tables/blockquotes (new `RenderedBlock` variants). Phase 5 adds theming — the most invasive change, threading a `&Theme` through all style-producing functions. New work should avoid hardcoding colors or styles that will need to be theme-configurable.
