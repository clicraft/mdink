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

> **Note:** The poll-based event loop introduced by streaming is now shared with
> the file watcher (Feature 10). The condition that forces poll mode includes
> `!streaming_done || has_pending_images || has_pending_math || watcher.is_watching()`.
> See [feature_nav.md](feature_nav.md) Feature 10 for details.

---

## Feature 13 — Terminal Diagnostics Logging

### Motivation

Different terminal emulators display LaTeX formula images with wildly different
quality. The root cause — terminal graphics protocol support — was invisible
because zero logging existed for the decision chain:

```
Picker::from_query_stdio() → picker: Option<Picker>
  → has_graphics_support() → bool
    → MathEngine::new(user_enabled, has_graphics) → enabled: bool
```

When `from_query_stdio()` fails (lxterminal), `picker = None`, MathEngine
disabled, formulas degrade to Unicode text. When it succeeds (WezTerm with
Sixel/Kitty), pixel images are crisp. Without logging, diagnosing this required
reading source code.

### Design

**Logging approach:** All new logs use the `log` facade at `info` level. The
default log level is `Warn`, so these are invisible unless the user sets
`--log-level info` or `MDINK_LOG_LEVEL=info`. Zero overhead in normal use.

**Terminal identity detection** (`main.rs`):
```rust
fn detect_terminal_identity() -> String {
    // Checks TERM_PROGRAM, KITTY_WINDOW_ID/KITTY_PID, WEZTERM_PANE,
    // GHOSTTY_RESOURCES_DIR, ALACRITTY_WINDOW_ID, WT_SESSION, TERM
    // Returns e.g. "wezterm (WEZTERM_PANE)" or "unknown (TERM=xterm-256color)"
}
```

**Log statements (all `info` level):**

| Location | Message |
|----------|---------|
| `main.rs` (startup) | `terminal identity: {identity}` |
| `main.rs` (picker) | `picker: detected protocol=Sixel, font_size=(10,20)` or `picker: query failed — ...` |
| `main.rs` (after picker) | `graphics protocol quality: sixel (high quality)` |
| `main.rs` (bg color) | `terminal bg color: rgb(30,30,30)` or `not detected (using black)` |
| `math/mod.rs` (MathEngine::new) | `MathEngine: user_enabled=true, graphics_available=true, enabled=true` |

**Protocol quality classification:**

| Picker result | Protocol | Quality label |
|---|---|---|
| `Err` | N/A | `none (no graphics support)` |
| `Ok`, `Halfblocks` | Halfblocks | `halfblocks (reduced quality)` |
| `Ok`, `Sixel` | Sixel | `sixel (high quality)` |
| `Ok`, `Kitty` | Kitty | `kitty (high quality)` |
| `Ok`, `Iterm2` | iTerm2 | `iterm2 (high quality)` |

### Example output

**WezTerm:**
```
INFO mdink: terminal identity: wezterm (WEZTERM_PANE)
INFO mdink: picker: detected protocol=Sixel, font_size=(10,20)
INFO mdink: graphics protocol quality: sixel (high quality)
INFO mdink: terminal bg color: rgb(30,30,30)
INFO mdink::math: MathEngine: user_enabled=true, graphics_available=true, enabled=true
```

**lxterminal:**
```
INFO mdink: terminal identity: unknown (TERM=xterm-256color)
INFO mdink: picker: query failed — ...
INFO mdink: graphics protocol quality: none (no graphics support)
INFO mdink: terminal bg color: not detected (using black)
INFO mdink::math: MathEngine: user_enabled=true, graphics_available=false, enabled=false
```

### Files changed

| File | Change |
|------|--------|
| `src/main.rs` | `detect_terminal_identity()` helper, picker result logging, protocol quality classification, bg color logging |
| `src/math/mod.rs` | `log::info!` in `MathEngine::new()` |

### Halfblocks fallback for math

When the terminal supports only the Halfblocks protocol, math pixel images are
disabled — formulas fall back to Unicode text. Halfblocks renders images at only
2 vertical pixels per cell, which produces illegible formulas. Unicode text is
sharper and more readable than blurry halfblock images.

**Implementation:** In `main.rs`, the `graphics_available` boolean passed to
`MathEngine::new()` is computed as:

```rust
let math_has_high_quality = picker.as_ref().is_some_and(|p| {
    p.protocol_type() != ProtocolType::Halfblocks
});
```

This means:
- `ProtocolType::Sixel` / `Kitty` / `Iterm2` → `graphics_available = true` → pixel rendering
- `ProtocolType::Halfblocks` → `graphics_available = false` → Unicode fallback
- `None` (no picker) → `graphics_available = false` → Unicode fallback

Regular images (ImageManager) are **not** affected — they still use whatever
protocol is available, including Halfblocks.

### Tests

| Test | What it verifies |
|------|-----------------|
| `test_halfblocks_is_not_high_quality` | Halfblocks → `false` |
| `test_sixel_is_high_quality` | Sixel → `true` |
| `test_kitty_is_high_quality` | Kitty → `true` |
| `test_iterm2_is_high_quality` | iTerm2 → `true` |
| `test_no_protocol_is_not_high_quality` | `None` → `false` |
| `test_halfblocks_disables_math_engine` | `MathEngine::new(true, false)` → disabled |
| `test_high_quality_enables_math_engine` | `MathEngine::new(true, true)` → enabled |

---

## Error Handling for Missing Local Files

### Motivation

When a local image or markdown file referenced in a document doesn't exist,
the TUI must handle the error gracefully without corrupting the display.

### Rules

1. **No `eprintln!()` during TUI operation.** After ratatui's alternate screen buffer
   is active, `eprintln!()` writes to stderr which intermixes with the TUI rendering,
   causing garbled output. All parser image fallback paths have been cleaned of
   `eprintln!()` calls.

2. **`ImageFallback` shows source path.** When an image can't be loaded, the fallback
   display shows `[image: alt_text (src_url)]` (or `[image: src_url]` if alt is empty)
   so the user can identify which file was missing.

3. **`follow_local_md()` uses status message.** When following a link to a missing
   local markdown file, `main.rs::follow_local_md()` catches the error, pops the
   navigation history entry, and sets `app.status_message` to show the error within
   the TUI status bar.

4. **`load_image()` returns descriptive errors.** The error messages from
   `ImageManager::load_image()` include the full resolved path, making it easy to
   diagnose file-not-found issues.

### Files affected

| File | Change |
|------|--------|
| `src/parser/mod.rs` | Removed all `eprintln!("warning: {e}")` from image fallback paths |
| `src/layout/mod.rs` | Updated `ImageFallback` display to include `src_url` |
| `src/images/mod.rs` | Error messages include full path |
| `src/main.rs` | `follow_local_md()` already uses `app.status_message` for errors |
