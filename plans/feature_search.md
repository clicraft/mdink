# Feature 6: Search / Find-in-page

## Overview

Add `/`-triggered search mode to mdink. The user types a query, presses Enter to
execute, and matches are highlighted in the rendered document. `n`/`N` navigate
between matches. Esc cancels search mode.

## Architecture

Search is a UI-only feature. It does **not** modify the parse or layout pipeline.
All search logic lives in `app/mod.rs` (state + key handling) and `renderer.rs`
(highlight overlay + search bar). Theme colors come from `theme/mod.rs`.

```
User presses /  ->  App enters search mode (SearchState.active = true)
Chars typed     ->  Appended to SearchState.query
Enter           ->  Execute: scan document.lines for matches, populate results
n / N           ->  Navigate matches, scroll to focused match
Esc             ->  Cancel search, clear highlights
```

## Data structures

### `SearchMatch` (in `app/mod.rs`)

```rust
pub struct SearchMatch {
    pub line_index: usize,   // index into document.lines
    pub byte_start: usize,   // byte offset within the line's plain text
    pub byte_end: usize,     // byte offset (exclusive)
}
```

### `SearchState` (in `app/mod.rs`)

```rust
pub struct SearchState {
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub focus: usize,        // index into matches (the "current" match)
    pub active: bool,        // true while the search bar is visible and accepting input
}
```

### `App.search` field

```rust
pub search: Option<SearchState>,
```

`None` = search never started or was fully cleared.

## Key handling changes (`app/mod.rs`)

### When `search.active == true` (typing mode)

| Key | Action |
|-----|--------|
| Printable char | Append to `query` |
| Backspace | Delete last char (if empty, cancel search) |
| Enter | Execute search, set `active = false` |
| Esc | Cancel search entirely (`search = None`) |

All other keys are ignored during active search input.

### When `search` is `Some` but `active == false` (results mode)

| Key | Action |
|-----|--------|
| `n` | Next match: `focus = (focus + 1) % matches.len()`, scroll to match |
| `N` | Previous match: `focus = (focus - 1) % matches.len()`, scroll to match |
| `/` | Re-enter search mode (reuse last query as starting point) |
| Esc | Clear search entirely (`search = None`) |

Other keys (j, k, q, etc.) work normally -- they fall through to the standard handler.

### When `search` is `None` (normal mode)

| Key | Action |
|-----|--------|
| `/` | Enter search mode: `search = Some(SearchState { active: true, .. })` |

## Search execution

When the user presses Enter with a non-empty query:

1. Extract plain text from each `DocumentLine::Text` and `DocumentLine::Code` line.
   Plain text = concatenation of all span content in the line.
2. Case-insensitive substring search for the query in each line's plain text.
3. Collect all `SearchMatch` results.
4. Set `focus = 0` and scroll viewport to the first match.

Guard: if query is empty, do nothing (stay in active mode).
Guard: limit query length to 256 bytes to prevent pathological regex behavior
(though we use simple substring search, not regex).

### Plain text extraction

For `DocumentLine::Text(line)` and `DocumentLine::Code(line)`:
```rust
fn line_plain_text(line: &Line<'static>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}
```

Other `DocumentLine` variants (`Empty`, `Rule`, `AsciiArt`, `ImageStart`,
`ImageContinuation`) are skipped.

## Renderer changes (`renderer.rs`)

### Match highlighting in document lines

When rendering a `DocumentLine::Text` or `DocumentLine::Code` line, check if
any `SearchMatch` entries reference this line index.

If yes, split the existing spans at match boundaries and apply the highlight style
to the matched portion. The focused match gets a distinct style (brighter).

Algorithm:
1. Build a plain text string from the line's spans (same as search does).
2. Build a byte-to-span-index map.
3. For each match on this line, note the `[byte_start, byte_end)` range.
4. Walk through spans, splitting at match boundaries, applying highlight style.

This is similar to the existing `build_spans_for_range` logic in `layout.rs`.

### Search bar (replaces status bar content when search is active)

When `app.search` is `Some`:
- If `active`: show `"/ {query}_"` (with cursor indicator) instead of normal status bar.
- If not active: show `"/{query} [{focus+1}/{matches.len()}]"` in the status bar,
  plus the normal scroll percentage.

The search bar uses the same status bar area (bottom row). No extra row is needed.

## Theme additions (`theme/mod.rs`)

### `SearchStyle` struct

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct SearchStyle {
    pub match_fg: Option<String>,
    pub match_bg: Option<String>,
    pub focused_fg: Option<String>,
    pub focused_bg: Option<String>,
}
```

Default: match = yellow bg + black fg, focused = bright white bg + black fg.

### MarkdownTheme addition

```rust
pub search: SearchStyle,
```

### Helper functions

```rust
pub fn search_match_style(s: &SearchStyle) -> Style { ... }
pub fn search_focused_style(s: &SearchStyle) -> Style { ... }
```

### Theme JSON updates

Add `"search"` section to all three built-in theme JSONs.

### `strip_colors` update

Strip `search.match_fg`, `search.match_bg`, `search.focused_fg`, `search.focused_bg`.

## Interaction with outline mode

When outline is open and search is active:
- Search bar replaces normal status bar hints (same bottom row).
- Outline navigation keys (Tab, BackTab, Enter) are handled first, before search.
- The `/` key should enter search mode even with outline open.

This means in the key priority chain:
1. Search active (typing)  -- captures all keys
2. Outline keys (Tab, BackTab, Enter, Esc, <, >) -- only when outline is open
3. Search results mode (n, N, /)
4. Normal keys (j, k, q, etc.)

## Edge cases

- Empty query: Enter does nothing, stays in typing mode.
- No matches: `matches` is empty, `n`/`N` are no-ops. Status shows "0/0".
- Unicode text: byte offsets are computed from the same plain-text string,
  so they always land on char boundaries.
- Match at line boundary: the match is on exactly one line. Cross-line matches
  are not supported (each line is searched independently).
- Very long document: search is O(total_text_length). No async needed for
  documents under the 100 MB guard.

## Files to modify

| File | Changes |
|------|---------|
| `src/app/mod.rs` | Add `SearchState`, `SearchMatch`, search key handling, `execute_search()`, `scroll_to_match()` |
| `src/app/tests.rs` | Tests for search state transitions, key handling, match navigation |
| `src/renderer.rs` | Highlight overlay logic, search bar in status bar |
| `src/theme/mod.rs` | `SearchStyle` struct, `search_match_style()`, `search_focused_style()`, strip_colors |
| `src/theme/tests.rs` | Tests for search style helpers |
| `src/theme/dark.json` | Add `"search"` section |
| `src/theme/light.json` | Add `"search"` section |
| `src/theme/dracula.json` | Add `"search"` section |

## Testing plan

1. Unit tests in `app/tests.rs`:
   - `/` enters search mode
   - Typing chars appends to query
   - Backspace deletes last char
   - Backspace on empty query cancels search
   - Enter executes search
   - Esc cancels search
   - `n` navigates to next match
   - `N` navigates to previous match
   - `n`/`N` wrap around
   - Empty query Enter is no-op
   - Search with no matches: focus stays 0, matches is empty
   - j/k still scroll during results mode

2. Unit tests in `theme/tests.rs`:
   - Default search styles produce correct colors
   - Roundtrip serialization preserves search fields
   - strip_colors clears search colors
