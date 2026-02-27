//! Layout engine: flattens RenderedBlock IR into DocumentLine sequences for rendering.
//!
//! This module is the second stage of the rendering pipeline. It takes
//! the block-level IR from the parser and produces a flat sequence of
//! `DocumentLine`s sized to fit a given terminal width.

use pulldown_cmark::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::parser::{ListItem, RenderedBlock, StyledSpan};

/// A pre-rendered document ready for viewport slicing and rendering.
///
/// Contains all lines laid out for a specific terminal width. Created
/// once on load and again on terminal resize.
pub struct PreRenderedDocument {
    /// All document lines in display order.
    pub lines: Vec<DocumentLine>,
    /// Total number of lines (== `lines.len()`).
    pub total_height: usize,
}

/// A single line of the pre-rendered document.
///
/// The renderer matches on this enum exhaustively to produce frame output.
pub enum DocumentLine {
    /// A line of styled text (paragraph, heading, list item, etc.).
    Text(Line<'static>),
    /// A line of syntax-highlighted code (no wrapping).
    Code(Line<'static>),
    /// An empty line used for inter-block spacing.
    Empty,
    /// A horizontal rule spanning the terminal width.
    Rule,
}

/// Flattens a sequence of `RenderedBlock`s into a `PreRenderedDocument`.
///
/// Each block is converted to one or more `DocumentLine`s. Text blocks
/// are word-wrapped to fit within `width` columns. An `Empty` line is
/// inserted between adjacent blocks for visual spacing.
pub fn flatten(blocks: &[RenderedBlock], width: u16) -> PreRenderedDocument {
    let mut lines: Vec<DocumentLine> = Vec::new();
    // Clamp to minimum width of 1 to avoid undefined textwrap behavior.
    let width = (width as usize).max(1);

    for (i, block) in blocks.iter().enumerate() {
        // Inter-block spacing (not before the first block).
        if i > 0 {
            lines.push(DocumentLine::Empty);
        }
        lines.extend(flatten_single_block(block, width, 0));
    }

    let total_height = lines.len();
    PreRenderedDocument { lines, total_height }
}

/// Flattens a single `RenderedBlock` into a `Vec<DocumentLine>`.
///
/// `list_depth` is the current nesting depth of the enclosing list (0 = top-level).
/// Used by `flatten_list` when recursively flattening child blocks.
fn flatten_single_block(block: &RenderedBlock, width: usize, list_depth: usize) -> Vec<DocumentLine> {
    match block {
        RenderedBlock::Heading { content, .. } => {
            let wrapped = wrap_styled_spans(content, width);
            if wrapped.is_empty() {
                vec![DocumentLine::Empty]
            } else {
                wrapped.into_iter().map(DocumentLine::Text).collect()
            }
        }
        RenderedBlock::Paragraph { content } => {
            let wrapped = wrap_styled_spans(content, width);
            if wrapped.is_empty() {
                vec![DocumentLine::Empty]
            } else {
                wrapped.into_iter().map(DocumentLine::Text).collect()
            }
        }
        RenderedBlock::CodeBlock { language, highlighted_lines } => {
            let mut result = Vec::new();
            // Emit language label header if language is specified.
            if !language.is_empty() {
                let label = Span::styled(
                    format!(" {language} "),
                    Style::default()
                        .fg(Color::Indexed(245))
                        .bg(Color::Indexed(235))
                        .add_modifier(Modifier::ITALIC),
                );
                result.push(DocumentLine::Code(Line::from(label)));
            }
            // Emit each highlighted line (no wrapping — code is literal).
            for line in highlighted_lines {
                result.push(DocumentLine::Code(line.clone()));
            }
            result
        }
        RenderedBlock::ThematicBreak => vec![DocumentLine::Rule],
        RenderedBlock::Spacer { lines: count } => {
            (0..*count).map(|_| DocumentLine::Empty).collect()
        }
        RenderedBlock::List { ordered, start, items } => {
            flatten_list(items, *ordered, *start, width, list_depth)
        }
        RenderedBlock::BlockQuote { children } => flatten_block_quote(children, width),
        RenderedBlock::Table { headers, alignments, rows } => {
            flatten_table(headers, alignments, rows, width)
        }
    }
}

// ── List layout ──────────────────────────────────────────────────────────────

/// Flattens a list into `DocumentLine`s with bullet/number prefixes and indentation.
///
/// Bullet characters by depth: `•` (0), `◦` (1), `▪` (2+).
/// Ordered lists use `n.` where `n = start + index`.
/// Task items use `☑` (checked) or `☐` (unchecked).
fn flatten_list(
    items: &[ListItem],
    ordered: bool,
    start: u64,
    width: usize,
    depth: usize,
) -> Vec<DocumentLine> {
    let indent = "  ".repeat(depth);
    let mut lines = Vec::new();

    for (i, item) in items.iter().enumerate() {
        let prefix_str = if let Some(checked) = item.task {
            if checked { "☑".to_string() } else { "☐".to_string() }
        } else if ordered {
            format!("{}.", start + i as u64)
        } else {
            match depth {
                0 => "•",
                1 => "◦",
                _ => "▪",
            }
            .to_string()
        };

        // First-line prefix: "{indent}{prefix} "
        let first_prefix = format!("{indent}{prefix_str} ");
        let first_prefix_width = first_prefix.width();
        // Continuation lines align with the start of the first line's content.
        let cont_prefix = " ".repeat(first_prefix_width);

        let content_width = width.saturating_sub(first_prefix_width).max(1);
        let wrapped = wrap_styled_spans(&item.content, content_width);

        if wrapped.is_empty() {
            // Empty item — emit just the prefix.
            lines.push(DocumentLine::Text(Line::from(Span::raw(first_prefix))));
        } else {
            for (j, content_line) in wrapped.into_iter().enumerate() {
                let pref = if j == 0 { first_prefix.clone() } else { cont_prefix.clone() };
                let mut spans = vec![Span::raw(pref)];
                spans.extend(content_line.spans);
                lines.push(DocumentLine::Text(Line::from(spans)));
            }
        }

        // Flatten child blocks (nested lists, code blocks, etc.) at the next depth.
        for child in &item.children {
            let child_lines = flatten_single_block(child, width, depth + 1);
            lines.extend(child_lines);
        }
    }

    lines
}

// ── Block quote layout ───────────────────────────────────────────────────────

/// Flattens a block quote into `DocumentLine`s prefixed with `│ `.
///
/// Every output line from each child block gets a `│ ` prefix applied.
/// Nested block quotes recursively add another `│ ` level.
/// The dim+italic modifier is merged into all text spans.
fn flatten_block_quote(children: &[RenderedBlock], width: usize) -> Vec<DocumentLine> {
    const PREFIX: &str = "│ ";
    const PREFIX_WIDTH: usize = 2; // "│ " is 2 display columns
    let inner_width = width.saturating_sub(PREFIX_WIDTH).max(1);
    let quote_style = Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC);

    let mut result = Vec::new();

    for (i, child) in children.iter().enumerate() {
        // Visual spacing between child blocks inside the quote.
        if i > 0 {
            result.push(DocumentLine::Text(Line::from(Span::styled(
                PREFIX.to_string(),
                quote_style,
            ))));
        }
        let child_lines = flatten_single_block(child, inner_width, 0);
        for line in child_lines {
            match line {
                DocumentLine::Text(l) => {
                    let mut spans = vec![Span::styled(PREFIX.to_string(), quote_style)];
                    for span in l.spans {
                        spans.push(Span {
                            content: span.content,
                            style: span.style.add_modifier(Modifier::DIM | Modifier::ITALIC),
                        });
                    }
                    result.push(DocumentLine::Text(Line::from(spans)));
                }
                DocumentLine::Empty => {
                    result.push(DocumentLine::Text(Line::from(Span::styled(
                        PREFIX.to_string(),
                        quote_style,
                    ))));
                }
                DocumentLine::Code(l) => {
                    // Code inside a block quote: add prefix but keep code formatting.
                    let mut spans = vec![Span::styled(PREFIX.to_string(), quote_style)];
                    spans.extend(l.spans);
                    result.push(DocumentLine::Code(Line::from(spans)));
                }
                DocumentLine::Rule => {
                    result.push(DocumentLine::Text(Line::from(vec![
                        Span::styled(PREFIX.to_string(), quote_style),
                        Span::styled("─".repeat(inner_width), quote_style),
                    ])));
                }
            }
        }
    }

    result
}

// ── Table layout ─────────────────────────────────────────────────────────────

/// Flattens a GFM table into `DocumentLine`s: header row, separator, body rows.
///
/// Column widths are auto-calculated from the widest content in each column.
/// If the total table width exceeds the terminal width, each column is capped
/// proportionally to fit — the table is truncated, not panicking.
fn flatten_table(
    headers: &[Vec<StyledSpan>],
    alignments: &[Alignment],
    rows: &[Vec<Vec<StyledSpan>>],
    width: usize,
) -> Vec<DocumentLine> {
    let ncols = headers.len();
    if ncols == 0 {
        return Vec::new();
    }

    // Calculate column widths: max of header and all body cells in that column.
    let mut col_widths: Vec<usize> = headers
        .iter()
        .map(|cell| cell.iter().map(|s| s.text.chars().count()).sum::<usize>().max(3))
        .collect();

    for row in rows {
        for (col, cell) in row.iter().enumerate() {
            if col < col_widths.len() {
                let w: usize = cell.iter().map(|s| s.text.chars().count()).sum();
                col_widths[col] = col_widths[col].max(w);
            }
        }
    }

    // Total display width: column widths + " │ " separators between columns.
    let sep_overhead = if ncols > 1 { (ncols - 1) * 3 } else { 0 };
    let total = col_widths.iter().sum::<usize>() + sep_overhead;
    if total > width {
        // Cap each column equally to fit within available width.
        let max_col = (width.saturating_sub(sep_overhead) / ncols).max(3);
        for w in &mut col_widths {
            *w = (*w).min(max_col);
        }
    }

    let header_style = Style::default().add_modifier(Modifier::BOLD);
    let sep_style = Style::default().add_modifier(Modifier::DIM);

    let mut result = Vec::new();
    result.push(DocumentLine::Text(Line::from(build_table_row(
        headers,
        &col_widths,
        alignments,
        Some(header_style),
    ))));
    result.push(DocumentLine::Text(Line::from(build_table_separator(
        &col_widths,
        sep_style,
    ))));
    for row in rows {
        result.push(DocumentLine::Text(Line::from(build_table_row(
            row,
            &col_widths,
            alignments,
            None,
        ))));
    }
    result
}

/// Builds one row of a table as a flat `Vec<Span>`.
fn build_table_row(
    cells: &[Vec<StyledSpan>],
    col_widths: &[usize],
    alignments: &[Alignment],
    extra_style: Option<Style>,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    for (col, &col_w) in col_widths.iter().enumerate() {
        if col > 0 {
            spans.push(Span::raw(" │ ".to_string()));
        }
        let cell = cells.get(col).map(Vec::as_slice).unwrap_or(&[]);
        let plain: String = cell.iter().map(|s| s.text.as_str()).collect();
        let align = alignments.get(col).copied().unwrap_or(Alignment::None);
        let padded = align_cell(&plain, col_w, align);

        let span = if let Some(style) = extra_style {
            Span::styled(padded, style)
        } else {
            Span::raw(padded)
        };
        spans.push(span);
    }

    spans
}

/// Builds the separator row (`───┼───`) for a table.
fn build_table_separator(col_widths: &[usize], style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, &w) in col_widths.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("─┼─".to_string(), style));
        }
        spans.push(Span::styled("─".repeat(w), style));
    }
    spans
}

