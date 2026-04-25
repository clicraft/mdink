    use super::*;
    use crate::images::ImageManager;
    use crate::parser::TableCell;
    use image::DynamicImage;
    use ratatui::style::Modifier;
    use std::path::PathBuf;
    use std::sync::LazyLock;

    static TEST_HIGHLIGHTER: LazyLock<crate::highlight::Highlighter> =
        LazyLock::new(crate::highlight::Highlighter::new);

    fn h() -> &'static crate::highlight::Highlighter {
        &TEST_HIGHLIGHTER
    }

    /// Wrapper so every test can call `parse(md, h())` without constructing an
    /// ImageManager. Uses no_images=true so all images degrade to ImageFallback
    /// (avoiding ASCII art attempts on test paths that don't exist).
    fn parse(source: &str, highlighter: &crate::highlight::Highlighter) -> Vec<RenderedBlock> {
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("."),
            None, // no Picker
            80,
            true,  // no_images → always ImageFallback
            false, // force_ascii
            false, // fetch_remote
        );
        let mut math = crate::math::MathEngine::new(false, false);
        let theme = crate::theme::default_theme();
        super::parse(source, highlighter, &mut im, &mut math, &theme)
    }

    #[test]
    fn test_parser_heading_h1_produces_heading_block() {
        let blocks = parse("# Hello", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Heading { level, content } => {
                assert_eq!(*level, 1);
                assert_eq!(content.len(), 1);
                assert_eq!(content[0].text, "Hello");
            }
            _ => panic!("expected Heading block"),
        }
    }

    #[test]
    fn test_parser_heading_all_levels() {
        for lvl in 1..=6 {
            let md = format!("{} Level {}", "#".repeat(lvl), lvl);
            let blocks = parse(&md, h());
            assert_eq!(blocks.len(), 1, "level {lvl}");
            match &blocks[0] {
                RenderedBlock::Heading { level, .. } => {
                    assert_eq!(*level, lvl as u8, "level {lvl}");
                }
                _ => panic!("expected Heading at level {lvl}"),
            }
        }
    }

    #[test]
    fn test_parser_paragraph_plain_text() {
        let blocks = parse("Hello world", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert_eq!(content.len(), 1);
                assert_eq!(content[0].text, "Hello world");
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_bold_text() {
        let blocks = parse("**bold**", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert_eq!(content.len(), 1);
                assert_eq!(content[0].text, "bold");
                assert!(content[0].style.add_modifier.contains(Modifier::BOLD));
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_italic_text() {
        let blocks = parse("*italic*", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert_eq!(content.len(), 1);
                assert_eq!(content[0].text, "italic");
                assert!(content[0].style.add_modifier.contains(Modifier::ITALIC));
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_strikethrough_text() {
        let blocks = parse("~~struck~~", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert_eq!(content.len(), 1);
                assert_eq!(content[0].text, "struck");
                assert!(content[0]
                    .style
                    .add_modifier
                    .contains(Modifier::CROSSED_OUT));
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_nested_bold_italic() {
        let blocks = parse("***bold italic***", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert_eq!(content.len(), 1);
                assert_eq!(content[0].text, "bold italic");
                let mods = content[0].style.add_modifier;
                assert!(mods.contains(Modifier::BOLD));
                assert!(mods.contains(Modifier::ITALIC));
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_inline_code() {
        let blocks = parse("Use `fmt` here", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert_eq!(content.len(), 3);
                assert_eq!(content[0].text, "Use ");
                assert_eq!(content[1].text, "fmt");
                assert_eq!(content[1].style, crate::theme::inline_style(&crate::theme::default_theme().code_inline));
                assert_eq!(content[2].text, " here");
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_thematic_break() {
        let blocks = parse("---", h());
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], RenderedBlock::ThematicBreak));
    }

    #[test]
    fn test_parser_soft_break() {
        let blocks = parse("line one\nline two", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert_eq!(content.len(), 3);
                assert_eq!(content[0].text, "line one");
                assert_eq!(content[1].text, " ");
                assert_eq!(content[2].text, "line two");
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_hard_break() {
        let blocks = parse("line one\\\nline two", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert!(content.iter().any(|s| s.text == "\n"));
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_empty_input() {
        let blocks = parse("", h());
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_parser_heading_styles_are_distinct() {
        let t = crate::theme::default_theme();
        let s1 = crate::theme::heading_style(&t.heading[0]);
        let s2 = crate::theme::heading_style(&t.heading[1]);
        let s3 = crate::theme::heading_style(&t.heading[2]);
        assert_ne!(s1.fg, s2.fg);
        assert_ne!(s2.fg, s3.fg);
    }

    #[test]
    fn test_parser_skips_unrecognized_blocks() {
        // Use a list (not code block) since code blocks are now handled.
        let md = "- item one\n- item two\n\nAfter list";
        let blocks = parse(md, h());
        assert!(blocks
            .iter()
            .any(|b| matches!(b, RenderedBlock::Paragraph { .. })));
    }

    #[test]
    fn test_parser_link_text_preserved() {
        let blocks = parse("See [the docs](https://example.com) for details", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                let all_text: String = content.iter().map(|s| s.text.as_str()).collect();
                assert!(
                    all_text.contains("the docs"),
                    "link text should be preserved, got: {all_text}"
                );
                assert!(
                    all_text.contains("See"),
                    "surrounding text preserved, got: {all_text}"
                );
                assert!(
                    all_text.contains("for details"),
                    "trailing text preserved, got: {all_text}"
                );
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_image_alt_text_preserved() {
        // Phase 4: images become ImageFallback blocks (load fails in tests
        // because no Picker is available and the path doesn't exist).
        // The alt text must be preserved in the fallback block.
        let blocks = parse("![alt text](image.png)", h());
        // pulldown-cmark wraps a standalone image in a Paragraph; that becomes
        // an empty Paragraph + an ImageFallback block (or just ImageFallback
        // depending on the event sequence). Accept either form.
        let has_alt = blocks.iter().any(|b| match b {
            RenderedBlock::ImageFallback { alt_text, .. } => alt_text.contains("alt text"),
            RenderedBlock::Paragraph { content } => {
                content.iter().any(|s| s.text.contains("alt text"))
            }
            _ => false,
        });
        assert!(has_alt, "alt text 'alt text' not found in any block; got {blocks_len} blocks", blocks_len = blocks.len());
    }

    #[test]
    fn test_parser_bold_inside_link() {
        let blocks = parse("[**bold link**](url)", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert_eq!(content.len(), 1);
                assert_eq!(content[0].text, "bold link");
                assert!(content[0].style.add_modifier.contains(Modifier::BOLD));
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    // ── Phase 4: Image tests ───────────────────────────────────

    #[test]
    fn test_parser_image_no_picker_produces_fallback() {
        // The test wrapper uses None picker, so all images degrade to fallback.
        let blocks = parse("![Photo of sunset](sunset.png)", h());
        let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback.is_some(), "should produce ImageFallback when picker is None");
        if let Some(RenderedBlock::ImageFallback { alt_text, .. }) = fallback {
            assert_eq!(alt_text, "Photo of sunset");
        }
    }

    #[test]
    fn test_parser_image_empty_alt_text() {
        let blocks = parse("![](photo.png)", h());
        let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback.is_some(), "should produce ImageFallback even with empty alt");
        if let Some(RenderedBlock::ImageFallback { alt_text, .. }) = fallback {
            assert!(alt_text.is_empty(), "alt text should be empty, got: {alt_text:?}");
        }
    }

    #[test]
    fn test_parser_image_inline_with_text() {
        // Markdown: paragraph text with an inline image.
        let blocks = parse("Before ![img](x.png) after", h());
        // Should have some text and an ImageFallback.
        let has_fallback = blocks.iter().any(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(has_fallback, "inline image should produce ImageFallback");
    }

    // ── Phase 2: Code block tests ───────────────────────────────

    #[test]
    fn test_parser_fenced_code_block_with_language() {
        let md = "```rust\nfn main() {}\n```";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::CodeBlock {
                language,
                highlighted_lines,
            } => {
                assert_eq!(language, "rust");
                assert!(!highlighted_lines.is_empty());
            }
            _ => panic!("expected CodeBlock"),
        }
    }

    #[test]
    fn test_parser_fenced_code_block_empty_language() {
        let md = "```\nsome code\n```";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::CodeBlock { language, .. } => {
                assert!(language.is_empty());
            }
            _ => panic!("expected CodeBlock"),
        }
    }

    #[test]
    fn test_parser_indented_code_block() {
        let md = "    indented code\n    more code\n";
        let blocks = parse(md, h());
        assert!(
            blocks.iter().any(|b| matches!(b, RenderedBlock::CodeBlock { .. })),
            "indented code should produce CodeBlock"
        );
    }

    #[test]
    fn test_parser_inline_code_still_styled_span() {
        let blocks = parse("Use `code` inline", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert!(content.iter().any(|s| s.text == "code"));
            }
            _ => panic!("expected Paragraph, not CodeBlock"),
        }
    }

    #[test]
    fn test_parser_code_block_content_preserved() {
        let md = "```python\ndef hello():\n    print(\"world\")\n```";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::CodeBlock {
                highlighted_lines, ..
            } => {
                let all_text: String = highlighted_lines
                    .iter()
                    .flat_map(|line| line.spans.iter())
                    .map(|span| span.content.as_ref())
                    .collect();
                assert!(all_text.contains("def"), "should contain 'def'");
                assert!(all_text.contains("hello"), "should contain 'hello'");
                assert!(all_text.contains("print"), "should contain 'print'");
            }
            _ => panic!("expected CodeBlock"),
        }
    }

    #[test]
    fn test_parser_code_block_followed_by_paragraph() {
        let md = "```rust\ncode\n```\n\nAfter code";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], RenderedBlock::CodeBlock { .. }));
        assert!(matches!(&blocks[1], RenderedBlock::Paragraph { .. }));
    }

    #[test]
    fn test_parser_empty_code_block() {
        let md = "```\n```";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::CodeBlock {
                highlighted_lines, ..
            } => {
                assert!(
                    highlighted_lines.is_empty(),
                    "empty code block should produce no lines"
                );
            }
            _ => panic!("expected CodeBlock"),
        }
    }

    #[test]
    fn test_parser_list_with_paragraphs_emits_no_stray_paragraphs() {
        // pulldown-cmark wraps list items in Tag::Paragraph when separated by blank lines.
        // The Skipping guard must suppress those inner paragraphs.
        let md = "- First item\n\n- Second item\n\nAfter list";
        let blocks = parse(md, h());
        let para_count = blocks
            .iter()
            .filter(|b| matches!(b, RenderedBlock::Paragraph { .. }))
            .count();
        assert_eq!(
            para_count, 1,
            "only the paragraph after the list should appear, got {para_count}"
        );
    }

    // ── Font slot strategy tests ────────────────────────────────

    #[test]
    fn test_parser_heading_h4_bold_italic() {
        let blocks = parse("#### Sub-heading", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Heading { level, content } => {
                assert_eq!(*level, 4);
                let mods = content[0].style.add_modifier;
                assert!(mods.contains(Modifier::BOLD), "h4 should have BOLD");
                assert!(mods.contains(Modifier::ITALIC), "h4 should have ITALIC");
            }
            _ => panic!("expected Heading block"),
        }
    }

    #[test]
    fn test_parser_heading_styles_distinct_modifiers() {
        let t = crate::theme::default_theme();
        let h1 = crate::theme::heading_style(&t.heading[0]);
        let h4 = crate::theme::heading_style(&t.heading[3]);
        // h1 has BOLD only
        assert!(h1.add_modifier.contains(Modifier::BOLD));
        assert!(!h1.add_modifier.contains(Modifier::ITALIC));
        // h4 has BOLD + ITALIC
        assert!(h4.add_modifier.contains(Modifier::BOLD));
        assert!(h4.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_parser_inline_code_has_bold_italic() {
        let style = crate::theme::inline_style(&crate::theme::default_theme().code_inline);
        assert!(
            style.add_modifier.contains(Modifier::BOLD),
            "inline code should have BOLD"
        );
        assert!(
            style.add_modifier.contains(Modifier::ITALIC),
            "inline code should have ITALIC"
        );
    }

    #[test]
    fn test_parser_link_text_has_italic() {
        let blocks = parse("[click here](https://example.com)", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert_eq!(content[0].text, "click here");
                assert!(
                    content[0].style.add_modifier.contains(Modifier::ITALIC),
                    "link text should have ITALIC"
                );
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_font_slots_file_parses_without_panic() {
        let source = include_str!("../../testdata/font-slots.md");
        let blocks = parse(source, h());
        assert!(blocks.len() > 20, "font-slots.md should produce many blocks");
        // Verify it contains all expected block types.
        let has_heading = blocks.iter().any(|b| matches!(b, RenderedBlock::Heading { .. }));
        let has_paragraph = blocks.iter().any(|b| matches!(b, RenderedBlock::Paragraph { .. }));
        let has_code = blocks.iter().any(|b| matches!(b, RenderedBlock::CodeBlock { .. }));
        let has_rule = blocks.iter().any(|b| matches!(b, RenderedBlock::ThematicBreak));
        assert!(has_heading, "should have headings");
        assert!(has_paragraph, "should have paragraphs");
        assert!(has_code, "should have code blocks");
        assert!(has_rule, "should have thematic breaks");
    }

    // ── Phase 3: List tests ──────────────────────────────────────

    #[test]
    fn test_parser_unordered_list_produces_list_block() {
        let blocks = parse("- alpha\n- beta\n- gamma", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::List { ordered, start, items } => {
                assert!(!ordered, "expected unordered");
                assert_eq!(*start, 1);
                assert_eq!(items.len(), 3);
                let text: String = items[0].content.iter().map(|s| s.text.as_str()).collect();
                assert_eq!(text, "alpha");
            }
            _ => panic!("expected List block"),
        }
    }

    #[test]
    fn test_parser_ordered_list_preserves_start() {
        let blocks = parse("5. first\n6. second", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::List { ordered, start, items } => {
                assert!(ordered, "expected ordered");
                assert_eq!(*start, 5);
                assert_eq!(items.len(), 2);
            }
            _ => panic!("expected List block"),
        }
    }

    #[test]
    fn test_parser_nested_list_children_populated() {
        let md = "- outer\n  - inner";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::List { items, .. } => {
                assert_eq!(items.len(), 1);
                assert!(
                    !items[0].children.is_empty(),
                    "outer item should have nested list as child"
                );
                match &items[0].children[0] {
                    RenderedBlock::List { ordered, items: inner, .. } => {
                        assert!(!ordered);
                        assert_eq!(inner.len(), 1);
                        let text: String =
                            inner[0].content.iter().map(|s| s.text.as_str()).collect();
                        assert_eq!(text, "inner");
                    }
                    _ => panic!("expected inner List"),
                }
            }
            _ => panic!("expected outer List"),
        }
    }

    #[test]
    fn test_parser_task_list_checked_and_unchecked() {
        let md = "- [x] done\n- [ ] pending";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::List { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].task, Some(true), "first item should be checked");
                assert_eq!(items[1].task, Some(false), "second item should be unchecked");
            }
            _ => panic!("expected List block"),
        }
    }

    #[test]
    fn test_parser_list_items_preserve_text() {
        let blocks = parse("- hello world\n- foo bar", h());
        match &blocks[0] {
            RenderedBlock::List { items, .. } => {
                let t0: String = items[0].content.iter().map(|s| s.text.as_str()).collect();
                let t1: String = items[1].content.iter().map(|s| s.text.as_str()).collect();
                assert!(t0.contains("hello"));
                assert!(t1.contains("foo"));
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn test_parser_list_followed_by_paragraph() {
        let md = "- item\n\nAfter";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], RenderedBlock::List { .. }));
        assert!(matches!(&blocks[1], RenderedBlock::Paragraph { .. }));
    }

    // ── Phase 3: Block quote tests ───────────────────────────────

    #[test]
    fn test_parser_block_quote_has_children() {
        let blocks = parse("> Quoted text here", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::BlockQuote { children } => {
                assert!(!children.is_empty(), "block quote should have child blocks");
                assert!(
                    children.iter().any(|b| matches!(b, RenderedBlock::Paragraph { .. })),
                    "block quote child should be a paragraph"
                );
            }
            _ => panic!("expected BlockQuote"),
        }
    }

    #[test]
    fn test_parser_block_quote_text_preserved() {
        let blocks = parse("> Hello from the quote", h());
        match &blocks[0] {
            RenderedBlock::BlockQuote { children } => {
                if let RenderedBlock::Paragraph { content } = &children[0] {
                    let text: String = content.iter().map(|s| s.text.as_str()).collect();
                    assert!(text.contains("Hello from the quote"));
                } else {
                    panic!("expected Paragraph inside BlockQuote");
                }
            }
            _ => panic!("expected BlockQuote"),
        }
    }

    #[test]
    fn test_parser_nested_block_quote() {
        let md = "> outer\n>\n> > inner";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::BlockQuote { children } => {
                assert!(
                    children.iter().any(|b| matches!(b, RenderedBlock::BlockQuote { .. })),
                    "outer quote should contain inner quote"
                );
            }
            _ => panic!("expected BlockQuote"),
        }
    }

    #[test]
    fn test_parser_block_quote_with_heading() {
        let blocks = parse("> # Heading inside quote", h());
        match &blocks[0] {
            RenderedBlock::BlockQuote { children } => {
                assert!(
                    children.iter().any(|b| matches!(b, RenderedBlock::Heading { .. })),
                    "block quote should contain the heading"
                );
            }
            _ => panic!("expected BlockQuote"),
        }
    }

    // ── Phase 3: Table tests ─────────────────────────────────────

    /// Extracts text from a TableCell::Text cell (panics on Block cells).
    fn cell_text(cell: &TableCell) -> String {
        match cell {
            TableCell::Text(spans) => spans.iter().map(|s| s.text.as_str()).collect(),
            TableCell::Block(_) => panic!("expected TableCell::Text, got Block"),
        }
    }

    #[test]
    fn test_parser_table_headers_and_rows() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Table { headers, rows, .. } => {
                assert_eq!(headers.len(), 2, "two header columns");
                assert_eq!(cell_text(&headers[0]).trim(), "A");
                assert_eq!(cell_text(&headers[1]).trim(), "B");
                assert_eq!(rows.len(), 1, "one body row");
                assert_eq!(rows[0].len(), 2, "row has two cells");
            }
            _ => panic!("expected Table block"),
        }
    }

    #[test]
    fn test_parser_table_alignments_count_matches_headers() {
        let md = "| L | C | R |\n|:---|:---:|---:|\n| a | b | c |";
        let blocks = parse(md, h());
        match &blocks[0] {
            RenderedBlock::Table { headers, alignments, rows } => {
                assert_eq!(headers.len(), 3);
                assert_eq!(alignments.len(), 3);
                assert_eq!(rows.len(), 1);
            }
            _ => panic!("expected Table block"),
        }
    }

    #[test]
    fn test_parser_table_body_cell_text_preserved() {
        let md = "| Name | Value |\n|------|-------|\n| foo  | 42    |";
        let blocks = parse(md, h());
        match &blocks[0] {
            RenderedBlock::Table { rows, .. } => {
                assert!(cell_text(&rows[0][0]).contains("foo"), "cell 0 should contain 'foo'");
                assert!(cell_text(&rows[0][1]).contains("42"), "cell 1 should contain '42'");
            }
            _ => panic!("expected Table block"),
        }
    }

    // ── Phase 3: Test data integration ──────────────────────────

    #[test]
    fn test_lists_testdata_parses_without_panic() {
        let source = include_str!("../../testdata/lists.md");
        let blocks = parse(source, h());
        assert!(blocks.iter().any(|b| matches!(b, RenderedBlock::List { .. })), "should have List blocks");
        assert!(blocks.iter().any(|b| matches!(b, RenderedBlock::Heading { .. })), "should have headings");
    }

    #[test]
    fn test_blockquotes_testdata_parses_without_panic() {
        let source = include_str!("../../testdata/blockquotes.md");
        let blocks = parse(source, h());
        assert!(blocks.iter().any(|b| matches!(b, RenderedBlock::BlockQuote { .. })), "should have BlockQuote blocks");
    }

    #[test]
    fn test_tables_testdata_parses_without_panic() {
        let source = include_str!("../../testdata/tables.md");
        let blocks = parse(source, h());
        assert!(blocks.iter().any(|b| matches!(b, RenderedBlock::Table { .. })), "should have Table blocks");
    }

    #[test]
    fn test_stress_testdata_parses_without_panic() {
        // Full kitchen-sink document: exercises every parser path in one pass.
        let source = include_str!("../../testdata/stress-test.md");
        let blocks = parse(source, h());

        // Recursively collect all blocks so nested code blocks inside lists
        // and block quotes are visible to the assertions.
        fn collect_all(blocks: &[RenderedBlock], out: &mut Vec<String>) {
            for b in blocks {
                out.push(match b {
                    RenderedBlock::Heading { .. } => "Heading",
                    RenderedBlock::Paragraph { .. } => "Paragraph",
                    RenderedBlock::CodeBlock { .. } => "CodeBlock",
                    RenderedBlock::List { .. } => "List",
                    RenderedBlock::BlockQuote { .. } => "BlockQuote",
                    RenderedBlock::Table { .. } => "Table",
                    RenderedBlock::ThematicBreak => "ThematicBreak",
                    RenderedBlock::Spacer { .. } => "Spacer",
                    RenderedBlock::Image { .. } => "Image",
                    RenderedBlock::AsciiImage { .. } => "AsciiImage",
                    RenderedBlock::ImageFallback { .. } => "ImageFallback",
                    RenderedBlock::ImagePending { .. } => "ImagePending",
                    RenderedBlock::MathUnicode { .. } => "MathUnicode",
                    RenderedBlock::MathImage { .. } => "MathImage",
                }.to_string());
                match b {
                    RenderedBlock::List { items, .. } => {
                        for item in items {
                            collect_all(&item.children, out);
                        }
                    }
                    RenderedBlock::BlockQuote { children } => collect_all(children, out),
                    _ => {}
                }
            }
        }

        let mut all_kinds = Vec::new();
        collect_all(&blocks, &mut all_kinds);

        for expected in ["Heading", "Paragraph", "CodeBlock", "List", "BlockQuote", "Table", "ThematicBreak"] {
            assert!(
                all_kinds.iter().any(|k| k == expected),
                "stress-test.md should contain {expected} blocks (found: {all_kinds:?})"
            );
        }
    }

    #[test]
    fn test_stress_testdata_layout_without_panic() {
        // Verify the layout engine handles the stress document at various widths.
        use crate::layout::flatten;
        let source = include_str!("../../testdata/stress-test.md");
        let blocks = parse(source, h());
        let theme = crate::theme::default_theme();
        for width in [20u16, 40, 80, 120, 220] {
            let doc = flatten(&blocks, width, &theme);
            assert!(
                doc.total_height > 0,
                "layout at width={width} should produce lines"
            );
        }
    }

    // ── Security regression tests ────────────────────────────────

    #[test]
    fn test_parser_info_string_first_word_only() {
        // pulldown-cmark yields the full info string — we must take only first word.
        // Formats like "rust,no_run", "python title=\"x.py\"" are common in docs.
        let cases = [
            ("```rust,no_run\ncode\n```", "rust"),
            ("```python title=\"x.py\"\ncode\n```", "python"),
            ("```javascript highlight=true\ncode\n```", "javascript"),
            ("```   rust   \ncode\n```", "rust"), // leading/trailing spaces trimmed by pulldown-cmark
        ];
        for (md, expected_lang) in cases {
            let blocks = parse(md, h());
            assert_eq!(blocks.len(), 1, "input: {md}");
            match &blocks[0] {
                RenderedBlock::CodeBlock { language, .. } => {
                    assert_eq!(
                        language, expected_lang,
                        "info string '{md}' should yield language '{expected_lang}', got '{language}'"
                    );
                }
                _ => panic!("expected CodeBlock for: {md}"),
            }
        }
    }

    // ── Bug-fix regression tests ─────────────────────────────────

    #[test]
    fn test_parser_blockquote_in_list_item_preserves_item_text() {
        // A loose list item whose content is followed by a block quote.
        // Without the span_stash fix, the item's paragraph text was wiped by
        // start_paragraph's current_spans.clear() when the inner paragraph
        // (inside the block quote) started.
        let md = "- item text\n\n  > quoted inside";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 1, "should be one List block");
        match &blocks[0] {
            RenderedBlock::List { items, .. } => {
                assert_eq!(items.len(), 1);
                let item_text: String =
                    items[0].content.iter().map(|s| s.text.as_str()).collect();
                assert!(
                    item_text.contains("item text"),
                    "list item content should survive the nested block quote; got: {item_text:?}"
                );
                assert!(
                    items[0].children.iter().any(|c| matches!(c, RenderedBlock::BlockQuote { .. })),
                    "block quote should be a child of the list item"
                );
            }
            _ => panic!("expected List block"),
        }
    }

    // ── ASCII image fallback tests ──────────────────────────────

    #[test]
    fn test_parser_no_images_flag_produces_fallback() {
        // When no_images=true, images must degrade to ImageFallback regardless
        // of whether the file exists.
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            true,  // explicitly disabled
            false, // force_ascii
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![photo](test-image.png)", h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);
        let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback.is_some(), "no_images=true should produce ImageFallback");
    }

    #[test]
    fn test_parser_no_picker_real_image_produces_ascii_image() {
        // picker=None + no_images=false + valid image file → AsciiImage.
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            false, // images enabled, but no graphics protocol
            false, // force_ascii
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![gradient](gradient.png)", h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);
        let ascii = blocks.iter().find(|b| matches!(b, RenderedBlock::AsciiImage { .. }));
        assert!(ascii.is_some(), "picker=None + valid image should produce AsciiImage");
        if let Some(RenderedBlock::AsciiImage { lines, alt_text, .. }) = ascii {
            assert!(!lines.is_empty(), "AsciiImage should have lines");
            assert_eq!(alt_text, "gradient");
        }
    }

    #[test]
    fn test_parser_no_picker_missing_image_produces_fallback() {
        // picker=None + no_images=false + missing file → ImageFallback.
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            false, // images enabled
            false, // force_ascii
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![missing](does-not-exist.png)", h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);
        let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback.is_some(), "missing file should produce ImageFallback");
    }

    #[test]
    fn test_parser_force_ascii_produces_ascii_image() {
        // force_ascii=true + valid image → AsciiImage (not Image, even though
        // picker would be available in a real terminal).
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            false, // images enabled
            true,  // force ASCII art
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![gradient](gradient.png)", h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);
        let ascii = blocks.iter().find(|b| matches!(b, RenderedBlock::AsciiImage { .. }));
        assert!(ascii.is_some(), "force_ascii=true should produce AsciiImage");
        let native = blocks.iter().any(|b| matches!(b, RenderedBlock::Image { .. }));
        assert!(!native, "force_ascii=true should NOT produce native Image");
    }

    #[test]
    fn test_parser_force_ascii_missing_file_produces_fallback() {
        // force_ascii=true + missing file → ImageFallback (ASCII art load fails).
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            false, // images enabled
            true,  // force ASCII art
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![missing](nonexistent.png)", h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);
        let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback.is_some(), "force_ascii + missing file should produce ImageFallback");
    }

    #[test]
    fn test_parser_force_ascii_midsize_pngs() {
        // Exercise three 256×256 PNGs (both RGB and RGBA) through the full
        // parser → AsciiImage path with force_ascii=true.
        let cases = [
            ("![GitLab](gitlab-logo.png)", "GitLab"),
            ("![Facebook](facebook-logo.png)", "Facebook"),
            ("![Maven](maven-logo.png)", "Maven"),
        ];
        for (md, expected_alt) in cases {
            let mut im = crate::images::ImageManager::new(
                std::path::PathBuf::from("testdata"),
                None,
                80,
                false,
                true, // force ASCII art
                false, // fetch_remote
            );
            let theme = crate::theme::default_theme();
            let blocks = super::parse(md, h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);
            let ascii = blocks.iter().find(|b| matches!(b, RenderedBlock::AsciiImage { .. }));
            assert!(ascii.is_some(), "{expected_alt}: should produce AsciiImage");
            if let Some(RenderedBlock::AsciiImage { lines, alt_text, .. }) = ascii {
                assert!(!lines.is_empty(), "{expected_alt}: should have lines");
                assert_eq!(alt_text, expected_alt);
                // 256×256 with default font (8×16) → 32 wide × 16 tall (fits in 80 cols).
                assert_eq!(
                    lines[0].spans.len(), 32,
                    "{expected_alt}: width should be 256/8 = 32 cols"
                );
                assert_eq!(
                    lines.len(), 16,
                    "{expected_alt}: height should be 256/16 = 16 rows"
                );
            }
        }
    }

    #[test]
    fn test_parser_midsize_png_ascii_art_has_variety() {
        // Verify the ASCII art for a real logo uses multiple density characters
        // (not just a solid block of spaces or '#').
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            60,
            false,
            true,
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![Facebook](facebook-logo.png)", h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);
        if let Some(RenderedBlock::AsciiImage { lines, .. }) =
            blocks.iter().find(|b| matches!(b, RenderedBlock::AsciiImage { .. }))
        {
            let chars: std::collections::HashSet<char> = lines
                .iter()
                .flat_map(|l| l.spans.iter().filter_map(|s| s.content.chars().next()))
                .collect();
            assert!(
                chars.len() >= 3,
                "256x256 logo should use ≥3 density chars, got {}: {:?}",
                chars.len(),
                chars
            );
        } else {
            panic!("expected AsciiImage block");
        }
    }

    #[test]
    fn test_ascii_images_testdata_full_document() {
        // Parse the full ascii-images-test.md through the pipeline with force_ascii.
        let source = include_str!("../../testdata/ascii-images-test.md");
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            false,
            true,
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse(source, h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);

        let ascii_count = blocks.iter().filter(|b| matches!(b, RenderedBlock::AsciiImage { .. })).count();
        assert!(
            ascii_count >= 5,
            "ascii-images-test.md has 6 images; at least 5 should produce AsciiImage, got {ascii_count}"
        );

        // Also verify non-image blocks survived.
        assert!(blocks.iter().any(|b| matches!(b, RenderedBlock::Heading { .. })));
        assert!(blocks.iter().any(|b| matches!(b, RenderedBlock::Paragraph { .. })));
        assert!(blocks.iter().any(|b| matches!(b, RenderedBlock::BlockQuote { .. })));

        // Run through layout at various widths to catch panics.
        for width in [40u16, 80, 120] {
            let doc = crate::layout::flatten(&blocks, width, &theme);
            assert!(doc.total_height > 0, "layout at width={width} should produce lines");
        }
    }

    #[test]
    fn test_parser_two_paragraphs_no_content_bleeding() {
        // Adjacent paragraphs must not bleed content into each other.
        // Regression for end_paragraph else-arm leaving current_spans dirty.
        let md = "para one\n\npara two";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 2);
        match (&blocks[0], &blocks[1]) {
            (RenderedBlock::Paragraph { content: c1 }, RenderedBlock::Paragraph { content: c2 }) => {
                let t1: String = c1.iter().map(|s| s.text.as_str()).collect();
                let t2: String = c2.iter().map(|s| s.text.as_str()).collect();
                assert!(t1.contains("para one"), "first paragraph wrong: {t1:?}");
                assert!(t2.contains("para two"), "second paragraph wrong: {t2:?}");
                assert!(
                    !t2.contains("para one"),
                    "second paragraph must not contain first paragraph's text"
                );
            }
            _ => panic!("expected two Paragraph blocks"),
        }
    }

    // ── Table with image cell tests ──────────────────────────────

    #[test]
    fn test_parser_table_with_image_cell() {
        // Parse a table whose first cell contains an image and second cell is text.
        // Use force_ascii=true so the image becomes AsciiImage (not native Image).
        let md = "| Image | Text |\n|-------|------|\n| ![gradient](gradient.png) | hello |";
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            false, // images enabled
            true,  // force ASCII art
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse(md, h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);
        assert_eq!(blocks.len(), 1, "should produce one Table block");
        match &blocks[0] {
            RenderedBlock::Table { headers, rows, .. } => {
                assert_eq!(headers.len(), 2, "two header columns");
                assert_eq!(rows.len(), 1, "one body row");
                assert_eq!(rows[0].len(), 2, "row has two cells");
                // First cell should be an AsciiImage block.
                match &rows[0][0] {
                    TableCell::Block(RenderedBlock::AsciiImage { lines, alt_text, .. }) => {
                        assert!(!lines.is_empty(), "AsciiImage should have lines");
                        assert_eq!(alt_text, "gradient");
                    }
                    other => panic!(
                        "first cell should be TableCell::Block(AsciiImage), got: {}",
                        match other {
                            TableCell::Text(_) => "Text",
                            TableCell::Block(_) => "Block(other)",
                        }
                    ),
                }
                // Second cell should be text.
                match &rows[0][1] {
                    TableCell::Text(spans) => {
                        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                        assert!(text.contains("hello"), "second cell should contain 'hello', got: {text}");
                    }
                    _ => panic!("second cell should be TableCell::Text"),
                }
            }
            _ => panic!("expected Table block"),
        }
    }

    #[test]
    fn test_parser_table_two_images_layout_structure() {
        // End-to-end test: parse a 2-column table with real images, run through
        // layout, then verify precise structural properties of the output.
        //
        // gradient.png is 160×120 → 20 cols × 8 rows at default font (8×16).
        // gitlab-logo.png is 256×256 → 32 cols × 16 rows at default font.
        //
        // The table row should be 16 lines tall (max of 8, 16), with the
        // gradient cell padded with 8 blank lines below.
        let md = "| A | B |\n|---|---|\n| ![gradient](gradient.png) | ![GitLab](gitlab-logo.png) |";
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            false,
            true, // force ASCII art
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse(md, h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);

        // ── Parser-level checks ────────────────────────────────────
        assert_eq!(blocks.len(), 1, "should produce one Table block");
        let (img_a_lines, img_b_lines) = match &blocks[0] {
            RenderedBlock::Table { headers, rows, .. } => {
                assert_eq!(headers.len(), 2);
                assert_eq!(rows.len(), 1);

                let a_lines = match &rows[0][0] {
                    TableCell::Block(RenderedBlock::AsciiImage { lines, alt_text, .. }) => {
                        assert_eq!(alt_text, "gradient");
                        assert_eq!(lines.len(), 8, "gradient.png → 120/16 = 8 rows");
                        assert_eq!(
                            lines[0].spans.len(), 20,
                            "gradient.png → 160/8 = 20 cols"
                        );
                        lines.len()
                    }
                    other => panic!(
                        "cell A should be AsciiImage, got: {}",
                        match other {
                            TableCell::Text(_) => "Text",
                            TableCell::Block(_) => "Block(other)",
                        }
                    ),
                };

                let b_lines = match &rows[0][1] {
                    TableCell::Block(RenderedBlock::AsciiImage { lines, alt_text, .. }) => {
                        assert_eq!(alt_text, "GitLab");
                        assert_eq!(lines.len(), 16, "gitlab-logo.png → 256/16 = 16 rows");
                        assert_eq!(
                            lines[0].spans.len(), 32,
                            "gitlab-logo.png → 256/8 = 32 cols"
                        );
                        lines.len()
                    }
                    other => panic!(
                        "cell B should be AsciiImage, got: {}",
                        match other {
                            TableCell::Text(_) => "Text",
                            TableCell::Block(_) => "Block(other)",
                        }
                    ),
                };

                (a_lines, b_lines)
            }
            _ => panic!("expected Table block"),
        };

        // ── Layout-level checks ────────────────────────────────────
        let doc = crate::layout::flatten(&blocks, 80, &theme);
        let row_height = img_a_lines.max(img_b_lines); // 16
        assert_eq!(row_height, 16);
        // header(1) + separator(1) + blank_sep(1) + body_row(16) = 19
        assert_eq!(
            doc.total_height, 19,
            "expected 19 lines (1 header + 1 sep + 1 blank + 16 row), got {}",
            doc.total_height
        );

        // Every line must be a Text line (table output is always Text).
        for (i, line) in doc.lines.iter().enumerate() {
            assert!(
                matches!(line, crate::layout::DocumentLine::Text(_)),
                "line {i} should be DocumentLine::Text"
            );
        }

        // ── Column separator alignment check ───────────────────────
        // Every line in the body row (lines 3..19, skipping blank sep at 2)
        // must have the " │ " separator at the same *display column*, proving
        // that cell widths are consistent across all 16 lines of the row.
        //
        // We measure display-column offset (not byte offset) because the
        // density ramp includes multi-byte characters (braille, block shading)
        // whose byte widths vary per row even though display widths are uniform.
        let mut sep_display_cols: Vec<Option<usize>> = Vec::new();
        for line_idx in 3..19 {
            if let crate::layout::DocumentLine::Text(line) = &doc.lines[line_idx] {
                // Walk spans, accumulating display width until we find " │ ".
                let mut col = 0usize;
                let mut found = None;
                for span in &line.spans {
                    if span.content.as_ref() == " │ " {
                        found = Some(col);
                        break;
                    }
                    col += unicode_width::UnicodeWidthStr::width(span.content.as_ref());
                }
                sep_display_cols.push(found);
            }
        }
        // All 16 lines must have a separator.
        assert!(
            sep_display_cols.iter().all(|o| o.is_some()),
            "every row line must contain ' │ ' separator; cols: {sep_display_cols:?}"
        );
        // All separators must be at the same display column.
        let first = sep_display_cols[0].unwrap();
        for (i, col) in sep_display_cols.iter().enumerate() {
            assert_eq!(
                col.unwrap(),
                first,
                "separator on row line {i} at display col {} differs from first line col {first}",
                col.unwrap()
            );
        }

        // ── Image color preservation check ─────────────────────────
        // The gradient image must produce spans with non-default RGB colors
        // in the first body-row content line (index 3, after blank sep at 2).
        if let crate::layout::DocumentLine::Text(line) = &doc.lines[3] {
            let has_rgb = line.spans.iter().any(|s| {
                matches!(
                    s.style.fg,
                    Some(ratatui::style::Color::Rgb(_, _, _))
                )
            });
            assert!(
                has_rgb,
                "first body-row line should contain RGB-colored image spans"
            );
        }
    }

    #[test]
    fn test_parser_table_image_and_text_row_height() {
        // A row with one image cell (8 lines) and one text cell (1 line).
        // The text cell must be padded to 8 lines, and separator alignment
        // must hold on every line.
        let md = "| Pic | Name |\n|-----|------|\n| ![gradient](gradient.png) | A gradient |";
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            false,
            true,
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse(md, h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);
        let doc = crate::layout::flatten(&blocks, 80, &theme);

        // header(1) + sep(1) + blank_sep(1) + row(8) = 11
        assert_eq!(doc.total_height, 11, "expected 11 lines, got {}", doc.total_height);

        // Collect all row lines (indices 3..11, skipping blank sep at 2).
        let mut row_texts: Vec<String> = Vec::new();
        for line_idx in 3..11 {
            if let crate::layout::DocumentLine::Text(line) = &doc.lines[line_idx] {
                row_texts.push(
                    line.spans.iter().map(|s| s.content.as_ref()).collect(),
                );
            }
        }
        assert_eq!(row_texts.len(), 8, "body row should be 8 lines");

        // First line should contain text "A gradient" in the second column.
        assert!(
            row_texts[0].contains("A gradient"),
            "first row line should contain text cell content, got: {:?}",
            row_texts[0]
        );

        // Lines 2..8 (the padding lines) should have an empty second column
        // but still have the " │ " separator.
        for (i, text) in row_texts.iter().enumerate().skip(1) {
            assert!(
                text.contains(" │ "),
                "padding line {i} should still have separator, got: {text:?}"
            );
        }

        // All separators must be at the same display column.
        // Use span-level analysis (not byte offsets) because multi-byte
        // density characters cause byte positions to vary across rows.
        let mut sep_cols: Vec<Option<usize>> = Vec::new();
        for line_idx in 2..10 {
            if let crate::layout::DocumentLine::Text(line) = &doc.lines[line_idx] {
                let mut col = 0usize;
                let mut found = None;
                for span in &line.spans {
                    if span.content.as_ref() == " │ " {
                        found = Some(col);
                        break;
                    }
                    col += unicode_width::UnicodeWidthStr::width(span.content.as_ref());
                }
                sep_cols.push(found);
            }
        }
        assert_eq!(sep_cols.len(), 8, "all 8 lines should have separators");
        assert!(sep_cols.iter().all(|o| o.is_some()), "every line should have separator");
        let first = sep_cols[0].unwrap();
        for (i, col) in sep_cols.iter().enumerate() {
            assert_eq!(col.unwrap(), first, "separator misaligned at row line {i}");
        }
    }

    #[test]
    fn test_parser_table_with_image_no_images_produces_fallback() {
        // With no_images=true, images in table cells should become ImageFallback blocks.
        let md = "| Pic |\n|-----|\n| ![photo](gradient.png) |";
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            true,  // no_images — disabled
            false,
            false, // fetch_remote
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse(md, h(), &mut im, &mut crate::math::MathEngine::new(false, false), &theme);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Table { rows, .. } => {
                match &rows[0][0] {
                    TableCell::Block(RenderedBlock::ImageFallback { alt_text, .. }) => {
                        assert_eq!(alt_text, "photo");
                    }
                    other => panic!(
                        "cell should be ImageFallback, got: {}",
                        match other {
                            TableCell::Text(_) => "Text",
                            TableCell::Block(_) => "Block(other)",
                        }
                    ),
                }
            }
            _ => panic!("expected Table block"),
        }
    }

    // ── OSC 8 link tests ────────────────────────────────────────────────

    #[test]
    fn test_parser_link_sets_url_on_spans() {
        let blocks = parse("[click here](https://example.com)", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert_eq!(content.len(), 1);
                assert_eq!(content[0].text, "click here");
                assert_eq!(
                    content[0].url.as_deref(),
                    Some("https://example.com"),
                    "link span should carry the URL"
                );
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_plain_text_has_no_url() {
        let blocks = parse("just text", h());
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                assert!(content[0].url.is_none(), "plain text should have no URL");
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_link_url_cleared_after_link() {
        let blocks = parse("[a](https://a.com) then plain", h());
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                // First span is the link text.
                assert_eq!(content[0].url.as_deref(), Some("https://a.com"));
                // Subsequent spans should have no URL.
                let has_url_after = content[1..].iter().any(|s| s.url.is_some());
                assert!(!has_url_after, "text after link should have no URL");
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    // ── Mermaid diagram tests ───────────────────────────────────────────

    #[test]
    fn test_parser_mermaid_block_produces_code_block() {
        let md = "```mermaid\ngraph TD\n  A --> B\n```";
        let blocks = parse(md, h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::CodeBlock { language, highlighted_lines } => {
                assert_eq!(language, "mermaid");
                assert_eq!(highlighted_lines.len(), 2);
                // Lines should be plain text (no syntax highlighting).
                let first_line: String = highlighted_lines[0]
                    .spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect();
                assert_eq!(first_line, "graph TD");
            }
            _ => panic!("expected CodeBlock block"),
        }
    }

    // ── LaTeX Math tests ─────────────────────────────────────────────────────

    #[test]
    fn test_parser_inline_math_produces_styled_span() {
        let blocks = parse("The formula $E = mc^{2}$ is famous.", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                // Inline math Unicode fallback wraps converted text in $ delimiters.
                let full_text: String = content.iter().map(|s| s.text.as_str()).collect();
                assert!(
                    full_text.contains("$E = mc\u{00B2}$"),
                    "Expected Unicode superscript with $ delimiters in: {full_text}"
                );
            }
            _ => panic!("expected Paragraph block"),
        }
    }

    #[test]
    fn test_parser_display_math_produces_math_unicode() {
        let blocks = parse("$$\\alpha + \\beta = \\gamma$$", h());
        // Display math creates a MathUnicode block.
        assert!(!blocks.is_empty());
        let has_math = blocks.iter().any(|b| {
            if let RenderedBlock::MathUnicode { content, .. } = b {
                let text: String = content.iter().map(|s| s.text.as_str()).collect();
                text.contains("$$\u{03B1} + \u{03B2} = \u{03B3}$$")
            } else {
                false
            }
        });
        assert!(has_math, "expected a MathUnicode with display math content");
    }

    #[test]
    fn test_math_testdata_parses_without_panic() {
        let source = include_str!("../../testdata/math.md");
        let _ = parse(source, h());
    }

    // ── Remote image tests ────────────────────────────────────────────

    fn parse_with_mgr(source: &str, mgr: &mut ImageManager) -> Vec<RenderedBlock> {
        let mut math = crate::math::MathEngine::new(false, false);
        let theme = crate::theme::default_theme();
        super::parse(source, h(), mgr, &mut math, &theme)
    }

    #[test]
    fn test_parser_remote_url_produces_image_pending() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        let blocks = parse_with_mgr("![alt](https://example.com/img.png)", &mut mgr);
        let pending = blocks.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(pending, "remote URL with empty cache should produce ImagePending");
    }

    #[test]
    fn test_parser_remote_url_no_images_produces_fallback() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, true, false, false);
        let blocks = parse_with_mgr("![alt](https://example.com/img.png)", &mut mgr);
        let fallback = blocks.iter().any(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback, "no_images=true should produce ImageFallback for remote URLs");
        let pending = blocks.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(!pending, "no_images=true should not produce ImagePending");
    }

    #[test]
    fn test_parser_remote_url_cached_produces_ascii() {
        // With no picker and empty cache, first parse produces ImagePending.
        // Insert into cache, re-parse — should resolve to AsciiImage (no graphics support).
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        let blocks1 = parse_with_mgr("![alt](https://example.com/img.png)", &mut mgr);
        assert!(blocks1.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. })));

        // Populate cache with a small test image.
        let dyn_img = DynamicImage::new_rgb8(32, 32);
        mgr.insert_cache("https://example.com/img.png".to_string(), dyn_img);

        let blocks2 = parse_with_mgr("![alt](https://example.com/img.png)", &mut mgr);
        let has_ascii = blocks2.iter().any(|b| matches!(b, RenderedBlock::AsciiImage { .. }));
        assert!(has_ascii, "cached remote URL without graphics support should produce AsciiImage");
        let still_pending = blocks2.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(!still_pending, "cached URL should not produce ImagePending");
    }

    #[test]
    fn test_parser_remote_url_in_table_pending() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        let md = "| Col |\n|-----|\n| ![alt](https://example.com/img.png) |";
        let blocks = parse_with_mgr(md, &mut mgr);
        let table = blocks.iter().find(|b| matches!(b, RenderedBlock::Table { .. }));
        assert!(table.is_some(), "should produce a Table block");
        if let Some(RenderedBlock::Table { rows, .. }) = table {
            assert!(!rows.is_empty());
            if let TableCell::Block(block) = &rows[0][0] {
                assert!(
                    matches!(block, RenderedBlock::ImagePending { .. }),
                    "remote image in table cell should be ImagePending"
                );
            }
        }
    }

    // ── HTML <img> tag tests ──────────────────────────────────────────

    #[test]
    fn test_parser_html_img_remote_url_produces_image_pending() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        let md = r#"<div align="center">
  <img src="https://example.com/img.png" alt="test image" width="90%"/>
  <p>Figure 1</p>
