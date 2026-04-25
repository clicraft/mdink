# Feature Plan: File Browser + Remote URL Fetch

## Feature 1 — Remote URL Fetch

### Motivation
Allow `mdink https://example.com/README.md` to fetch and render remote markdown files
without requiring the user to download first.

### Design
- **Dependency:** `ureq = "3"` (sync HTTP client, no async runtime needed)
- **Detection:** In `main.rs`, before file reading, check if `file` starts with `http://` or `https://`
- **Fetch flow:**
  1. `ureq::get(url).call()` — fetch content synchronously
  2. Read response body as `String` with same 100 MB size guard (`Read::take`)
  3. Set `display_name` to the URL (sanitized of control chars)
  4. Set `base_path` to `PathBuf::from(".")` (no relative image resolution for URLs)
  5. Pass fetched content to existing `parser::parse()` pipeline
- **Error handling:**
  - Network failure → stderr message + `process::exit(EX_NOINPUT)`
  - HTTP 4xx/5xx → stderr message with status code + `process::exit(EX_NOINPUT)`
  - Non-UTF-8 body → stderr message + `process::exit(EX_DATAERR)`
  - Body too large → stderr message + `process::exit(EX_DATAERR)`
- **Timing:** URL fetch happens BEFORE terminal init (errors print to stderr normally)
- **Security:** No shell command execution from URLs. URL is only passed to ureq.

### Files changed
| File | Change |
|------|--------|
| `Cargo.toml` | Add `ureq = "3"` |
| `src/main.rs` | Add URL detection branch in file-reading section |

---

## Feature 3 — File Browser

### Motivation
Allow users to browse and open other `.md` files without leaving the TUI.

### Design
- **Keybinding:** `f` toggles the file browser overlay
- **State:** `FileBrowserState` struct in `app/mod.rs`:
  - `entries: Vec<PathBuf>` — discovered `.md` files
  - `selected: usize` — cursor position
  - `scroll: usize` — scroll offset for long lists
- **App fields:**
  - `file_browser: Option<FileBrowserState>` — None when hidden
  - `file_selected: Option<PathBuf>` — set when user selects a file; main.rs reads this
- **Navigation:** When file browser is active:
  - `j`/`k`/Up/Down: navigate
  - Enter: select file (sets `file_selected`, closes browser)
  - Esc/`f`: close browser
- **File discovery:** `discover_md_files()` function:
  - Recursive walk from CWD using `std::fs::read_dir`
  - Depth limit: 5 levels
  - Extensions: `.md`, `.markdown`
  - Max files: 500
  - Sorted alphabetically by relative path
- **Rendering:** Bordered popup overlay (like outline dropdown):
  - Centered on screen, 60% width, up to 80% height
  - Title: "Open File (Enter: open, Esc: close)"
  - Shows paths relative to CWD
  - Selected entry highlighted
- **Main loop integration:**
  - After event handling, check `app.file_selected`
  - If set: read new file, re-parse, re-flatten, update app state, clear selection

### Files changed
| File | Change |
|------|--------|
| `src/app/mod.rs` | Add `FileBrowserState`, fields, key handling |
| `src/renderer.rs` | Add `draw_file_browser()` overlay |
| `src/main.rs` | Add file reload logic after event handling |

### Guards
- 500 file limit for directory walk
- 5 level depth limit for recursion
- 100 MB file size guard (existing) for selected file

---

## Feature 4 — Link Navigation

### Motivation
Links (`[text](url)`) are rendered with italic styling but are not interactive.
Users must manually copy URLs or switch to a browser. This feature adds link
navigation: enter link mode to cycle through all links, follow a link to load
a new document or open it externally, and navigate back with a history stack.

### Design

**Keybindings:**
- `l` — enter link mode (only when document has links)
- `Tab` / `Shift+Tab` — cycle through links
- `Enter` — follow selected link
- `Esc` — exit link mode
- `Backspace` — go back to previous document (when history exists)

**State structs:**

```rust
// layout/mod.rs — collected during flatten()
pub struct LinkEntry {
    pub line_index: usize,  // first line of containing block
    pub url: String,
}

// app/mod.rs — navigation history
pub struct NavHistoryEntry {
    pub source: String,
    pub base_path: PathBuf,
    pub filename: String,
    pub scroll_offset: usize,
}
```

**Link collection:** `extract_block_links()` walks `RenderedBlock` recursively,
extracts URLs from `StyledSpan::url` fields. Only text-content blocks yield
links (headings, paragraphs, lists, block quotes, tables).
Uses exhaustive match per standards §8.3.

**Link following:**
- Local `.md` / `.markdown` → load and render inside mdink
- Remote `.md` (http/https, ends in `.md`) → fetch and render inside mdink **only when `--fetch-remote-markdown` is set**; otherwise open in system browser
- Other URLs → open with system browser (`xdg-open` / `open` / `explorer.exe`)