/// Pads or truncates `text` to exactly `width` display columns with given alignment.
fn align_cell(text: &str, width: usize, alignment: Alignment) -> String {
    let char_count = text.chars().count();
    if char_count >= width {
        // Truncate to fit — prevents overflow into adjacent columns.
        text.chars().take(width).collect()
    } else {
        let padding = width - char_count;
        match alignment {
            Alignment::Right => format!("{:>width$}", text, width = width),
            Alignment::Center => {
                let left = padding / 2;
                let right = padding - left;
                format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
            }
            _ => format!("{:<width$}", text, width = width),
        }
    }
}

// ── Text wrapping ─────────────────────────────────────────────────────────────

/// Wraps styled spans to fit within a given width, preserving styles.
///
/// Algorithm:
/// 1. Concatenate all span text into a single plain-text string, building
///    a parallel byte-to-style map.
/// 2. Use `textwrap::wrap()` to determine line break positions.
/// 3. Walk a cursor through the plain text for each wrapped line, skipping
///    whitespace break points, then extract styled spans by consulting
///    the byte-to-style map.
fn wrap_styled_spans(spans: &[StyledSpan], width: usize) -> Vec<Line<'static>> {
    if spans.is_empty() {
        return Vec::new();
    }

    // Handle hard breaks (\n) by splitting into sub-paragraphs.
    if spans.iter().any(|s| s.text.contains('\n')) {
        return wrap_with_hard_breaks(spans, width);
    }

    // 1. Build plain text and parallel byte-to-style map.
    let mut plain = String::new();
    let mut byte_styles: Vec<Style> = Vec::new();
    for span in spans {
        for _ in span.text.bytes() {
            byte_styles.push(span.style);
        }
        plain.push_str(&span.text);
    }

    if plain.is_empty() {
        return Vec::new();
    }

    // 2. Wrap the plain text.
    let wrap_options = textwrap::Options::new(width)
        .word_separator(textwrap::WordSeparator::UnicodeBreakProperties);
    let wrapped_lines = textwrap::wrap(&plain, &wrap_options);

    // 3. Map each wrapped line back to styled spans using a monotonic cursor.
    let mut result = Vec::with_capacity(wrapped_lines.len());
    let mut cursor: usize = 0;

    for wrapped_text in &wrapped_lines {
        let wrapped_str: &str = wrapped_text.as_ref();

        // Skip whitespace between wrapped lines (break points consumed by textwrap).
        // Only advance forward — the cursor never goes backward.
        while cursor < plain.len() {
            if plain[cursor..].starts_with(wrapped_str) {
                break;
            }
            // Advance by one character (not one byte) to stay on char boundaries.
            let ch_len = plain[cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            cursor += ch_len;
        }

        // Guard: if cursor exhausted `plain` without finding this line, textwrap
        // returned a `Cow::Owned` string with modified content (e.g. soft hyphen
        // stripped by UnicodeBreakProperties). Emitting built_spans_for_range would
        // either produce empty spans (silent data loss) or slice on a non-char
        // boundary (panic). Fall back to emitting the wrapped text directly instead.
        if cursor >= plain.len() && !plain.ends_with(wrapped_str) {
            result.push(Line::from(Span::raw(wrapped_str.to_string())));
            continue;
        }

        let line_start = cursor;
        let line_end = cursor + wrapped_str.len();
        // Clamp to plain text length for safety.
        let line_end = line_end.min(plain.len());

        // Verify the end is on a char boundary before slicing. If not (can only
        // happen with Cow::Owned from textwrap), emit the text directly.
        if !plain.is_char_boundary(line_end) {
            result.push(Line::from(Span::raw(wrapped_str.to_string())));
            cursor = line_end.min(plain.len());
            continue;
        }

        let line_spans = build_spans_for_range(&plain, &byte_styles, line_start, line_end);
        result.push(Line::from(line_spans));

        cursor = line_end;
    }

    result
}