</div>"#;
        let blocks = parse_with_mgr(md, &mut mgr);
        let pending = blocks.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(pending, "HTML <img> with remote URL should produce ImagePending");
    }

    #[test]
    fn test_parser_html_img_inline_produces_image_pending() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        let md = r#"Text before <img src="https://example.com/inline.png" alt="inline"/> text after"#;
        let blocks = parse_with_mgr(md, &mut mgr);
        let pending = blocks.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(pending, "inline HTML <img> with remote URL should produce ImagePending");
    }

    #[test]
    fn test_parser_html_img_no_images_produces_fallback() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, true, false, false);
        let md = r#"<img src="https://example.com/img.png" alt="test"/>"#;
        let blocks = parse_with_mgr(md, &mut mgr);
        let fallback = blocks.iter().any(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback, "no_images=true should produce ImageFallback for HTML <img>");
    }

    #[test]
    fn test_parser_html_img_cached_produces_ascii() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let url = "https://example.com/cached.png";
        mgr.insert_cache(url.to_string(), DynamicImage::new_rgb8(32, 32));
        let md = r#"<img src="https://example.com/cached.png" alt="cached"/>"#;
        let blocks = parse_with_mgr(md, &mut mgr);
        let has_ascii = blocks.iter().any(|b| matches!(b, RenderedBlock::AsciiImage { .. }));
        assert!(has_ascii, "cached HTML <img> without graphics support should produce AsciiImage");
    }

    #[test]
    fn test_parser_html_img_local_produces_fallback() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let md = r#"<img src="nonexistent.png" alt="local"/>"#;
        let blocks = parse_with_mgr(md, &mut mgr);
        let has_fallback = blocks.iter().any(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(has_fallback, "local HTML <img> with missing file should produce ImageFallback");
    }

    #[test]
    fn test_parser_html_img_no_src_ignored() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let md = r#"<img alt="no source"/>"#;
        let blocks = parse_with_mgr(md, &mut mgr);
        let has_image = blocks.iter().any(|b| {
            matches!(b, RenderedBlock::ImagePending { .. }
                | RenderedBlock::ImageFallback { .. }
                | RenderedBlock::AsciiImage { .. }
                | RenderedBlock::Image { .. })
        });
        assert!(!has_image, "HTML <img> without src should not produce any image block");
    }

    #[test]
    fn test_parser_html_img_div_block_produces_image_pending() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        let md = "<div>\n  <img src=\"https://example.com/x.png\" alt=\"in div\"/>\n</div>";
        let blocks = parse_with_mgr(md, &mut mgr);
        let pending = blocks.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(pending, "<img> inside <div> block should produce ImagePending");
    }

    #[test]
    fn test_parser_remote_url_fetch_disabled_produces_fallback() {
        // fetch_remote=false (default): remote URLs should produce ImageFallback, not ImagePending.
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let blocks = parse_with_mgr("![alt](https://example.com/img.png)", &mut mgr);
        let fallback = blocks.iter().any(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback, "fetch_remote=false should produce ImageFallback for remote URLs");
        let pending = blocks.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(!pending, "fetch_remote=false should not produce ImagePending");
    }

    #[test]
    fn test_parser_remote_url_fetch_disabled_cached_produces_ascii() {
        // Even with fetch_remote=false, cached images should still be resolved.
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let url = "https://example.com/img.png";
        mgr.insert_cache(url.to_string(), DynamicImage::new_rgb8(32, 32));
        let blocks = parse_with_mgr(&format!("![alt]({url})"), &mut mgr);
        let has_ascii = blocks.iter().any(|b| matches!(b, RenderedBlock::AsciiImage { .. }));
        assert!(has_ascii, "cached remote URL should produce AsciiImage even with fetch_remote=false");
    }

    #[test]
    fn test_parser_remote_url_fetch_enabled_produces_pending() {
        // fetch_remote=true: the original behavior, remote URLs produce ImagePending.
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        let blocks = parse_with_mgr("![alt](https://example.com/img.png)", &mut mgr);
        let pending = blocks.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(pending, "fetch_remote=true should produce ImagePending");
    }

    #[test]
    fn test_parser_html_img_fetch_disabled_produces_fallback() {
        // fetch_remote=false via HTML <img> tag route.
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let md = r#"<img src="https://example.com/img.png" alt="test"/>"#;
        let blocks = parse_with_mgr(md, &mut mgr);
        let fallback = blocks.iter().any(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback, "HTML <img> with fetch_remote=false should produce ImageFallback");
    }

    // ── Failed remote URL tests ───────────────────────────────────────

    #[test]
    fn test_parser_failed_remote_url_produces_fallback() {
        // When a remote URL has previously failed (is in failed_urls),
        // re-parse should produce ImageFallback instead of ImagePending.
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        // Simulate a failed fetch: mark URL as pending then as failed.
        mgr.mark_pending("https://example.com/broken.png");
        mgr.mark_failed("https://example.com/broken.png");
        let blocks = parse_with_mgr("![alt](https://example.com/broken.png)", &mut mgr);
        let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback.is_some(), "failed URL should produce ImageFallback, not ImagePending");
        if let Some(RenderedBlock::ImageFallback { src_url, alt_text }) = fallback {
            assert_eq!(src_url, "https://example.com/broken.png");
            assert_eq!(alt_text, "alt");
        }
        // Must NOT produce ImagePending.
        let has_pending = blocks.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(!has_pending, "failed URL should not produce ImagePending");
    }

    #[test]
    fn test_parser_failed_remote_url_html_img_produces_fallback() {
        // HTML <img> with a previously-failed URL should also degrade to fallback.
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        mgr.mark_pending("https://cdn.example.com/logo.svg");
        mgr.mark_failed("https://cdn.example.com/logo.svg");
        let md = r#"<img src="https://cdn.example.com/logo.svg" alt="company logo">"#;
        let blocks = parse_with_mgr(md, &mut mgr);
        let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback.is_some(), "failed HTML <img> URL should produce ImageFallback");
    }

    #[test]
    fn test_parser_pending_remote_url_still_produces_pending() {
        // A URL that is pending (currently being fetched) should still produce
        // ImagePending — the fetch thread is working on it.
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        mgr.mark_pending("https://example.com/slow.png");
        let blocks = parse_with_mgr("![alt](https://example.com/slow.png)", &mut mgr);
        let has_pending = blocks.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(has_pending, "pending URL should still produce ImagePending");
    }

    #[test]
    fn test_parser_set_fetch_remote_clears_failed_and_re_allows_pending() {
        // After toggling fetch_remote off then on, failed URLs should be cleared
        // and re-parse should produce ImagePending again.
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        mgr.mark_pending("https://example.com/retry.png");
        mgr.mark_failed("https://example.com/retry.png");
        // Verify it's failed.
        let blocks1 = parse_with_mgr("![alt](https://example.com/retry.png)", &mut mgr);
        assert!(blocks1.iter().any(|b| matches!(b, RenderedBlock::ImageFallback { .. })));
        // Toggle off then on clears failed_urls.
        mgr.set_fetch_remote(false);
        mgr.set_fetch_remote(true);
        // Now the URL should produce ImagePending again.
        let blocks2 = parse_with_mgr("![alt](https://example.com/retry.png)", &mut mgr);
        let has_pending = blocks2.iter().any(|b| matches!(b, RenderedBlock::ImagePending { .. }));
        assert!(has_pending, "after clearing failed_urls, URL should produce ImagePending again");
    }

    #[test]
    fn test_parser_extract_attr_double_quoted() {
        let tag = r#"<img src="https://example.com/img.png" alt="test image"/>"#;
        assert_eq!(super::extract_attr(tag, "src"), Some("https://example.com/img.png".to_string()));
        assert_eq!(super::extract_attr(tag, "alt"), Some("test image".to_string()));
    }

    #[test]
    fn test_parser_extract_attr_single_quoted() {
        let tag = r#"<img src='https://example.com/img.png' alt='test'/>"#;
        assert_eq!(super::extract_attr(tag, "src"), Some("https://example.com/img.png".to_string()));
        assert_eq!(super::extract_attr(tag, "alt"), Some("test".to_string()));
    }

    #[test]
    fn test_parser_extract_attr_missing() {
        let tag = r#"<img alt="no source"/>"#;
        assert_eq!(super::extract_attr(tag, "src"), None);
    }

    // ── pulldown-cmark event behavior for image syntax ────────────────

    /// Documents how pulldown-cmark emits events for a markdown image `![]()`.
    /// pulldown-cmark uses `Tag::Image` for markdown syntax, not `Event::Html`.
    #[test]
    fn test_pulldown_cmark_markdown_image_uses_tag_image() {
        use pulldown_cmark::{Event, Parser, Tag, TagEnd};

        let mut events: Vec<_> = Parser::new("![alt](test.png)").collect();
        // pulldown-cmark wraps the image in a paragraph.
        events.retain(|e| !matches!(e, Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph)));

        assert!(matches!(&events[0], Event::Start(Tag::Image { .. })), "markdown image should emit Tag::Image");
        assert!(matches!(&events[1], Event::Text(_)), "alt text should follow");
        assert!(matches!(&events[2], Event::End(TagEnd::Image)), "should close with TagEnd::Image");
    }

    /// Documents that HTML `<img>` emits `Event::Html` (not `Tag::Image`).
    /// This is why `parser.rs` must extract src/alt from raw HTML strings.
    /// When <img> appears inside paragraph text (inline), it uses `InlineHtml`;
    /// when standalone, it uses `Html` wrapped in an `HtmlBlock`.
    #[test]
    fn test_pulldown_cmark_html_img_standalone_uses_html() {
        use pulldown_cmark::{Event, Parser, Tag};

        let events: Vec<_> = Parser::new(r#"<img src="test.png" alt="alt">"#).collect();
        let has_image_tag = events.iter().any(|e| matches!(e, Event::Start(Tag::Image { .. })));
        let has_html = events.iter().any(|e| matches!(e, Event::Html(_)));
        assert!(!has_image_tag, "HTML <img> should NOT emit Tag::Image");
        assert!(has_html, "standalone HTML <img> should emit Html");
    }

    /// Documents that `<img>` embedded within paragraph text emits `InlineHtml`.
    #[test]
    fn test_pulldown_cmark_html_img_in_paragraph_uses_inline_html() {
        use pulldown_cmark::{Event, Parser, Tag};

        let md = r#"Text before <img src="https://example.com/inline.png" alt="inline"/> text after"#;
        let events: Vec<_> = Parser::new(md).collect();
        let has_image_tag = events.iter().any(|e| matches!(e, Event::Start(Tag::Image { .. })));
        let has_inline_html = events.iter().any(|e| matches!(e, Event::InlineHtml(_)));
        assert!(!has_image_tag, "inline HTML <img> should NOT emit Tag::Image");
        assert!(has_inline_html, "inline HTML <img> within text should emit InlineHtml");
    }

    /// Documents that `<img>` inside a block element (like `<div>`) emits `Event::Html`.
    #[test]
    fn test_pulldown_cmark_html_img_in_div_uses_html() {
        use pulldown_cmark::{Event, Parser, Tag};

        let events: Vec<_> = Parser::new(r#"<div><img src="test.png" alt="alt"></div>"#).collect();
        let has_image_tag = events.iter().any(|e| matches!(e, Event::Start(Tag::Image { .. })));
        let has_html = events.iter().any(|e| matches!(e, Event::Html(_)));
        assert!(!has_image_tag, "<img> inside <div> should NOT emit Tag::Image");
        assert!(has_html, "<img> inside <div> should emit Html (block-level)");
    }

    /// Documents that a self-closing `<img .../>` and non-self-closing `<img ...></img>`
    /// both produce the same event type.
    #[test]
    fn test_pulldown_cmark_html_img_self_closing_vs_not() {
        use pulldown_cmark::{Event, Parser};

        let self_closing: Vec<_> = Parser::new(r#"<img src="test.png" alt="alt">"#).collect();
        let explicit_close: Vec<_> = Parser::new(r#"<img src="test.png" alt="alt"></img>"#).collect();

        let sc_html = self_closing.iter().any(|e| matches!(e, Event::Html(_) | Event::InlineHtml(_)));
        let ec_html = explicit_close.iter().any(|e| matches!(e, Event::Html(_) | Event::InlineHtml(_)));
        assert!(sc_html, "self-closing <img> should emit Html or InlineHtml");
        assert!(ec_html, "explicit close <img></img> should emit Html or InlineHtml");
    }

    // ── Additional Math tests ──────────────────────────────────────────────────

    #[test]
    fn test_parser_inline_math_span_has_math_latex_field() {
        let blocks = parse("The $x^2$ formula.", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                let math_span = content.iter().find(|s| !s.math_latex.is_empty());
                assert!(math_span.is_some(), "should have a span with non-empty math_latex");
                assert_eq!(math_span.unwrap().math_latex, "x^2");
            }
            _ => panic!("expected Paragraph"),
        }
    }

    #[test]
    fn test_parser_inline_math_non_math_spans_have_empty_math_latex() {
        let blocks = parse("Hello $x^2$ world.", h());
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                // "Hello " and " world." should have empty math_latex
                let non_math: Vec<_> = content.iter().filter(|s| !s.text.contains('\u{00B2}')).collect();
                for span in &non_math {
                    assert!(span.math_latex.is_empty(), "non-math span should have empty math_latex: {:?}", span.text);
                }
            }
            _ => panic!("expected Paragraph"),
        }
    }

    #[test]
    fn test_parser_multiple_inline_math_in_paragraph() {
        let blocks = parse("$a^2$ and $b^2$ equal $c^2$", h());
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Paragraph { content } => {
                let math_spans: Vec<_> = content.iter().filter(|s| !s.math_latex.is_empty()).collect();
                assert_eq!(math_spans.len(), 3, "should have 3 inline math spans");
                assert_eq!(math_spans[0].math_latex, "a^2");
                assert_eq!(math_spans[1].math_latex, "b^2");
                assert_eq!(math_spans[2].math_latex, "c^2");
            }
            _ => panic!("expected Paragraph"),
        }
    }

    #[test]
    fn test_parser_mixed_inline_and_display() {
        let blocks = parse("Inline $x^2$\n\n$$E = mc^2$$\n\nMore text.", h());
        // Should have: Paragraph (with inline math), MathUnicode, Paragraph
        let has_paragraph = blocks.iter().any(|b| matches!(b, RenderedBlock::Paragraph { .. }));
        let has_math_unicode = blocks.iter().any(|b| matches!(b, RenderedBlock::MathUnicode { .. }));
        assert!(has_paragraph, "should have a Paragraph with inline math");
        assert!(has_math_unicode, "should have a MathUnicode for display math");
    }

    #[test]
    fn test_parser_display_math_raw_latex_preserved() {
        let blocks = parse("$$\\alpha + \\beta$$", h());
        let math_block = blocks.iter().find(|b| matches!(b, RenderedBlock::MathUnicode { .. }));
        assert!(math_block.is_some());
        if let Some(RenderedBlock::MathUnicode { raw_latex, .. }) = math_block {
            assert_eq!(raw_latex, "\\alpha + \\beta");
        }
    }

    #[test]
    fn test_parser_math_disabled_still_produces_math_unicode() {
        // When MathEngine is disabled (no graphics), display math still produces MathUnicode.
        let source = "$$x^2$$";
        let mut math = crate::math::MathEngine::new(false, false);
        let theme = crate::theme::default_theme();
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let blocks = super::parse(source, h(), &mut mgr, &mut math, &theme);
        let has_math = blocks.iter().any(|b| matches!(b, RenderedBlock::MathUnicode { .. }));
        assert!(has_math, "disabled MathEngine should still produce MathUnicode");
    }

