//! Layout engine: flattens RenderedBlock IR into DocumentLine sequences for rendering.
//!
//! This module is the second stage of the rendering pipeline. It takes
//! the block-level IR from the parser and produces a flat sequence of
//! `DocumentLine`s sized to fit a given terminal width.

use pulldown_cmark::Alignment;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::parser::{ListItem, RenderedBlock, StyledSpan, TableCell};
use crate::theme::{self, MarkdownTheme};

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
    /// A line of colored ASCII art (no wrapping, no code-block background).
    AsciiArt(Line<'static>),
    /// The first line of an image — triggers rendering at draw time.
    ImageStart { protocol_index: usize, height: u16 },
    /// Continuation lines for an image — reserves vertical space for scrolling.
    ImageContinuation,
}

/// Flattens a sequence of `RenderedBlock`s into a `PreRenderedDocument`.
///
/// Each block is converted to one or more `DocumentLine`s. Text blocks
/// are word-wrapped to fit within `width` columns. An `Empty` line is
/// inserted between adjacent blocks for visual spacing.
pub fn flatten(blocks: &[RenderedBlock], width: u16, theme: &MarkdownTheme) -> PreRenderedDocument {
    let mut lines: Vec<DocumentLine> = Vec::new();
    // Clamp to minimum width of 1 to avoid undefined textwrap behavior.
    let width = (width as usize).max(1);

    for (i, block) in blocks.iter().enumerate() {
        // Inter-block spacing (not before the first block).
        if i > 0 {
            lines.push(DocumentLine::Empty);
        }
        lines.extend(flatten_single_block(block, width, 0, theme));
    }

    let total_height = lines.len();
    PreRenderedDocument { lines, total_height }
}

/// Flattens a single `RenderedBlock` into a `Vec<DocumentLine>`.
///
/// `list_depth` is the current nesting depth of the enclosing list (0 = top-level).
/// Used by `flatten_list` when recursively flattening child blocks.
fn flatten_single_block(block: &RenderedBlock, width: usize, list_depth: usize, theme: &MarkdownTheme) -> Vec<DocumentLine> {
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
                    theme::code_label_style(&theme.code_block),
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
            flatten_list(items, *ordered, *start, width, list_depth, theme)
        }
        RenderedBlock::BlockQuote { children } => flatten_block_quote(children, width, list_depth, theme),
        RenderedBlock::Table { headers, alignments, rows } => {
            flatten_table(headers, alignments, rows, width, theme)
        }
        RenderedBlock::AsciiImage { lines, .. } => {
            lines.iter().map(|l| DocumentLine::AsciiArt(l.clone())).collect()
        }
        // Phase 4: images are rendered by the renderer via StatefulImage.
        // Layout emits ImageStart for the first row (renderer draws there) and
        // ImageContinuation for remaining rows (reserve scroll space).
        RenderedBlock::Image { protocol_index, height_cells, alt_text, .. } => {
            let height = *height_cells;
            if height == 0 {
                // Degenerate case: emit a fallback text line so the image
                // is not completely invisible.
                return vec![DocumentLine::Text(Line::from(Span::raw(
                    format!("[image: {alt_text}]"),
                )))];
            }
            let mut lines = Vec::with_capacity(height as usize);
            lines.push(DocumentLine::ImageStart { protocol_index: *protocol_index, height });
            for _ in 1..height {
                lines.push(DocumentLine::ImageContinuation);
            }
            lines
        }
        RenderedBlock::ImageFallback { alt_text } => {
            let wrapped = wrap_styled_spans(
                &[crate::parser::StyledSpan {
                    text: format!("[image: {}]", alt_text),
                    style: theme::inline_style(&theme.image_alt),
                }],
                width,
            );
            if wrapped.is_empty() {
                vec![DocumentLine::Empty]
            } else {
                wrapped.into_iter().map(DocumentLine::Text).collect()
            }
        }
    }
}

// ── List layout ──────────────────────────────────────────────────────────────

