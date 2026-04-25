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

use crate::app::{App, SearchMatch};
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
            width: area.width.saturating_sub(panel_w + 1 + OUTLINE_CONTENT_PAD).max(1),
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
    let mut image_renders = 0usize;
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

            // Collect search matches for this line (if any).
            let line_matches = collect_line_matches(app, line_idx);

            // Check if this line is the selected link line (for link mode highlighting).
            let is_link_highlight = app.link_mode
                && app.document.links.get(app.link_selected).is_some_and(|l| l.line_index == line_idx);

            // Check if this line is the selected image line (for image mode highlighting).
            // For multi-line images (ImageStart + ImageContinuation), all rows are highlighted.
            let is_image_highlight = app.image_mode
                && app.document.images.get(app.image_selected).is_some_and(|img| {
                    let start = img.line_index;
                    if line_idx < start {
                        return false;
                    }
                    // For ImageStart, count continuation lines to find the block extent.
                    let doc = &app.document.lines;
                    if start < doc.len() {
                        if let DocumentLine::ImageStart { height, .. } = &doc[start] {
                            return line_idx < start + *height as usize;
                        }
                    }
                    // For non-ImageStart blocks (AsciiArt, Text fallback), highlight only the start line.
                    line_idx == start
                });

            match &app.document.lines[line_idx] {
                DocumentLine::Text(line) => {
                    let rendered = if is_link_highlight || is_image_highlight {
                        let focused_style = theme::search_focused_style(&app.theme.search);
                        apply_link_highlight(line, focused_style, &line_matches, app)
                    } else if line_matches.is_empty() {
                        line.clone()
                    } else {
                        apply_search_highlights(line, &line_matches, app)
                    };
                    let paragraph = Paragraph::new(rendered);
                    frame.render_widget(paragraph, line_area);

                    // Overlay inline math images at their recorded column positions.
                    for entry in &app.document.inline_images {
                        if entry.line_index == line_idx {
                            let img_rect = Rect {
                                x: content_area.x + entry.col_offset,
                                y,
                                width: entry.width,
                                height: 1,
                            };
                            // Bounds check: image must fit within the content area.
                            if img_rect.x + img_rect.width <= content_area.x + content_area.width {
                                let protocol = images.get_protocol(entry.protocol_index);
                                let widget = StatefulImage::default();
                                frame.render_stateful_widget(widget, img_rect, protocol);
                                image_renders += 1;
                            }
                        }
                    }
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
                    let code_line = if is_link_highlight || is_image_highlight {
                        let focused_style = theme::search_focused_style(&app.theme.search);
                        apply_link_highlight(&Line::from(spans), focused_style, &line_matches, app)
                    } else if line_matches.is_empty() {
                        Line::from(spans)
                    } else {
                        apply_search_highlights(&Line::from(spans), &line_matches, app)
                    };
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
                DocumentLine::AsciiArt(line) => {
                    let rendered = if is_image_highlight {
                        let focused_style = theme::search_focused_style(&app.theme.search);
                        apply_link_highlight(line, focused_style, &line_matches, app)
                    } else {
                        line.clone()
                    };
                    let paragraph = Paragraph::new(rendered);
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
                        image_renders += 1;
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

    // Draw file browser overlay if active.
    if app.file_browser.is_some() {
        draw_file_browser(frame, app, content_area);
    }

    // Draw status bar at the bottom row.
    draw_status_bar(frame, app, area);

    if image_renders > 0 {
        log::debug!("rendered {image_renders} image protocols in frame");
    }
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
    for y in rect.y..rect.y.saturating_add(rect.height) {
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
    for y in rect.y..rect.y.saturating_add(rect.height) {
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
        // Tagged with the *following* heading's index so `compute_outline_scroll`'s
        // `rposition` search on the previous heading stops at its real content rows.
        if i > 0 {
            rows.push(OutlineVisualRow {
                text: String::new(),
                style: theme::outline_bg_style(outline_style),
                heading_index: i,
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
                    byte_end = remaining
                        .chars()
                        .next()
                        .expect("remaining is non-empty")
                        .len_utf8();
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
    let first_row = rows.iter().position(|r| r.heading_index == selected);
    debug_assert!(first_row.is_some(), "selected heading {selected} has no visual rows");
    let first_row = first_row.unwrap_or(0);
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

// ── File browser overlay ─────────────────────────────────────────────────────

/// Draws the file browser as a centered overlay popup.
fn draw_file_browser(frame: &mut Frame, app: &App, content_area: Rect) {
    let browser = match &app.file_browser {
        Some(b) => b,
        None => return,
    };

    // Size the popup: 60% width, up to 80% height.
    let popup_w = (content_area.width as u32 * 60 / 100).max(20).min(content_area.width as u32) as u16;
    let popup_h = (content_area.height as u32 * 80 / 100)
        .max(5)
        .min(content_area.height as u32) as u16;

    // Center the popup.
    let popup_x = content_area.x + (content_area.width.saturating_sub(popup_w)) / 2;
    let popup_y = content_area.y + (content_area.height.saturating_sub(popup_h)) / 2;
    let popup_rect = Rect {
        x: popup_x,
        y: popup_y,
        width: popup_w,
        height: popup_h,
    };

    // Use outline styles for consistency with the existing UI.
    let border_style = theme::outline_border_style(&app.theme.outline);
    let bg_style = theme::outline_bg_style(&app.theme.outline);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(bg_style)
        .title(" Open File (Enter: open, Esc: close) ");
    frame.render_widget(block, popup_rect);

    // Inner area for file entries.
    let inner = Rect {
        x: popup_rect.x + 1,
        y: popup_rect.y + 1,
        width: popup_rect.width.saturating_sub(2),
        height: popup_rect.height.saturating_sub(2),
    };
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let visible_rows = inner.height as usize;
    let total = browser.entries.len();

    // Compute scroll to keep the selected entry visible.
    let scroll = {
        let mut s = browser.scroll;
        if browser.selected < s {
            s = browser.selected;
        }
        if browser.selected >= s + visible_rows {
            s = browser.selected.saturating_sub(visible_rows - 1);
        }
        s
    };

    let selected_style = theme::outline_selected_style(&app.theme.outline);
    let normal_style = bg_style;

    for (i, entry) in browser.entries.iter().skip(scroll).enumerate() {
        if i >= visible_rows {
            break;
        }
        let idx = scroll + i;
        let y = inner.y + i as u16;
        let line_area = Rect { x: inner.x, y, width: inner.width, height: 1 };

        let display = entry.display().to_string();
        let padded = format!("{:<width$}", display, width = inner.width as usize);
        let style = if idx == browser.selected { selected_style } else { normal_style };
        let span = Span::styled(padded, style);
        frame.render_widget(Paragraph::new(Line::from(span)), line_area);
    }

    // File count indicator at the bottom-right of the border.
    if total > 0 {
        let count_text = format!(" {}/{} ", browser.selected + 1, total);
        let count_w = count_text.width();
        if count_w < popup_rect.width as usize {
            let count_x = popup_rect.x + popup_rect.width - count_w as u16 - 1;
            let count_y = popup_rect.y + popup_rect.height - 1;
            let count_area = Rect { x: count_x, y: count_y, width: count_w as u16, height: 1 };
            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(count_text, border_style))),
                count_area,
            );
        }
    }
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

    let status_style = theme::status_bar_style(&app.theme.status_bar);

    // Search mode: show search bar instead of normal status bar.
    if let Some(search) = &app.search {
        let status_text = if search.active {
            format!(" /{}_", search.query)
        } else if search.matches.is_empty() {
            format!(" /{} [no matches] | n:next N:prev Esc:clear", search.query)
        } else {
            format!(
                " /{} [{}/{}] | n:next N:prev Esc:clear",
                search.query,
                search.focus + 1,
                search.matches.len(),
            )
        };
        let padded = format!("{:<width$}", status_text, width = area.width as usize);
        let status_line = Line::from(Span::styled(padded, status_style));
        let paragraph = Paragraph::new(status_line);
        frame.render_widget(paragraph, status_area);
        return;
    }

    // Goto-line mode: show :input_ prompt.
    if let Some(input) = &app.goto_input {
        let status_text = format!(" :{input}_ | Enter:jump Esc:cancel");
        let padded = format!("{:<width$}", status_text, width = area.width as usize);
        let status_line = Line::from(Span::styled(padded, status_style));
        let paragraph = Paragraph::new(status_line);
        frame.render_widget(paragraph, status_area);
        return;
    }

    // Link mode: show link URL and navigation hints.
    if app.link_mode {
        if let Some(link) = app.document.links.get(app.link_selected) {
            let status_text = format!(
                " {} [{}/{}] | Tab:next Shift+Tab:prev Enter:follow Esc:close Back:back",
                link.url,
                app.link_selected + 1,
                app.document.links.len(),
            );
            let padded = format!("{:<width$}", status_text, width = area.width as usize);
            let status_line = Line::from(Span::styled(padded, status_style));
            let paragraph = Paragraph::new(status_line);
            frame.render_widget(paragraph, status_area);
            return;
        }
    }

    // Image mode: show image URL and navigation hints.
    if app.image_mode {
        if let Some(img) = app.document.images.get(app.image_selected) {
            let status_text = format!(
                " {} [{}/{}] | Tab:next Shift+Tab:prev Enter:open Esc:close",
                img.url,
                app.image_selected + 1,
                app.document.images.len(),
            );
            let padded = format!("{:<width$}", status_text, width = area.width as usize);
            let status_line = Line::from(Span::styled(padded, status_style));
            let paragraph = Paragraph::new(status_line);
            frame.render_widget(paragraph, status_area);
            return;
        }
    }

    let percent = app.scroll_percent();
    let total_lines = app.document.total_height;
    let current_line = if total_lines == 0 {
        0
    } else {
        app.scroll_offset + 1
    };

    // Transient status message overrides the normal status bar for one frame.
    if let Some(msg) = &app.status_message {
        let status_text = format!(" {} ", msg);
        let padded = format!("{:<width$}", status_text, width = area.width as usize);
        let status_line = Line::from(Span::styled(padded, status_style));
        let paragraph = Paragraph::new(status_line);
        frame.render_widget(paragraph, status_area);
        return;
    }

    // Build hint text based on active UI mode.
    let hints = if app.file_browser.is_some() {
        "j/k:nav Enter:open Esc:close"
    } else if app.outline.is_some() {
        if area.width >= OUTLINE_MIN_COLS {
            "Tab:nav Enter:jump <>:size o:close"
        } else {
            "Tab:nav Enter:jump Esc:close"
        }
    } else if app.print_preview && app.last_exported_pdf.is_some() {
        "y:export-pdf o:open p:exit-preview"
    } else if app.print_preview {
        "y:export-pdf p:exit-preview"
    } else if !app.nav_history.is_empty() {
        "/:search o:outline f:files l:links Back:back"
    } else {
        "/:search o:outline f:files l:links"
    };

    let theme_name = if app.print_preview {
        "print"
    } else {
        &app.theme.name
    };
    let status_text = format!(
        " {} | {}% | {}/{} | {} | t:{} ",
        app.filename, percent, current_line, total_lines, hints, theme_name
    );

    // Pad the status text to fill the entire width.
    let padded = format!("{:<width$}", status_text, width = area.width as usize);
    let status_line = Line::from(Span::styled(padded, status_style));
    let paragraph = Paragraph::new(status_line);
    frame.render_widget(paragraph, status_area);
}

// ── Search highlight helpers ──────────────────────────────────────────────────

/// Collects search matches that fall on the given line index.
///
/// Returns references to the `SearchMatch` entries for this line,
/// paired with whether each match is the focused one.
fn collect_line_matches(app: &App, line_idx: usize) -> Vec<(&SearchMatch, bool)> {
    match &app.search {
        Some(state) if !state.matches.is_empty() => {
            state
                .matches
                .iter()
                .enumerate()
                .filter(|(_, m)| m.line_index == line_idx)
                .map(|(i, m)| (m, i == state.focus))
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Applies search match highlighting to a line's spans.
///
/// Algorithm:
/// 1. Build a plain text string from all spans + a parallel array of original styles.
/// 2. Mark byte ranges that are search matches with the highlight style.
/// 3. Reconstruct spans, splitting at match boundaries.
fn apply_search_highlights(
    line: &Line<'static>,
    matches: &[(&SearchMatch, bool)],
    app: &App,
) -> Line<'static> {
    if matches.is_empty() {
        return line.clone();
    }

    let match_style = theme::search_match_style(&app.theme.search);
    let focused_style = theme::search_focused_style(&app.theme.search);

    // Build plain text and parallel byte-to-style map from existing spans.
    let mut plain = String::new();
    let mut byte_styles: Vec<Style> = Vec::new();
    for span in &line.spans {
        let text: &str = span.content.as_ref();
        for _ in text.bytes() {
            byte_styles.push(span.style);
        }
        plain.push_str(text);
    }

    if plain.is_empty() {
        return line.clone();
    }

    // Override styles at match ranges.
    for &(m, is_focused) in matches {
        let style = if is_focused { focused_style } else { match_style };
        let start = m.byte_start.min(byte_styles.len());
        let end = m.byte_end.min(byte_styles.len());
        for byte_style in byte_styles.iter_mut().take(end).skip(start) {
            *byte_style = style;
        }
    }

    // Reconstruct spans by grouping consecutive bytes with the same style.
    let mut result_spans: Vec<Span<'static>> = Vec::new();
    if byte_styles.is_empty() {
        return line.clone();
    }

    let mut run_start = 0;
    let mut run_style = byte_styles[0];

    for (i, _ch) in plain.char_indices().skip(1) {
        if byte_styles[i] != run_style {
            let text = &plain[run_start..i];
            if !text.is_empty() {
                result_spans.push(Span::styled(text.to_string(), run_style));
            }
            run_start = i;
            run_style = byte_styles[i];
        }
    }
    // Emit final run.
    let text = &plain[run_start..];
    if !text.is_empty() {
        result_spans.push(Span::styled(text.to_string(), run_style));
    }

    Line::from(result_spans)
}

/// Applies link highlight (focused style) to an entire line, plus search highlights.
///
/// When link mode is active and the current line contains the selected link,
/// the focused search style is applied to all text on the line. Search matches
/// are then layered on top.
fn apply_link_highlight(
    line: &Line<'static>,
    link_style: Style,
    search_matches: &[(&SearchMatch, bool)],
    app: &App,
) -> Line<'static> {
    let match_style = theme::search_match_style(&app.theme.search);
    let focused_style = theme::search_focused_style(&app.theme.search);

    // Build plain text and byte-to-style map.
    let mut plain = String::new();
    let mut byte_styles: Vec<Style> = Vec::new();
    for span in &line.spans {
        let text: &str = span.content.as_ref();
        for _ in text.bytes() {
            byte_styles.push(link_style);
        }
        plain.push_str(text);
    }

    if plain.is_empty() {
        return line.clone();
    }

    // Layer search matches on top of the link highlight.
    for &(m, is_focused) in search_matches {
        let style = if is_focused { focused_style } else { match_style };
        let start = m.byte_start.min(byte_styles.len());
        let end = m.byte_end.min(byte_styles.len());
        for byte_style in byte_styles.iter_mut().take(end).skip(start) {
            *byte_style = style;
        }
    }

    // Reconstruct spans.
    let mut result_spans: Vec<Span<'static>> = Vec::new();
    if byte_styles.is_empty() {
        return line.clone();
    }

    let mut run_start = 0;
    let mut run_style = byte_styles[0];

    for (i, _ch) in plain.char_indices().skip(1) {
        if byte_styles[i] != run_style {
            let text = &plain[run_start..i];
            if !text.is_empty() {
                result_spans.push(Span::styled(text.to_string(), run_style));
            }
            run_start = i;
            run_style = byte_styles[i];
        }
    }
    let text = &plain[run_start..];
    if !text.is_empty() {
        result_spans.push(Span::styled(text.to_string(), run_style));
    }

    Line::from(result_spans)
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
        // Separators are tagged with the *following* heading's index so
        // compute_outline_scroll's rposition stops at real content rows.
        assert_eq!(rows[1].heading_index, 1, "separator before B tagged with B's index");
        assert_eq!(rows[3].heading_index, 2, "separator before C tagged with C's index");
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

    // ── compute_outline_scroll tests ────────────────────────────────

    #[test]
    fn test_scroll_empty_rows() {
        let rows: Vec<OutlineVisualRow> = vec![];
        assert_eq!(compute_outline_scroll(&rows, 0, 0, 5), 0);
    }

    #[test]
    fn test_scroll_zero_visible() {
        let headings = vec![heading(1, "A")];
        let style = theme::OutlinePanelStyle::default();
        let rows = build_outline_visual_rows(&headings, 40, 0, &style);
        assert_eq!(compute_outline_scroll(&rows, 0, 0, 0), 0);
    }

    #[test]
    fn test_scroll_selected_visible_no_change() {
        let headings = vec![heading(1, "A"), heading(2, "B"), heading(3, "C")];
        let style = theme::OutlinePanelStyle::default();
        let rows = build_outline_visual_rows(&headings, 40, 0, &style);
        // All 5 rows fit in 10 visible rows — scroll stays at 0.
        assert_eq!(compute_outline_scroll(&rows, 0, 0, 10), 0);
    }

    #[test]
    fn test_scroll_keeps_selected_heading_in_view() {
        let headings = vec![heading(1, "A"), heading(2, "B"), heading(3, "C")];
        let style = theme::OutlinePanelStyle::default();
        // selected=2 (heading C) — rows: 0=A, 1=sep, 2=B, 3=sep, 4=C
        let rows = build_outline_visual_rows(&headings, 40, 2, &style);
        // With only 1 visible row, scroll must land on heading C (row 4).
        let scroll = compute_outline_scroll(&rows, 0, 2, 1);
        assert_eq!(scroll, 4);
        assert!(!rows[scroll].text.is_empty(), "scrolled-to row must be heading text, not separator");
    }

    #[test]
    fn test_scroll_selected_above_scrolls_up() {
        let headings = vec![heading(1, "A"), heading(2, "B"), heading(3, "C")];
        let style = theme::OutlinePanelStyle::default();
        let rows = build_outline_visual_rows(&headings, 40, 0, &style);
        // Current scroll=4, selected=0 (heading A at row 0) — must scroll up.
        let scroll = compute_outline_scroll(&rows, 4, 0, 2);
        assert_eq!(scroll, 0);
    }
}
