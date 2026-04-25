//! Layout engine: flattens RenderedBlock IR into DocumentLine sequences for rendering.
//!
//! This module is the second stage of the rendering pipeline. It takes
//! the block-level IR from the parser and produces a flat sequence of
//! `DocumentLine`s sized to fit a given terminal width.

use pulldown_cmark::Alignment;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::parser::{ListItem, RenderedBlock, StyledSpan, TableCell};
use crate::theme::{self, MarkdownTheme};

/// Metadata for a heading captured during layout for outline navigation.
pub struct HeadingEntry {
    /// Heading level (1–3).
    pub level: u8,
    /// Plain text content (formatting stripped).
    pub text: String,
    /// Index into `PreRenderedDocument::lines` where this heading starts.
    pub line_index: usize,
}

/// A hyperlink collected during layout for link navigation.
pub struct LinkEntry {
    /// Index into `PreRenderedDocument::lines` where this link's block starts.
    pub line_index: usize,
    /// The link URL (from `StyledSpan::url`).
    pub url: String,
}

/// An image entry collected during layout for image navigation.
pub struct ImageEntry {
    /// Index into `PreRenderedDocument::lines` where this image's block starts.
    pub line_index: usize,
    /// The image source URL or local file path.
    pub url: String,
}

/// An inline math image within a text line, positioned at a specific column.
pub struct InlineImageEntry {
    /// Index into `PreRenderedDocument::lines` where this image appears.
    pub line_index: usize,
    /// Index into `ImageManager::protocols` for renderer access.
    pub protocol_index: usize,
    /// Column offset (in terminal cells) where the image starts within the line.
    pub col_offset: u16,
    /// Width of the image in terminal columns.
    pub width: u16,
}

