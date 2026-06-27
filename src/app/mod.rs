//! Application state: scroll position, viewport dimensions, quit flag.
//!
//! `App` is a pure state container — it never imports `ratatui::Frame` or
//! performs any rendering. The renderer reads from `&App` to determine
//! what to draw.

use std::ops::Range;
use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::layout::{DocumentLine, PreRenderedDocument};
use crate::theme::MarkdownTheme;

/// Maximum length of a search query (bytes). Prevents pathological allocations.
const MAX_SEARCH_QUERY_LEN: usize = 256;

/// A single search match within the document.
pub struct SearchMatch {
    /// Index into `document.lines`.
    pub line_index: usize,
    /// Byte offset (inclusive) within the line's plain text.
    pub byte_start: usize,
    /// Byte offset (exclusive) within the line's plain text.
    pub byte_end: usize,
}

/// State for the search / find-in-page feature.
pub struct SearchState {
    /// The current search query string.
    pub query: String,
    /// All matches found in the document.
    pub matches: Vec<SearchMatch>,
    /// Index into `matches` for the currently focused match.
    pub focus: usize,
    /// When true, the search bar is visible and accepting keyboard input.
    pub active: bool,
}

/// State for the outline panel when visible.
pub struct OutlineState {
    /// Index into `document.headings` for the currently selected heading.
    pub selected: usize,
}

/// State for the file browser overlay.
pub struct FileBrowserState {
    /// Discovered `.md` files (relative paths, sorted alphabetically).
    pub entries: Vec<PathBuf>,
    /// Index of the currently selected entry.
    pub selected: usize,
    /// Scroll offset for the visible portion of the file list.
    pub scroll: usize,
}

/// Built-in theme names, in cycling order.
pub const THEME_CYCLE: &[&str] = &["dark", "light", "dracula"];

/// Application state for the TUI viewer.
///
/// Holds the pre-rendered document, scroll position, viewport size,
/// and session metadata. Methods handle keyboard input and scroll
/// arithmetic.
pub struct App {
    /// The active theme controlling all visual styling.
    pub theme: MarkdownTheme,
    /// The pre-rendered document (all lines laid out for display).
    pub document: PreRenderedDocument,
    /// Current vertical scroll offset (0 = top of document).
    pub scroll_offset: usize,
    /// Number of visible lines in the content area (excludes status bar).
    pub viewport_height: usize,
    /// Name of the file being displayed (shown in the status bar).
    pub filename: String,
    /// When true, the event loop should exit.
    pub quit: bool,
    /// Outline panel state. `None` = hidden.
    pub outline: Option<OutlineState>,
    /// When true, main.rs should re-flatten the document (e.g. after outline toggle).
    pub needs_reflatten: bool,
    /// Heading index set by Enter in outline mode; main.rs resolves this
    /// to a line index after any pending reflatten, then scrolls there.
    pub pending_jump: Option<usize>,
    /// Session-only outline width override (percentage). `None` = use theme default.
    /// Set by `<`/`>` keys at runtime; not persisted.
    pub outline_width_percent: Option<u16>,
    /// When true, the event loop should re-parse and re-render the document.
    pub refresh_requested: bool,
    /// Index into `THEME_CYCLE` for the current theme.
    pub theme_index: usize,
    /// When true, the event loop should load the next theme and re-render.
    pub theme_cycle_requested: bool,
    /// Search / find-in-page state. `None` = no search active.
    pub search: Option<SearchState>,
    /// File browser state. `None` = hidden.
    pub file_browser: Option<FileBrowserState>,
    /// Set when user selects a file in the browser; main.rs reads and clears this.
    pub file_selected: Option<PathBuf>,
    /// Whether print preview mode is active (white-bg print theme).
    pub print_preview: bool,
    /// When true, the event loop should load or unload the print theme.
    pub print_preview_changed: bool,
    /// Theme index saved before entering print preview (to restore on exit).
    pub saved_theme_index: usize,
    /// When true, the event loop should export the document as PDF.
    pub pdf_export_requested: bool,
    /// Directory of the source file (for PDF output placement).
    pub source_path: PathBuf,
    /// Transient status message shown in the status bar for one frame.
    pub status_message: Option<String>,
    /// Path of the last successfully exported PDF (enables "o:open" hint).
    pub last_exported_pdf: Option<PathBuf>,
    /// When true, the event loop should open the last exported PDF.
    pub open_pdf_requested: bool,
}