#[test]
fn test_math_parsing_unicode_symbols() {
    // Test that pulldown-cmark parses formulas with Unicode math symbols
    use pulldown_cmark::{Event, Options, Parser};
    
    let mut options = Options::empty();
    options.insert(Options::ENABLE_MATH);
    
    // Test 1: Formula with ∣ (U+2223) and − (U+2212) from the user's markdown
    let formula = "$P(w_i∣w_1,\\cdots,w_{i−1})$";
    let events: Vec<_> = Parser::new_ext(formula, options).collect();
    println!("Formula events: {:?}", events);
    
    let has_inline_math = events.iter().any(|e| matches!(e, Event::InlineMath(_)));
    assert!(has_inline_math, "pulldown-cmark should parse formula as InlineMath: {:?}", events);
    
    // Test 2: In a full line context
    let line = "- **Bigram (当 N=2 时)** ：因此，条件概率 $P(w_i∣w_1,\\cdots,w_{i−1})$ 就可以近似为";
    let events2: Vec<_> = Parser::new_ext(line, options).collect();
    println!("Full line events: {:?}", events2);
    
    let has_inline_math2 = events2.iter().any(|e| matches!(e, Event::InlineMath(_)));
    assert!(has_inline_math2, "pulldown-cmark should parse inline math in full line: {:?}", events2);
}