/// A pre-rendered document ready for viewport slicing and rendering.
///
/// Contains all lines laid out for a specific terminal width. Created
/// once on load and again on terminal resize.
pub struct PreRenderedDocument {
    /// All document lines in display order.
    pub lines: Vec<DocumentLine>,
    /// Total number of lines (== `lines.len()`).
    pub total_height: usize,
    /// Headings (h1–h3) collected during layout for outline navigation.
    pub headings: Vec<HeadingEntry>,
    /// Hyperlinks collected during layout for link navigation.
    pub links: Vec<LinkEntry>,
    /// Image entries collected during layout for image navigation.
    pub images: Vec<ImageEntry>,
    /// Inline math images with their column positions within text lines.
    pub inline_images: Vec<InlineImageEntry>,
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

/// Extracts all hyperlink URLs from a block's inline content.
///
/// Walks the block recursively (lists, block quotes) and returns URLs
/// in document order. Only text-content blocks yield links (per v1 scope).
/// Uses exhaustive match per standards §8.3.
fn extract_block_links(block: &RenderedBlock) -> Vec<String> {
    match block {
        RenderedBlock::Heading { content, .. }
        | RenderedBlock::Paragraph { content } => {
            content.iter().filter_map(|s| s.url.clone()).collect()
        }
        RenderedBlock::List { items, .. } => {
            let mut links = Vec::new();
            for item in items {
                links.extend(item.content.iter().filter_map(|s| s.url.clone()));
                for child in &item.children {
                    links.extend(extract_block_links(child));
                }
            }
            links
        }
        RenderedBlock::BlockQuote { children } => {
            let mut links = Vec::new();
            for child in children {
                links.extend(extract_block_links(child));
            }
            links
        }
        RenderedBlock::Table { headers, rows, .. } => {
            let mut links = Vec::new();
            for cell in headers {
                links.extend(extract_table_cell_links(cell));
            }
            for row in rows {
                for cell in row {
                    links.extend(extract_table_cell_links(cell));
                }
            }
            links
        }
        // No links in these block types.
        RenderedBlock::CodeBlock { .. }
        | RenderedBlock::ThematicBreak
        | RenderedBlock::Spacer { .. }
        | RenderedBlock::Image { .. }
        | RenderedBlock::AsciiImage { .. }
        | RenderedBlock::ImageFallback { .. }
        | RenderedBlock::ImagePending { .. }
        | RenderedBlock::MathUnicode { .. }
        | RenderedBlock::MathImage { .. } => Vec::new(),
    }
}

/// Extracts hyperlink URLs from a single table cell.
fn extract_table_cell_links(cell: &TableCell) -> Vec<String> {
    match cell {
        TableCell::Text(spans) => spans.iter().filter_map(|s| s.url.clone()).collect(),
        TableCell::Block(block) => extract_block_links(block),
    }
}

/// Extracts image source URLs from a `RenderedBlock`.
///
/// Walks the block recursively (lists, block quotes, tables) and returns
/// image URLs in document order. Uses exhaustive match per standards §8.3.
fn extract_block_images(block: &RenderedBlock) -> Vec<String> {
    match block {
        RenderedBlock::Image { src_url, .. } => vec![src_url.clone()],
        RenderedBlock::AsciiImage { src_url, .. } => vec![src_url.clone()],
        RenderedBlock::ImageFallback { src_url, .. } => vec![src_url.clone()],
        RenderedBlock::ImagePending { url, .. } => vec![url.clone()],
        RenderedBlock::List { items, .. } => {
            let mut imgs = Vec::new();
            for item in items {
                for child in &item.children {
                    imgs.extend(extract_block_images(child));
                }
            }
            imgs
        }
        RenderedBlock::BlockQuote { children } => {
            let mut imgs = Vec::new();
            for child in children {
                imgs.extend(extract_block_images(child));
            }
            imgs
        }
        RenderedBlock::Table { headers, rows, .. } => {
            let mut imgs = Vec::new();
            for cell in headers {
                imgs.extend(extract_table_cell_images(cell));
            }
            for row in rows {
                for cell in row {
                    imgs.extend(extract_table_cell_images(cell));
                }
            }
            imgs
        }
        // No images in these block types.
        RenderedBlock::Heading { .. }
        | RenderedBlock::Paragraph { .. }
        | RenderedBlock::CodeBlock { .. }
        | RenderedBlock::ThematicBreak
        | RenderedBlock::Spacer { .. }
        | RenderedBlock::MathUnicode { .. }
        | RenderedBlock::MathImage { .. } => Vec::new(),
    }
}

/// Extracts image source URLs from a single table cell.
fn extract_table_cell_images(cell: &TableCell) -> Vec<String> {
    match cell {
        TableCell::Text(_) => Vec::new(),
        TableCell::Block(block) => extract_block_images(block),
    }
}

/// Flattens a sequence of `RenderedBlock`s into a `PreRenderedDocument`.
///
/// Each block is converted to one or more `DocumentLine`s. Text blocks
/// are word-wrapped to fit within `width` columns. An `Empty` line is
/// inserted between adjacent blocks for visual spacing.
pub fn flatten(blocks: &[RenderedBlock], width: u16, theme: &MarkdownTheme) -> PreRenderedDocument {
    let mut lines: Vec<DocumentLine> = Vec::new();
    let mut headings: Vec<HeadingEntry> = Vec::new();
    let mut links: Vec<LinkEntry> = Vec::new();
    let mut images: Vec<ImageEntry> = Vec::new();
    let mut inline_images: Vec<InlineImageEntry> = Vec::new();
    // Clamp to minimum width of 1 to avoid undefined textwrap behavior.
    let width = (width as usize).max(1);

    for (i, block) in blocks.iter().enumerate() {
        // Inter-block spacing (not before the first block).
        if i > 0 {
            lines.push(DocumentLine::Empty);
        }

        // Collect heading metadata before layout discards the level.
        if let RenderedBlock::Heading { level, content } = block {
            if *level <= 3 {
                let text: String = content.iter().map(|s| s.text.as_str()).collect();
                headings.push(HeadingEntry {
                    level: *level,
                    text,
                    line_index: lines.len(),
                });
            }
        }

        let block_start = lines.len();
        let block_images = extract_block_images(block);
        match block {
            RenderedBlock::Table { headers, alignments, rows } => {
                // Tables need per-row link tracking for correct highlighting.
                let (table_lines, table_link_offsets) =
                    flatten_table(headers, alignments, rows, width, theme);
                for (offset, url) in table_link_offsets {
                    links.push(LinkEntry {
                        line_index: block_start + offset,
                        url,
                    });
                }
                lines.extend(table_lines);
            }
            RenderedBlock::List { .. } => {
                // Lists need per-item link tracking for correct highlighting.
                let (list_lines, list_link_offsets, list_inline_metas) =
                    flatten_block_with_links(block, width, 0, theme);
                for (offset, url) in list_link_offsets {
                    links.push(LinkEntry {
                        line_index: block_start + offset,
                        url,
                    });
                }
                // Collect inline image metadata with absolute line indices.
                for (rel_line, metas) in list_inline_metas.iter().enumerate() {
                    for meta in metas {
                        inline_images.push(InlineImageEntry {
                            line_index: block_start + rel_line,
                            protocol_index: meta.protocol_index,
                            col_offset: meta.col_offset,
                            width: meta.width,
                        });
                    }
                }
                lines.extend(list_lines);
            }
            _ => {
                let block_links = extract_block_links(block);
                let (block_lines, _link_offsets, block_inline_metas) =
                    flatten_block_with_links(block, width, 0, theme);
                // Collect inline image metadata with absolute line indices.
                for (rel_line, metas) in block_inline_metas.iter().enumerate() {
                    for meta in metas {
                        inline_images.push(InlineImageEntry {
                            line_index: block_start + rel_line,
                            protocol_index: meta.protocol_index,
                            col_offset: meta.col_offset,
                            width: meta.width,
                        });
                    }
                }
                lines.extend(block_lines);
                for url in block_links {
                    links.push(LinkEntry {
                        line_index: block_start,
                        url,
                    });
                }
            }
        }
        for url in block_images {
            images.push(ImageEntry {
                line_index: block_start,
                url,
            });
        }
    }

    let total_height = lines.len();
    PreRenderedDocument { lines, total_height, headings, links, images, inline_images }
}

/// Flattens a single `RenderedBlock` into `Vec<DocumentLine>`.
///
/// `list_depth` is the current nesting depth of the enclosing list (0 = top-level).
/// Used by `flatten_list` when recursively flattening child blocks.
/// Discards inline image metadata (not needed inside list items for top-level collection).
fn flatten_single_block(block: &RenderedBlock, width: usize, list_depth: usize, theme: &MarkdownTheme) -> Vec<DocumentLine> {
    flatten_block_with_links(block, width, list_depth, theme).0
}

/// Flattens a block into lines, per-row link offsets, and inline image metadata.
///
/// Returns `(lines, link_offsets, inline_image_metas)` where link_offsets are
/// relative to the start of the returned lines. Only List and Table blocks produce
/// non-empty link_offsets; other blocks return empty. Inline image metadata is
/// propagated from text-containing blocks.
#[allow(clippy::type_complexity)]
fn flatten_block_with_links(block: &RenderedBlock, width: usize, list_depth: usize, theme: &MarkdownTheme) -> (Vec<DocumentLine>, Vec<(usize, String)>, Vec<Vec<InlineImageMeta>>) {
    match block {
        RenderedBlock::List { ordered, start, items } => {
            let (lines, link_offsets, inline_metas) = flatten_list(items, *ordered, *start, width, list_depth, theme);
            (lines, link_offsets, inline_metas)
        }
        RenderedBlock::Table { headers, alignments, rows } => {
            let (lines, link_offsets) = flatten_table(headers, alignments, rows, width, theme);
            (lines, link_offsets, Vec::new())
        }
        other => {
            let (lines, metas) = flatten_plain_block(other, width, list_depth, theme);
            (lines, Vec::new(), metas)
        }
    }
}

/// Flattens a non-List, non-Table block into lines (no per-row link tracking).
fn flatten_plain_block(block: &RenderedBlock, width: usize, list_depth: usize, theme: &MarkdownTheme) -> (Vec<DocumentLine>, Vec<Vec<InlineImageMeta>>) {
    match block {
        RenderedBlock::Heading { content, .. } => {
            let (wrapped, metas) = wrap_styled_spans(content, width);
            if wrapped.is_empty() {
                (vec![DocumentLine::Empty], metas)
            } else {
                (wrapped.into_iter().map(DocumentLine::Text).collect(), metas)
            }
        }
        RenderedBlock::Paragraph { content } => {
            let (wrapped, metas) = wrap_styled_spans(content, width);
            if wrapped.is_empty() {
                (vec![DocumentLine::Empty], metas)
            } else {
                (wrapped.into_iter().map(DocumentLine::Text).collect(), metas)
            }
        }
        RenderedBlock::CodeBlock { language, highlighted_lines } => {
            let mut result = Vec::new();
            // Emit language label header if language is specified.
            if !language.is_empty() {
                let label = Span::styled(
                    format!(" {} ", language_display_name(language)),
                    theme::code_label_style(&theme.code_block),
                );
                result.push(DocumentLine::Code(Line::from(label)));
            }
            // Emit each highlighted line (no wrapping — code is literal).
            for line in highlighted_lines {
                result.push(DocumentLine::Code(line.clone()));
            }
            (result, Vec::new())
        }
        RenderedBlock::ThematicBreak => (vec![DocumentLine::Rule], Vec::new()),
        RenderedBlock::Spacer { lines: count } => {
            ((0..*count).map(|_| DocumentLine::Empty).collect(), Vec::new())
        }
        RenderedBlock::BlockQuote { children } => {
            let (lines, metas) = flatten_block_quote_with_metas(children, width, list_depth, theme);
            (lines, metas)
        }
        RenderedBlock::AsciiImage { lines, src_url: _, .. } => {
            (lines.iter().map(|l| DocumentLine::AsciiArt(l.clone())).collect(), Vec::new())
        }
        // Phase 4: images are rendered by the renderer via StatefulImage.
        // Layout emits ImageStart for the first row (renderer draws there) and
        // ImageContinuation for remaining rows (reserve scroll space).
        RenderedBlock::Image { protocol_index, height_cells, alt_text, src_url: _, .. } => {
            let height = *height_cells;
            if height == 0 {
                // Degenerate case: emit a fallback text line so the image
                // is not completely invisible.
                return (vec![DocumentLine::Text(Line::from(Span::raw(
                    format!("[image: {alt_text}]"),
                )))], Vec::new());
            }
            let mut lines = Vec::with_capacity(height as usize);
            lines.push(DocumentLine::ImageStart { protocol_index: *protocol_index, height });
            for _ in 1..height {
                lines.push(DocumentLine::ImageContinuation);
            }
            (lines, Vec::new())
        }
        RenderedBlock::ImageFallback { alt_text, src_url } => {
            let label = if alt_text.is_empty() {
                format!("[image: {}]", src_url)
            } else {
                format!("[image: {} ({})]", alt_text, src_url)
            };
            let (wrapped, _metas) = wrap_styled_spans(
                &[crate::parser::StyledSpan {
                    text: label,
                    style: theme::inline_style(&theme.image_alt),
                    url: None,
                    math_latex: String::new(),
                    math_image: None,
                }],
                width,
            );
            if wrapped.is_empty() {
                (vec![DocumentLine::Empty], Vec::new())
            } else {
                (wrapped.into_iter().map(DocumentLine::Text).collect(), Vec::new())
            }
        }
        RenderedBlock::ImagePending { alt_text, .. } => {
            // Show dim placeholder while remote image loads.
            let (wrapped, _metas) = wrap_styled_spans(
                &[crate::parser::StyledSpan {
                    text: format!("[loading: {}]", if alt_text.is_empty() { "image" } else { alt_text }),
                    style: Style::default().add_modifier(Modifier::DIM),
                    url: None,
                    math_latex: String::new(),
                    math_image: None,
                }],
                width,
            );
            if wrapped.is_empty() {
                (vec![DocumentLine::Empty], Vec::new())
            } else {
                (wrapped.into_iter().map(DocumentLine::Text).collect(), Vec::new())
            }
        }
        // List and Table are handled by flatten_block_with_links, but we need
        // these arms for exhaustiveness. They should not be reached directly.
        RenderedBlock::List { ordered, start, items } => {
            let (lines, _link_offsets, metas) = flatten_list(items, *ordered, *start, width, list_depth, theme);
            (lines, metas)
        }
        RenderedBlock::Table { headers, alignments, rows } => {
            let (lines, _link_offsets) = flatten_table(headers, alignments, rows, width, theme);
            (lines, Vec::new())
        }
        // Display math: Unicode text (wraps like a Paragraph).
        RenderedBlock::MathUnicode { content, raw_latex: _ } => {
            let (wrapped, metas) = wrap_styled_spans(content, width);
            if wrapped.is_empty() {
                (vec![DocumentLine::Empty], metas)
            } else {
                (wrapped.into_iter().map(DocumentLine::Text).collect(), metas)
            }
        }
        // Display math: pixel image (same layout as Image).
        RenderedBlock::MathImage { protocol_index, height_cells, raw_latex: _, .. } => {
            let height = *height_cells;
            if height == 0 {
                return (vec![DocumentLine::Empty], Vec::new());
            }
            let mut lines = Vec::with_capacity(height as usize);
            lines.push(DocumentLine::ImageStart { protocol_index: *protocol_index, height });
            for _ in 1..height {
                lines.push(DocumentLine::ImageContinuation);
            }
            (lines, Vec::new())
        }
    }
}

// ── List layout ──────────────────────────────────────────────────────────────

/// Flattens a list into `DocumentLine`s with bullet/number prefixes and indentation.
///
/// Returns `(lines, link_offsets)` where each `link_offsets` entry is
/// `(row_offset_within_lines, url)` — the offset is relative to the start
/// of the returned lines vector, so the caller adjusts by `block_start`.
///
/// Bullet characters and task markers are configured by the theme.
/// Ordered lists use `n.` where `n = start + index`.
#[allow(clippy::type_complexity)]
fn flatten_list(
    items: &[ListItem],
    ordered: bool,
    start: u64,
    width: usize,
    depth: usize,
    theme: &MarkdownTheme,
) -> (Vec<DocumentLine>, Vec<(usize, String)>, Vec<Vec<InlineImageMeta>>) {
    let indent = " ".repeat(theme.list.indent_size as usize * depth);
    let mut lines = Vec::new();
    let mut link_offsets: Vec<(usize, String)> = Vec::new();
    let mut all_inline_metas: Vec<Vec<InlineImageMeta>> = Vec::new();

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
        let cont_prefix_width = first_prefix_width;

        let content_width = width.saturating_sub(first_prefix_width).max(1);
        let (wrapped, inline_metas) = wrap_styled_spans(&item.content, content_width);

        // Build the first-line prefix as styled spans: indent (raw) + marker (styled) + space (raw).
        let prefix_spans = vec![
            Span::raw(indent.clone()),
            Span::styled(prefix_str, marker_style),
            Span::raw(" ".to_string()),
        ];

        // Collect links from this item's inline content at the current line offset.
        let item_line_start = lines.len();
        for url in item.content.iter().filter_map(|s| s.url.clone()) {
            link_offsets.push((item_line_start, url));
        }

        if wrapped.is_empty() {
            // Empty item — emit just the prefix.
            lines.push(DocumentLine::Text(Line::from(prefix_spans)));
            all_inline_metas.push(Vec::new());
        } else {
            for (j, content_line) in wrapped.into_iter().enumerate() {
                let prefix_width = if j == 0 { first_prefix_width } else { cont_prefix_width };
                let mut spans = if j == 0 {
                    prefix_spans.clone()
                } else {
                    vec![Span::raw(cont_prefix.clone())]
                };
                spans.extend(content_line.spans);
                lines.push(DocumentLine::Text(Line::from(spans)));

                // Shift inline image col_offsets by the prefix width.
                let line_metas = inline_metas.get(j).cloned().unwrap_or_default();
                let shifted = line_metas.into_iter().map(|m| InlineImageMeta {
                    col_offset: m.col_offset + prefix_width as u16,
                    ..m
                }).collect();
                all_inline_metas.push(shifted);
            }
        }

        // Flatten child blocks. Nested lists handle their own indentation via
        // depth+1. Non-list children (code blocks, blockquotes, paragraphs)
        // need an explicit indent prefix to align under the item's content.
        for child in &item.children {
            match child {
                RenderedBlock::List { .. } => {
                    let (child_lines, child_link_offsets, child_inline_metas) =
                        flatten_block_with_links(child, width, depth + 1, theme);
                    for (offset, url) in child_link_offsets {
                        link_offsets.push((lines.len() + offset, url));
                    }
                    // Shift child inline image col_offsets by the continuation prefix width.
                    for metas in child_inline_metas {
                        let shifted = metas.into_iter().map(|m| InlineImageMeta {
                            col_offset: m.col_offset + cont_prefix_width as u16,
                            ..m
                        }).collect();
                        all_inline_metas.push(shifted);
                    }
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
                                all_inline_metas.push(Vec::new());
                            }
                            DocumentLine::Code(l) => {
                                let mut spans = vec![Span::raw(cont_prefix.clone())];
                                spans.extend(l.spans);
                                lines.push(DocumentLine::Code(Line::from(spans)));
                                all_inline_metas.push(Vec::new());
                            }
                            DocumentLine::AsciiArt(l) => {
                                let mut spans = vec![Span::raw(cont_prefix.clone())];
                                spans.extend(l.spans);
                                lines.push(DocumentLine::AsciiArt(Line::from(spans)));
                                all_inline_metas.push(Vec::new());
                            }
                            other => {
                                lines.push(other);
                                all_inline_metas.push(Vec::new());
                            }
                        }
                    }
                }
            }
        }
    }

    (lines, link_offsets, all_inline_metas)
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