impl App {
    /// Creates a new `App` with the given document and filename.
    ///
    /// Scroll starts at the top; viewport height is set to 0 and must
    /// be updated by `main.rs` before each draw call.
    pub fn new(
        document: PreRenderedDocument,
        filename: String,
        theme: MarkdownTheme,
        source_path: PathBuf,
    ) -> Self {
        let theme_index = THEME_CYCLE
            .iter()
            .position(|&n| n == theme.name)
            .unwrap_or(0);
        Self {
            theme,
            document,
            scroll_offset: 0,
            viewport_height: 0,
            filename,
            quit: false,
            outline: None,
            needs_reflatten: false,
            pending_jump: None,
            outline_width_percent: None,
            refresh_requested: false,
            theme_index,
            theme_cycle_requested: false,
            search: None,
            file_browser: None,
            file_selected: None,
            print_preview: false,
            print_preview_changed: false,
            saved_theme_index: theme_index,
            pdf_export_requested: false,
            source_path,
            status_message: None,
            last_exported_pdf: None,
            open_pdf_requested: false,
        }
    }

    /// Returns the name of the next theme in the cycle.
    pub fn next_theme_name(&self) -> &'static str {
        let next = (self.theme_index + 1) % THEME_CYCLE.len();
        THEME_CYCLE[next]
    }

    /// Advances the theme index to the next position.
    pub fn advance_theme_index(&mut self) {
        self.theme_index = (self.theme_index + 1) % THEME_CYCLE.len();
    }

    /// Dispatches a key event to the appropriate scroll or quit action.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Priority 0: File browser captures all keys when open.
        if self.file_browser.is_some() {
            self.handle_file_browser_key(key);
            return;
        }

        // Priority 1: Search input mode captures all keys.
        if self.search.as_ref().is_some_and(|s| s.active) {
            self.handle_search_input(key);
            return;
        }

        // Priority 2: Outline-specific keys when outline is visible.
        if self.outline.is_some() {
            match key.code {
                KeyCode::Tab if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.outline_select_next();
                    return;
                }
                KeyCode::BackTab => {
                    self.outline_select_prev();
                    return;
                }
                KeyCode::Enter => {
                    self.outline_jump();
                    return;
                }
                KeyCode::Esc => {
                    self.outline = None;
                    self.needs_reflatten = true;
                    return;
                }
                KeyCode::Char('>') => {
                    self.outline_grow();
                    return;
                }
                KeyCode::Char('<') => {
                    self.outline_shrink();
                    return;
                }
                _ => {} // fall through to normal keys
            }
        }

        // Priority 3: Search results mode (n/N navigate, Esc clears, / re-enters).
        if self.search.is_some() {
            match key.code {
                KeyCode::Char('n') => {
                    self.search_next();
                    return;
                }
                KeyCode::Char('N') => {
                    self.search_prev();
                    return;
                }
                KeyCode::Esc => {
                    self.search = None;
                    return;
                }
                KeyCode::Char('/') => {
                    // Re-enter search input mode, preserving the current query.
                    if let Some(state) = &mut self.search {
                        state.active = true;
                    }
                    return;
                }
                _ => {} // fall through to normal keys
            }
        }

        // Priority 4: Normal keys.
        match key.code {
            // Open last exported PDF (only when one exists in print preview)
            KeyCode::Char('o') if self.print_preview && self.last_exported_pdf.is_some() => {
                self.open_pdf_requested = true;
            }
            // Enter search mode
            KeyCode::Char('/') => self.enter_search_mode(),
            // Toggle outline
            KeyCode::Char('o') => self.toggle_outline(),
            // Scroll down 1 line
            KeyCode::Char('j') | KeyCode::Down => self.scroll_down(1),
            // Scroll up 1 line
            KeyCode::Char('k') | KeyCode::Up => self.scroll_up(1),
            // Scroll down half-page
            KeyCode::Char('d') | KeyCode::PageDown => {
                let half = self.viewport_height / 2;
                self.scroll_down(half.max(1));
            }
            // Scroll up half-page
            KeyCode::Char('u') | KeyCode::PageUp => {
                let half = self.viewport_height / 2;
                self.scroll_up(half.max(1));
            }
            // Scroll to top
            KeyCode::Char('g') | KeyCode::Home => self.scroll_to_top(),
            // Scroll to bottom (Shift+g = 'G')
            KeyCode::Char('G') | KeyCode::End => self.scroll_to_bottom(),
            // Refresh / re-render
            KeyCode::Char('r') => self.refresh_requested = true,
            // Cycle theme (suppressed during print preview)
            KeyCode::Char('t') if !self.print_preview => {
                self.theme_cycle_requested = true;
            }
            // Toggle print preview
            KeyCode::Char('p') => {
                self.print_preview = !self.print_preview;
                if self.print_preview {
                    self.saved_theme_index = self.theme_index;
                }
                self.print_preview_changed = true;
            }
            // Export PDF (only in print preview)
            KeyCode::Char('y') if self.print_preview => {
                self.pdf_export_requested = true;
            }
            // Open file browser
            KeyCode::Char('f') => self.open_file_browser(),
            // Quit
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            // Ctrl+C also quits
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.quit = true;
            }
            _ => {}
        }
    }

    /// Toggles the outline panel on/off.
    fn toggle_outline(&mut self) {
        if self.outline.is_some() {
            self.outline = None;
            self.needs_reflatten = true;
        } else if !self.document.headings.is_empty() {
            self.outline = Some(OutlineState { selected: 0 });
            self.needs_reflatten = true;
        }
    }

    /// Selects the next heading in the outline (wraps around).
    fn outline_select_next(&mut self) {
        if let Some(state) = &mut self.outline {
            let count = self.document.headings.len();
            if count > 0 {
                state.selected = (state.selected + 1) % count;
            }
        }
    }

    /// Selects the previous heading in the outline (wraps around).
    fn outline_select_prev(&mut self) {
        if let Some(state) = &mut self.outline {
            let count = self.document.headings.len();
            if count > 0 {
                state.selected = if state.selected == 0 { count - 1 } else { state.selected - 1 };
            }
        }
    }

    /// Sets pending_jump to the selected heading's index.
    ///
    /// The heading index is resolved to a line index in main.rs *after*
    /// any pending reflatten, so the jump targets the correct line in
    /// the final layout.
    fn outline_jump(&mut self) {
        if let Some(state) = &self.outline {
            if state.selected < self.document.headings.len() {
                self.pending_jump = Some(state.selected);
            }
        }
    }

    /// Returns the effective outline width percentage (override or theme default).
    fn effective_outline_percent(&self) -> u16 {
        self.outline_width_percent.unwrap_or(self.theme.outline.width_percent)
    }

    /// Returns the outline panel width in columns for the given terminal width.
    ///
    /// Applies the percentage from the runtime override (if set) or the theme,
    /// then clamps to at most 1/3 of the terminal width.
    pub fn outline_panel_cols(&self, terminal_width: u16) -> u16 {
        let percent = self.effective_outline_percent();
        debug_assert!(percent <= 100, "outline percent {percent} exceeds 100");
        let from_percent = (terminal_width as u32 * percent as u32 / 100) as u16;
        from_percent.min(terminal_width / 3)
    }

    /// Increases the outline panel width by 2 percentage points (capped at 33%).
    fn outline_grow(&mut self) {
        let new = (self.effective_outline_percent() + 2).min(33);
        self.outline_width_percent = Some(new);
        self.needs_reflatten = true;
    }

    /// Decreases the outline panel width by 2 percentage points (min 10%).
    fn outline_shrink(&mut self) {
        let new = self.effective_outline_percent().saturating_sub(2).max(10);
        self.outline_width_percent = Some(new);
        self.needs_reflatten = true;
    }

    /// Returns the range of line indices visible in the current viewport.
    pub fn visible_range(&self) -> Range<usize> {
        let end = (self.scroll_offset + self.viewport_height).min(self.document.total_height);
        self.scroll_offset..end
    }

    /// Scrolls down by `n` lines, clamped to the maximum scroll position.
    pub fn scroll_down(&mut self, n: usize) {
        let max = self.max_scroll();
        // Use saturating_add so overflow before .min() cannot wrap to a small value.
        // (scroll_up already uses saturating_sub symmetrically.)
        self.scroll_offset = self.scroll_offset.saturating_add(n).min(max);
    }

    /// Scrolls up by `n` lines, clamped to 0.
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scrolls to the top of the document.
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    /// Scrolls to the bottom of the document.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.max_scroll();
    }

    /// Returns the maximum valid scroll offset.
    ///
    /// When the document is shorter than the viewport, returns 0 (no scrolling).
    pub fn max_scroll(&self) -> usize {
        self.document
            .total_height
            .saturating_sub(self.viewport_height)
    }

    /// Returns the current scroll position as a percentage (0–100).
    ///
    /// Returns 100 when the document fits within the viewport or when
    /// scrolled to the bottom.
    pub fn scroll_percent(&self) -> u16 {
        let max = self.max_scroll();
        if max == 0 {
            return 100;
        }
        ((self.scroll_offset as f64 / max as f64) * 100.0) as u16
    }

    // ── File browser ──────────────────────────────────────────────────────────

    /// Opens the file browser with `.md` files discovered from the current directory.
    fn open_file_browser(&mut self) {
        let entries = discover_md_files();
        if entries.is_empty() {
            return;
        }
        self.file_browser = Some(FileBrowserState {
            entries,
            selected: 0,
            scroll: 0,
        });
    }

    /// Handles key input while the file browser is open.
    fn handle_file_browser_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('f') => {
                self.file_browser = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.file_browser {
                    if state.selected + 1 < state.entries.len() {
                        state.selected += 1;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.file_browser {
                    state.selected = state.selected.saturating_sub(1);
                }
            }
            KeyCode::Char('d') | KeyCode::PageDown => {
                if let Some(state) = &mut self.file_browser {
                    let jump = (self.viewport_height / 2).max(1);
                    state.selected = (state.selected + jump).min(state.entries.len().saturating_sub(1));
                }
            }
            KeyCode::Char('u') | KeyCode::PageUp => {
                if let Some(state) = &mut self.file_browser {
                    let jump = (self.viewport_height / 2).max(1);
                    state.selected = state.selected.saturating_sub(jump);
                }
            }
            KeyCode::Char('g') | KeyCode::Home => {
                if let Some(state) = &mut self.file_browser {
                    state.selected = 0;
                }
            }
            KeyCode::Char('G') | KeyCode::End => {
                if let Some(state) = &mut self.file_browser {
                    state.selected = state.entries.len().saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let path = self.file_browser.as_ref().map(|s| s.entries[s.selected].clone());
                self.file_browser = None;
                self.file_selected = path;
            }
            KeyCode::Char('q') => {
                self.file_browser = None;
            }
            _ => {}
        }
    }

    // ── Search ────────────────────────────────────────────────────────────────

    /// Enters search input mode with an empty query.
    fn enter_search_mode(&mut self) {
        self.search = Some(SearchState {
            query: String::new(),
            matches: Vec::new(),
            focus: 0,
            active: true,
        });
    }

    /// Handles key input while in search typing mode.
    fn handle_search_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search = None;
            }
            KeyCode::Enter => {
                self.execute_search();
            }
            KeyCode::Backspace => {
                if let Some(state) = &mut self.search {
                    if state.query.is_empty() {
                        // Backspace on empty query cancels search.
                        self.search = None;
                    } else {
                        state.query.pop();
                    }
                }
            }
            KeyCode::Char(c) => {
                if let Some(state) = &mut self.search {
                    if state.query.len() < MAX_SEARCH_QUERY_LEN {
                        state.query.push(c);
                    }
                }
            }
            _ => {}
        }
    }

    /// Executes the search: scans all document lines for case-insensitive
    /// substring matches, populates results, and scrolls to the first match.
    fn execute_search(&mut self) {
        let state = match &mut self.search {
            Some(s) => s,
            None => return,
        };

        // Empty query: stay in active mode, do nothing.
        if state.query.is_empty() {
            return;
        }

        state.active = false;

        let query_lower = state.query.to_lowercase();
        let mut matches = Vec::new();

        for (line_idx, doc_line) in self.document.lines.iter().enumerate() {
            let plain = line_plain_text(doc_line);
            if plain.is_empty() {
                continue;
            }
            let plain_lower = plain.to_lowercase();
            let mut search_start = 0;
            while let Some(pos) = plain_lower[search_start..].find(&query_lower) {
                let byte_start = search_start + pos;
                let byte_end = byte_start + query_lower.len();
                matches.push(SearchMatch {
                    line_index: line_idx,
                    byte_start,
                    byte_end,
                });
                // Advance past this match to find overlapping/subsequent matches.
                search_start = byte_start + 1;
                if search_start >= plain_lower.len() {
                    break;
                }
            }
        }

        state.matches = matches;
        state.focus = 0;

        // Scroll to the first match.
        self.scroll_to_focused_match();
    }

    /// Navigates to the next search match (wraps around).
    fn search_next(&mut self) {
        if let Some(state) = &mut self.search {
            if !state.matches.is_empty() {
                state.focus = (state.focus + 1) % state.matches.len();
            }
        }
        self.scroll_to_focused_match();
    }

    /// Navigates to the previous search match (wraps around).
    fn search_prev(&mut self) {
        if let Some(state) = &mut self.search {
            if !state.matches.is_empty() {
                state.focus = if state.focus == 0 {
                    state.matches.len() - 1
                } else {
                    state.focus - 1
                };
            }
        }
        self.scroll_to_focused_match();
    }

    /// Scrolls the viewport so the currently focused match is visible.
    fn scroll_to_focused_match(&mut self) {
        let line_index = match &self.search {
            Some(state) if !state.matches.is_empty() => {
                state.matches[state.focus].line_index
            }
            _ => return,
        };

        let max = self.max_scroll();
        // If the match line is above the viewport, scroll up to it.
        if line_index < self.scroll_offset {
            self.scroll_offset = line_index.min(max);
        }
        // If the match line is below the viewport, scroll down to put it in view.
        else if line_index >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = line_index
                .saturating_sub(self.viewport_height / 2)
                .min(max);
        }
    }
}

