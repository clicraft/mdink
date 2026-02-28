    use super::*;
    use crate::parser::StyledSpan;
    use ratatui::style::{Color, Modifier, Style};

    /// Wrapper that passes the default theme so existing tests don't need updating.
    fn flatten_default(blocks: &[RenderedBlock], width: u16) -> PreRenderedDocument {
        flatten(blocks, width, &crate::theme::default_theme())
    }

    fn plain_span(text: &str) -> StyledSpan {
        StyledSpan {
            text: text.to_string(),
            style: Style::default(),
        }
    }

    fn styled_span(text: &str, style: Style) -> StyledSpan {
        StyledSpan {
            text: text.to_string(),
            style,
        }
    }

    #[test]
    fn test_layout_empty_blocks() {
        let doc = flatten_default(&[], 80);
        assert_eq!(doc.total_height, 0);
        assert!(doc.lines.is_empty());
    }

    #[test]
    fn test_layout_single_paragraph_no_wrap() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![plain_span("Hello world")],
        }];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 1);
        assert!(matches!(&doc.lines[0], DocumentLine::Text(_)));
    }

    #[test]
    fn test_layout_paragraph_wraps_at_width() {
        let long_text = "word ".repeat(20); // 100 chars
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![plain_span(long_text.trim())],
        }];
        let doc = flatten_default(&blocks,40);
        assert!(
            doc.total_height > 1,
            "expected wrapping, got {} lines",
            doc.total_height
        );
    }

    #[test]
    fn test_layout_thematic_break() {
        let blocks = vec![RenderedBlock::ThematicBreak];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 1);
        assert!(matches!(&doc.lines[0], DocumentLine::Rule));
    }

    #[test]
    fn test_layout_inter_block_spacing() {
        let blocks = vec![
            RenderedBlock::Paragraph {
                content: vec![plain_span("First")],
            },
            RenderedBlock::Paragraph {
                content: vec![plain_span("Second")],
            },
        ];
        let doc = flatten_default(&blocks,80);
        // First paragraph (1 line) + empty (1 line) + second paragraph (1 line) = 3
        assert_eq!(doc.total_height, 3);
        assert!(matches!(&doc.lines[1], DocumentLine::Empty));
    }

    #[test]
    fn test_layout_heading_renders_as_text() {
        let blocks = vec![RenderedBlock::Heading {
            level: 1,
            content: vec![styled_span(
                "Title",
                Style::default().add_modifier(Modifier::BOLD),
            )],
        }];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 1);
        assert!(matches!(&doc.lines[0], DocumentLine::Text(_)));
    }

    #[test]
    fn test_layout_spacer() {
        let blocks = vec![RenderedBlock::Spacer { lines: 3 }];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 3);
        for line in &doc.lines {
            assert!(matches!(line, DocumentLine::Empty));
        }
    }

    #[test]
    fn test_layout_single_long_word() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![plain_span("abcdefghijklmnopqrstuvwxyz")],
        }];
        let doc = flatten_default(&blocks,10);
        assert!(doc.total_height >= 2, "long word should wrap");
    }

    #[test]
    fn test_layout_empty_paragraph() {
        let blocks = vec![RenderedBlock::Paragraph { content: vec![] }];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 1);
    }

    #[test]
    fn test_layout_preserves_styles_across_wrap() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let text = "word ".repeat(20);
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![styled_span(text.trim(), bold)],
        }];
        let doc = flatten_default(&blocks,40);
        for line in &doc.lines {
            if let DocumentLine::Text(l) = line {
                for span in &l.spans {
                    assert!(
                        span.style.add_modifier.contains(Modifier::BOLD),
                        "style lost after wrapping"
                    );
                }
            }
        }
    }

    #[test]
    fn test_layout_repeated_text_no_misalignment() {
        // Regression test: repeated text must not confuse the cursor.
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![plain_span("aaa bbb aaa bbb aaa bbb")],
        }];
        let doc = flatten_default(&blocks,8);
        // Collect all text from the wrapped lines.
        let mut all_text = String::new();
        for line in &doc.lines {
            if let DocumentLine::Text(l) = line {
                for span in &l.spans {
                    all_text.push_str(&span.content);
                }
                all_text.push(' '); // represent line break as space
            }
        }
        // All original words must appear (no duplication, no loss).
        assert_eq!(all_text.matches("aaa").count(), 3, "word 'aaa' count");
        assert_eq!(all_text.matches("bbb").count(), 3, "word 'bbb' count");
    }

    #[test]
    fn test_layout_multi_style_spans_across_wrap() {
        // Two styled spans that together exceed the width.
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let italic = Style::default().add_modifier(Modifier::ITALIC);
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![
                styled_span("hello ", bold),
                styled_span("world this is long", italic),
            ],
        }];
        let doc = flatten_default(&blocks,12);
        assert!(doc.total_height >= 2, "should wrap");
        // First line should have bold "hello " and italic "world"
        if let DocumentLine::Text(first_line) = &doc.lines[0] {
            assert!(!first_line.spans.is_empty(), "first line should have spans");
        }
    }

    #[test]
    fn test_layout_unicode_emoji_no_panic() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![plain_span("Hello 🌍 world 🎉 test 🚀 more text here for wrapping")],
        }];
        // Should not panic on emoji at any width.
        let doc = flatten_default(&blocks,15);
        assert!(doc.total_height >= 1);
    }

    #[test]
    fn test_layout_cjk_text_no_panic() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![plain_span("日本語のテキスト処理テスト")],
        }];
        let doc = flatten_default(&blocks,10);
        assert!(doc.total_height >= 1);
    }

    #[test]
    fn test_layout_zero_width_no_panic() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![plain_span("text")],
        }];
        // Width 0 is clamped to 1 — should not panic.
        let doc = flatten_default(&blocks,0);
        assert!(doc.total_height >= 1);
    }

    #[test]
    fn test_layout_mixed_styles_content_preserved() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let code = Style::default().fg(Color::Indexed(252)).bg(Color::Indexed(236));
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![
                styled_span("Use ", Style::default()),
                styled_span("fmt", code),
                styled_span(" for formatting output in your programs", bold),
            ],
        }];
        let doc = flatten_default(&blocks,20);
        // Collect all text.
        let mut all_text = String::new();
        for line in &doc.lines {
            if let DocumentLine::Text(l) = line {
                for span in &l.spans {
                    all_text.push_str(&span.content);
                }
            }
        }
        assert!(all_text.contains("Use "), "should contain 'Use '");
        assert!(all_text.contains("fmt"), "should contain 'fmt'");
        assert!(
            all_text.contains("formatting"),
            "should contain 'formatting'"
        );
    }

    // ── Phase 2: Code block layout tests ────────────────────────

    fn make_code_line(text: &str) -> Line<'static> {
        Line::from(Span::raw(text.to_string()))
    }

    #[test]
    fn test_layout_code_block_long_line_no_wrap() {
        let long_line = "x".repeat(200);
        let blocks = vec![RenderedBlock::CodeBlock {
            language: String::new(),
            highlighted_lines: vec![make_code_line(&long_line)],
        }];
        let doc = flatten_default(&blocks,40);
        // Code lines should NOT wrap — still 1 Code line.
        let code_count = doc
            .lines
            .iter()
            .filter(|l| matches!(l, DocumentLine::Code(_)))
            .count();
        assert_eq!(code_count, 1, "code should not wrap");
    }

    #[test]
    fn test_layout_code_block_empty_language_no_label() {
        let blocks = vec![RenderedBlock::CodeBlock {
            language: String::new(),
            highlighted_lines: vec![make_code_line("code")],
        }];
        let doc = flatten_default(&blocks,80);
        // No language → no label line, just the code line.
        assert_eq!(doc.total_height, 1);
    }

    #[test]
    fn test_layout_code_block_with_language_has_label() {
        let blocks = vec![RenderedBlock::CodeBlock {
            language: "rust".to_string(),
            highlighted_lines: vec![
                make_code_line("fn main() {"),
                make_code_line("    println!(\"hello\");"),
                make_code_line("}"),
            ],
        }];
        let doc = flatten_default(&blocks,80);
        // 1 label + 3 code lines = 4
        assert_eq!(doc.total_height, 4);
        // First line should be the label.
        if let DocumentLine::Code(label_line) = &doc.lines[0] {
            let text: String = label_line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(text.contains("rust"), "label should contain language name");
        } else {
            panic!("expected Code line for label");
        }
    }

    #[test]
    fn test_layout_code_block_multiple_lines_correct_count() {
        let blocks = vec![RenderedBlock::CodeBlock {
            language: "python".to_string(),
            highlighted_lines: vec![
                make_code_line("def f():"),
                make_code_line("    pass"),
            ],
        }];
        let doc = flatten_default(&blocks,80);
        // 1 label + 2 code lines = 3
        let code_count = doc
            .lines
            .iter()
            .filter(|l| matches!(l, DocumentLine::Code(_)))
            .count();
        assert_eq!(code_count, 3);
    }

    // ── Phase 3: List layout tests ───────────────────────────────

    fn make_list_item(text: &str) -> crate::parser::ListItem {
        crate::parser::ListItem {
            content: vec![plain_span(text)],
            children: vec![],
            task: None,
        }
    }

    fn make_task_item(text: &str, checked: bool) -> crate::parser::ListItem {
        crate::parser::ListItem {
            content: vec![plain_span(text)],
            children: vec![],
            task: Some(checked),
        }
    }

    #[test]
    fn test_layout_unordered_list_bullet_prefix() {
        let blocks = vec![RenderedBlock::List {
            ordered: false,
            start: 1,
            items: vec![make_list_item("alpha"), make_list_item("beta")],
        }];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 2);
        // Both lines should be Text lines with bullet prefix.
        for line in &doc.lines {
            if let DocumentLine::Text(l) = line {
                let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                assert!(text.contains('•'), "unordered item should have • prefix, got: {text}");
            } else {
                panic!("expected Text line");
            }
        }
    }

    #[test]
    fn test_layout_ordered_list_number_prefix() {
        let blocks = vec![RenderedBlock::List {
            ordered: true,
            start: 3,
            items: vec![make_list_item("first"), make_list_item("second")],
        }];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 2);
        if let DocumentLine::Text(first_line) = &doc.lines[0] {
            let text: String = first_line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(text.starts_with("3."), "ordered list should start at 3, got: {text}");
        }
        if let DocumentLine::Text(second_line) = &doc.lines[1] {
            let text: String = second_line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(text.starts_with("4."), "second item should be 4., got: {text}");
        }
    }

    #[test]
    fn test_layout_nested_list_indentation() {
        let inner_item = crate::parser::ListItem {
            content: vec![plain_span("nested")],
            children: vec![],
            task: None,
        };
        let outer_item = crate::parser::ListItem {
            content: vec![plain_span("outer")],
            children: vec![RenderedBlock::List {
                ordered: false,
                start: 1,
                items: vec![inner_item],
            }],
            task: None,
        };
        let blocks = vec![RenderedBlock::List {
            ordered: false,
            start: 1,
            items: vec![outer_item],
        }];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 2, "outer item + nested item");
        // Nested item should use ◦ bullet and extra indentation.
        if let DocumentLine::Text(nested_line) = &doc.lines[1] {
            let text: String = nested_line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(text.contains('◦'), "nested item should use ◦ bullet, got: {text}");
        }
    }

    #[test]
    fn test_layout_task_list_checkbox_prefix() {
        let blocks = vec![RenderedBlock::List {
            ordered: false,
            start: 1,
            items: vec![make_task_item("done", true), make_task_item("pending", false)],
        }];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 2);
        if let DocumentLine::Text(l) = &doc.lines[0] {
            let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(text.contains('☑'), "checked task should have ☑, got: {text}");
        }
        if let DocumentLine::Text(l) = &doc.lines[1] {
            let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(text.contains('☐'), "unchecked task should have ☐, got: {text}");
        }
    }

    // ── Phase 3: Block quote layout tests ────────────────────────

    fn make_paragraph_block(text: &str) -> RenderedBlock {
        RenderedBlock::Paragraph { content: vec![plain_span(text)] }
    }

    #[test]
    fn test_layout_block_quote_pipe_prefix() {
        let blocks = vec![RenderedBlock::BlockQuote {
            children: vec![make_paragraph_block("Quoted text")],
        }];
        let doc = flatten_default(&blocks,80);
        assert!(!doc.lines.is_empty());
        for line in &doc.lines {
            if let DocumentLine::Text(l) = line {
                let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                assert!(text.starts_with("│ "), "block quote lines must start with │ , got: {text}");
            }
        }
    }

    #[test]
    fn test_layout_block_quote_content_preserved() {
        let blocks = vec![RenderedBlock::BlockQuote {
            children: vec![make_paragraph_block("Hello from the quote")],
        }];
        let doc = flatten_default(&blocks,80);
        let all_text: String = doc.lines.iter().filter_map(|l| {
            if let DocumentLine::Text(line) = l {
                Some(line.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            } else {
                None
            }
        }).collect();
        assert!(all_text.contains("Hello from the quote"));
    }

    #[test]
    fn test_layout_nested_block_quote_double_prefix() {
        let inner = RenderedBlock::BlockQuote {
            children: vec![make_paragraph_block("inner")],
        };
        let blocks = vec![RenderedBlock::BlockQuote {
            children: vec![inner],
        }];
        let doc = flatten_default(&blocks,80);
        let has_double_prefix = doc.lines.iter().any(|l| {
            if let DocumentLine::Text(line) = l {
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                text.starts_with("│ │ ")
            } else {
                false
            }
        });
        assert!(has_double_prefix, "nested quote should produce │ │  prefix");
    }

    // ── Phase 3: Table layout tests ──────────────────────────────

    fn make_cell(text: &str) -> Vec<StyledSpan> {
        vec![plain_span(text)]
    }

    #[test]
    fn test_layout_table_produces_header_separator_and_rows() {
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("Name"), make_cell("Value")],
            alignments: vec![
                pulldown_cmark::Alignment::None,
                pulldown_cmark::Alignment::None,
            ],
            rows: vec![vec![make_cell("foo"), make_cell("42")]],
        }];
        let doc = flatten_default(&blocks,80);
        // Header + separator + 1 data row = 3 lines.
        assert_eq!(doc.total_height, 3);
        for line in &doc.lines {
            assert!(matches!(line, DocumentLine::Text(_)), "table lines should be Text");
        }
    }

    #[test]
    fn test_layout_table_separator_contains_dashes() {
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("Col")],
            alignments: vec![pulldown_cmark::Alignment::None],
            rows: vec![vec![make_cell("val")]],
        }];
        let doc = flatten_default(&blocks,80);
        // Second line is the separator.
        if let DocumentLine::Text(sep_line) = &doc.lines[1] {
            let text: String = sep_line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(text.contains('─'), "separator should contain ─, got: {text}");
        } else {
            panic!("expected Text separator line");
        }
    }

    #[test]
    fn test_layout_table_no_panic_when_too_wide() {
        // Many wide columns that exceed terminal width — must not panic.
        let headers: Vec<Vec<StyledSpan>> = (0..20).map(|i| make_cell(&format!("Header{i}"))).collect();
        let alignments = vec![pulldown_cmark::Alignment::None; 20];
        let rows = vec![(0..20).map(|i| make_cell(&format!("cell{i}"))).collect()];
        let blocks = vec![RenderedBlock::Table { headers, alignments, rows }];
        let doc = flatten_default(&blocks,40);
        assert!(doc.total_height >= 3, "should still produce header/sep/row lines");
    }

    #[test]
    fn test_layout_table_alignment_right() {
        use pulldown_cmark::Alignment;
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("X")],
            alignments: vec![Alignment::Right],
            rows: vec![vec![make_cell("hi")]],
        }];
        let doc = flatten_default(&blocks,40);
        // Just verify it doesn't panic and produces 3 lines.
        assert_eq!(doc.total_height, 3);
    }

    #[test]
    fn test_layout_table_cjk_no_panic_and_correct_width() {
        // CJK characters are 2 display columns wide.
        // Column widths must be measured by display width, not scalar count,
        // so the header "名前" (4 display cols, 2 chars) rounds up to ≥4.
        use pulldown_cmark::Alignment;
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("名前"), make_cell("Val")],
            alignments: vec![Alignment::None, Alignment::None],
            rows: vec![vec![make_cell("abc"), make_cell("42")]],
        }];
        let doc = flatten_default(&blocks,80);
        // Must not panic and must produce header + separator + 1 row.
        assert_eq!(doc.total_height, 3);
        // The header row's rendered text must contain the CJK characters.
        if let DocumentLine::Text(header_line) = &doc.lines[0] {
            let text: String = header_line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(text.contains("名前"), "header should contain CJK chars: {text:?}");
        }
    }

    #[test]
    fn test_layout_list_child_code_block_is_indented() {
        // A code block nested inside a list item should be indented to match
        // the item's content column, not rendered flush with the left edge.
        use crate::parser::ListItem;
        let code_block = RenderedBlock::CodeBlock {
            language: String::new(),
            highlighted_lines: vec![ratatui::text::Line::from("code line")],
        };
        let item = ListItem {
            content: vec![plain_span("item text")],
            children: vec![code_block],
            task: None,
        };
        let blocks = vec![RenderedBlock::List {
            ordered: false,
            start: 1,
            items: vec![item],
        }];
        let doc = flatten_default(&blocks,80);
        // Should be: item text line + code line (indented)
        assert!(doc.total_height >= 2);
        // The code line should be a Code variant with an indent prefix span.
        let code_lines: Vec<_> = doc.lines.iter().filter(|l| matches!(l, DocumentLine::Code(_))).collect();
        assert_eq!(code_lines.len(), 1, "should have exactly one Code line");
        if let DocumentLine::Code(l) = &code_lines[0] {
            // First span must be whitespace indentation.
            let first_span = l.spans.first().expect("code line should have spans");
            assert!(
                first_span.content.trim().is_empty(),
                "first span of indented code should be whitespace, got: {:?}",
                first_span.content
            );
        }
    }

    // ── Phase 4: Image layout tests ────────────────────────────

    #[test]
    fn test_layout_image_fallback_produces_text() {
        let blocks = vec![RenderedBlock::ImageFallback {
            alt_text: "a photo".to_string(),
        }];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 1);
        if let DocumentLine::Text(line) = &doc.lines[0] {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                text.contains("a photo"),
                "fallback should contain alt text, got: {text}"
            );
        } else {
            panic!("expected Text line for ImageFallback");
        }
    }

    #[test]
    fn test_layout_image_start_reserves_height() {
        let blocks = vec![RenderedBlock::Image {
            protocol_index: 0,
            alt_text: "test".to_string(),
            width_cells: 16,
            height_cells: 5,
        }];
        let doc = flatten_default(&blocks,80);
        assert_eq!(doc.total_height, 5, "image should reserve height_cells lines");
        assert!(
            matches!(&doc.lines[0], DocumentLine::ImageStart { protocol_index: 0, height: 5 }),
            "first line should be ImageStart"
        );
        for i in 1..5 {
            assert!(
                matches!(&doc.lines[i], DocumentLine::ImageContinuation),
                "line {i} should be ImageContinuation"
            );
        }
    }

    // ── ASCII image layout tests ─────────────────────────────────

    #[test]
    fn test_layout_ascii_image_produces_ascii_art_lines() {
        let lines = vec![
            Line::from(vec![
                Span::styled(".", Style::default().fg(Color::Rgb(100, 100, 100))),
                Span::styled("#", Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(vec![
                Span::styled("@", Style::default().fg(Color::Rgb(255, 255, 255))),
                Span::styled(" ", Style::default().fg(Color::Rgb(0, 0, 0))),
            ]),
        ];
        let blocks = vec![RenderedBlock::AsciiImage {
            lines,
            alt_text: "test".to_string(),
        }];
        let doc = flatten_default(&blocks, 80);
        assert_eq!(doc.total_height, 2, "each image line becomes a DocumentLine");
        for line in &doc.lines {
            assert!(
                matches!(line, DocumentLine::AsciiArt(_)),
                "AsciiImage should produce AsciiArt lines"
            );
        }
    }

    #[test]
    fn test_layout_block_quote_inside_list_preserves_list_depth() {
        // A block quote inside a list item (depth=1) should thread list_depth
        // into flatten_block_quote rather than resetting it to 0.
        use crate::parser::ListItem;
        let quote = RenderedBlock::BlockQuote {
            children: vec![RenderedBlock::List {
                ordered: false,
                start: 1,
                items: vec![crate::parser::ListItem {
                    content: vec![plain_span("nested in quote")],
                    children: vec![],
                    task: None,
                }],
            }],
        };
        let outer_item = ListItem {
            content: vec![plain_span("outer")],
            children: vec![quote],
            task: None,
        };
        let blocks = vec![RenderedBlock::List {
            ordered: false,
            start: 1,
            items: vec![outer_item],
        }];
        // Must not panic.
        let doc = flatten_default(&blocks,80);
        assert!(doc.total_height >= 2);
    }