/// Wrapper around `flatten_block_quote` that returns the tuple format
/// expected by `flatten_plain_block`. Inline image metadata is not
/// propagated from block quotes (prefix offsets would be wrong).
fn flatten_block_quote_with_metas(children: &[RenderedBlock], width: usize, list_depth: usize, theme: &MarkdownTheme) -> (Vec<DocumentLine>, Vec<Vec<InlineImageMeta>>) {
    (flatten_block_quote(children, width, list_depth, theme), Vec::new())
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
) -> (Vec<DocumentLine>, Vec<(usize, String)>) {
    let ncols = headers.len();
    if ncols == 0 {
        return (Vec::new(), Vec::new());
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
    let mut link_offsets: Vec<(usize, String)> = Vec::new();

    // Header row (may be multi-line if headers contain images).
    let header_cell_lines: Vec<Vec<Vec<Span<'static>>>> = headers
        .iter()
        .enumerate()
        .map(|(col, cell)| {
            let align = alignments.get(col).copied().unwrap_or(Alignment::None);
            flatten_cell_to_lines(cell, col_widths[col], align, Some(header_style))
        })
        .collect();
    // Collect links from header cells at the header row offset.
    for cell in headers {
        for url in extract_table_cell_links(cell) {
            link_offsets.push((result.len(), url));
        }
    }
    result.extend(build_multi_line_row(&header_cell_lines, &col_widths));

    // Separator row.
    result.push(DocumentLine::Text(Line::from(build_table_separator(
        &col_widths,
        sep_style,
    ))));

    // Body rows. Insert a blank separator between rows when any row wraps.
    let mut prev_row_was_multi = false;
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
        let row_height = cell_lines.iter().map(|cl| cl.len()).max().unwrap_or(1);
        // Add blank separator between multi-line rows for readability.
        if prev_row_was_multi || (row_height > 1 && !result.is_empty()) {
            result.push(build_blank_table_row(&col_widths));
        }
        // Collect links from body cells at the current row offset.
        for cell in row {
            for url in extract_table_cell_links(cell) {
                link_offsets.push((result.len(), url));
            }
        }
        result.extend(build_multi_line_row(&cell_lines, &col_widths));
        prev_row_was_multi = row_height > 1;
    }

    (result, link_offsets)
}