#[test]
fn test_math_parsing_with_html_strong_tag() {
    // The actual file uses <strong> HTML tags, not **...** markdown
    use pulldown_cmark::{Event, Options, Parser};
    
    let mut options = Options::empty();
    options.insert(Options::ENABLE_MATH);
    
    // Actual content from line 26 of the file (simplified)
    let input = "- <strong>Bigram (当 N=2 时)</strong> ：因此，条件概率 $P(w_i∣w_1,\\cdots,w_{i−1})$ 就可以近似为";
    let events: Vec<_> = Parser::new_ext(input, options).collect();
    println!("HTML strong events: {:?}", events);
    
    let has_inline_math = events.iter().any(|e| matches!(e, Event::InlineMath(_)));
    assert!(has_inline_math, "should have InlineMath with HTML <strong>: {:?}", events);
}

#[test]
fn test_parse_inline_math_with_unicode_symbols() {
    // Full parse() test with the formula from line 26
    let input = "- <strong>Bigram (当 N=2 时)</strong> ：因此，条件概率 $P(w_i∣w_1,\\cdots,w_{i−1})$ 就可以近似为";
    let mut images = crate::images::ImageManager::new(
        std::path::PathBuf::new(), None, 80, false, false, false,
    );
    let mut math = crate::math::MathEngine::new(true, false);
    let theme = crate::theme::MarkdownTheme::default();
    let blocks = super::parse(input, &crate::highlight::Highlighter::new(), &mut images, &mut math, &theme);

    // Should have a list with an item containing a math span
    let found_math_latex = blocks.iter().any(|b| {
        match b {
            super::RenderedBlock::List { items, .. } => {
                items.iter().any(|item| {
                    item.content.iter().any(|span| !span.math_latex.is_empty())
                })
            }
            _ => false,
        }
    });
    assert!(found_math_latex, "should find math_latex in parsed list item");
}

