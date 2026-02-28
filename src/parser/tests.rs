    use super::*;
    use crate::parser::TableCell;
    use ratatui::style::Modifier;
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
        );
        let theme = crate::theme::default_theme();
        super::parse(source, highlighter, &mut im, &theme)
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
            RenderedBlock::ImageFallback { alt_text } => alt_text.contains("alt text"),
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
        if let Some(RenderedBlock::ImageFallback { alt_text }) = fallback {
            assert_eq!(alt_text, "Photo of sunset");
        }
    }

    #[test]
    fn test_parser_image_empty_alt_text() {
        let blocks = parse("![](photo.png)", h());
        let fallback = blocks.iter().find(|b| matches!(b, RenderedBlock::ImageFallback { .. }));
        assert!(fallback.is_some(), "should produce ImageFallback even with empty alt");
        if let Some(RenderedBlock::ImageFallback { alt_text }) = fallback {
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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![photo](test-image.png)", h(), &mut im, &theme);
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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![gradient](gradient.png)", h(), &mut im, &theme);
        let ascii = blocks.iter().find(|b| matches!(b, RenderedBlock::AsciiImage { .. }));
        assert!(ascii.is_some(), "picker=None + valid image should produce AsciiImage");
        if let Some(RenderedBlock::AsciiImage { lines, alt_text }) = ascii {
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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![missing](does-not-exist.png)", h(), &mut im, &theme);
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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![gradient](gradient.png)", h(), &mut im, &theme);
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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![missing](nonexistent.png)", h(), &mut im, &theme);
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
            );
            let theme = crate::theme::default_theme();
            let blocks = super::parse(md, h(), &mut im, &theme);
            let ascii = blocks.iter().find(|b| matches!(b, RenderedBlock::AsciiImage { .. }));
            assert!(ascii.is_some(), "{expected_alt}: should produce AsciiImage");
            if let Some(RenderedBlock::AsciiImage { lines, alt_text }) = ascii {
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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse("![Facebook](facebook-logo.png)", h(), &mut im, &theme);
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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse(source, h(), &mut im, &theme);

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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse(md, h(), &mut im, &theme);
        assert_eq!(blocks.len(), 1, "should produce one Table block");
        match &blocks[0] {
            RenderedBlock::Table { headers, rows, .. } => {
                assert_eq!(headers.len(), 2, "two header columns");
                assert_eq!(rows.len(), 1, "one body row");
                assert_eq!(rows[0].len(), 2, "row has two cells");
                // First cell should be an AsciiImage block.
                match &rows[0][0] {
                    TableCell::Block(RenderedBlock::AsciiImage { lines, alt_text }) => {
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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse(md, h(), &mut im, &theme);

        // ── Parser-level checks ────────────────────────────────────
        assert_eq!(blocks.len(), 1, "should produce one Table block");
        let (img_a_lines, img_b_lines) = match &blocks[0] {
            RenderedBlock::Table { headers, rows, .. } => {
                assert_eq!(headers.len(), 2);
                assert_eq!(rows.len(), 1);

                let a_lines = match &rows[0][0] {
                    TableCell::Block(RenderedBlock::AsciiImage { lines, alt_text }) => {
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
                    TableCell::Block(RenderedBlock::AsciiImage { lines, alt_text }) => {
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
        // header(1) + separator(1) + body_row(16) = 18
        assert_eq!(
            doc.total_height, 18,
            "expected 18 lines (1 header + 1 sep + 16 row), got {}",
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
        // Every line in the body row (lines 2..18) must have the " │ "
        // separator at the same *display column*, proving that cell widths
        // are consistent across all 16 lines of the multi-line row.
        //
        // We measure display-column offset (not byte offset) because the
        // density ramp includes multi-byte characters (braille, block shading)
        // whose byte widths vary per row even though display widths are uniform.
        let mut sep_display_cols: Vec<Option<usize>> = Vec::new();
        for line_idx in 2..18 {
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
        // in the first body-row line (line index 2). This confirms that
        // image colors survive the flatten_cell_to_lines → truncate path.
        if let crate::layout::DocumentLine::Text(line) = &doc.lines[2] {
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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse(md, h(), &mut im, &theme);
        let doc = crate::layout::flatten(&blocks, 80, &theme);

        // header(1) + sep(1) + row(8) = 10
        assert_eq!(doc.total_height, 10, "expected 10 lines, got {}", doc.total_height);

        // Collect all row lines (indices 2..10).
        let mut row_texts: Vec<String> = Vec::new();
        for line_idx in 2..10 {
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
        );
        let theme = crate::theme::default_theme();
        let blocks = super::parse(md, h(), &mut im, &theme);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            RenderedBlock::Table { rows, .. } => {
                match &rows[0][0] {
                    TableCell::Block(RenderedBlock::ImageFallback { alt_text }) => {
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