/// Flattens a list into `DocumentLine`s with bullet/number prefixes and indentation.
///
/// Bullet characters and task markers are configured by the theme.
/// Ordered lists use `n.` where `n = start + index`.
fn flatten_list(
    items: &[ListItem],
    ordered: bool,
    start: u64,
    width: usize,
    depth: usize,
    theme: &MarkdownTheme,
) -> Vec<DocumentLine> {
    let indent = " ".repeat(theme.list.indent_size as usize * depth);
    let mut lines = Vec::new();

    for (i, item) in items.iter().enumerate() {
        let (prefix_str, marker_style) = if let Some(checked) = item.task {
            if checked {
                (theme.list.task_checked.clone(), theme::list_task_checked_style(&theme.list))
            } else {
                (theme.list.task_unchecked.clone(), theme::list_task_unchecked_style(&theme.list))
            }
        } else if ordered {
            (format!("{}.", start + i as u64), theme::list_number_style(&theme.list))
        } else {
            let bullet = theme.list.bullet
                .get(depth)
                .or(theme.list.bullet.last())
                .cloned()
                .unwrap_or_else(|| "•".to_string());
            (bullet, theme::list_bullet_style(&theme.list))
        };

        // First-line prefix: "{indent}{prefix} "
        let first_prefix_width = indent.width() + prefix_str.width() + 1;
        // Continuation lines align with the start of the first line's content.
        let cont_prefix = " ".repeat(first_prefix_width);

        let content_width = width.saturating_sub(first_prefix_width).max(1);
        let wrapped = wrap_styled_spans(&item.content, content_width);

        // Build the first-line prefix as styled spans: indent (raw) + marker (styled) + space (raw).
        let prefix_spans = vec![
            Span::raw(indent.clone()),
            Span::styled(prefix_str, marker_style),
            Span::raw(" ".to_string()),
        ];

        if wrapped.is_empty() {
            // Empty item — emit just the prefix.
            lines.push(DocumentLine::Text(Line::from(prefix_spans)));
        } else {
            for (j, content_line) in wrapped.into_iter().enumerate() {
                let mut spans = if j == 0 {
                    prefix_spans.clone()
                } else {
                    vec![Span::raw(cont_prefix.clone())]
                };
                spans.extend(content_line.spans);
                lines.push(DocumentLine::Text(Line::from(spans)));
            }
        }

        // Flatten child blocks. Nested lists handle their own indentation via
        // depth+1. Non-list children (code blocks, blockquotes, paragraphs)
        // need an explicit indent prefix to align under the item's content.
        for child in &item.children {
            match child {
                RenderedBlock::List { .. } => {
                    let child_lines = flatten_single_block(child, width, depth + 1, theme);
                    lines.extend(child_lines);
                }
                _ => {
                    let child_width = width.saturating_sub(first_prefix_width).max(1);
                    let child_lines = flatten_single_block(child, child_width, depth + 1, theme);
                    for line in child_lines {
                        match line {
                            DocumentLine::Text(l) => {
                                let mut spans = vec![Span::raw(cont_prefix.clone())];
                                spans.extend(l.spans);
                                lines.push(DocumentLine::Text(Line::from(spans)));
                            }
                            DocumentLine::Code(l) => {
                                let mut spans = vec![Span::raw(cont_prefix.clone())];
                                spans.extend(l.spans);
                                lines.push(DocumentLine::Code(Line::from(spans)));
                            }
                            DocumentLine::AsciiArt(l) => {
                                let mut spans = vec![Span::raw(cont_prefix.clone())];
                                spans.extend(l.spans);
                                lines.push(DocumentLine::AsciiArt(Line::from(spans)));
                            }
                            other => lines.push(other),
                        }
                    }
                }
            }
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
fn flatten_block_quote(children: &[RenderedBlock], width: usize, list_depth: usize, theme: &MarkdownTheme) -> Vec<DocumentLine> {
    let prefix = &theme.block_quote.prefix;
    let prefix_width = prefix.width();
    let inner_width = width.saturating_sub(prefix_width).max(1);
    let prefix_style = theme::quote_prefix_style(&theme.block_quote);
    let content_modifier = theme::quote_content_style(&theme.block_quote);

    let mut result = Vec::new();

    for (i, child) in children.iter().enumerate() {
        // Visual spacing between child blocks inside the quote.
        if i > 0 {
            result.push(DocumentLine::Text(Line::from(Span::styled(
                prefix.to_string(),
                prefix_style,
            ))));
        }
        // Thread list_depth so nested lists inside block quotes render with
        // the correct bullet depth rather than resetting to depth 0.
        let child_lines = flatten_single_block(child, inner_width, list_depth, theme);
        for line in child_lines {
            match line {
                DocumentLine::Text(l) => {
                    let mut spans = vec![Span::styled(prefix.to_string(), prefix_style)];
                    for span in l.spans {
                        spans.push(Span {
                            content: span.content,
                            style: span.style.patch(content_modifier),
                        });
                    }
                    result.push(DocumentLine::Text(Line::from(spans)));
                }
                DocumentLine::Empty => {
                    result.push(DocumentLine::Text(Line::from(Span::styled(
                        prefix.to_string(),
                        prefix_style,
                    ))));
                }
                DocumentLine::Code(l) => {
                    // Code inside a block quote: add prefix but keep code formatting.
                    let mut spans = vec![Span::styled(prefix.to_string(), prefix_style)];
                    spans.extend(l.spans);
                    result.push(DocumentLine::Code(Line::from(spans)));
                }
                DocumentLine::Rule => {
                    let rule_char = &theme.thematic_break.char_;
                    let rule_width = rule_char.width().max(1);
                    result.push(DocumentLine::Text(Line::from(vec![
                        Span::styled(prefix.to_string(), prefix_style),
                        Span::styled(rule_char.repeat(inner_width / rule_width), prefix_style),
                    ])));
                }
                DocumentLine::AsciiArt(l) => {
                    let mut spans = vec![Span::styled(prefix.to_string(), prefix_style)];
                    spans.extend(l.spans);
                    result.push(DocumentLine::AsciiArt(Line::from(spans)));
                }
                // Images inside block quotes: pass through with prefix.
                // The renderer draws at absolute position within the allocated area.
                DocumentLine::ImageStart { .. } | DocumentLine::ImageContinuation => {
                    result.push(line);
                }
            }
        }
    }

    result
}

// ── Table layout ─────────────────────────────────────────────────────────────

/// Flattens a GFM table into `DocumentLine`s: header row(s), separator, body rows.
///
/// Column widths are auto-calculated from the widest content in each column.
/// If the total table width exceeds the terminal width, each column is capped
/// proportionally to fit — the table is truncated, not panicking.
///
/// Rows containing block-level cells (e.g. ASCII art images) may span multiple
/// terminal lines. Shorter cells in the same row are padded with blank lines.
fn flatten_table(
    headers: &[TableCell],
    alignments: &[Alignment],
    rows: &[Vec<TableCell>],
    width: usize,
    theme: &MarkdownTheme,
) -> Vec<DocumentLine> {
    let ncols = headers.len();
    if ncols == 0 {
        return Vec::new();
    }

    // Calculate column widths: max of header and all body cells in that column.
    let mut col_widths: Vec<usize> = headers
        .iter()
        .map(|cell| cell_display_width(cell).max(3))
        .collect();

    for row in rows {
        for (col, cell) in row.iter().enumerate() {
            if col < col_widths.len() {
                col_widths[col] = col_widths[col].max(cell_display_width(cell));
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

    let header_style = theme::table_header_style(&theme.table);
    let sep_style = theme::table_border_style(&theme.table);

    let mut result = Vec::new();

    // Header row (may be multi-line if headers contain images).
    let header_cell_lines: Vec<Vec<Vec<Span<'static>>>> = headers
        .iter()
        .enumerate()
        .map(|(col, cell)| {
            let align = alignments.get(col).copied().unwrap_or(Alignment::None);
            flatten_cell_to_lines(cell, col_widths[col], align, Some(header_style))
        })
        .collect();
    result.extend(build_multi_line_row(&header_cell_lines, &col_widths));

    // Separator row.
    result.push(DocumentLine::Text(Line::from(build_table_separator(
        &col_widths,
        sep_style,
    ))));

    // Body rows.
    for row in rows {
        let cell_lines: Vec<Vec<Vec<Span<'static>>>> = col_widths
            .iter()
            .enumerate()
            .map(|(col, &col_w)| {
                let cell = row.get(col);
                let align = alignments.get(col).copied().unwrap_or(Alignment::None);
                match cell {
                    Some(c) => flatten_cell_to_lines(c, col_w, align, None),
                    None => vec![vec![Span::raw(" ".repeat(col_w))]],
                }
            })
            .collect();
        result.extend(build_multi_line_row(&cell_lines, &col_widths));
    }

    result
}

/// Returns the display width of a single table cell for column-width calculation.
fn cell_display_width(cell: &TableCell) -> usize {
    match cell {
        TableCell::Text(spans) => spans.iter().map(|s| s.text.width()).sum(),
        TableCell::Block(RenderedBlock::AsciiImage { lines, .. }) => lines
            .first()
            .map(|l| l.spans.iter().map(|s| s.content.width()).sum())
            .unwrap_or(0),
        TableCell::Block(RenderedBlock::ImageFallback { alt_text }) => {
            format!("[image: {}]", alt_text).width()
        }
        TableCell::Block(_) => 5, // "[...]"
    }
}

/// Converts one table cell into lines of spans (outer vec = lines, inner = spans per line).
///
/// Text cells produce a single line (padded/truncated to `col_width`).
/// AsciiImage cells produce N lines (one per image row), each truncated to `col_width`.
fn flatten_cell_to_lines(
    cell: &TableCell,
    col_width: usize,
    alignment: Alignment,
    header_style: Option<Style>,
) -> Vec<Vec<Span<'static>>> {
    let lines = match cell {
        TableCell::Text(spans) => {
            let plain: String = spans.iter().map(|s| s.text.as_str()).collect();
            let padded = align_cell(&plain, col_width, alignment);
            let span = if let Some(style) = header_style {
                Span::styled(padded, style)
            } else {
                Span::raw(padded)
            };
            vec![vec![span]]
        }
        TableCell::Block(RenderedBlock::AsciiImage { lines, .. }) => {
            lines
                .iter()
                .map(|line| truncate_spans_to_width(&line.spans, col_width))
                .collect()
        }
        TableCell::Block(RenderedBlock::ImageFallback { alt_text }) => {
            let text = format!("[image: {}]", alt_text);
            let padded = align_cell(&text, col_width, alignment);
            let span = if let Some(style) = header_style {
                Span::styled(padded, style)
            } else {
                Span::raw(padded)
            };
            vec![vec![span]]
        }
        TableCell::Block(_) => {
            let padded = align_cell("[...]", col_width, alignment);
            vec![vec![Span::raw(padded)]]
        }
    };
    // Guard: minimum 1 line per cell.
    if lines.is_empty() {
        vec![vec![Span::raw(" ".repeat(col_width))]]
    } else {
        lines
    }
}

/// Truncates a slice of spans to fit within `max_width` display columns,
/// padding with spaces if the spans are shorter.
fn truncate_spans_to_width(spans: &[Span<'_>], max_width: usize) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let mut used = 0usize;
    for span in spans {
        let sw = span.content.width();
        if used + sw <= max_width {
            out.push(Span::styled(span.content.to_string(), span.style));
            used += sw;
        } else {
            break;
        }
    }
    if used < max_width {
        out.push(Span::raw(" ".repeat(max_width - used)));
    }
    out
}

/// Composes per-cell line arrays into `DocumentLine`s for one table row.
///
/// Each cell may have a different number of lines (e.g. text = 1, image = 16).
/// The row height is the maximum across all cells. Shorter cells are padded
/// with blank lines below (top-aligned).
fn build_multi_line_row(
    cell_lines: &[Vec<Vec<Span<'static>>>],
    col_widths: &[usize],
) -> Vec<DocumentLine> {
    let row_height = cell_lines.iter().map(|cl| cl.len()).max().unwrap_or(1);
    let mut result = Vec::with_capacity(row_height);

    for line_idx in 0..row_height {
        let mut spans = Vec::new();
        for (col, cl) in cell_lines.iter().enumerate() {
            if col > 0 {
                spans.push(Span::raw(" │ ".to_string()));
            }
            if line_idx < cl.len() {
                spans.extend(cl[line_idx].iter().cloned());
            } else {
                // Empty padding below shorter cells.
                let w = col_widths.get(col).copied().unwrap_or(3);
                spans.push(Span::raw(" ".repeat(w)));
            }
        }
        result.push(DocumentLine::Text(Line::from(spans)));
    }

    result
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
///
/// Uses `UnicodeWidthStr` / `UnicodeWidthChar` so that wide characters (e.g. CJK,
/// emoji) are counted by their terminal cell width, not by Unicode scalar count.
fn align_cell(text: &str, width: usize, alignment: Alignment) -> String {
    let display_width = text.width();
    if display_width >= width {
        // Truncate char-by-char to avoid splitting wide characters mid-cell.
        let mut taken = 0usize;
        text.chars()
            .take_while(|c| {
                let cw = c.width().unwrap_or(0);
                if taken + cw <= width {
                    taken += cw;
                    true
                } else {
                    false
                }
            })
            .collect()
    } else {
        let padding = width - display_width;
        match alignment {
            Alignment::Right => format!("{}{}", " ".repeat(padding), text),
            Alignment::Center => {
                let left = padding / 2;
                let right = padding - left;
                format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
            }
            _ => format!("{}{}", text, " ".repeat(padding)),
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
mod tests;
