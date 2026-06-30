//! Markdown parser: converts pulldown-cmark events into the RenderedBlock IR.
//!
//! This module is the first stage of the rendering pipeline. It consumes
//! a markdown source string and produces a `Vec<RenderedBlock>` — the
//! intermediate representation consumed by the layout engine.

use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};
use ratatui::style::Style;
use ratatui::text::Line;
use std::borrow::Cow;

use crate::theme::{self, MarkdownTheme};

/// A rendered markdown block ready for layout.
///
/// Each variant corresponds to a markdown block-level element.
/// Inline styling is carried via `Vec<StyledSpan>` in content fields.
// `level` and `Spacer` are forward-declared for Phase 5 theming; allow until then.
#[allow(dead_code)]
pub enum RenderedBlock {
    /// Heading with level (1–6). Content carries inline styles.
    Heading { level: u8, content: Vec<StyledSpan> },
    /// A paragraph of text with inline formatting.
    Paragraph { content: Vec<StyledSpan> },
    /// A fenced or indented code block with syntax highlighting.
    CodeBlock {
        /// Language from the fence info string (empty for indented/unfenced).
        language: String,
        /// Pre-highlighted lines ready for layout.
        highlighted_lines: Vec<Line<'static>>,
    },
    /// A horizontal rule / thematic break.
    ThematicBreak,
    /// Vertical spacing between blocks.
    Spacer { lines: u16 },
    /// An ordered or unordered list.
    List {
        /// `true` for ordered lists, `false` for bullet lists.
        ordered: bool,
        /// Starting number for ordered lists (usually 1).
        start: u64,
        /// The list items in order.
        items: Vec<ListItem>,
    },
    /// A block quote containing nested blocks.
    BlockQuote { children: Vec<RenderedBlock> },
    /// A GFM table with headers, column alignments, and body rows.
    Table {
        /// Header cells (one `TableCell` per column).
        headers: Vec<TableCell>,
        /// Per-column alignment from the separator row.
        alignments: Vec<Alignment>,
        /// Body rows (one `Vec<TableCell>` per row).
        rows: Vec<Vec<TableCell>>,
    },
    /// A successfully loaded image with a terminal graphics protocol.
    Image {
        /// Index into `ImageManager::protocols` for renderer access.
        protocol_index: usize,
        /// Alt text for accessibility / fallback display.
        alt_text: String,
        /// Image width in terminal cell columns.
        width_cells: u16,
        /// Image height in terminal cell rows.
        height_cells: u16,
    },
    /// A colored ASCII art rendering of an image (used when no graphics protocol is available).
    AsciiImage {
        /// Pre-rendered colored lines (one Line per row of ASCII art).
        lines: Vec<Line<'static>>,
        /// Alt text for accessibility.
        alt_text: String,
    },
    /// An image that could not be loaded (missing file, no graphics support, etc.).
    ImageFallback {
        /// Alt text to display in place of the image.
        alt_text: String,
    },
}

/// A single item in an ordered or unordered list.
pub struct ListItem {
    /// Inline content for the first paragraph of this item.
    pub content: Vec<StyledSpan>,
    /// Nested blocks (sub-lists, code blocks, etc.).
    pub children: Vec<RenderedBlock>,
    /// Task list state: `None` = not a task, `Some(true)` = checked, `Some(false)` = unchecked.
    pub task: Option<bool>,
}

/// A text span with associated style information.
///
/// Multiple `StyledSpan`s compose a line of styled text. Each span
/// carries a contiguous run of text sharing the same `ratatui::Style`.
pub struct StyledSpan {
    /// The text content of this span.
    pub text: String,
    /// The ratatui style to apply when rendering.
    pub style: Style,
    /// Optional hyperlink URL for OSC 8 terminal links.
    pub url: Option<String>,
}

/// A single cell in a GFM table.
///
/// Most cells contain inline text, but image cells carry a full `RenderedBlock`
/// (always `AsciiImage` or `ImageFallback` — native `Image` blocks are forced
/// to ASCII art inside tables because `StatefulProtocol` cannot render in
/// sub-regions of table rows).
pub enum TableCell {
    /// Inline text content (the common case).
    Text(Vec<StyledSpan>),
    /// A block-level element occupying the entire cell (e.g. ASCII art image).
    Block(RenderedBlock),
}

/// Parser state machine states.
///
/// Tracks what block-level element we are currently inside. Events are
/// interpreted differently depending on the active state.
enum ParserState {
    /// Not inside any block — waiting for the next block-level start event.
    TopLevel,
    /// Inside a heading block; `level` is 1–6.
    InHeading { level: u8 },
    /// Inside a paragraph block.
    InParagraph,
    /// Inside a paragraph that is itself inside a list item.
    /// Text accumulates into `current_spans` without emitting a Paragraph block.
    InListItemParagraph,
    /// Inside a fenced or indented code block; accumulating text.
    InCodeBlock { language: String, buffer: String },
    /// Inside an unrecognized block that we skip in this phase.
    /// We count nesting depth so we know when the matching End arrives.
    Skipping { depth: u32 },
    /// Inside a list; accumulating `ListItem`s.
    InList { ordered: bool, start: u64, items: Vec<ListItem> },
    /// Inside a single list item; accumulating content and child blocks.
    InListItem { children: Vec<RenderedBlock>, task: Option<bool> },
    /// Inside a block quote; accumulating child blocks.
    InBlockQuote { children: Vec<RenderedBlock> },
    /// Inside an image tag; accumulating alt text. Image is loaded on End(Image).
    InImage { dest_url: String, alt_buffer: String },
    /// Inside a GFM table; accumulating header/body cells row by row.
    InTable {
        headers: Vec<TableCell>,
        alignments: Vec<Alignment>,
        rows: Vec<Vec<TableCell>>,
        current_row: Vec<TableCell>,
        in_head: bool,
        /// Staging area for a block emitted during cell parsing (e.g. an image).
        cell_block: Option<RenderedBlock>,
    },
}

/// Computes the effective style by merging the current base style with
/// all active inline modifiers from the style stack.
fn effective_style(style_stack: &[Style]) -> Style {
    style_stack
        .iter()
        .fold(Style::default(), |acc, s| acc.patch(*s))
}

/// Converts a pulldown-cmark `HeadingLevel` to a `u8` (1–6).
fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

// ── ParseContext ─────────────────────────────────────────────────────────────

