//! Renderer: converts DocumentLine sequences to Ratatui Frame output.
//!
//! This is the final stage of the rendering pipeline. It reads from
//! `&App` to determine which lines are visible, then draws them to
//! the terminal frame along with a status bar.
//!
//! This module never imports `pulldown_cmark` — it only sees
//! `DocumentLine` and `App`.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui_image::StatefulImage;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::App;
use crate::images::ImageManager;
use crate::layout::DocumentLine;
use crate::theme;

/// Terminals wider than this threshold use a side panel; narrower use a dropdown.
pub const OUTLINE_MIN_COLS: u16 = 101;

/// Horizontal padding between the outline border and the document content area.
pub const OUTLINE_CONTENT_PAD: u16 = 1;

/// Draws the current view of the document and status bar to the frame.
///
/// The content area occupies all rows except the last, which is reserved
/// for the status bar. For extremely small terminals (height < 2), only
/// the status bar is rendered.
pub fn draw(frame: &mut Frame, app: &App, images: &mut ImageManager) {
    let area = frame.area();

    // Reserve the bottom row for the status bar.
    let full_content_height = area.height.saturating_sub(1);

    // Determine outline mode.
    let side_panel = app.outline.is_some() && area.width >= OUTLINE_MIN_COLS;
    let drop_down = app.outline.is_some() && !side_panel;

    // Calculate layout rects.
    let (panel_rect, border_rect, content_area) = if side_panel {
        let panel_w = app.outline_panel_cols(area.width);
        let pr = Rect {
            x: area.x,
            y: area.y,
            width: panel_w,
            height: full_content_height,
        };
        let br = Rect {
            x: area.x + panel_w,
            y: area.y,
            width: 1,
            height: full_content_height,
        };
        let cr = Rect {
            x: area.x + panel_w + 1 + OUTLINE_CONTENT_PAD,
            y: area.y,
            width: area.width.saturating_sub(panel_w + 1 + OUTLINE_CONTENT_PAD),
            height: full_content_height,
        };
        (Some(pr), Some(br), cr)
    } else {
        let cr = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: full_content_height,
        };
        (None, None, cr)
    };

    // Draw visible document lines.
    let content_height = content_area.height as usize;
    if content_height > 0 {
        let range = app.visible_range();
        for (i, line_idx) in range.enumerate() {
            if i >= content_height {
                break;
            }
            if line_idx >= app.document.lines.len() {
                break;
            }

            let y = content_area.y.saturating_add(i as u16);
            let line_area = Rect {
                x: content_area.x,
                y,
                width: content_area.width,
                height: 1,
            };

            match &app.document.lines[line_idx] {
                DocumentLine::Text(line) => {
                    let paragraph = Paragraph::new(line.clone());
                    frame.render_widget(paragraph, line_area);
                }
                DocumentLine::Code(line) => {
                    let code_bg = theme::code_block_bg(&app.theme.code_block);
                    let bg_style = Style::default().bg(code_bg.unwrap_or_default());
                    let mut spans = if code_bg.is_some() {
                        vec![Span::styled(" ", bg_style)]
                    } else {
                        vec![Span::raw(" ".to_string())]
                    };
                    for span in &line.spans {
                        let mut style = span.style;
                        if let Some(bg) = code_bg {
                            style.bg = Some(bg);
                        }
                        spans.push(Span::styled(span.content.to_string(), style));
                    }
                    let used: usize = spans.iter().map(|s| s.content.width()).sum();
                    let remaining = (content_area.width as usize).saturating_sub(used);
                    if remaining > 0 && code_bg.is_some() {
                        spans.push(Span::styled(
                            " ".repeat(remaining),
                            bg_style,
                        ));
                    }
                    let code_line = Line::from(spans);
                    let paragraph = Paragraph::new(code_line);
                    frame.render_widget(paragraph, line_area);
                }
                DocumentLine::Empty => {}
                DocumentLine::Rule => {
                    let char_ = &app.theme.thematic_break.char_;
                    let char_width = char_.width().max(1);
                    let rule_char = char_.repeat(content_area.width as usize / char_width);
                    let rule_line =
                        Line::from(Span::styled(rule_char, theme::rule_style(&app.theme.thematic_break)));
                    let paragraph = Paragraph::new(rule_line);
                    frame.render_widget(paragraph, line_area);
                }
                DocumentLine::ImageStart { protocol_index, height } => {
                    let available = content_area.height.saturating_sub(i as u16);
                    let render_height = (*height).min(available);
                    if render_height > 0 {
                        let img_area = Rect {
                            x: content_area.x,
                            y,
                            width: content_area.width,
                            height: render_height,
                        };
                        let protocol = images.get_protocol(*protocol_index);
                        let widget = StatefulImage::default();
                        frame.render_stateful_widget(widget, img_area, protocol);
                    }
                }
                DocumentLine::ImageContinuation => {}
            }
        }
    }

    // Draw outline modes.
    if let Some(panel_rect) = panel_rect {
        draw_outline_panel(frame, app, panel_rect);
    }
    if let Some(border_rect) = border_rect {
        draw_outline_border(frame, app, border_rect);
    }
    if drop_down {
        draw_outline_dropdown(frame, app, content_area);
    }

    // Draw status bar at the bottom row.
    draw_status_bar(frame, app, area);
}