/// Returns the display width of a single table cell for column-width calculation.
fn cell_display_width(cell: &TableCell) -> usize {
    match cell {
        TableCell::Text(spans) => spans.iter().map(|s| s.text.width()).sum(),
        TableCell::Block(RenderedBlock::AsciiImage { lines, .. }) => lines
            .first()
            .map(|l| l.spans.iter().map(|s| s.content.width()).sum())
            .unwrap_or(0),
        TableCell::Block(RenderedBlock::ImageFallback { alt_text, src_url }) => {
            if alt_text.is_empty() {
                format!("[image: {}]", src_url).width()
            } else {
                format!("[image: {} ({})]", alt_text, src_url).width()
            }
        }
        TableCell::Block(_) => 5, // "[...]"
    }
}

/// Converts one table cell into lines of spans (outer vec = lines, inner = spans per line).
///
/// Text cells are word-wrapped to `col_width` and may produce multiple lines.
/// AsciiImage cells produce N lines (one per image row), each truncated to `col_width`.
fn flatten_cell_to_lines(
    cell: &TableCell,
    col_width: usize,
    alignment: Alignment,
    header_style: Option<Style>,
) -> Vec<Vec<Span<'static>>> {
    let lines = match cell {
        TableCell::Text(spans) => {
            let (wrapped, _metas) = wrap_styled_spans(spans, col_width);
            wrapped
                .into_iter()
                .map(|line| {
                    let line_width: usize =
                        line.spans.iter().map(|s| s.content.width()).sum();
                    let mut out_spans: Vec<Span<'static>> = line
                        .spans
                        .into_iter()
                        .map(|s| {
                            if let Some(style) = header_style {
                                Span::styled(s.content.into_owned(), style)
                            } else {
                                Span::styled(s.content.into_owned(), s.style)
                            }
                        })
                        .collect();
                    if line_width < col_width {
                        let padding = col_width - line_width;
                        match alignment {
                            Alignment::Right => {
                                out_spans.insert(0, Span::raw(" ".repeat(padding)));
                            }
                            Alignment::Center => {
                                let left = padding / 2;
                                let right = padding - left;
                                out_spans.insert(0, Span::raw(" ".repeat(left)));
                                out_spans.push(Span::raw(" ".repeat(right)));
                            }
                            _ => {
                                out_spans.push(Span::raw(" ".repeat(padding)));
                            }
                        }
                    }
                    out_spans
                })
                .collect()
        }
        TableCell::Block(RenderedBlock::AsciiImage { lines, .. }) => {
            lines
                .iter()
                .map(|line| truncate_spans_to_width(&line.spans, col_width))
                .collect()
        }
        TableCell::Block(RenderedBlock::ImageFallback { alt_text, src_url }) => {
            let text = if alt_text.is_empty() {
                format!("[image: {}]", src_url)
            } else {
                format!("[image: {} ({})]", alt_text, src_url)
            };
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

/// Builds a blank row with column separators (spaces + `│`) for visual spacing.
fn build_blank_table_row(col_widths: &[usize]) -> DocumentLine {
    let mut spans = Vec::new();
    for (i, &w) in col_widths.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" │ ".to_string()));
        }
        spans.push(Span::raw(" ".repeat(w)));
    }
    DocumentLine::Text(Line::from(spans))
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

