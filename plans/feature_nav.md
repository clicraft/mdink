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