// ── Outline panel (side, wide terminals) ─────────────────────────────────────

/// Draws the outline as a persistent left-side panel.
fn draw_outline_panel(frame: &mut Frame, app: &App, rect: Rect) {
    let outline_state = match &app.outline {
        Some(s) => s,
        None => return,
    };

    // Fill background.
    let bg_style = theme::outline_bg_style(&app.theme.outline);
    for y in rect.y..rect.y + rect.height {
        let line_area = Rect { x: rect.x, y, width: rect.width, height: 1 };
        let fill = Span::styled(" ".repeat(rect.width as usize), bg_style);
        frame.render_widget(Paragraph::new(Line::from(fill)), line_area);
    }

    // Render heading entries with soft-wrap.
    let entries = build_outline_visual_rows(
        &app.document.headings,
        rect.width as usize,
        outline_state.selected,
        &app.theme.outline,
    );

    // Ensure the selected entry is visible by adjusting scroll.
    let panel_height = rect.height as usize;
    let scroll = compute_outline_scroll(
        &entries,
        0,
        outline_state.selected,
        panel_height,
    );

    for (row, entry) in entries.iter().skip(scroll).enumerate() {
        if row >= panel_height {
            break;
        }
        let y = rect.y + row as u16;
        let line_area = Rect { x: rect.x, y, width: rect.width, height: 1 };
        let padded = format!("{:<width$}", entry.text, width = rect.width as usize);
        let styled = Span::styled(padded, entry.style);
        frame.render_widget(Paragraph::new(Line::from(styled)), line_area);
    }
}

/// Draws the vertical border between outline panel and content.
fn draw_outline_border(frame: &mut Frame, app: &App, rect: Rect) {
    let border_style = theme::outline_border_style(&app.theme.outline);
    for y in rect.y..rect.y + rect.height {
        let line_area = Rect { x: rect.x, y, width: 1, height: 1 };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("│", border_style))),
            line_area,
        );
    }
}

// ── Outline dropdown (narrow terminals) ──────────────────────────────────────