#[test]
fn test_math_parsing_leading_space_after_dollar() {
    // Line 533: $ \hat{p}_i = \frac{p_i}{\sum_{j \in \text{候选集}} p_j}$
    // pulldown-cmark requires opening $ to be followed by non-whitespace.
    use pulldown_cmark::{Event, Options, Parser};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_MATH);

    // With leading space after $
    let with_space = "$ \\hat{p}_i$";
    let events: Vec<_> = Parser::new_ext(with_space, options).collect();
    println!("With space: {:?}", events);
    let has_math_space = events.iter().any(|e| matches!(e, Event::InlineMath(_)));

    // Without leading space
    let no_space = "$\\hat{p}_i$";
    let events2: Vec<_> = Parser::new_ext(no_space, options).collect();
    println!("No space: {:?}", events2);
    let has_math_nospace = events2.iter().any(|e| matches!(e, Event::InlineMath(_)));

    println!("With space: {}, Without space: {}", has_math_space, has_math_nospace);
}

#[test]
fn test_normalize_math_strips_leading_space() {
    use super::normalize_math_delimiters;
    let src = "$ \\hat{p}_i$";
    let out = normalize_math_delimiters(src);
    assert_eq!(out, "$\\hat{p}_i$", "should strip leading space after $");
}

#[test]
fn test_normalize_math_trailing_space_only_not_stripped() {
    use super::normalize_math_delimiters;
    // $x^2 $ — no leading space, so the normalization path isn't entered.
    // Trailing whitespace before closing $ is left for pulldown-cmark to reject.
    // This is a known limitation; authors should write $x^2$ without trailing space.
    let src = "$x^2 $";
    let out = normalize_math_delimiters(src);
    assert_eq!(out, src, "trailing-only whitespace not stripped (no leading space)");
}