/// Per-line inline image metadata collected during word wrapping.
#[derive(Clone)]
struct InlineImageMeta {
    /// Index into `ImageManager::protocols` for renderer access.
    protocol_index: usize,
    /// Column offset within the line where the image starts.
    col_offset: u16,
    /// Width of the image in terminal columns.
    width: u16,
}

/// Wraps styled spans to fit within a given width, preserving styles.
///
/// Algorithm:
/// 1. Concatenate all span text into a single plain-text string, building
///    parallel byte-to-style and byte-to-math-image maps.
/// 2. Use `textwrap::wrap()` to determine line break positions.
/// 3. Walk a cursor through the plain text for each wrapped line, skipping
///    whitespace break points, then extract styled spans by consulting
///    the byte-to-style map.
///
/// Returns `(wrapped_lines, per_line_inline_image_metadata)`.
fn wrap_styled_spans(spans: &[StyledSpan], width: usize) -> (Vec<Line<'static>>, Vec<Vec<InlineImageMeta>>) {
    if spans.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // Handle hard breaks (\n) by splitting into sub-paragraphs.
    if spans.iter().any(|s| s.text.contains('\n')) {
        return wrap_with_hard_breaks(spans, width);
    }

    // 1. Build plain text and parallel byte-to-style/url/math_image maps.
    let mut plain = String::new();
    let mut byte_styles: Vec<Style> = Vec::new();
    let mut byte_urls: Vec<Option<&str>> = Vec::new();
    // Track which byte offsets belong to an inline math image span.
    // Each entry is (protocol_index, width_cells) for the containing span.
    let mut byte_math_image: Vec<Option<(usize, u16)>> = Vec::new();
    for span in spans {
        let url_ref = span.url.as_deref();
        let math_meta = span.math_image.as_ref().map(|m| (m.protocol_index, m.width_cells));
        for _ in span.text.bytes() {
            byte_styles.push(span.style);
            byte_urls.push(url_ref);
            byte_math_image.push(math_meta);
        }
        plain.push_str(&span.text);
    }

    if plain.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // 2. Wrap the plain text.
    let wrap_options = textwrap::Options::new(width)
        .word_separator(textwrap::WordSeparator::UnicodeBreakProperties);
    let wrapped_lines = textwrap::wrap(&plain, &wrap_options);

    // 3. Map each wrapped line back to styled spans using a monotonic cursor.
    let mut result = Vec::with_capacity(wrapped_lines.len());
    let mut all_metas = Vec::with_capacity(wrapped_lines.len());
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
            all_metas.push(Vec::new());
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
            all_metas.push(Vec::new());
            cursor = line_end.min(plain.len());
            continue;
        }

        let line_spans = build_spans_for_range(&plain, &byte_styles, &byte_urls, line_start, line_end);
        result.push(Line::from(line_spans));

        // Collect inline math image metadata for this line.
        // Walk the byte range and compute column offsets.
        let line_metas = collect_inline_image_metas(
            &plain, &byte_math_image, line_start, line_end,
        );
        all_metas.push(line_metas);

        cursor = line_end;
    }

    (result, all_metas)
}