/// Builds styled `Span`s for a byte range of the plain text.
///
/// Walks through the range by characters, grouping consecutive bytes
/// that share the same style into a single `Span`. All slicing happens
/// at character boundaries.
fn build_spans_for_range(
    plain: &str,
    byte_styles: &[Style],
    start: usize,
    end: usize,
) -> Vec<Span<'static>> {
    if start >= end || start >= plain.len() {
        return Vec::new();
    }

    let segment = &plain[start..end];
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run_start = start;
    let mut run_style = byte_styles[start];

    for (i, _ch) in segment.char_indices() {
        let abs_pos = start + i;
        if byte_styles[abs_pos] != run_style {
            let text = &plain[run_start..abs_pos];
            if !text.is_empty() {
                spans.push(Span::styled(text.to_string(), run_style));
            }
            run_start = abs_pos;
            run_style = byte_styles[abs_pos];
        }
    }

    // Emit final run.
    let text = &plain[run_start..end];
    if !text.is_empty() {
        spans.push(Span::styled(text.to_string(), run_style));
    }

    spans
}

/// Handles text containing hard breaks by splitting at `\n` boundaries
/// first, then wrapping each segment independently.
fn wrap_with_hard_breaks(spans: &[StyledSpan], width: usize) -> Vec<Line<'static>> {
    let mut groups: Vec<Vec<StyledSpan>> = Vec::new();
    let mut current_group: Vec<StyledSpan> = Vec::new();

    for span in spans {
        if span.text.contains('\n') {
            let parts: Vec<&str> = span.text.split('\n').collect();
            for (i, part) in parts.iter().enumerate() {
                if !part.is_empty() {
                    current_group.push(StyledSpan {
                        text: part.to_string(),
                        style: span.style,
                    });
                }
                if i < parts.len() - 1 {
                    groups.push(std::mem::take(&mut current_group));
                }
            }
        } else {
            current_group.push(StyledSpan {
                text: span.text.clone(),
                style: span.style,
            });
        }
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    let mut result = Vec::new();
    for group in &groups {
        let wrapped = wrap_styled_spans(group, width);
        if wrapped.is_empty() {
            result.push(Line::from(Vec::<Span<'static>>::new()));
        } else {
            result.extend(wrapped);
        }
    }

    result
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