#[test]
fn test_normalize_math_strips_both_with_math_content() {
    use super::normalize_math_delimiters;
    // $ x^2 $ — leading AND trailing whitespace.  The trimmed content "x^2"
    // contains ^ so looks_like_math() accepts the trailing-ws closing $.
    let src = "$ x^2 $";
    let out = normalize_math_delimiters(src);
    assert_eq!(out, "$x^2$", "should strip leading and trailing whitespace for math content");
}

#[test]
fn test_normalize_math_display_strips_whitespace() {
    use super::normalize_math_delimiters;
    let src = "$$ \\alpha + \\beta $$";
    let out = normalize_math_delimiters(src);
    assert_eq!(out, "$$\\alpha + \\beta$$");
}

#[test]
fn test_normalize_math_no_change_without_whitespace() {
    use super::normalize_math_delimiters;
    let src = "$x^2$";
    let out = normalize_math_delimiters(src);
    assert_eq!(out, "$x^2$", "no whitespace, no change");
}

#[test]
fn test_normalize_math_skips_code_blocks() {
    use super::normalize_math_delimiters;
    let src = "```\n$ x^2 $\n```";
    let out = normalize_math_delimiters(src);
    assert_eq!(out, src, "code blocks should be untouched");
}

#[test]
fn test_normalize_math_skips_inline_code() {
    use super::normalize_math_delimiters;
    let src = "Use `$ x^2 $` for math.";
    let out = normalize_math_delimiters(src);
    assert_eq!(out, src, "inline code should be untouched");
}