/// Collects inline math image metadata for a byte range of the plain text.
///
/// Walks through the range by characters, tracking the running display width
/// to compute `col_offset` for each inline math image span encountered.
fn collect_inline_image_metas(
    plain: &str,
    byte_math_image: &[Option<(usize, u16)>],
    line_start: usize,
    line_end: usize,
) -> Vec<InlineImageMeta> {
    let mut metas = Vec::new();
    let mut col: u16 = 0;
    let mut i = line_start;

    while i < line_end {
        // Check if this byte starts (or is inside) a math image span.
        if let Some((protocol_index, width_cells)) = byte_math_image.get(i).and_then(|m| *m) {
            // Advance to end of this math image span.
            while i < line_end && byte_math_image.get(i).and_then(|m| *m) == Some((protocol_index, width_cells)) {
                // Safe: loop invariant guarantees i < line_end <= plain.len().
                let ch = plain[i..].chars().next().unwrap();
                col += ch.width().unwrap_or(0) as u16;
                i += ch.len_utf8();
            }
            // Record the metadata with col_offset at the start of the span.
            // col_offset = col minus the width we just counted (it was the image width).
            let img_col = col - width_cells;
            metas.push(InlineImageMeta {
                protocol_index,
                col_offset: img_col,
                width: width_cells,
            });
        } else {
            // Regular character — advance by one character.
            // Safe: loop invariant guarantees i < line_end <= plain.len().
            let ch = plain[i..].chars().next().unwrap();
            col += ch.width().unwrap_or(0) as u16;
            i += ch.len_utf8();
        }
    }

    metas
}