/// Accumulates all mutable parser state across a single `parse()` call.
///
/// Exists solely as an implementation detail of `parse()` — it is created
/// in `ParseContext::new`, driven by `process()`, and consumed to return
/// the final `Vec<RenderedBlock>`. Not part of the public API.
struct ParseContext<'a> {
    highlighter: &'a crate::highlight::Highlighter,
    images: &'a mut crate::images::ImageManager,
    theme: &'a MarkdownTheme,
    blocks: Vec<RenderedBlock>,
    /// Block-level state machine (never empty while parsing).
    state_stack: Vec<ParserState>,
    /// Inline formatting modifier stack (push on Start, pop on End).
    style_stack: Vec<Style>,
    /// Spans accumulated for the block or cell currently being built.
    current_spans: Vec<StyledSpan>,
    /// Saved list-item spans while a nested paragraph (inside a block quote
    /// that is itself inside a list item) temporarily uses `current_spans`.
    /// Restored when that nested paragraph ends. Single-level stash is
    /// sufficient because pulldown-cmark produces at most one such context
    /// level per list item before the next End(Item) event arrives.
    span_stash: Vec<StyledSpan>,
    /// The URL of the link currently being parsed, if any.
    /// Set on `Start(Tag::Link)`, cleared on `End(TagEnd::Link)`.
    current_link_url: Option<String>,
}

impl<'a> ParseContext<'a> {
    fn new(
        highlighter: &'a crate::highlight::Highlighter,
        images: &'a mut crate::images::ImageManager,
        theme: &'a MarkdownTheme,
    ) -> Self {
        Self {
            highlighter,
            images,
            theme,
            blocks: Vec::new(),
            state_stack: vec![ParserState::TopLevel],
            style_stack: Vec::new(),
            current_spans: Vec::new(),
            span_stash: Vec::new(),
            current_link_url: None,
        }
    }

    /// Drives the pulldown-cmark event stream and returns the finished blocks.
    fn process(mut self, source: &str) -> Vec<RenderedBlock> {
        let options = Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_MATH;

        for event in Parser::new_ext(source, options) {
            if self.state_stack.is_empty() {
                // State stack underflow — parser invariant violated. Stop here
                // rather than panic so partially-parsed output is still returned.
                debug_assert!(false, "parser state stack underflow");
                break;
            }
            self.on_event(event);
        }

        self.blocks
    }

    // ── Event routing ────────────────────────────────────────────────────────

    /// Routes each event to the appropriate handler based on current state.
    fn on_event(&mut self, event: Event) {
        if matches!(self.state_stack.last(), Some(ParserState::InCodeBlock { .. })) {
            self.on_code_block_event(event);
        } else if matches!(self.state_stack.last(), Some(ParserState::InImage { .. })) {
            self.on_image_event(event);
        } else if matches!(self.state_stack.last(), Some(ParserState::Skipping { .. })) {
            self.on_skipping_event(event);
        } else if matches!(self.state_stack.last(), Some(ParserState::InTable { .. })) {
            self.on_table_event(event);
        } else {
            self.dispatch(event);
        }
    }