**Remote markdown fetch control:**
- CLI flag: `--fetch-remote-markdown`
- Env var: `MDINK_FETCH_REMOTE_MD`
- Config key: `fetch_remote_markdown`
- Default: `false` — remote `.md` links open in system browser
- Precedence: CLI > env var > config > default

**Back navigation:** History stack pushed before loading new document,
popped on error or when user presses Backspace. Restores source, path,
filename, and scroll position. Load errors display via `app.status_message`
within the TUI — no `eprintln!()` which would corrupt the display.

**UI:** Selected link line highlighted with focused search match style.
Status bar shows link URL, index, and navigation hints.

**Entry behavior (link mode):**
When entering link mode with `l`, the cursor targets the nearest link to the
current viewport rather than always jumping to the document's first link:

1. **Visible first:** If any link is on screen, select the first visible one
   (no scrolling needed).
2. **Forward search:** If no link is visible, search forward from the viewport
   bottom to find the next link after the screen.
3. **Backward search:** If no link exists after the viewport, search backward
   from the viewport top to find the nearest link before the screen.
4. **Nearest wins:** When both forward and backward candidates exist, pick the
   one closer to the viewport; on equal distance, prefer forward.

This logic lives in `App::nearest_entry_index()`, shared with image mode via
the `EntryWithLine` trait.

### Files changed
| File | Change |
|------|--------|
| `src/layout/mod.rs` | Add `LinkEntry`, `links` field on `PreRenderedDocument`, `extract_block_links()`, collect in `flatten()` |
| `src/app/mod.rs` | Add `NavHistoryEntry`, link mode fields on `App`, key handling priority 2, navigation methods |
| `src/renderer.rs` | Add link line highlighting, link mode status bar |
| `src/main.rs` | Handle `link_follow_requested` / `back_requested`, `follow_local_md()`, `follow_remote_md()`, `open_url_with_system_browser()` |

### Guards
- Fragment identifiers (`#section`) stripped before following
- History entry popped on load error (no orphan entries)
- Link mode disabled when document has no links
- Empty-link-text spans (`[](url)`) produce `LinkEntry` but are harmless
- `follow_local_md` and `follow_remote_md` call `image_manager.clear_all()` (not `clear_protocols()`) to clear the remote image cache when loading a new document

---

## Feature 11 — Image Navigation

### Motivation
Images in the document (local or remote) are rendered inline but cannot be
interacted with. This feature adds image navigation mode: cycle through all
images, open one in the system viewer, and see image URLs in the status bar.

### Design

**Keybindings:**
- `i` — toggle image mode (enter when off, exit when on)
- `Tab` / `Shift+Tab` — cycle through images
- `Enter` — open selected image with system viewer (`xdg-open` / `open` / `explorer.exe`)
- `Esc` — exit image mode

**State:**
```rust
// layout/mod.rs
pub struct ImageEntry {
    pub line_index: usize,
    pub url: String,
}
```

Collected during `flatten()` via `extract_block_images()`, exhaustive match on
all `RenderedBlock` variants per standards §8.3.

**Entry behavior (same as link mode):**
Uses `App::nearest_entry_index()` via the `EntryWithLine` trait:

1. **Visible first:** Select the first image already on screen.
2. **Forward search:** First image after viewport bottom.
3. **Backward search:** Last image before viewport top.
4. **Nearest wins:** Closer image selected; equal distance prefers forward.

**EntryWithLine trait** (`src/app/mod.rs`, private):
```rust
trait EntryWithLine {
    fn line_index(&self) -> usize;
}
// Implemented for LinkEntry and ImageEntry.
```

### Files changed
| File | Change |
|------|--------|
| `src/layout/mod.rs` | Add `ImageEntry`, `images` field on `PreRenderedDocument`, `extract_block_images()`, collect in `flatten()` |
| `src/app/mod.rs` | Add `EntryWithLine` trait, `nearest_entry_index()`, image mode fields on `App`, key handling priority 2.5, navigation methods |
| `src/renderer.rs` | Add image line highlighting, image mode status bar |
| `src/main.rs` | Handle `image_follow_requested`, open image with system viewer |

### Guards
- Image mode disabled when document has no images
- `i` toggles off when already in image mode
- Enter keeps image mode active (does not exit)

---

## Feature 10 — File Watcher (Auto-Reload)

### Motivation
When editing a markdown file in one terminal and viewing it in mdink in another,
the user currently must press `r` to refresh. Automatic file change detection
reloads the document as soon as it is saved, preserving scroll position.

### Design

**Approach: mtime polling (zero new dependencies)**

Both OS-level file watching (`notify` crate) and mtime polling require the event
loop to always use `event::poll(Duration::from_millis(200))` instead of blocking
on `event::read()`. Since the latency cost is identical, mtime polling avoids
adding a dependency. The `stat()` overhead is negligible.

**`FileWatcher` struct** (`src/main.rs`, private):

```rust
struct FileWatcher {
    path: Option<(PathBuf, std::time::SystemTime)>,
}
```

- `new(file: &str)` — watches local files; returns `None`-path for stdin/URLs
- `check(&mut self) -> bool` — `true` when mtime changed; `false` on no change,
  not watching, or file deleted (graceful degradation — keep showing last content)