/// Draws the outline as a dropdown overlay at the top of the content area.
fn draw_outline_dropdown(frame: &mut Frame, app: &App, content_area: Rect) {
    let outline_state = match &app.outline {
        Some(s) => s,
        None => return,
    };

    // Inner width for headings (subtract border columns).
    let inner_width = content_area.width.saturating_sub(4) as usize;
    if inner_width == 0 {
        return;
    }

    let entries = build_outline_visual_rows(
        &app.document.headings,
        inner_width,
        outline_state.selected,
        &app.theme.outline,
    );

    // Height: heading rows + 2 (border), capped at half of content area.
    let max_height = (content_area.height / 2).max(3) as usize;
    let dropdown_h = (entries.len() + 2).min(max_height) as u16;

    let dropdown_rect = Rect {
        x: content_area.x.saturating_add(1),
        y: content_area.y,
        width: content_area.width.saturating_sub(2),
        height: dropdown_h,
    };

    // Draw bordered block.
    let border_style = theme::outline_border_style(&app.theme.outline);
    let bg_style = theme::outline_bg_style(&app.theme.outline);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(bg_style)
        .title(" Outline (Enter: jump, Esc: close) ");
    frame.render_widget(block, dropdown_rect);

    // Render entries inside the bordered area.
    let inner_rect = Rect {
        x: dropdown_rect.x + 1,
        y: dropdown_rect.y + 1,
        width: dropdown_rect.width.saturating_sub(2),
        height: dropdown_rect.height.saturating_sub(2),
    };

    let visible_rows = inner_rect.height as usize;
    let scroll = compute_outline_scroll(
        &entries,
        0,
        outline_state.selected,
        visible_rows,
    );

    for (i, entry) in entries.iter().skip(scroll).enumerate() {
        if i >= visible_rows {
            break;
        }
        let y = inner_rect.y + i as u16;
        let line_area = Rect { x: inner_rect.x, y, width: inner_rect.width, height: 1 };
        let padded = format!("{:<width$}", entry.text, width = inner_rect.width as usize);
        let styled = Span::styled(padded, entry.style);
        frame.render_widget(Paragraph::new(Line::from(styled)), line_area);
    }
}

// ── Shared outline helpers ───────────────────────────────────────────────────

/// A single visual row in the outline (one heading may span multiple rows after soft-wrap).
struct OutlineVisualRow {
    text: String,
    style: Style,
    /// Index of the heading this row belongs to (for scroll tracking).
    heading_index: usize,
}

/// Extra indentation added to continuation lines when a heading soft-wraps.
const OUTLINE_HANG_INDENT: usize = 2;

/// Builds visual rows for all headings, applying indentation and soft-wrap.
fn build_outline_visual_rows(
    headings: &[crate::layout::HeadingEntry],
    max_width: usize,
    selected: usize,
    outline_style: &theme::OutlinePanelStyle,
) -> Vec<OutlineVisualRow> {
    let max_width = max_width.max(1);
    let mut rows = Vec::new();

    for (i, heading) in headings.iter().enumerate() {
        // Insert a blank separator row before each heading except the first.
        if i > 0 {
            rows.push(OutlineVisualRow {
                text: String::new(),
                style: theme::outline_bg_style(outline_style),
                heading_index: i.saturating_sub(1),
            });
        }

        let indent = ((heading.level as usize).saturating_sub(1)) * 2;
        let indent_str = " ".repeat(indent);
        let cont_indent = indent + OUTLINE_HANG_INDENT;
        let cont_indent_str = " ".repeat(cont_indent);
        let style = if i == selected {
            theme::outline_selected_style(outline_style)
        } else {
            theme::outline_entry_style(outline_style, heading.level)
        };

        let available = max_width.saturating_sub(indent).max(1);
        let cont_available = max_width.saturating_sub(cont_indent).max(1);

        // Soft-wrap the heading text.
        let text = &heading.text;
        if text.width() <= available {
            rows.push(OutlineVisualRow {
                text: format!("{indent_str}{text}"),
                style,
                heading_index: i,
            });
        } else {
            // Simple character-level wrapping with hanging indent.
            let mut remaining = text.as_str();
            let mut is_first = true;
            while !remaining.is_empty() {
                let (cur_indent_str, cur_available) = if is_first {
                    (&indent_str, available)
                } else {
                    (&cont_indent_str, cont_available)
                };
                let mut taken_width = 0;
                let mut byte_end = 0;
                for ch in remaining.chars() {
                    let cw = ch.width().unwrap_or(0);
                    if taken_width + cw > cur_available {
                        break;
                    }
                    taken_width += cw;
                    byte_end += ch.len_utf8();
                }
                if byte_end == 0 {
                    // Character wider than available space; take at least one.
                    byte_end = remaining.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                }
                let (chunk, rest) = remaining.split_at(byte_end);
                rows.push(OutlineVisualRow {
                    text: format!("{cur_indent_str}{chunk}"),
                    style,
                    heading_index: i,
                });
                remaining = rest;
                is_first = false;
            }
        }
    }

    rows
}