    /// Handles events when inside a fenced/indented code block.
    ///
    /// Accumulates text into the buffer; on `End(CodeBlock)` runs syntax
    /// highlighting and emits the finished `CodeBlock` block.
    fn on_code_block_event(&mut self, event: Event) {
        match event {
            Event::Text(text) => {
                if let Some(ParserState::InCodeBlock { buffer, .. }) =
                    self.state_stack.last_mut()
                {
                    buffer.push_str(&sanitize_text(&text));
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(ParserState::InCodeBlock { language, buffer }) =
                    self.state_stack.pop()
                {
                    let highlighted_lines = if language == "mermaid" {
                        // Mermaid diagrams have no syntect grammar — render as
                        // plain text lines so the source is readable without
                        // hitting syntect's unknown-language fallback.
                        buffer
                            .lines()
                            .map(|line| Line::from(line.to_string()))
                            .collect()
                    } else {
                        self.highlighter.highlight_code(
                            &buffer,
                            &language,
                            &self.theme.syntect_theme,
                        )
                    };
                    self.emit_block(RenderedBlock::CodeBlock { language, highlighted_lines });
                }
            }
            // Ignore all other events (syntax, meta) inside a code block.
            _ => {}
        }
    }

    /// Handles events when inside an image tag.
    ///
    /// Accumulates alt text; on `End(Image)` attempts to load the image
    /// via `ImageManager`. Falls back to `ImageFallback` on any error.
    fn on_image_event(&mut self, event: Event) {
        match event {
            Event::Text(text) => {
                if let Some(ParserState::InImage { alt_buffer, .. }) =
                    self.state_stack.last_mut()
                {
                    alt_buffer.push_str(&sanitize_text(&text));
                }
            }
            Event::End(TagEnd::Image) => {
                if let Some(ParserState::InImage { dest_url, alt_buffer }) =
                    self.state_stack.pop()
                {
                    // Native Image blocks use StatefulProtocol which requires a
                    // dedicated Rect — they can't render inside table cell spans.
                    // Force ASCII art when we're inside a table context.
                    let in_table = self
                        .state_stack
                        .iter()
                        .any(|s| matches!(s, ParserState::InTable { .. }));

                    if self.images.images_disabled() {
                        // User explicitly disabled images via --no-images.
                        self.emit_block(RenderedBlock::ImageFallback {
                            alt_text: alt_buffer,
                        });
                    } else if in_table
                        || self.images.prefer_ascii()
                        || !self.images.has_graphics_support()
                    {
                        // Inside table, user forced ASCII art, or no graphics protocol.
                        self.emit_ascii_or_fallback(&dest_url, alt_buffer);
                    } else {
                        // Terminal supports a graphics protocol — try native rendering.
                        match self.images.load_image(&dest_url) {
                            Ok((protocol_index, width_cells, height_cells)) => {
                                self.emit_block(RenderedBlock::Image {
                                    protocol_index,
                                    alt_text: alt_buffer,
                                    width_cells,
                                    height_cells,
                                });
                            }
                            Err(e) => {
                                // Native failed — try ASCII art before giving up.
                                eprintln!("warning: {e}");
                                self.emit_ascii_or_fallback(&dest_url, alt_buffer);
                            }
                        }
                    }
                }
            }
            // Images don't contain block-level content — ignore everything else.
            _ => {}
        }
    }

    /// Handles events when inside an unrecognized block being skipped.
    ///
    /// Tracks nesting depth via `Skipping { depth }` so that nested
    /// unrecognized blocks don't prematurely end the skip.
    fn on_skipping_event(&mut self, event: Event) {
        // Copy depth out to release the shared borrow before mutation below.
        let depth = match self.state_stack.last() {
            Some(ParserState::Skipping { depth }) => *depth,
            _ => return,
        };
        match event {
            Event::Start(_) => {
                self.state_stack.pop();
                self.state_stack.push(ParserState::Skipping { depth: depth + 1 });
            }
            Event::End(_) if depth == 0 => {
                self.state_stack.pop();
            }
            Event::End(_) => {
                self.state_stack.pop();
                self.state_stack.push(ParserState::Skipping { depth: depth - 1 });
            }
            _ => {}
        }
    }

    /// Handles events when inside a GFM table.
    ///
    /// All inline content (text, code, emphasis) flows through `current_spans`.
    /// Structural events (TableHead, TableRow, TableCell) manage the accumulator state.
    fn on_table_event(&mut self, event: Event) {
        match event {
            Event::Start(Tag::TableHead) => {
                if let Some(ParserState::InTable { in_head, .. }) = self.state_stack.last_mut() {
                    *in_head = true;
                }
            }
            Event::End(TagEnd::TableHead) => {
                if let Some(ParserState::InTable { in_head, .. }) = self.state_stack.last_mut() {
                    *in_head = false;
                }
            }
            Event::Start(Tag::TableRow) => {}
            Event::End(TagEnd::TableRow) => {
                if let Some(ParserState::InTable { rows, current_row, in_head, .. }) =
                    self.state_stack.last_mut()
                {
                    if !*in_head {
                        let row = std::mem::take(current_row);
                        if !row.is_empty() {
                            rows.push(row);
                        }
                    }
                }
            }
            Event::Start(Tag::TableCell) => {
                self.current_spans.clear();
                // Clear any stale cell_block from a previous cell.
                if let Some(ParserState::InTable { cell_block, .. }) =
                    self.state_stack.last_mut()
                {
                    *cell_block = None;
                }
            }
            Event::End(TagEnd::TableCell) => {
                let text = std::mem::take(&mut self.current_spans);
                if let Some(ParserState::InTable {
                    headers,
                    current_row,
                    in_head,
                    cell_block,
                    ..
                }) = self.state_stack.last_mut()
                {
                    let cell = if let Some(block) = cell_block.take() {
                        // Image block takes precedence; text in same cell is dropped.
                        TableCell::Block(block)
                    } else {
                        TableCell::Text(text)
                    };
                    if *in_head {
                        headers.push(cell);
                    } else {
                        current_row.push(cell);
                    }
                }
            }
            Event::End(TagEnd::Table) => {
                match self.state_stack.pop() {
                    Some(ParserState::InTable { headers, alignments, rows, .. }) => {
                        self.emit_block(RenderedBlock::Table { headers, alignments, rows });
                    }
                    other => {
                        debug_assert!(false, "End(Table) without InTable state: {other:?}");
                    }
                }
            }
            // Inline content inside table cells
            Event::Text(text) => self.push_text(&text),
            Event::Code(text) => self.push_inline_code(&text),
            Event::SoftBreak | Event::HardBreak => {}
            Event::Start(Tag::Emphasis) => {
                self.push_style(theme::inline_style(&self.theme.emphasis));
            }
            Event::Start(Tag::Strong) => {
                self.push_style(theme::inline_style(&self.theme.strong));
            }
            Event::End(TagEnd::Emphasis | TagEnd::Strong) => self.pop_style(),
            Event::Start(Tag::Link { dest_url, .. }) => {
                self.current_link_url = Some(dest_url.to_string());
                self.push_style(theme::inline_style(&self.theme.link));
            }
            Event::End(TagEnd::Link) => {
                self.current_link_url = None;
                self.pop_style();
            }
            // Images inside table cells: push InImage state so subsequent events
            // (alt text, End(Image)) route through on_image_event. When the image
            // finishes, emit_block sees InTable on the stack and routes the block
            // to cell_block instead of the top-level blocks vec.
            Event::Start(Tag::Image { dest_url, .. }) => {
                self.state_stack.push(ParserState::InImage {
                    dest_url: dest_url.to_string(),
                    alt_buffer: String::new(),
                });
            }
            _ => {}
        }
    }

    /// Dispatches normal (non-code-block, non-skipping, non-table) events.
    fn dispatch(&mut self, event: Event) {
        match event {
            // ── Block-level start ────────────────────────────────────
            Event::Start(Tag::Heading { level, .. }) => self.start_heading(level),
            Event::Start(Tag::Paragraph) => self.start_paragraph(),
            Event::Start(Tag::CodeBlock(kind)) => self.start_code_block(kind),

            // ── Phase 3: Structured blocks ───────────────────────────
            Event::Start(Tag::List(first_number)) => self.start_list(first_number),
            Event::Start(Tag::Item) => self.start_list_item(),
            Event::Start(Tag::BlockQuote(_)) => self.start_block_quote(),
            Event::Start(Tag::Table(alignments)) => self.start_table(alignments),

            // ── Inline passthrough ───────────────────────────────────
            // Links: render text in the link style; capture URL for OSC 8.
            Event::Start(Tag::Link { dest_url, .. }) => {
                self.current_link_url = Some(dest_url.to_string());
                self.push_style(theme::inline_style(&self.theme.link));
            }
            // Images: enter InImage state to accumulate alt text, then load on End.
            Event::Start(Tag::Image { dest_url, .. }) => {
                self.state_stack.push(ParserState::InImage {
                    dest_url: dest_url.to_string(),
                    alt_buffer: String::new(),
                });
            }

            // ── Inline formatting ────────────────────────────────────
            Event::Start(Tag::Emphasis) => {
                self.push_style(theme::inline_style(&self.theme.emphasis));
            }
            Event::Start(Tag::Strong) => {
                self.push_style(theme::inline_style(&self.theme.strong));
            }
            Event::Start(Tag::Strikethrough) => {
                self.push_style(theme::inline_style(&self.theme.strikethrough));
            }

            // Any unrecognized block tag — skip until its matching End.
            // MUST be last among Start arms so it doesn't shadow specific variants above.
            Event::Start(_) => self.state_stack.push(ParserState::Skipping { depth: 0 }),

            // ── Block-level end ──────────────────────────────────────
            Event::End(TagEnd::Heading(_)) => self.end_heading(),
            Event::End(TagEnd::Paragraph) => self.end_paragraph(),
            Event::End(TagEnd::List(_)) => self.end_list(),
            Event::End(TagEnd::Item) => self.end_list_item(),
            Event::End(TagEnd::BlockQuote(_)) => self.end_block_quote(),

            // ── Inline end ───────────────────────────────────────────
            Event::End(TagEnd::Link) => {
                self.current_link_url = None;
                self.pop_style();
            }
            // End(Image) is handled by on_image_event — should not reach here.
            Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough) => {
                self.pop_style();
            }

            // ── Text content ─────────────────────────────────────────
            Event::Text(text) => self.push_text(&text),
            Event::Code(text) => self.push_inline_code(&text),
            Event::SoftBreak => self.push_soft_break(),
            Event::HardBreak => self.push_hard_break(),
            Event::Rule => self.emit_block(RenderedBlock::ThematicBreak),
            Event::TaskListMarker(checked) => self.handle_task_list_marker(checked),

            // ── Math ────────────────────────────────────────────────
            Event::InlineMath(text) => self.push_inline_math(&text),
            Event::DisplayMath(text) => self.push_display_math(&text),

            // ── Ignored ──────────────────────────────────────────────
            // End events for passthrough/skipped tags have no handler.
            Event::End(_) => {}
            Event::FootnoteReference(_)
            | Event::InlineHtml(_)
            | Event::Html(_) => {}
        }
    }