/// Maximum recursion depth for directory walking.
const FILE_BROWSER_MAX_DEPTH: usize = 5;
/// Maximum number of files to collect.
const FILE_BROWSER_MAX_FILES: usize = 500;

/// Discovers `.md` and `.markdown` files in the current directory, recursively.
///
/// Returns sorted relative paths, limited to `FILE_BROWSER_MAX_FILES` entries
/// and `FILE_BROWSER_MAX_DEPTH` directory levels.
fn discover_md_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => return files,
    };
    walk_dir(&cwd, &cwd, 0, &mut files);
    files.sort();
    files.truncate(FILE_BROWSER_MAX_FILES);
    files
}

/// Recursive directory walker for markdown file discovery.
fn walk_dir(base: &std::path::Path, dir: &std::path::Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > FILE_BROWSER_MAX_DEPTH || files.len() >= FILE_BROWSER_MAX_FILES {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        // Use the entry's own type so symlinks are never followed: a symlinked
        // file or directory could escape the working-directory subtree (or form
        // a traversal loop). Skip symlinks entirely.
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            // Skip hidden directories.
            if entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with('.'))
            {
                continue;
            }
            subdirs.push(path);
        } else if file_type.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
                    if let Ok(rel) = path.strip_prefix(base) {
                        files.push(rel.to_path_buf());
                        if files.len() >= FILE_BROWSER_MAX_FILES {
                            return;
                        }
                    }
                }
            }
        }
    }
    for subdir in subdirs {
        walk_dir(base, &subdir, depth + 1, files);
        if files.len() >= FILE_BROWSER_MAX_FILES {
            return;
        }
    }
}

/// Extracts the plain text content from a `DocumentLine`.
///
/// Returns the concatenation of all span contents for `Text`, `Code`, and
/// `AsciiArt` lines. Returns an empty string for other line types.
fn line_plain_text(line: &DocumentLine) -> String {
    match line {
        DocumentLine::Text(l) | DocumentLine::Code(l) | DocumentLine::AsciiArt(l) => {
            l.spans.iter().map(|s| s.content.as_ref()).collect()
        }
        DocumentLine::Empty
        | DocumentLine::Rule
        | DocumentLine::ImageStart { .. }
        | DocumentLine::ImageContinuation => String::new(),
    }
}

#[cfg(test)]
mod tests;
