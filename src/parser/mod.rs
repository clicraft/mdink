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

use crate::images::ImageManager;
use crate::math::MathEngine;
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
        /// The image source URL or local file path (for image navigation mode).
        src_url: String,
        /// Alt text for accessibility / fallback display.
        alt_text: String,
        /// Image width in terminal cell columns.
        width_cells: u16,
        /// Image height in terminal cell rows.
        height_cells: u16,
        /// Natural pixel width (for resize recalculation via cache).
        px_width: u32,
        /// Natural pixel height (for resize recalculation via cache).
        px_height: u32,
    },
    /// A colored ASCII art rendering of an image (used when no graphics protocol is available).
    AsciiImage {
        /// Pre-rendered colored lines (one Line per row of ASCII art).
        lines: Vec<Line<'static>>,
        /// The image source URL or local file path (for image navigation mode).
        src_url: String,
        /// Alt text for accessibility.
        alt_text: String,
    },
    /// An image that could not be loaded (missing file, no graphics support, etc.).
    ImageFallback {
        /// The image source URL or local file path (for image navigation mode).
        src_url: String,
        /// Alt text to display in place of the image.
        alt_text: String,
    },
    /// A remote image awaiting background fetch.
    /// Produced during parse for http:// and https:// image URLs.
    /// The event loop resolves this to Image/AsciiImage/ImageFallback
    /// once the fetch completes and the result is cached.
    ImagePending {
        /// The URL to fetch.
        url: String,
        /// Alt text for accessibility / fallback display.
        alt_text: String,
    },
    /// Display math formula rendered as Unicode text (immediate, universal fallback).
    /// Always produced first. May be replaced by MathImage after async rendering.
    MathUnicode {
        /// Styled content (wraps like a Paragraph).
        content: Vec<StyledSpan>,
        /// Original LaTeX source (cache key for async rendering).
        raw_latex: String,
    },
    /// Display math formula rendered as a pixel image via terminal graphics protocol.
    /// Produced after async rendering completes and cache is warm.
    MathImage {
        /// Index into `ImageManager::protocols` for renderer access.
        protocol_index: usize,
        /// Image width in terminal cell columns.
        width_cells: u16,
        /// Image height in terminal cell rows.
        height_cells: u16,
        /// Natural pixel width (for resize recalculation via cache).
        px_width: u32,
        /// Natural pixel height (for resize recalculation via cache).
        px_height: u32,
        /// Original LaTeX source (cache key on resize).
        raw_latex: String,
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
    /// Non-empty for inline math spans. Contains the original LaTeX source.
    /// Used by queue_pending_math_renders() to find inline formulas for async rendering.
    /// Empty for all non-math spans.
    pub math_latex: String,
    /// Inline math image metadata. `Some` when this span represents a rendered
    /// inline math image (re-parse with warm cache). `None` for all other spans.
    /// When present, `text` contains NBSP characters as an invisible placeholder.
    pub math_image: Option<InlineMathImage>,
}