    // ── Block emission ───────────────────────────────────────────────────────

    /// Routes a completed block to the correct collector.
    ///
    /// Walks the state stack from the top to find the nearest open container
    /// (`InListItem` or `InBlockQuote`). If found, the block is pushed to that
    /// container's `children` vec. Otherwise it is appended to the top-level
    /// `blocks` vec. This makes every handler automatically work inside nested
    /// lists and block quotes without special-casing.
    fn emit_block(&mut self, block: RenderedBlock) {
        for state in self.state_stack.iter_mut().rev() {
            match state {
                ParserState::InListItem { children, .. } => {
                    children.push(block);
                    return;
                }
                ParserState::InBlockQuote { children } => {
                    children.push(block);
                    return;
                }
                ParserState::InTable { cell_block, .. } => {
                    *cell_block = Some(block);
                    return;
                }
                _ => {}
            }
        }
        self.blocks.push(block);
    }

    /// Attempts ASCII art rendering; falls back to `ImageFallback` on error.
    fn emit_ascii_or_fallback(&mut self, dest_url: &str, alt_text: String) {
        match self.images.load_ascii_image(dest_url) {
            Ok(lines) => {
                self.emit_block(RenderedBlock::AsciiImage { lines, alt_text });
            }
            Err(e) => {
                eprintln!("warning: {e}");
                self.emit_block(RenderedBlock::ImageFallback { alt_text });
            }
        }
    }

    // ── Block handlers ───────────────────────────────────────────────────────

    fn start_heading(&mut self, level: HeadingLevel) {
        let lvl = heading_level_to_u8(level);
        self.style_stack.push(theme::heading_style(&self.theme.heading[lvl as usize - 1]));
        self.current_spans.clear();
        self.state_stack.push(ParserState::InHeading { level: lvl });
    }

    fn end_heading(&mut self) {
        // Pop state first; only pop style if state confirms we were in a heading.
        // This prevents corrupting the style stack on malformed event sequences.
        let level = match self.state_stack.pop() {
            Some(ParserState::InHeading { level }) => {
                self.style_stack.pop();
                level
            }
            other => {
                debug_assert!(
                    false,
                    "End(Heading) without InHeading state: got {other:?}"
                );
                // Do NOT emit a corrupted Heading block — in release builds, clear
                // current_spans to prevent their content from leaking into the next
                // block, then return without emitting anything.
                self.current_spans.clear();
                return;
            }
        };
        let content = std::mem::take(&mut self.current_spans);
        self.emit_block(RenderedBlock::Heading { level, content });
    }

    fn start_paragraph(&mut self) {
        // Immediately inside a list item: paragraph text flows into the item's
        // content rather than becoming a child Paragraph block.
        if matches!(self.state_stack.last(), Some(ParserState::InListItem { .. })) {
            self.state_stack.push(ParserState::InListItemParagraph);
            return;
        }
        // Inside a blockquote that is itself inside a list item: stash the list
        // item's accumulated spans so this paragraph can reuse current_spans.
        // end_paragraph restores the stash after emitting the paragraph block.
        let in_list_item = self
            .state_stack
            .iter()
            .rev()
            .any(|s| matches!(s, ParserState::InListItem { .. }));
        if in_list_item {
            self.span_stash = std::mem::take(&mut self.current_spans);
        } else {
            self.current_spans.clear();
        }
        self.state_stack.push(ParserState::InParagraph);
    }

    fn end_paragraph(&mut self) {
        match self.state_stack.pop() {
            Some(ParserState::InParagraph) => {
                let content = std::mem::take(&mut self.current_spans);
                // Restore any list-item spans saved by start_paragraph when we
                // entered a paragraph inside a blockquote-inside-a-list-item.
                if !self.span_stash.is_empty() {
                    self.current_spans = std::mem::take(&mut self.span_stash);
                }
                self.emit_block(RenderedBlock::Paragraph { content });
            }
            Some(ParserState::InListItemParagraph) => {
                // Content stays in current_spans for the enclosing list item.
                // Add a space separator in case another paragraph follows.
                if !self.current_spans.is_empty() {
                    self.current_spans.push(StyledSpan {
                        text: " ".to_string(),
                        style: Style::default(),
                        url: None,
                    });
                }
            }
            other => {
                debug_assert!(
                    false,
                    "End(Paragraph) in unexpected state: {other:?}"
                );
                // Prevent content from leaking into the next block.
                self.current_spans.clear();
                self.span_stash.clear();
            }
        }
    }

    fn start_code_block(&mut self, kind: CodeBlockKind) {
        let language = match kind {
            // pulldown-cmark yields the full info string (e.g. "rust,no_run" or
            // "python title=\"x.py\""). Take only the first whitespace-delimited
            // token so syntect lookup and the label display get the bare language name.
            CodeBlockKind::Fenced(lang) => lang
                .split_whitespace()
                .next()
                .unwrap_or("")
                .split(',')
                .next()
                .unwrap_or("")
                .to_string(),
            CodeBlockKind::Indented => String::new(),
        };
        self.state_stack
            .push(ParserState::InCodeBlock { language, buffer: String::new() });
    }

    // ── Phase 3: List handlers ───────────────────────────────────────────────

    fn start_list(&mut self, first_number: Option<u64>) {
        let ordered = first_number.is_some();
        let start = first_number.unwrap_or(1);
        self.state_stack.push(ParserState::InList { ordered, start, items: Vec::new() });
    }

    fn end_list(&mut self) {
        match self.state_stack.pop() {
            Some(ParserState::InList { ordered, start, items }) => {
                self.emit_block(RenderedBlock::List { ordered, start, items });
            }
            other => {
                debug_assert!(false, "End(List) without InList state: {other:?}");
            }
        }
    }

    fn start_list_item(&mut self) {
        self.current_spans.clear();
        // Clear any stale stash from a previous item at this nesting level.
        self.span_stash.clear();
        self.state_stack
            .push(ParserState::InListItem { children: Vec::new(), task: None });
    }

    fn end_list_item(&mut self) {
        // Trim the trailing separator space added by end_paragraph for
        // InListItemParagraph — it's only needed between consecutive paragraphs.
        if self.current_spans.last().is_some_and(|s| s.text == " ") {
            self.current_spans.pop();
        }
        // Validate state BEFORE taking content — prevents spans being silently
        // discarded in the mismatch arm (and prevents content leaking into the
        // next item if we continue after the error).
        match self.state_stack.pop() {
            Some(ParserState::InListItem { children, task }) => {
                let content = std::mem::take(&mut self.current_spans);
                let item = ListItem { content, children, task };
                match self.state_stack.last_mut() {
                    Some(ParserState::InList { items, .. }) => {
                        items.push(item);
                    }
                    other => {
                        debug_assert!(false, "End(Item) parent is not InList: {other:?}");
                    }
                }
            }
            other => {
                debug_assert!(false, "End(Item) without InListItem state: {other:?}");
                // current_spans intentionally NOT taken — content stays in place
                // so it doesn't vanish silently in release builds.
            }
        }
    }