/// Builds styled `Span`s for a byte range of the plain text.
///
/// Walks through the range by characters, grouping consecutive bytes
/// that share the same style and URL into a single `Span`. When a URL
/// is present, the span text is wrapped in OSC 8 hyperlink escape
/// sequences so terminals that support OSC 8 render clickable links.
/// All slicing happens at character boundaries.
fn build_spans_for_range(
    plain: &str,
    byte_styles: &[Style],
    byte_urls: &[Option<&str>],
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
    let mut run_url = byte_urls[start];

    for (i, _ch) in segment.char_indices() {
        let abs_pos = start + i;
        if byte_styles[abs_pos] != run_style || byte_urls[abs_pos] != run_url {
            let text = &plain[run_start..abs_pos];
            if !text.is_empty() {
                spans.push(make_span(text, run_style, run_url));
            }
            run_start = abs_pos;
            run_style = byte_styles[abs_pos];
            run_url = byte_urls[abs_pos];
        }
    }

    // Emit final run.
    let text = &plain[run_start..end];
    if !text.is_empty() {
        spans.push(make_span(text, run_style, run_url));
    }

    spans
}

/// Creates a `Span` from text, style, and optional URL.
///
/// When a URL is present, the text is wrapped in OSC 8 hyperlink escape
/// sequences: `ESC ] 8 ; ; url ST text ESC ] 8 ; ; ST`. The URL is
/// sanitized to strip control characters that could inject terminal escapes.
fn make_span(text: &str, style: Style, _url: Option<&str>) -> Span<'static> {
    // OSC 8 hyperlink escapes are disabled: ratatui's buffer treats embedded
    // escape bytes as literal text, causing raw `]8;;` to appear on screen.
    // Links are still styled (italic) but not clickable until ratatui adds
    // native OSC 8 support.  See https://github.com/ratatui/ratatui/issues/1028
    Span::styled(text.to_string(), style)
}

