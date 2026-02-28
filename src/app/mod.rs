//! Application state: scroll position, viewport dimensions, quit flag.
//!
//! `App` is a pure state container — it never imports `ratatui::Frame` or
//! performs any rendering. The renderer reads from `&App` to determine
//! what to draw.

use std::ops::Range;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::layout::PreRenderedDocument;
use crate::theme::MarkdownTheme;

/// State for the outline panel when visible.
pub struct OutlineState {
    /// Index into `document.headings` for the currently selected heading.
    pub selected: usize,
}

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
}

impl App {
    /// Creates a new `App` with the given document and filename.
    ///
    /// Scroll starts at the top; viewport height is set to 0 and must
    /// be updated by `main.rs` before each draw call.
    pub fn new(document: PreRenderedDocument, filename: String, theme: MarkdownTheme) -> Self {
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
        }
    }

    /// Dispatches a key event to the appropriate scroll or quit action.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Outline-specific keys when outline is visible.
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

        match key.code {
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
}

#[cfg(test)]
mod tests;