    fn handle_task_list_marker(&mut self, checked: bool) {
        if let Some(ParserState::InListItem { task, .. }) = self.state_stack.last_mut() {
            *task = Some(checked);
        }
    }

    // ── Phase 3: Block quote handlers ───────────────────────────────────────

    fn start_block_quote(&mut self) {
        self.state_stack.push(ParserState::InBlockQuote { children: Vec::new() });
    }

    fn end_block_quote(&mut self) {
        match self.state_stack.pop() {
            Some(ParserState::InBlockQuote { children }) => {
                self.emit_block(RenderedBlock::BlockQuote { children });
            }
            other => {
                debug_assert!(false, "End(BlockQuote) without InBlockQuote state: {other:?}");
            }
        }
    }

    // ── Phase 3: Table handlers ──────────────────────────────────────────────

    fn start_table(&mut self, alignments: Vec<Alignment>) {
        self.state_stack.push(ParserState::InTable {
            headers: Vec::new(),
            alignments,
            rows: Vec::new(),
            current_row: Vec::new(),
            in_head: false,
            cell_block: None,
        });
    }

    // ── Style stack helpers ──────────────────────────────────────────────────

    fn push_style(&mut self, style: Style) {
        self.style_stack.push(style);
    }

    fn pop_style(&mut self) {
        debug_assert!(!self.style_stack.is_empty(), "pop_style on empty style_stack");
        self.style_stack.pop();
    }

    // ── Span builders ────────────────────────────────────────────────────────

    fn push_text(&mut self, text: &str) {
        let style = effective_style(&self.style_stack);
        let url = self.current_link_url.clone();
        self.current_spans.push(StyledSpan { text: sanitize_text(text).into_owned(), style, url });
    }

    fn push_inline_code(&mut self, text: &str) {
        let url = self.current_link_url.clone();
        self.current_spans
            .push(StyledSpan { text: sanitize_text(text).into_owned(), style: theme::inline_style(&self.theme.code_inline), url });
    }

    fn push_soft_break(&mut self) {
        let style = effective_style(&self.style_stack);
        self.current_spans.push(StyledSpan { text: " ".to_string(), style, url: None });
    }

    fn push_hard_break(&mut self) {
        let style = effective_style(&self.style_stack);
        self.current_spans.push(StyledSpan { text: "\n".to_string(), style, url: None });
    }

    // ── Math handlers ───────────────────────────────────────────────────────

    fn push_inline_math(&mut self, text: &str) {
        let style = theme::inline_style(&self.theme.math_inline);
        let converted = sanitize_text(&unicode_math(text)).into_owned();
        self.current_spans.push(StyledSpan {
            text: format!("${converted}$"),
            style,
            url: None,
        });
    }

    fn push_display_math(&mut self, text: &str) {
        let style = theme::inline_style(&self.theme.math_display);
        let converted = sanitize_text(&unicode_math(text)).into_owned();
        let content = vec![StyledSpan {
            text: format!("$${converted}$$"),
            style,
            url: None,
        }];
        self.emit_block(RenderedBlock::Paragraph { content });
    }
}

/// Strips terminal-dangerous control characters from untrusted document text.
///
/// Markdown content is attacker-controlled (local file, stdin, or fetched URL).
/// Raw C0 control bytes — most importantly ESC (0x1B) — survive UTF-8 decoding
/// and, in `--print` mode, are written verbatim to the terminal (see
/// `print_styled_line` in `main.rs`), enabling terminal escape-sequence
/// injection: window-title/clipboard (OSC 52)/screen manipulation and, on
/// permissive terminals, answerback-driven command execution.
///
/// Removes every Unicode control character (C0, DEL, C1) EXCEPT the two the
/// pipeline depends on structurally: TAB (code indentation) and LF (breaks).
/// Returns `Cow::Borrowed` when the input is already clean (the common case),
/// so well-formed documents pay only a scan, not an allocation.
fn sanitize_text(s: &str) -> Cow<'_, str> {
    let needs_filter = s.chars().any(|c| c.is_control() && c != '\t' && c != '\n');
    if !needs_filter {
        return Cow::Borrowed(s);
    }
    Cow::Owned(
        s.chars()
            .filter(|&c| !c.is_control() || c == '\t' || c == '\n')
            .collect(),
    )
}

// ── LaTeX-to-Unicode conversion ──────────────────────────────────────────────

/// Best-effort conversion of LaTeX math to a single-line Unicode approximation.
///
/// This is the *fallback* renderer: mdink does not typeset real equations, so
/// the goal here is to be **correct and unambiguous**, not pretty. Braces group
/// invisibly, fractions render as `a/b` (parenthesizing compound operands),
/// roots as `√(…)`, matrices as `[a b; c d]`, and super/subscripts use real
/// Unicode glyphs only when every character maps — otherwise they fall back to
/// `^(…)` / `_(…)` so nothing ever leaks raw LaTeX like `\pi`.
///
/// Unrecognized commands pass through verbatim (`\foo` → `\foo`).
fn unicode_math(input: &str) -> String {
    let mut chars = input.chars().peekable();
    convert_seq(&mut chars, false)
}

type CharStream<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// Converts a run of math tokens. When `in_group`, stops at (and consumes) the
/// matching unescaped `}` — this is how `{...}` grouping is made invisible.
fn convert_seq(chars: &mut CharStream<'_>, in_group: bool) -> String {
    let mut out = String::new();
    while let Some(&ch) = chars.peek() {
        match ch {
            '}' if in_group => {
                chars.next();
                break;
            }
            '\\' => {
                chars.next();
                out.push_str(&parse_backslash(chars));
            }
            '{' => {
                chars.next();
                out.push_str(&convert_seq(chars, true));
            }
            '^' => {
                chars.next();
                let atom = read_atom(chars);
                out.push_str(&apply_script(&atom, true));
            }
            '_' => {
                chars.next();
                let atom = read_atom(chars);
                out.push_str(&apply_script(&atom, false));
            }
            '\'' => {
                chars.next();
                out.push('\u{2032}'); // prime
            }
            '&' => {
                chars.next();
                out.push(' '); // stray alignment tab outside a matrix
            }
            _ => {
                chars.next();
                out.push(ch);
            }
        }
    }
    out
}

