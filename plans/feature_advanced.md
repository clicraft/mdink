# Feature Advanced: LaTeX Math + Streaming Render

## Feature 7 — LaTeX Math Rendering

### Goal
Render `$inline$` and `$$display$$` math expressions using Unicode approximations.

### Implementation Steps

1. **Enable math in pulldown-cmark** (`src/parser/mod.rs`)
   - Add `Options::ENABLE_MATH` to `ParseContext::process()`
   - Move `Event::InlineMath(text)` and `Event::DisplayMath(text)` from ignored section to active handling

2. **Handle InlineMath events** (`src/parser/mod.rs`)
   - Push `$` delimiter + unicode-converted math text + `$` delimiter as styled spans
   - Use `math_inline` theme style

3. **Handle DisplayMath events** (`src/parser/mod.rs`)
   - Emit a `RenderedBlock::Paragraph` with `$$` + content + `$$`
   - Use `math_display` theme style

4. **Unicode math converter** (`src/parser/mod.rs`)
   - `unicode_math()` function: best-effort LaTeX-to-Unicode conversion
   - Greek letters, operators, arrows, limited superscript/subscript
   - Unrecognized commands pass through as-is

5. **Theme support** (`src/theme/mod.rs`)
   - Add `math_inline: InlineStyle` and `math_display: InlineStyle` to `MarkdownTheme`
   - Add to `Default`, `strip_colors()`, `sanitize()` if needed
   - Add `math_inline_style()` and `math_display_style()` helpers

### Files Changed
- `src/parser/mod.rs` — event handling + unicode_math()
- `src/theme/mod.rs` — new style fields
- `src/theme/{dark,light,dracula}.json` — optional math style entries

---

## Feature 4 — Streaming Render

### Goal
When stdin is piped, read incrementally and re-render as content arrives.

### Implementation Steps

1. **Detect streaming mode** (`src/main.rs`)
   - Use `std::io::IsTerminal` to check if stdin is a TTY
   - When `file == "-"` and stdin is not a terminal: enter streaming mode

2. **Reader thread** (`src/main.rs`)
   - Spawn thread that reads stdin line-by-line via `BufRead::read_line()`
   - Send chunks via `std::sync::mpsc::channel`
   - On EOF or error: drop sender (closes channel)

3. **Poll-based event loop** (`src/main.rs`)
   - Replace `event::read()` with `event::poll(Duration::from_millis(50))`
   - After each poll cycle: `receiver.try_recv()` for new data
   - When new data: append to accumulated source, re-parse, re-flatten, update App

4. **Auto-scroll** (`src/main.rs`)
   - Track `at_bottom: bool` — true when `scroll_offset >= max_scroll`
   - After re-render: if was at bottom, scroll to new bottom

5. **Memory guard** (`src/main.rs`)
   - Cap accumulated stdin at 100 MB (same as file guard)
   - When exceeded: stop accepting new data, render what we have

### Files Changed
- `src/main.rs` — streaming state, reader thread, poll loop

### Thread Safety
- mpsc channel: single producer (reader thread), single consumer (main thread)
- No shared mutable state; all data flows through channel
- Reader thread exits when stdin closes or main thread drops receiver