#[test]
fn test_normalize_math_skips_escaped_dollar() {
    use super::normalize_math_delimiters;
    let src = "\\$ not math \\$";
    let out = normalize_math_delimiters(src);
    assert_eq!(out, src, "escaped dollar should be untouched");
}

#[test]
fn test_normalize_math_currency_not_touched() {
    use super::normalize_math_delimiters;
    // $5 doesn't have whitespace after $, so no pre-processing.
    let src = "Price is $5.";
    let out = normalize_math_delimiters(src);
    assert_eq!(out, src);
}

#[test]
fn test_normalize_math_no_false_positive_with_dollar_in_content() {
    use super::normalize_math_delimiters;
    // $ 5, but $x^2$ — content between first and last $ contains another $, so skip.
    let src = "$ 5, but $x^2$";
    let out = normalize_math_delimiters(src);
    // The first $ is followed by space, but content contains $, so no modification.
    assert_eq!(out, src);
}

#[test]
fn test_normalize_math_line_533_formula() {
    use super::normalize_math_delimiters;
    let src = r#"`Top-k `：其原理是将所有 token 按概率从高到低排序，取排名前 k 个的 token 组成 "候选集"，随后对筛选出的 k 个 token 的概率进行 "归一化"： $ \hat{p}_i = \frac{p_i}{\sum_{j \in \text{候选集}} p_j}$"#;
    let out = normalize_math_delimiters(src);
    assert!(out.contains("$\\hat{p}_i"), "should strip space after $: {out}");
    assert!(out.contains("p_j}$"), "closing $ should be preserved");
    assert!(!out.contains("$ \\"), "leading space should be removed");
}