/// Reads and converts a single "atom": a `{...}` group, one `\command`, or one
/// bare character. The operand of `^`, `_`, `\frac`, `\sqrt`, accents, etc.
fn read_atom(chars: &mut CharStream<'_>) -> String {
    match chars.peek() {
        Some('{') => {
            chars.next();
            convert_seq(chars, true)
        }
        Some('\\') => {
            chars.next();
            parse_backslash(chars)
        }
        Some(_) => chars.next().map(|c| c.to_string()).unwrap_or_default(),
        None => String::new(),
    }
}

/// Handles everything after a `\`: a named command (with its arguments) or an
/// escaped non-alphabetic character.
fn parse_backslash(chars: &mut CharStream<'_>) -> String {
    if matches!(chars.peek(), Some(c) if c.is_ascii_alphabetic()) {
        let mut name = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_alphabetic() {
                name.push(c);
                chars.next();
            } else {
                break;
            }
        }
        return parse_command(chars, &name);
    }
    // Escaped single character (\{ \} \\ \% \, …).
    match chars.next() {
        Some('\\') => "\n".to_string(),      // line break
        Some(' ' | ',' | ';' | ':') => " ".to_string(),
        Some('!') => String::new(),          // negative thin space
        Some('|') => "\u{2016}".to_string(), // \| → ‖
        Some(c) => c.to_string(),            // \{ \} \# \$ \% \_ \& …
        None => "\\".to_string(),
    }
}

/// Dispatches a named LaTeX command to its Unicode rendering.
fn parse_command(chars: &mut CharStream<'_>, name: &str) -> String {
    match name {
        "frac" | "dfrac" | "tfrac" | "cfrac" => {
            let num = read_atom(chars);
            let den = read_atom(chars);
            format!("{}/{}", paren_if_compound(&num), paren_if_compound(&den))
        }
        "binom" | "dbinom" | "tbinom" => {
            let n = read_atom(chars);
            let k = read_atom(chars);
            format!("C({n}, {k})")
        }
        "pmod" => format!("(mod {})", read_atom(chars)),
        "bmod" => "mod".to_string(),
        "sqrt" => {
            let index = read_optional_bracket(chars);
            let radicand = paren_if_compound(&read_atom(chars));
            match index.as_deref() {
                None | Some("") | Some("2") => format!("\u{221A}{radicand}"),
                Some("3") => format!("\u{221B}{radicand}"),
                Some("4") => format!("\u{221C}{radicand}"),
                Some(n) => format!("{}\u{221A}{radicand}", apply_script(n, true)),
            }
        }
        "begin" => parse_environment(chars),
        "end" => {
            let _ = read_raw_group(chars);
            String::new()
        }
        // Literal-text wrappers: keep content verbatim, drop styling.
        "text" | "textrm" | "textbf" | "textit" | "textsf" | "texttt" | "mbox"
        | "operatorname" => read_raw_group(chars),
        // Math-style wrappers: convert content, drop styling.
        "mathrm" | "mathbf" | "mathit" | "mathsf" | "mathtt" | "mathnormal"
        | "boldsymbol" | "bm" | "mathcal" | "mathfrak" | "mathscr" => read_atom(chars),
        "mathbb" | "mathds" => read_raw_group(chars).chars().map(blackboard_char).collect(),
        // Accents: a combining mark applied to the (converted) base.
        "hat" | "widehat" => accent(read_atom(chars), '\u{0302}'),
        "bar" | "overline" => accent(read_atom(chars), '\u{0304}'),
        "vec" => accent(read_atom(chars), '\u{20D7}'),
        "tilde" | "widetilde" => accent(read_atom(chars), '\u{0303}'),
        "dot" => accent(read_atom(chars), '\u{0307}'),
        "ddot" => accent(read_atom(chars), '\u{0308}'),
        "phantom" | "hphantom" | "vphantom" => {
            let _ = read_atom(chars);
            String::new()
        }
        // Sizing/delimiter prefixes are invisible; the delimiter char follows.
        // `\left.` / `\right.` use `.` as an *invisible* delimiter — eat it.
        "left" | "right" | "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr"
        | "Bigl" | "Bigr" | "biggl" | "biggr" | "Biggl" | "Biggr" => {
            if chars.peek() == Some(&'.') {
                chars.next();
            }
            String::new()
        }
        _ => {
            if let Some(sym) = latex_symbol(name) {
                sym.to_string()
            } else if is_math_function(name) {
                name.to_string()
            } else {
                format!("\\{name}")
            }
        }
    }
}

/// Reads an optional `[...]` argument (e.g. the index of `\sqrt[3]{x}`).
fn read_optional_bracket(chars: &mut CharStream<'_>) -> Option<String> {
    if chars.peek() != Some(&'[') {
        return None;
    }
    chars.next();
    let mut inner = String::new();
    for c in chars.by_ref() {
        if c == ']' {
            break;
        }
        inner.push(c);
    }
    Some(unicode_math(&inner))
}

/// Reads a `{...}` group (or a single char) as *raw* text, without converting
/// its contents. Used for environment names and literal-text wrappers.
fn read_raw_group(chars: &mut CharStream<'_>) -> String {
    if chars.peek() != Some(&'{') {
        return chars.next().map(|c| c.to_string()).unwrap_or_default();
    }
    chars.next();
    let mut inner = String::new();
    let mut depth = 1u32;
    for c in chars.by_ref() {
        match c {
            '{' => {
                depth += 1;
                inner.push(c);
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                inner.push(c);
            }
            _ => inner.push(c),
        }
    }
    inner
}

/// Renders `\begin{env} … \end{env}` as a compact single-line matrix.
fn parse_environment(chars: &mut CharStream<'_>) -> String {
    let env = read_raw_group(chars);
    // Collect the raw body up to the matching \end{...}.
    let mut body = String::new();
    while let Some(&c) = chars.peek() {
        if c == '\\' {
            chars.next();
            let mut cmd = String::new();
            while let Some(&a) = chars.peek() {
                if a.is_ascii_alphabetic() {
                    cmd.push(a);
                    chars.next();
                } else {
                    break;
                }
            }
            if cmd == "end" {
                let _ = read_raw_group(chars);
                break;
            } else if cmd.is_empty() {
                // Escaped char (incl. the `\\` row separator): preserve both.
                body.push('\\');
                if let Some(n) = chars.next() {
                    body.push(n);
                }
            } else {
                body.push('\\');
                body.push_str(&cmd);
            }
        } else {
            chars.next();
            body.push(c);
        }
    }
    render_matrix(&env, &body)
}