- `set_file(&mut self, file: &str)` — updates tracked file on navigation
- `is_watching(&self) -> bool` — whether a local file is being tracked

**Event loop integration:**

1. Poll-mode condition extended with `watcher.is_watching()` (alongside streaming,
   pending images, pending math)
2. On each poll timeout, `watcher.check()` compares current mtime against stored
3. On change: re-read file via `load_file_for_browser()`, re-parse, re-flatten
4. Scroll preservation: at-bottom users stay at bottom; others are clamped to
   new `max_scroll()` (handles documents that shrink when edited)

**Navigation updates:** `watcher.set_file(&app.filename)` called at every site
that changes the active file:
- File browser selection
- `follow_local_md()` (watches new local file)
- `follow_remote_md()` (sets `None` — URLs not watchable)
- Back navigation (restores previous watched file)

### Files changed
| File | Change |
|------|--------|
| `src/main.rs` | Add `FileWatcher` struct + methods (~50 lines), create watcher in `main()`, add `watcher` param to `run_event_loop()`, `follow_local_md()`, `follow_remote_md()`, add file change check block in event loop, update watcher at all navigation sites |

### Tests
| Test | What it verifies |
|------|-----------------|
| `test_watcher_not_created_for_stdin` | `-` → not watching |
| `test_watcher_not_created_for_url` | URLs → not watching |
| `test_watcher_created_for_local_file` | Local file → watching |
| `test_watcher_no_change_returns_false` | Same mtime → no reload |
| `test_watcher_detects_mtime_change` | File write → detected |
| `test_watcher_file_deleted_returns_false` | Deleted → graceful degradation |
| `test_watcher_set_file_updates_path` | Navigation updates watcher |
| `test_watcher_nonexistent_file_not_watching` | Missing file → not watching |

### Edge cases handled
- **File deleted:** `check()` returns `false`, current content stays visible
- **Atomic saves (write+rename):** mtime changes on rename, detected normally
- **Editor truncation:** Empty file still reloads (valid markdown)
- **Non-local sources:** stdin and URLs never activate watching (no poll overhead)
- **Rapid saves:** Coalesced naturally — at most one reload per 200ms poll cycle

---

## Feature 12 — Goto Line (`:42`)

### Motivation

Users can scroll line-by-line (`j`/`k`), half-page (`d`/`u`), or jump to top/bottom
(`g`/`G`), but there is no way to jump directly to a specific line number. This is
essential for navigating long documents where the user knows the target line (e.g.
from a compiler error message or reference).

### Design

**Keybinding:** `:` enters goto-line input mode (mirrors the search `/` pattern).

**State:**
```rust
// app/mod.rs — simple String, no separate struct needed
pub goto_input: Option<String>,   // Some when in goto-line input mode
```

**Key handling priority:** Inserted between search input (priority 1) and link mode
(priority 2) as priority 1.5. When `goto_input` is `Some`, all keys go to
`handle_goto_input()`.

**Input behavior:**
- Only ASCII digits are accepted (max 7 digits to prevent overflow).
- Non-digit keys are silently ignored.
- `Enter` parses the input as `usize`, sets `scroll_offset = (line - 1).min(max_scroll())`,
  then clears `goto_input`. Line numbers are 1-based (user-facing).
- `Backspace` removes last digit; on empty input, cancels mode.
- `Esc` cancels without jumping.

**Status bar:** Shows `:42_ | Enter:jump Esc:cancel` when in goto mode (rendered in
`src/renderer.rs`, inserted before the link mode status display).

### Files changed

| File | Change |
|------|--------|
| `src/app/mod.rs` | Add `goto_input` field, `handle_goto_input()` method, `:` keybinding, priority 1.5 check |
| `src/renderer.rs` | Status bar prompt display |
| `src/app/tests.rs` | 11 tests: enter mode, digits, non-digits ignored, Enter jumps, Esc cancels, Backspace, clamp, empty/zero no-op |

### Tests

| Test | What it verifies |
|------|-----------------|
| `test_goto_colon_enters_mode` | `:` sets `goto_input = Some("")` |
| `test_goto_accepts_digits` | Typing `42` → `Some("42")` |
| `test_goto_ignores_non_digits` | Letters/spaces ignored |
| `test_goto_esc_cancels` | Esc clears mode, no scroll |
| `test_goto_backspace_on_empty_cancels` | Empty backspace exits mode |
| `test_goto_backspace_removes_digit` | Backspace pops last char |
| `test_goto_enter_scrolls_to_line` | `:50` Enter → offset = 49 |
| `test_goto_line_1_scrolls_to_top` | `:1` Enter → offset = 0 |
| `test_goto_line_clamped_to_max_scroll` | `:99999` clamped to `max_scroll()` |
| `test_goto_empty_enter_does_nothing` | Empty input + Enter → no scroll change |
| `test_goto_zero_does_nothing` | `:0` → no scroll change (0 is not a valid line) |