/// Computes the scroll offset to ensure the selected heading is visible.
fn compute_outline_scroll(
    rows: &[OutlineVisualRow],
    current_scroll: usize,
    selected: usize,
    visible_rows: usize,
) -> usize {
    if rows.is_empty() || visible_rows == 0 {
        return 0;
    }

    // Find the first and last visual row for the selected heading.
    let first_row = rows.iter().position(|r| r.heading_index == selected).unwrap_or(0);
    let last_row = rows.iter().rposition(|r| r.heading_index == selected).unwrap_or(first_row);

    let mut scroll = current_scroll;
    // Scroll up if selected is above the visible area.
    if first_row < scroll {
        scroll = first_row;
    }
    // Scroll down if selected is below the visible area.
    if last_row >= scroll + visible_rows {
        scroll = last_row.saturating_sub(visible_rows - 1);
    }
    scroll
}

// ── Status bar ───────────────────────────────────────────────────────────────

/// Renders the status bar at the bottom row of the given area.
fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status_y = area.y + area.height.saturating_sub(1);
    let status_area = Rect {
        x: area.x,
        y: status_y,
        width: area.width,
        height: 1,
    };

    let percent = app.scroll_percent();
    let total_lines = app.document.total_height;
    let current_line = if total_lines == 0 {
        0
    } else {
        app.scroll_offset + 1
    };

    // Build hint text based on outline state and mode.
    let hints = if app.outline.is_some() {
        if area.width >= OUTLINE_MIN_COLS {
            "Tab:nav Enter:jump <>:size o:close"
        } else {
            "Tab:nav Enter:jump Esc:close"
        }
    } else {
        "o:outline"
    };

    let status_text = format!(
        " {} | {}% | {}/{} | {} ",
        app.filename, percent, current_line, total_lines, hints
    );

    let status_style = theme::status_bar_style(&app.theme.status_bar);

    // Pad the status text to fill the entire width.
    let padded = format!("{:<width$}", status_text, width = area.width as usize);
    let status_line = Line::from(Span::styled(padded, status_style));
    let paragraph = Paragraph::new(status_line);
    frame.render_widget(paragraph, status_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::HeadingEntry;

    fn heading(level: u8, text: &str) -> HeadingEntry {
        HeadingEntry {
            level,
            text: text.to_string(),
            line_index: 0,
        }
    }

    #[test]
    fn test_outline_rows_include_separators() {
        let headings = vec![heading(1, "A"), heading(2, "B"), heading(3, "C")];
        let style = theme::OutlinePanelStyle::default();
        let rows = build_outline_visual_rows(&headings, 40, 0, &style);
        // 3 headings + 2 separator rows = 5 rows total.
        assert_eq!(rows.len(), 5);
        // Separators are at indices 1 and 3.
        assert!(rows[1].text.is_empty(), "separator row should be empty");
        assert!(rows[3].text.is_empty(), "separator row should be empty");
    }

    #[test]
    fn test_outline_no_separator_before_first() {
        let headings = vec![heading(1, "First"), heading(2, "Second")];
        let style = theme::OutlinePanelStyle::default();
        let rows = build_outline_visual_rows(&headings, 40, 0, &style);
        // First row must be the first heading, not a blank.
        assert!(!rows[0].text.is_empty());
        assert!(rows[0].text.contains("First"));
    }

    #[test]
    fn test_outline_hanging_indent() {
        // Level 2 → base indent = 2. Use narrow width to force wrapping.
        let headings = vec![heading(2, "ABCDEFGHIJ")];
        let style = theme::OutlinePanelStyle::default();
        // Width 8: indent=2, available=6 → first line fits 6 chars, rest wraps.
        // Continuation indent = 2 + 2 = 4, cont_available = 4.
        let rows = build_outline_visual_rows(&headings, 8, 0, &style);
        assert!(rows.len() >= 2, "should wrap into at least 2 rows");
        // First line: 2 spaces indent.
        let first_indent = rows[0].text.len() - rows[0].text.trim_start().len();
        assert_eq!(first_indent, 2);
        // Continuation line: 4 spaces indent (2 base + 2 hanging).
        let cont_indent = rows[1].text.len() - rows[1].text.trim_start().len();
        assert_eq!(cont_indent, 4);
    }
}