/// Inline math image metadata attached to a `StyledSpan`.
///
/// Present when the math formula has been rendered to a pixel image
/// (re-parse with warm cache). The span's text is replaced with NBSP
/// characters whose count equals `width_cells`.
pub struct InlineMathImage {
    /// Index into `ImageManager::protocols` for renderer access.
    pub protocol_index: usize,
    /// Image width in terminal cell columns.
    pub width_cells: u16,
    /// Natural pixel width (for resize recalculation via cache).
    #[allow(dead_code)]
    pub px_width: u32,
    /// Natural pixel height (for resize recalculation via cache).
    #[allow(dead_code)]
    pub px_height: u32,
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
    math: &'a mut MathEngine,
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
        math: &'a mut MathEngine,
        theme: &'a MarkdownTheme,
    ) -> Self {
        Self {
            highlighter,
            images,
            math,
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
                    buffer.push_str(&text);
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
                    alt_buffer.push_str(&text);
                }
            }
            Event::End(TagEnd::Image) => {
                if let Some(ParserState::InImage { dest_url, alt_buffer }) =
                    self.state_stack.pop()
                {
                    let in_table = self
                        .state_stack
                        .iter()
                        .any(|s| matches!(s, ParserState::InTable { .. }));
                    self.load_and_emit_image(dest_url, alt_buffer, in_table);
                }
            }
            // Images don't contain block-level content — ignore everything else.
            _ => {}
        }
    }

    /// Central image routing: given a URL and alt text, emits the correct
    /// `RenderedBlock` variant (Image, AsciiImage, ImagePending, ImageFallback).
    ///
    /// Shared by both markdown `![](url)` images and HTML `<img>` tags.
    fn load_and_emit_image(&mut self, dest_url: String, alt_text: String, in_table: bool) {
        if self.images.images_disabled() {
            self.emit_block(RenderedBlock::ImageFallback { src_url: dest_url, alt_text });
        } else if ImageManager::is_remote_url(&dest_url) {
            if let Some(dyn_img) = self.images.get_cached(&dest_url) {
                let cloned = dyn_img.clone();
                self.resolve_cached_remote(&cloned, &dest_url, &alt_text, in_table);
            } else if !self.images.fetch_remote() {
                // Remote fetching disabled → immediate fallback.
                self.emit_block(RenderedBlock::ImageFallback { src_url: dest_url, alt_text });
            } else if self.images.is_failed_url(&dest_url) {
                // Previously failed → degrade to fallback (don't re-queue).
                self.emit_block(RenderedBlock::ImageFallback { src_url: dest_url, alt_text });
            } else {
                // Remote fetching enabled → emit pending, queue will send to fetch thread.
                self.emit_block(RenderedBlock::ImagePending {
                    url: dest_url,
                    alt_text,
                });
            }
        } else if in_table
            || self.images.prefer_ascii()
            || !self.images.has_graphics_support()
        {
            self.emit_ascii_or_fallback(&dest_url, alt_text);
        } else {
            match self.images.load_image(&dest_url) {
                Ok((protocol_index, width_cells, height_cells, px_width, px_height)) => {
                    self.emit_block(RenderedBlock::Image {
                        protocol_index,
                        src_url: dest_url,
                        alt_text,
                        width_cells,
                        height_cells,
                        px_width,
                        px_height,
                    });
                }
                Err(_) => {
                    self.emit_ascii_or_fallback(&dest_url, alt_text);
                }
            }
        }
    }

    /// Tries to extract an `<img>` tag from raw HTML and emit an image block.
    ///
    /// Handles both `Event::Html` (block-level, may contain the entire
    /// `<div>...</div>`) and `Event::InlineHtml` (self-contained tag).
    /// Returns true if an `<img>` was found and processed.
    fn try_handle_html_img(&mut self, html: &str) -> bool {
        // Find <img (case-insensitive) within the HTML string.
        let lower = html.to_ascii_lowercase();
        let Some(img_start) = lower.find("<img") else {
            return false;
        };
        // Extract from the original html (preserving case for URLs).
        let img_tag = &html[img_start..];
        let src = extract_attr(img_tag, "src");
        let alt = extract_attr(img_tag, "alt").unwrap_or_default();

        if let Some(src) = src {
            let in_table = self
                .state_stack
                .iter()
                .any(|s| matches!(s, ParserState::InTable { .. }));
            self.load_and_emit_image(src, alt, in_table);
            true
        } else {
            false
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
            // Check for <img> tags in HTML events even while skipping.
            Event::Html(html) => {
                self.try_handle_html_img(&html);
            }
            Event::InlineHtml(html) => {
                self.try_handle_html_img(&html);
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
            Event::FootnoteReference(_) => {}
            // ── HTML img tags ────────────────────────────────────────
            // pulldown-cmark emits HTML <img> as InlineHtml/Html rather
            // than Tag::Image. Extract src/alt and route through the same
            // image loading pipeline.
            Event::InlineHtml(html) => {
                self.try_handle_html_img(&html);
            }
            Event::Html(html) => {
                self.try_handle_html_img(&html);
            }
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
        let src_url = dest_url.to_string();
        match self.images.load_ascii_image(dest_url) {
            Ok(lines) => {
                self.emit_block(RenderedBlock::AsciiImage { lines, src_url, alt_text });
            }
            Err(_) => {
                self.emit_block(RenderedBlock::ImageFallback { src_url, alt_text });
            }
        }
    }

    /// Resolves a cached remote image into the appropriate block variant.
    ///
    /// Follows the same routing as local images: native graphics when available,
    /// ASCII art in tables or when forced, fallback on error.
    fn resolve_cached_remote(
        &mut self,
        dyn_img: &image::DynamicImage,
        dest_url: &str,
        alt_text: &str,
        in_table: bool,
    ) {
        let in_table_or_ascii = in_table
            || self.images.prefer_ascii()
            || !self.images.has_graphics_support();

        if in_table_or_ascii {
            match self.images.load_ascii_image_from_memory(dyn_img) {
                Ok(lines) => {
                    self.emit_block(RenderedBlock::AsciiImage {
                        lines,
                        src_url: dest_url.to_string(),
                        alt_text: alt_text.to_string(),
                    });
                }
                Err(_) => {
                    self.emit_block(RenderedBlock::ImageFallback {
                        src_url: dest_url.to_string(),
                        alt_text: alt_text.to_string(),
                    });
                }
            }
        } else {
            match self.images.load_image_from_memory(dyn_img.clone()) {
                Ok((protocol_index, width_cells, height_cells, px_width, px_height)) => {
                    self.emit_block(RenderedBlock::Image {
                        protocol_index,
                        src_url: dest_url.to_string(),
                        alt_text: alt_text.to_string(),
                        width_cells,
                        height_cells,
                        px_width,
                        px_height,
                    });
                }
                Err(_) => {
                    self.emit_block(RenderedBlock::ImageFallback {
                        src_url: dest_url.to_string(),
                        alt_text: alt_text.to_string(),
                    });
                }
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
                        math_latex: String::new(),
                        math_image: None,
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
        self.current_spans.push(StyledSpan { text: text.to_string(), style, url, math_latex: String::new(), math_image: None });
    }

    fn push_inline_code(&mut self, text: &str) {
        let url = self.current_link_url.clone();
        self.current_spans
            .push(StyledSpan { text: text.to_string(), style: theme::inline_style(&self.theme.code_inline), url, math_latex: String::new(), math_image: None });
    }

    fn push_soft_break(&mut self) {
        let style = effective_style(&self.style_stack);
        self.current_spans.push(StyledSpan { text: " ".to_string(), style, url: None, math_latex: String::new(), math_image: None });
    }

    fn push_hard_break(&mut self) {
        let style = effective_style(&self.style_stack);
        self.current_spans.push(StyledSpan { text: "\n".to_string(), style, url: None, math_latex: String::new(), math_image: None });
    }

    // ── Math handlers ───────────────────────────────────────────────────────

    fn push_inline_math(&mut self, text: &str) {
        let raw_latex = text.to_string();
        let style = theme::inline_style(&self.theme.math_inline);

        // Check cache for previously rendered image (only when pixel rendering is enabled).
        if self.math.enabled() {
        if let Some(dyn_img) = self.math.get_cached(&raw_latex) {
            let dyn_img = dyn_img.clone();
            match self.images.load_image_from_memory(dyn_img) {
                Ok((idx, w, _h, pw, ph)) => {
                    // Inline: keep as span within the current paragraph, NOT a standalone block.
                    // Use NBSP characters as placeholder — textwrap treats NBSP as non-breaking,
                    // so the placeholder stays together as one unit during word wrapping.
                    let w = w.max(1);
                    self.current_spans.push(StyledSpan {
                        text: "\u{00A0}".repeat(w as usize),
                        style,
                        url: None,
                        math_latex: raw_latex,
                        math_image: Some(InlineMathImage {
                            protocol_index: idx,
                            width_cells: w,
                            px_width: pw,
                            px_height: ph,
                        }),
                    });
                    return;
                }
                Err(_) => { /* fall through to Unicode approximation */ }
            }
        }
        } // end math.enabled() guard

        // Unicode approximation (immediate display, or image load failed).
        let converted = crate::math::unicode_math(text);
        self.current_spans.push(StyledSpan {
            text: format!("${converted}$"),
            style,
            url: None,
            math_latex: raw_latex,
            math_image: None,
        });
    }

    fn push_display_math(&mut self, text: &str) {
        let raw_latex = text.to_string();
        let style = theme::inline_style(&self.theme.math_display);

        // Check cache for previously rendered image (only when pixel rendering is enabled).
        if self.math.enabled() {
        if let Some(dyn_img) = self.math.get_cached(&raw_latex) {
            let dyn_img = dyn_img.clone();
            match self.images.load_image_from_memory(dyn_img) {
                Ok((idx, w, h, pw, ph)) => {
                    self.emit_block(RenderedBlock::MathImage {
                        protocol_index: idx,
                        width_cells: w,
                        height_cells: h,
                        px_width: pw,
                        px_height: ph,
                        raw_latex,
                    });
                    return;
                }
                Err(_) => { /* fall through to Unicode */ }
            }
        }
        } // end math.enabled() guard

        // Unicode approximation (immediate display).
        let converted = crate::math::unicode_math(text);
        let content = vec![StyledSpan {
            text: format!("$${converted}$$"),
            style,
            url: None,
            math_latex: String::new(), // display math tracked via block, not span
            math_image: None,
        }];
        self.emit_block(RenderedBlock::MathUnicode { content, raw_latex });
    }
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Extracts the value of an HTML attribute from a tag string.
///
/// Handles double-quoted, single-quoted, and unquoted attribute values.
/// Returns `None` if the attribute is not found.
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=", attr);
    let lower = tag.to_ascii_lowercase();
    let pos = lower.find(&pattern)?;
    let rest = &tag[pos + pattern.len()..];

    let first = rest.chars().next()?;
    if first == '"' || first == '\'' {
        // Quoted value: find closing quote.
        let start = first.len_utf8();
        rest[start..].find(first).map(|end| rest[start..start + end].to_string())
    } else {
        // Unquoted value: ends at whitespace or `>`.
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '>')
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

/// Parses a markdown source string into the RenderedBlock IR.
///
/// Enables GFM extensions (strikethrough, tables, tasklists) so that
/// user markdown containing these features doesn't break.
/// Images are loaded via `images` during parsing; if loading fails
/// they degrade to `ImageFallback` blocks.
/// Pre-processes markdown source to normalize math delimiters.
///
/// pulldown-cmark's math extension (following the GFM spec) requires that the
/// opening `$` is immediately followed by a non-whitespace character and the
/// closing `$` is immediately preceded by a non-whitespace character. Some
/// markdown authors write `$ \hat{p}_i$` with a space after `$`, which prevents
/// pulldown-cmark from recognizing it as math.
///
/// This function strips leading whitespace after `$`/`$$` and trailing whitespace
/// before the matching closing delimiter, so that pulldown-cmark can parse the
/// math expression correctly.
///
/// Guards against false positives:
/// - Fenced code blocks (`` ``` ``) and inline code (`` ` ``) are left untouched.
/// - The content between delimiters must not contain unescaped `$` (prevents
///   matching the wrong pair when `$` appears in prose like `$5, not $10`).
/// - The closing `$` must be preceded by non-whitespace and not followed by a digit.
///
/// Returns true if `s` looks like a LaTeX math expression rather than prose.
/// Used as a guard when the closing `$` is preceded by whitespace — we only
/// accept such a match when the content is clearly math.
fn looks_like_math(s: &str) -> bool {
    s.contains('\\') // LaTeX command: \hat, \frac, etc.
        || s.contains('^') // superscript
        || s.contains('_') && s.contains('{') // subscript with braces
        || s.contains('\\') // (belt-and-suspenders)
}

fn normalize_math_delimiters(src: &str) -> String {
    let mut result = String::with_capacity(src.len());
    let mut in_code_fence = false;
    let mut i = 0;
    let b = src.as_bytes();
    let len = src.len();

    while i < len {
        // ── Fenced code blocks ───────────────────────────────────────────
        if !in_code_fence && b[i] == b'`' && src[i..].starts_with("```") {
            in_code_fence = true;
            let end = src[i..].find('\n').map_or(len - i, |p| p + 1);
            result.push_str(&src[i..i + end]);
            i += end;
            continue;
        }
        if in_code_fence {
            if b[i] == b'`' && src[i..].starts_with("```") {
                in_code_fence = false;
                let end = src[i..].find('\n').map_or(len - i, |p| p + 1);
                result.push_str(&src[i..i + end]);
                i += end;
            } else {
                // Safe: loop invariant guarantees i < len, so chars() is non-empty.
                let ch = src[i..].chars().next().unwrap();
                result.push(ch);
                i += ch.len_utf8();
            }
            continue;
        }

        // ── Inline code ──────────────────────────────────────────────────
        if b[i] == b'`' {
            let mut j = i + 1;
            while j < len && b[j] != b'`' {
                j += 1;
            }
            if j < len {
                j += 1;
            }
            result.push_str(&src[i..j]);
            i = j;
            continue;
        }

        // ── Escaped dollar ───────────────────────────────────────────────
        if b[i] == b'\\' && i + 1 < len && b[i + 1] == b'$' {
            result.push_str("\\$");
            i += 2;
            continue;
        }

        // ── Dollar sign ──────────────────────────────────────────────────
        if b[i] == b'$' {
            // Display math: $$...$$
            if i + 1 < len && b[i + 1] == b'$' {
                let after_open = i + 2;
                let content_start = src[after_open..]
                    .find(|c: char| !c.is_whitespace())
                    .map_or(after_open, |p| after_open + p);

                // Find closing $$
                if let Some(rel) = src[content_start..].find("$$") {
                    let close = content_start + rel;
                    let content = src[content_start..close].trim_end();
                    // Content must not contain unescaped $ (avoids matching wrong pair).
                    if !content.contains('$') {
                        result.push_str("$$");
                        result.push_str(content);
                        result.push_str("$$");
                        i = close + 2;
                        continue;
                    }
                }
                // No valid closing $$ or content contains $, output as-is.
                result.push_str("$$");
                i += 2;
                continue;
            }

            // Inline math: $...$ — only pre-process when followed by whitespace.
            if i + 1 < len && (b[i + 1] as char).is_whitespace() {
                let after_open = i + 1;
                let content_start = src[after_open..]
                    .find(|c: char| !c.is_whitespace())
                    .map_or(after_open, |p| after_open + p);

                // Find closing $ on the same line (inline math must not span lines).
                let line_end = src[content_start..]
                    .find('\n')
                    .map_or(len, |p| content_start + p);
                let search = &src[content_start..line_end];

                // Collect all candidate closing $ positions (not $$, not followed by digit).
                let mut candidates: Vec<usize> = Vec::new();
                let mut j = 0;
                while j < search.len() {
                    if search.as_bytes()[j] == b'$' {
                        if j + 1 < search.len() && search.as_bytes()[j + 1] == b'$' {
                            j += 2;
                            continue;
                        }
                        let next_byte_pos = content_start + j + 1;
                        if next_byte_pos < len {
                            // Safe: next_byte_pos < len, so chars() is non-empty.
                            let next_ch = src[next_byte_pos..].chars().next().unwrap();
                            if next_ch.is_ascii_digit() {
                                j += 1;
                                continue;
                            }
                        }
                        candidates.push(j);
                    }
                    j += 1;
                }

                // Try candidates in order.  Prefer $ preceded by non-whitespace;
                // fall back to $ preceded by whitespace only if the trimmed content
                // looks like math (contains LaTeX commands or operators).
                let mut matched = false;
                for &rel_close in &candidates {
                    let close = content_start + rel_close;
                    let content = src[content_start..close].trim_end();
                    if content.contains('$') || content.is_empty() {
                        continue;
                    }
                    // Safe: rel_close > 0 guarantees at least one char before position.
                    let preceded_by_ws = rel_close > 0
                        && src[content_start + rel_close - 1..].chars().next().unwrap().is_whitespace();
                    if preceded_by_ws && !looks_like_math(content) {
                        continue;
                    }
                    result.push('$');
                    result.push_str(content);
                    result.push('$');
                    i = close + 1;
                    matched = true;
                    break;
                }
                if matched {
                    continue;
                }
                // No valid closing $ found, output opening $ as-is.
                result.push('$');
                i += 1;
                continue;
            }

            // $ not followed by whitespace — already fine, copy as-is.
            result.push('$');
            i += 1;
            continue;
        }

        // Safe: loop invariant guarantees i < len, so chars() is non-empty.
        let ch = src[i..].chars().next().unwrap();
        result.push(ch);
        i += ch.len_utf8();
    }

    result
}

pub fn parse(
    source: &str,
    highlighter: &crate::highlight::Highlighter,
    images: &mut crate::images::ImageManager,
    math: &mut MathEngine,
    theme: &MarkdownTheme,
) -> Vec<RenderedBlock> {
    let source = normalize_math_delimiters(source);
    ParseContext::new(highlighter, images, math, theme).process(&source)
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
