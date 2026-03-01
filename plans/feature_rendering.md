# Feature 2 (OSC 8 Links) + Feature 5 (Mermaid Diagrams) Implementation Plan

## Feature 2: OSC 8 Clickable Links

### Overview
Make links in markdown documents clickable in terminals that support OSC 8 hyperlinks.
The URL data is threaded through the entire pipeline (parser -> layout -> renderer).

### Changes

#### 1. `src/parser/mod.rs` — Add `url` field to `StyledSpan`
- Add `pub url: Option<String>` field to `StyledSpan` struct.
- Add `current_link_url: Option<String>` field to `ParseContext`.
- On `Start(Tag::Link { dest_url, .. })`: store URL in `current_link_url` AND push style.
- On `End(TagEnd::Link)`: clear `current_link_url` AND pop style.
- In `push_text()`: if `current_link_url.is_some()`, clone it into the `StyledSpan.url`.
- In `push_inline_code()`: same as above.
- In `push_soft_break()` and `push_hard_break()`: pass `url: None` (whitespace doesn't need links).
- Update the separator space in `end_paragraph()` with `url: None`.
- Update ALL other `StyledSpan { .. }` construction sites with `url: None`.

Construction sites to update:
1. `parser/mod.rs:660` — InListItemParagraph separator space
2. `parser/mod.rs:805` — `push_text()`
3. `parser/mod.rs:810` — `push_inline_code()`
4. `parser/mod.rs:815` — `push_soft_break()`
5. `parser/mod.rs:820` — `push_hard_break()`
6. `layout/mod.rs:167` — ImageFallback wrap
7. `layout/mod.rs:809` — `wrap_with_hard_breaks` split span
8. `layout/mod.rs:819` — `wrap_with_hard_breaks` non-newline span
9. `layout/tests.rs:11` — `plain_span()` helper
10. `layout/tests.rs:18` — `styled_span()` helper

#### 2. `src/layout/mod.rs` — Preserve URL through word-wrap
- Add a parallel `byte_urls: Vec<Option<String>>` in `wrap_styled_spans()` alongside `byte_styles`.
- Populate it the same way as `byte_styles` (one entry per byte of the span text).
- In `build_spans_for_range()`: accept `byte_urls` param, look up URL for each run,
  and embed it into a custom structure or directly into the span.
- Since ratatui `Span` doesn't support URLs natively, the URL must be embedded
  in the span text as OSC 8 escape sequences at this stage or at render time.
- DECISION: Embed OSC 8 in the span text at render time (renderer.rs), not in layout.
  Layout just needs to thread the URL metadata through. Use a new return type or
  attach URL data alongside spans.
- SIMPLEST APPROACH: Embed OSC 8 sequences directly in span text content during
  `build_spans_for_range()`. This way ratatui outputs them verbatim. The layout
  module doesn't need to know about OSC 8; we just need the URL to survive wrapping.
- FINAL DECISION: Build the OSC 8 wrapping in the renderer when converting
  DocumentLine spans to rendered output. This keeps layout clean and URL-unaware.
  BUT layout needs to propagate URLs somehow. The cleanest way: build_spans_for_range
  returns `Vec<(Span, Option<String>)>` and the renderer wraps spans that have URLs.

  Actually, the simplest approach that avoids changing the DocumentLine type:
  inject OSC 8 sequences into span text in `build_spans_for_range`. This works
  because ratatui passes text through to the terminal verbatim.

#### 3. `src/renderer.rs` — OSC 8 output
- The OSC 8 sequences are embedded in span text by layout, so renderer needs no changes.
- Terminals that support OSC 8 will make text clickable; others ignore the sequences.

#### 4. URL sanitization
- Strip control characters (0x00-0x1f, 0x7f) from URLs before embedding in OSC 8.
- This prevents injection of terminal escape sequences via malicious URLs.

### Testing
- Add parser test: link produces StyledSpan with url field set.
- Add layout test: URL survives word wrapping.

---

## Feature 5: Mermaid Diagrams

### Overview
Detect `mermaid` code blocks and render them as labeled code blocks.
No new dependencies, no new RenderedBlock variant needed.

### Changes

#### 1. `src/parser/mod.rs` — Detect mermaid code blocks
- In `on_code_block_event()` when `End(CodeBlock)`: check if `language == "mermaid"`.
- If mermaid: skip syntax highlighting (syntect has no mermaid grammar).
  Instead, convert the raw text to plain `Line`s.
- The code block still renders as a `RenderedBlock::CodeBlock` with `language: "mermaid"`.
- Layout already handles CodeBlock with a language label, so "mermaid" appears as the header.

### Testing
- Parser test: mermaid code block produces CodeBlock with language "mermaid" and
  unhighlighted plain text lines.