/// Strips control characters from a URL to prevent terminal escape injection.
///
/// Removes ASCII control characters (0x00-0x1f, 0x7f) so that a malicious
/// URL cannot inject arbitrary terminal escape sequences via OSC 8.
/// Currently unused (OSC 8 disabled), retained for when ratatui adds native support.
#[allow(dead_code)]
fn sanitize_url(url: &str) -> String {
    url.chars()
        .filter(|c| !c.is_ascii_control())
        .collect()
}

/// Maps a markdown fence language tag to a display-friendly name.
/// SQL dialect tags like "sql.postgresql" become "SQL (PostgreSQL)".
/// All registered aliases are covered. Unknown tags pass through unchanged.
fn language_display_name(language: &str) -> &str {
    match language {
        "sql.mysql" | "sql-mysql" | "mysql" => "SQL (MySQL)",
        "sql.postgresql" | "sql.postgres" | "sql-postgresql" | "postgresql" | "pgsql" => {
            "SQL (PostgreSQL)"
        }
        "sql.oracle" | "sql-oracle" | "oracle" | "plsql" | "sql.plsql" => "SQL (Oracle)",
        "sql.mssql" | "sql-mssql" | "sql.tsql" | "sql-tsql" | "sql.sqlserver" | "tsql"
        | "t-sql" | "mssql" => "SQL (T-SQL)",
        other => other,
    }
}

/// Handles text containing hard breaks by splitting at `\n` boundaries
/// first, then wrapping each segment independently.
fn wrap_with_hard_breaks(spans: &[StyledSpan], width: usize) -> (Vec<Line<'static>>, Vec<Vec<InlineImageMeta>>) {
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
                        url: span.url.clone(),
                        math_latex: String::new(),
                        math_image: None,
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
                url: span.url.clone(),
                math_latex: String::new(),
                math_image: None,
            });
        }
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    let mut result = Vec::new();
    let mut all_metas = Vec::new();
    for group in &groups {
        let (wrapped, metas) = wrap_styled_spans(group, width);
        if wrapped.is_empty() {
            result.push(Line::from(Vec::<Span<'static>>::new()));
            all_metas.push(Vec::new());
        } else {
            result.extend(wrapped);
            all_metas.extend(metas);
        }
    }

    (result, all_metas)
}

#[cfg(test)]
mod tests;
