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
use ratatui::widgets::Paragraph;
use ratatui_image::StatefulImage;
use unicode_width::UnicodeWidthStr;

use crate::app::App;
use crate::images::ImageManager;
use crate::layout::DocumentLine;
use crate::theme;

/// Draws the current view of the document and status bar to the frame.
///
/// The content area occupies all rows except the last, which is reserved
/// for the status bar. For extremely small terminals (height < 2), only
/// the status bar is rendered.
pub fn draw(frame: &mut Frame, app: &App, images: &mut ImageManager) {
    let area = frame.area();

    // Reserve the bottom row for the status bar.
    let content_height = area.height.saturating_sub(1) as usize;
    let content_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: content_height as u16,
    };

    // Draw visible document lines.
    if content_height > 0 {
        let range = app.visible_range();
        for (i, line_idx) in range.enumerate() {
            if i >= content_height {
                break;
            }
            if line_idx >= app.document.lines.len() {
                break;
            }

            // saturating_add prevents u16 overflow if area.y is non-zero and i is large.
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
                    // Override background on every span and add left padding.
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
                    // Fill remaining width with background.
                    // Use display width (columns), not byte length, to handle multi-byte
                    // characters correctly (e.g. Unicode operators, CJK, arrows).
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
                DocumentLine::Empty => {
                    // Nothing to render — blank line.
                }
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
                    let paragraph = Paragraph::new(line.clone());
                    frame.render_widget(paragraph, line_area);
                }
                DocumentLine::ImageStart { protocol_index, height } => {
                    // Clamp height to remaining viewport space so the image
                    // doesn't overwrite the status bar.
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
                DocumentLine::ImageContinuation => {
                    // Space already reserved by the ImageStart rendering above.
                }
            }
        }
    }

    // Draw status bar at the bottom row.
    draw_status_bar(frame, app, area);
}

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

    let status_text = format!(
        " {} | {}% | {}/{} ",
        app.filename, percent, current_line, total_lines
    );

    let status_style = theme::status_bar_style(&app.theme.status_bar);

    // Pad the status text to fill the entire width.
    let padded = format!("{:<width$}", status_text, width = area.width as usize);
    let status_line = Line::from(Span::styled(padded, status_style));
    let paragraph = Paragraph::new(status_line);
    frame.render_widget(paragraph, status_area);
}