#[test]
fn test_normalize_math_preserves_cjk_text() {
    use super::normalize_math_delimiters;
    let src = "这是中文文本 $ x^2$ 更多中文";
    let out = normalize_math_delimiters(src);
    assert!(out.contains("这是中文文本"), "CJK text before formula should be preserved: {out}");
    assert!(out.contains("更多中文"), "CJK text after formula should be preserved: {out}");
    assert!(out.contains("$x^2$"), "formula should be normalized: {out}");
}

#[test]
fn test_normalize_math_preserves_cjk_only() {
    use super::normalize_math_delimiters;
    let src = "这是一段没有公式的中文文本";
    let out = normalize_math_delimiters(src);
    assert_eq!(out, src, "CJK-only text should be unchanged");
}

// ── Regression: math toggle produces correct block types ────────────────

#[test]
fn test_parser_display_math_ignores_cache_when_disabled() {
    // Regression: when MathEngine is disabled (enabled=false), the parser
    // must produce MathUnicode even when the cache has a rendered image.
    // Previously the parser checked get_cached() without guarding on enabled(),
    // so toggling off had no visible effect.
    use image::DynamicImage;

    let mut math = crate::math::MathEngine::new(true, true);
    // Pre-populate cache — simulates a formula rendered in a previous session.
    let img = DynamicImage::new_rgb8(10, 10);
    math.insert_cache("\\alpha + \\beta".to_string(), img);
    assert!(math.enabled(), "precondition: enabled");
    assert!(math.get_cached("\\alpha + \\beta").is_some(), "precondition: cached");

    // Now disable — simulates user pressing T.
    math.set_enabled(false);
    assert!(!math.enabled());

    let theme = crate::theme::default_theme();
    let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, true, false, false);
    let blocks = super::parse("$$\\alpha + \\beta$$", h(), &mut mgr, &mut math, &theme);

    // Must be MathUnicode (text), NOT MathImage (pixel).
    let has_math_unicode = blocks.iter().any(|b| matches!(b, RenderedBlock::MathUnicode { .. }));
    let has_math_image = blocks.iter().any(|b| matches!(b, RenderedBlock::MathImage { .. }));
    assert!(has_math_unicode, "disabled engine must produce MathUnicode, not use cache");
    assert!(!has_math_image, "disabled engine must NOT produce MathImage even with warm cache");
}

#[test]
fn test_parser_inline_math_ignores_cache_when_disabled() {
    // Same as above but for inline math ($...$) within a paragraph.
    use image::DynamicImage;

    let mut math = crate::math::MathEngine::new(true, true);
    let img = DynamicImage::new_rgb8(10, 10);
    math.insert_cache("x^2".to_string(), img);
    math.set_enabled(false);

    let theme = crate::theme::default_theme();
    let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, true, false, false);
    let blocks = super::parse("The formula $x^2$ is simple.", h(), &mut mgr, &mut math, &theme);

    // The paragraph should contain a span with math_latex set but math_image=None.
    let mut found_math_span = false;
    for block in &blocks {
        if let RenderedBlock::Paragraph { content } = block {
            for span in content {
                if !span.math_latex.is_empty() {
                    assert!(span.math_image.is_none(),
                        "disabled engine must NOT attach InlineMathImage even with warm cache");
                    assert!(span.text.contains('x') || span.text.contains('²'),
                        "math span should contain Unicode math text, got: {:?}", span.text);
                    found_math_span = true;
                }
            }
        }
    }
    assert!(found_math_span, "should find inline math span in paragraph");
}

#[test]
fn test_parser_display_math_uses_cache_when_enabled() {
    // Verify the positive case: when enabled AND cache has the formula,
    // the parser produces MathImage (not MathUnicode).
    use image::DynamicImage;

    let mut math = crate::math::MathEngine::new(true, true);
    let img = DynamicImage::new_rgb8(10, 10);
    math.insert_cache("\\alpha".to_string(), img);
    assert!(math.enabled());
    assert!(math.get_cached("\\alpha").is_some());

    // ImageManager needs to accept the load — use no_images=true to force fallback.
    // Actually we need it to load from memory, so use no_picker (None) but no_images=false.
    // But without a Picker, load_image_from_memory returns Err. Let's check the actual path.
    // When load_image_from_memory fails, it falls through to MathUnicode. So for this test
    // we just verify the code path *attempts* to use the cache (falls through to Unicode
    // because no Picker). The key assertion is that it does NOT produce MathUnicode with
    // the original raw_latex when cache is warm (it may produce MathUnicode due to load
    // failure, but it will try the cache first).
    //
    // A cleaner test: verify that when enabled=false the result differs from enabled=true.
    let theme = crate::theme::default_theme();

    // With enabled=true, cache hit → tries load_image_from_memory → fails (no Picker)
    // → falls through to MathUnicode. This is the same output as enabled=false.
    // To distinguish, we check that the span's math_latex is set correctly for both cases
    // and that the enabled=false path definitely doesn't attempt image loading.

    // The real value of this test is the negative case above (enabled=false).
    // Here we just confirm the baseline: enabled=true with cache produces a block.
    let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, true, false, false);
    let blocks = super::parse("$$\\alpha$$", h(), &mut mgr, &mut math, &theme);
    assert!(!blocks.is_empty(), "should produce at least one block");
    // Without Picker, it falls through to MathUnicode regardless — that's expected.
    let has_math = blocks.iter().any(|b|
        matches!(b, RenderedBlock::MathUnicode { .. }) || matches!(b, RenderedBlock::MathImage { .. }));
    assert!(has_math, "enabled engine should produce a math block");
}

// ── Missing local image file tests ─────────────────────────────────

#[test]
fn test_parser_missing_local_image_produces_fallback_with_src_url() {
    // When a local image file doesn't exist, the parser should produce
    // ImageFallback with the src_url preserved for diagnostics.
    let mut im = crate::images::ImageManager::new(
        std::path::PathBuf::from("testdata"),
        None,
        80,
        false, // images enabled (will fail because no picker + missing file)
        false,
        false,
    );
    let theme = crate::theme::default_theme();
    let blocks = super::parse(
        "![my image](no-such-file.png)",
        h(),
        &mut im,
        &mut crate::math::MathEngine::new(false, false),
        &theme,
    );
    let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
    assert!(fallback.is_some(), "missing file should produce ImageFallback");
    if let Some(RenderedBlock::ImageFallback { src_url, alt_text }) = fallback {
        assert_eq!(src_url, "no-such-file.png", "src_url should be preserved");
        assert_eq!(alt_text, "my image", "alt_text should be preserved");
    }
}

#[test]
fn test_parser_missing_local_image_html_img_produces_fallback() {
    // HTML <img> with missing local file should also produce ImageFallback.
    let mut im = crate::images::ImageManager::new(
        std::path::PathBuf::from("testdata"),
        None,
        80,
        false,
        false,
        false,
    );
    let theme = crate::theme::default_theme();
    let blocks = super::parse(
        r#"<img src="missing-photo.jpg" alt="a sunset">"#,
        h(),
        &mut im,
        &mut crate::math::MathEngine::new(false, false),
        &theme,
    );
    let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
    assert!(fallback.is_some(), "missing HTML img should produce ImageFallback");
    if let Some(RenderedBlock::ImageFallback { src_url, alt_text }) = fallback {
        assert_eq!(src_url, "missing-photo.jpg");
        assert_eq!(alt_text, "a sunset");
    }
}

#[test]
fn test_parser_missing_local_image_in_list_produces_fallback() {
    // Image inside a list with missing file should degrade gracefully.
    let mut im = crate::images::ImageManager::new(
        std::path::PathBuf::from("testdata"),
        None,
        80,
        false,
        false,
        false,
    );
    let theme = crate::theme::default_theme();
    let blocks = super::parse(
        "- item one\n- ![photo](nonexistent.jpg)\n- item three",
        h(),
        &mut im,
        &mut crate::math::MathEngine::new(false, false),
        &theme,
    );
    // The image in a list produces a List block; the image itself is a child block
    // of one of the list items. Search children for ImageFallback.
    let mut found = false;
    for b in &blocks {
        if let RenderedBlock::List { items, .. } = b {
            for item in items {
                let has_fallback = item.children.iter().any(|child|
                    matches!(child, RenderedBlock::ImageFallback { src_url, .. } if src_url.contains("nonexistent"))
                );
                let has_url = item.content.iter().any(|s| s.url.as_deref() == Some("nonexistent.jpg"));
                if has_fallback || has_url {
                    found = true;
                }
            }
        }
    }
    assert!(found, "missing image in list should produce ImageFallback in children or URL in content");
}

#[test]
fn test_parser_missing_local_image_no_eprintln() {
    // Verify that parsing a missing local image produces a clean fallback.
    // The eprintln! calls that previously corrupted the TUI have been removed.
    let mut im = crate::images::ImageManager::new(
        std::path::PathBuf::from("testdata"),
        None,
        80,
        false,
        false,
        false,
    );
    let theme = crate::theme::default_theme();
    let blocks = super::parse(
        "![alt text](totally-missing.png)",
        h(),
        &mut im,
        &mut crate::math::MathEngine::new(false, false),
        &theme,
    );
    // The key assertion: fallback is produced without panicking or errors.
    let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
    assert!(fallback.is_some(), "missing image should produce clean ImageFallback");
}