/// Joins matrix cells into `a b; c d` and wraps them per environment delimiter.
fn render_matrix(env: &str, body: &str) -> String {
    let rows: Vec<String> = body
        .split("\\\\")
        .map(|row| {
            row.split('&')
                .map(|cell| unicode_math(cell.trim()))
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|r| !r.is_empty())
        .collect();
    let inner = rows.join("; ");
    match env {
        "pmatrix" => format!("({inner})"),
        "bmatrix" => format!("[{inner}]"),
        "Bmatrix" => format!("{{{inner}}}"),
        "vmatrix" => format!("|{inner}|"),
        "Vmatrix" => format!("\u{2016}{inner}\u{2016}"),
        "cases" => format!("{{ {inner}"),
        _ => inner,
    }
}

/// Applies a super- (`sup = true`) or subscript to an already-converted atom.
/// Uses real Unicode glyphs only when *every* character maps; otherwise emits
/// `^(...)` / `_(...)` so nothing renders as broken raw LaTeX.
fn apply_script(atom: &str, sup: bool) -> String {
    let mapped: Option<String> = atom
        .chars()
        .map(|c| if sup { superscript(c) } else { subscript(c) })
        .collect();
    if let Some(s) = mapped {
        return s;
    }
    let marker = if sup { '^' } else { '_' };
    if atom.chars().count() <= 1 {
        format!("{marker}{atom}")
    } else {
        format!("{marker}({atom})")
    }
}

/// Wraps `s` in parentheses when it is a compound expression, so that
/// `\frac{1}{2a}` reads as `1/(2a)` rather than the ambiguous `1/2a`.
fn paren_if_compound(s: &str) -> String {
    if s.chars().count() <= 1 || already_bracketed(s) {
        s.to_string()
    } else {
        format!("({s})")
    }
}

/// True if `s` is already enclosed in a single matching `(...)`/`[...]` pair.
fn already_bracketed(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let close = match chars.first() {
        Some('(') => ')',
        Some('[') => ']',
        _ => return false,
    };
    if chars.last() != Some(&close) {
        return false;
    }
    let mut depth = 0i32;
    for (i, &c) in chars.iter().enumerate() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => {
                depth -= 1;
                if depth == 0 && i != chars.len() - 1 {
                    return false; // closes before the end → not a single wrap
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// Appends a combining accent mark to a base string (`\hat{x}` → `x̂`).
fn accent(mut base: String, mark: char) -> String {
    base.push(mark);
    base
}

/// Maps a LaTeX command name to a standalone Unicode symbol.
///
/// Structural commands (`\frac`, `\sqrt`, `\begin`, accents, text wrappers, …)
/// are handled in `parse_command`; this table is only single-token symbols.
fn latex_symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        // ── Greek lowercase ──
        "alpha" => "α", "beta" => "β", "gamma" => "γ", "delta" => "δ",
        "epsilon" => "ε", "varepsilon" => "ε", "zeta" => "ζ", "eta" => "η",
        "theta" => "θ", "vartheta" => "ϑ", "iota" => "ι", "kappa" => "κ",
        "lambda" => "λ", "mu" => "μ", "nu" => "ν", "xi" => "ξ",
        "omicron" => "ο", "pi" => "π", "varpi" => "ϖ", "rho" => "ρ",
        "varrho" => "ϱ", "sigma" => "σ", "varsigma" => "ς", "tau" => "τ",
        "upsilon" => "υ", "phi" => "ϕ", "varphi" => "φ", "chi" => "χ",
        "psi" => "ψ", "omega" => "ω",
        // ── Greek uppercase ──
        "Gamma" => "Γ", "Delta" => "Δ", "Theta" => "Θ", "Lambda" => "Λ",
        "Xi" => "Ξ", "Pi" => "Π", "Sigma" => "Σ", "Upsilon" => "Υ",
        "Phi" => "Φ", "Psi" => "Ψ", "Omega" => "Ω",
        // ── Binary operators ──
        "times" => "×", "div" => "÷", "pm" => "±", "mp" => "∓",
        "cdot" => "·", "ast" => "∗", "star" => "⋆", "circ" => "∘",
        "bullet" => "•", "oplus" => "⊕", "ominus" => "⊖", "otimes" => "⊗",
        "oslash" => "⊘", "odot" => "⊙", "setminus" => "∖", "wr" => "≀",
        "amalg" => "⨿", "sqcup" => "⊔", "sqcap" => "⊓", "uplus" => "⊎",
        "dagger" => "†", "ddagger" => "‡",
        // ── Relations ──
        "leq" | "le" => "≤", "geq" | "ge" => "≥", "neq" | "ne" => "≠",
        "approx" => "≈", "approxeq" => "≊", "equiv" => "≡", "cong" => "≅",
        "simeq" => "≃", "sim" => "∼", "propto" => "∝", "asymp" => "≍",
        "doteq" => "≐", "ll" => "≪", "gg" => "≫", "prec" => "≺",
        "succ" => "≻", "preceq" => "⪯", "succeq" => "⪰", "lesssim" => "≲",
        "gtrsim" => "≳", "subset" => "⊂", "supset" => "⊃",
        "subseteq" => "⊆", "supseteq" => "⊇", "subsetneq" => "⊊",
        "supsetneq" => "⊋", "sqsubseteq" => "⊑", "sqsupseteq" => "⊒",
        "in" => "∈", "notin" => "∉", "ni" => "∋", "perp" => "⊥",
        "parallel" => "∥", "mid" => "∣", "models" => "⊨", "vdash" => "⊢",
        "dashv" => "⊣",
        // ── Set / logic ──
        "cup" => "∪", "cap" => "∩", "emptyset" => "∅", "varnothing" => "∅",
        "land" | "wedge" => "∧", "lor" | "vee" => "∨", "neg" | "lnot" => "¬",
        "forall" => "∀", "exists" => "∃", "nexists" => "∄", "top" => "⊤",
        "bot" => "⊥", "therefore" => "∴", "because" => "∵",
        "implies" => "⟹", "impliedby" => "⟸", "iff" => "⟺",
        // ── Big operators ──
        "sum" => "∑", "prod" => "∏", "coprod" => "∐", "int" => "∫",
        "iint" => "∬", "iiint" => "∭", "oint" => "∮", "bigcup" => "⋃",
        "bigcap" => "⋂", "bigoplus" => "⨁", "bigotimes" => "⨂",
        "bigvee" => "⋁", "bigwedge" => "⋀", "bigsqcup" => "⨆",
        // ── Calculus / analysis ──
        "partial" => "∂", "nabla" => "∇", "infty" => "∞", "Re" => "ℜ",
        "Im" => "ℑ", "wp" => "℘", "ell" => "ℓ", "hbar" => "ℏ",
        "aleph" => "ℵ", "beth" => "ℶ", "angle" => "∠", "measuredangle" => "∡",
        "triangle" => "△", "square" => "□", "diamond" => "⋄", "surd" => "√",
        // ── Arrows ──
        "to" | "rightarrow" => "→", "leftarrow" | "gets" => "←",
        "leftrightarrow" => "↔", "Rightarrow" => "⇒", "Leftarrow" => "⇐",
        "Leftrightarrow" => "⇔", "uparrow" => "↑", "downarrow" => "↓",
        "updownarrow" => "↕", "Uparrow" => "⇑", "Downarrow" => "⇓",
        "mapsto" => "↦", "longmapsto" => "⟼", "hookrightarrow" => "↪",
        "hookleftarrow" => "↩", "longrightarrow" => "⟶", "longleftarrow" => "⟵",
        "Longrightarrow" => "⟹", "Longleftarrow" => "⟸",
        "nearrow" => "↗", "searrow" => "↘", "swarrow" => "↙", "nwarrow" => "↖",
        // ── Delimiters ──
        "langle" => "⟨", "rangle" => "⟩", "lceil" => "⌈", "rceil" => "⌉",
        "lfloor" => "⌊", "rfloor" => "⌋", "lvert" | "rvert" | "vert" => "|",
        "lVert" | "rVert" | "Vert" => "‖", "backslash" => "\\",
        // ── Dots & misc ──
        "ldots" | "dots" | "cdots" | "dotsc" | "dotsb" => "…",
        "vdots" => "⋮", "ddots" => "⋱", "prime" => "′", "checkmark" => "✓",
        // ── Spacing ──
        "quad" | "qquad" | "thinspace" | "medspace" | "thickspace"
        | "space" | "enspace" => " ",
        "displaystyle" | "textstyle" | "scriptstyle" | "limits" | "nolimits" => "",
        _ => return None,
    })
}

/// True for LaTeX operator-name functions, which render as their bare name
/// (`\sin` → `sin`, `\lim` → `lim`).
fn is_math_function(name: &str) -> bool {
    matches!(
        name,
        "sin" | "cos" | "tan" | "cot" | "sec" | "csc"
            | "sinh" | "cosh" | "tanh" | "coth"
            | "arcsin" | "arccos" | "arctan"
            | "log" | "ln" | "lg" | "exp"
            | "lim" | "limsup" | "liminf" | "max" | "min" | "sup" | "inf"
            | "det" | "dim" | "ker" | "deg" | "gcd" | "hom" | "arg"
            | "Pr" | "mod"
    )
}

/// Maps a Latin letter to its blackboard-bold form (`\mathbb{R}` → `ℝ`).
fn blackboard_char(c: char) -> char {
    match c {
        'A' => '\u{1D538}', 'B' => '\u{1D539}', 'C' => '\u{2102}',
        'D' => '\u{1D53B}', 'E' => '\u{1D53C}', 'F' => '\u{1D53D}',
        'G' => '\u{1D53E}', 'H' => '\u{210D}', 'I' => '\u{1D540}',
        'J' => '\u{1D541}', 'K' => '\u{1D542}', 'L' => '\u{1D543}',
        'M' => '\u{1D544}', 'N' => '\u{2115}', 'O' => '\u{1D546}',
        'P' => '\u{2119}', 'Q' => '\u{211A}', 'R' => '\u{211D}',
        'S' => '\u{1D54A}', 'T' => '\u{1D54B}', 'U' => '\u{1D54C}',
        'V' => '\u{1D54D}', 'W' => '\u{1D54E}', 'X' => '\u{1D54F}',
        'Y' => '\u{1D550}', 'Z' => '\u{2124}',
        other => other,
    }
}

/// Unicode superscript glyph for a character, if one exists.
fn superscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴',
        '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹',
        '+' => '⁺', '-' => '⁻', '=' => '⁼', '(' => '⁽', ')' => '⁾',
        'a' => 'ᵃ', 'b' => 'ᵇ', 'c' => 'ᶜ', 'd' => 'ᵈ', 'e' => 'ᵉ',
        'f' => 'ᶠ', 'g' => 'ᵍ', 'h' => 'ʰ', 'i' => 'ⁱ', 'j' => 'ʲ',
        'k' => 'ᵏ', 'l' => 'ˡ', 'm' => 'ᵐ', 'n' => 'ⁿ', 'o' => 'ᵒ',
        'p' => 'ᵖ', 'r' => 'ʳ', 's' => 'ˢ', 't' => 'ᵗ', 'u' => 'ᵘ',
        'v' => 'ᵛ', 'w' => 'ʷ', 'x' => 'ˣ', 'y' => 'ʸ', 'z' => 'ᶻ',
        ' ' => ' ',
        _ => return None,
    })
}

/// Unicode subscript glyph for a character, if one exists.
fn subscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀', '1' => '₁', '2' => '₂', '3' => '₃', '4' => '₄',
        '5' => '₅', '6' => '₆', '7' => '₇', '8' => '₈', '9' => '₉',
        '+' => '₊', '-' => '₋', '=' => '₌', '(' => '₍', ')' => '₎',
        'a' => 'ₐ', 'e' => 'ₑ', 'h' => 'ₕ', 'i' => 'ᵢ', 'j' => 'ⱼ',
        'k' => 'ₖ', 'l' => 'ₗ', 'm' => 'ₘ', 'n' => 'ₙ', 'o' => 'ₒ',
        'p' => 'ₚ', 'r' => 'ᵣ', 's' => 'ₛ', 't' => 'ₜ', 'u' => 'ᵤ',
        'v' => 'ᵥ', 'x' => 'ₓ',
        ' ' => ' ',
        _ => return None,
    })
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Parses a markdown source string into the RenderedBlock IR.
///
/// Enables GFM extensions (strikethrough, tables, tasklists) so that
/// user markdown containing these features doesn't break.
/// Images are loaded via `images` during parsing; if loading fails
/// they degrade to `ImageFallback` blocks.
pub fn parse(
    source: &str,
    highlighter: &crate::highlight::Highlighter,
    images: &mut crate::images::ImageManager,
    theme: &MarkdownTheme,
) -> Vec<RenderedBlock> {
    ParseContext::new(highlighter, images, theme).process(source)
}

/// Allows `ParserState` to be used in debug_assert messages.
impl std::fmt::Debug for ParserState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserState::TopLevel => write!(f, "TopLevel"),
            ParserState::InHeading { level } => write!(f, "InHeading({level})"),
            ParserState::InParagraph => write!(f, "InParagraph"),
            ParserState::InListItemParagraph => write!(f, "InListItemParagraph"),
            ParserState::InCodeBlock { language, .. } => {
                write!(f, "InCodeBlock({language})")
            }
            ParserState::Skipping { depth } => write!(f, "Skipping({depth})"),
            ParserState::InList { ordered, .. } => write!(f, "InList(ordered={ordered})"),
            ParserState::InListItem { .. } => write!(f, "InListItem"),
            ParserState::InBlockQuote { .. } => write!(f, "InBlockQuote"),
            ParserState::InImage { dest_url, .. } => write!(f, "InImage({dest_url})"),
            ParserState::InTable { .. } => write!(f, "InTable"),
        }
    }
}

#[cfg(test)]
mod tests;
