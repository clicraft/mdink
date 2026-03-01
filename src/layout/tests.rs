    use super::*;
    use crate::parser::{StyledSpan, TableCell};
    use ratatui::style::{Color, Modifier, Style};

    /// Wrapper that passes the default theme so existing tests don't need updating.
    fn flatten_default(blocks: &[RenderedBlock], width: u16) -> PreRenderedDocument {
        flatten(blocks, width, &crate::theme::default_theme())
    }

    fn plain_span(text: &str) -> StyledSpan {
        StyledSpan {
            text: text.to_string(),
            style: Style::default(),
            url: None,
        }
    }

    fn styled_span(text: &str, style: Style) -> StyledSpan {
        StyledSpan {
            text: text.to_string(),
            style,
            url: None,
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

    fn make_cell(text: &str) -> TableCell {
        TableCell::Text(vec![plain_span(text)])
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
        let headers: Vec<TableCell> = (0..20).map(|i| make_cell(&format!("Header{i}"))).collect();
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
    fn test_table_cell_wraps_long_text() {
        use pulldown_cmark::Alignment;
        let long_text = "This is a long description that should wrap to multiple lines";
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("Key"), make_cell("Description")],
            alignments: vec![Alignment::None, Alignment::None],
            rows: vec![vec![make_cell("A"), make_cell(long_text)]],
        }];
        // Width 40: two columns share ~17 cols each (minus separators).
        // "This is a long description..." at 60 chars must wrap.
        let doc = flatten_default(&blocks, 40);
        // Header (1 line) + separator (1) + data row (>1 line) = >3 total.
        assert!(
            doc.total_height > 3,
            "long cell should wrap to multiple lines, got {} lines",
            doc.total_height
        );
        // Verify the full text is present (not truncated).
        let all_text: String = doc
            .lines
            .iter()
            .filter_map(|l| match l {
                DocumentLine::Text(line) => {
                    Some(line.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
                }
                _ => None,
            })
            .collect();
        // All words from the long text should appear across the wrapped lines.
        for word in long_text.split_whitespace() {
            assert!(all_text.contains(word), "word '{word}' missing from table output");
        }
    }

    #[test]
    fn test_table_wrapped_cell_aligned_right() {
        use pulldown_cmark::Alignment;
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("Val")],
            alignments: vec![Alignment::Right],
            rows: vec![vec![make_cell("a fairly long cell value here")]],
        }];
        let doc = flatten_default(&blocks, 20);
        // With right alignment, each wrapped line should have leading spaces.
        // Skip header (line 0) and separator (line 1), check data row lines.
        for line in doc.lines.iter().skip(2) {
            if let DocumentLine::Text(l) = line {
                let first_content = &l.spans[0].content;
                // Right-aligned lines should start with padding spaces.
                assert!(
                    first_content.starts_with(' ') || first_content.trim().is_empty(),
                    "right-aligned line should have leading space: {first_content:?}"
                );
            }
        }
    }

    #[test]
    fn test_table_multi_row_different_heights() {
        use pulldown_cmark::Alignment;
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("A"), make_cell("B")],
            alignments: vec![Alignment::None, Alignment::None],
            rows: vec![vec![
                make_cell("short"),
                make_cell("this cell has much longer text that must wrap to several lines"),
            ]],
        }];
        let doc = flatten_default(&blocks, 40);
        // The row should have multiple lines; the short cell should be padded.
        assert!(doc.total_height > 3, "multi-height row should produce extra lines");
        // All data row lines (after header + separator) should contain the " │ " separator.
        for line in doc.lines.iter().skip(2) {
            if let DocumentLine::Text(l) = line {
                let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                assert!(text.contains("│"), "each row line should have column separator: {text:?}");
            }
        }
    }

    #[test]
    fn test_table_cell_single_line_no_change() {
        use pulldown_cmark::Alignment;
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("K"), make_cell("V")],
            alignments: vec![Alignment::None, Alignment::None],
            rows: vec![vec![make_cell("a"), make_cell("b")]],
        }];
        let doc = flatten_default(&blocks, 80);
        // Short cells that fit: header + separator + 1 data row = exactly 3.
        assert_eq!(doc.total_height, 3, "short cells should not wrap");
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

    // ── Outline heading collection tests ─────────────────────────

    #[test]
    fn test_flatten_collects_h1_h2_h3() {
        let blocks = vec![
            RenderedBlock::Heading { level: 1, content: vec![plain_span("Title")] },
            RenderedBlock::Heading { level: 2, content: vec![plain_span("Section")] },
            RenderedBlock::Heading { level: 3, content: vec![plain_span("Sub")] },
            RenderedBlock::Heading { level: 4, content: vec![plain_span("Deep")] },
        ];
        let doc = flatten_default(&blocks, 80);
        assert_eq!(doc.headings.len(), 3, "h4 should not be collected");
        assert_eq!(doc.headings[0].level, 1);
        assert_eq!(doc.headings[0].text, "Title");
        assert_eq!(doc.headings[1].level, 2);
        assert_eq!(doc.headings[1].text, "Section");
        assert_eq!(doc.headings[2].level, 3);
        assert_eq!(doc.headings[2].text, "Sub");
    }

    #[test]
    fn test_flatten_no_headings() {
        let blocks = vec![
            RenderedBlock::Paragraph { content: vec![plain_span("Just text")] },
        ];
        let doc = flatten_default(&blocks, 80);
        assert!(doc.headings.is_empty());
    }

    #[test]
    fn test_heading_plain_text_strips_formatting() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let italic = Style::default().add_modifier(Modifier::ITALIC);
        let blocks = vec![
            RenderedBlock::Heading {
                level: 1,
                content: vec![
                    styled_span("Bold", bold),
                    plain_span(" and "),
                    styled_span("italic", italic),
                ],
            },
        ];
        let doc = flatten_default(&blocks, 80);
        assert_eq!(doc.headings.len(), 1);
        assert_eq!(doc.headings[0].text, "Bold and italic");
    }

    #[test]
    fn test_heading_line_index_accounts_for_spacing() {
        let blocks = vec![
            RenderedBlock::Paragraph { content: vec![plain_span("First")] },
            RenderedBlock::Heading { level: 1, content: vec![plain_span("Title")] },
        ];
        let doc = flatten_default(&blocks, 80);
        assert_eq!(doc.headings.len(), 1);
        // Paragraph (1 line) + Empty (1 line) = heading starts at index 2.
        assert_eq!(doc.headings[0].line_index, 2);
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

    // ── Table with image cell tests ─────────────────────────────

    #[test]
    fn test_layout_table_with_ascii_image_cell() {
        // One text cell + one AsciiImage cell (3 lines).
        // Expected: header(1) + sep(1) + row(3) = 5 lines total.
        let image_lines = vec![
            Line::from(vec![
                Span::styled(".", Style::default().fg(Color::Rgb(100, 100, 100))),
                Span::styled("#", Style::default().fg(Color::Rgb(200, 200, 200))),
            ]),
            Line::from(vec![
                Span::styled("@", Style::default().fg(Color::Rgb(255, 255, 255))),
                Span::styled(" ", Style::default().fg(Color::Rgb(0, 0, 0))),
            ]),
            Line::from(vec![
                Span::styled("*", Style::default().fg(Color::Rgb(128, 128, 128))),
                Span::styled("~", Style::default().fg(Color::Rgb(64, 64, 64))),
            ]),
        ];
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("Text"), make_cell("Image")],
            alignments: vec![
                pulldown_cmark::Alignment::None,
                pulldown_cmark::Alignment::None,
            ],
            rows: vec![vec![
                make_cell("hello"),
                TableCell::Block(RenderedBlock::AsciiImage {
                    lines: image_lines,
                    alt_text: "test".to_string(),
                }),
            ]],
        }];
        let doc = flatten_default(&blocks, 80);
        // header(1) + sep(1) + blank_sep(1) + row(3, because image is 3 lines) = 6
        assert_eq!(
            doc.total_height, 6,
            "table with 3-line image cell should produce 6 lines, got {}",
            doc.total_height
        );
        // All lines should be Text (table output is always Text lines).
        for (i, line) in doc.lines.iter().enumerate() {
            assert!(
                matches!(line, DocumentLine::Text(_)),
                "line {i} should be Text"
            );
        }
    }

    #[test]
    fn test_layout_table_image_truncated_to_column() {
        // Image wider than column width — spans should be truncated.
        let wide_line = Line::from(vec![
            Span::styled("ABCDEFGHIJ", Style::default().fg(Color::Rgb(255, 0, 0))),
            Span::styled("KLMNOPQRST", Style::default().fg(Color::Rgb(0, 255, 0))),
        ]);
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("Img")],
            alignments: vec![pulldown_cmark::Alignment::None],
            rows: vec![vec![TableCell::Block(RenderedBlock::AsciiImage {
                lines: vec![wide_line],
                alt_text: "wide".to_string(),
            })]],
        }];
        // Use a narrow terminal (15 cols) so the image must be truncated.
        let doc = flatten_default(&blocks, 15);
        assert_eq!(doc.total_height, 3, "header + sep + 1-line image row");
        // The image row (line index 2) should not exceed the column width.
        if let DocumentLine::Text(row_line) = &doc.lines[2] {
            let row_width: usize = row_line.spans.iter().map(|s| s.content.width()).sum();
            assert!(
                row_width <= 15,
                "image row should be truncated to fit terminal width, got {row_width}"
            );
        } else {
            panic!("expected Text line for image row");
        }
    }

    #[test]
    fn test_layout_table_multiline_row_width_consistency() {
        // Core invariant: every terminal line within one multi-line table row
        // must have the same total display width. If this fails, the " │ "
        // column separators would visually zig-zag.
        //
        // Build a synthetic 2-column table:
        //   col 0: text cell "hello"  (1 line)
        //   col 1: 5-line image       (5 lines)
        // The text cell gets 4 padding lines; all 5 lines must match in width.
        let image_lines: Vec<Line<'static>> = (0..5)
            .map(|i| {
                Line::from(vec![
                    Span::styled(
                        "#",
                        Style::default().fg(Color::Rgb(50 * i, 100, 200)),
                    ),
                    Span::styled(
                        "@",
                        Style::default().fg(Color::Rgb(200, 50 * i, 100)),
                    ),
                    Span::styled(
                        ".",
                        Style::default().fg(Color::Rgb(100, 200, 50 * i)),
                    ),
                ])
            })
            .collect();

        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("Text"), make_cell("Image")],
            alignments: vec![
                pulldown_cmark::Alignment::None,
                pulldown_cmark::Alignment::None,
            ],
            rows: vec![vec![
                make_cell("hello"),
                TableCell::Block(RenderedBlock::AsciiImage {
                    lines: image_lines,
                    alt_text: "synth".to_string(),
                }),
            ]],
        }];
        let doc = flatten_default(&blocks, 60);
        // header(1) + sep(1) + blank_sep(1) + row(5) = 8
        assert_eq!(doc.total_height, 8);

        // Measure every line's display width in the body row (indices 3..8).
        let mut widths: Vec<usize> = Vec::new();
        for line_idx in 3..8 {
            if let DocumentLine::Text(line) = &doc.lines[line_idx] {
                let w: usize = line.spans.iter().map(|s| s.content.width()).sum();
                widths.push(w);
            }
        }
        assert_eq!(widths.len(), 5, "body row should be 5 lines");

        // All 5 lines must have the same display width.
        let first_width = widths[0];
        for (i, &w) in widths.iter().enumerate() {
            assert_eq!(
                w, first_width,
                "row line {i} has width {w}, expected {first_width} (same as first line)"
            );
        }

        // The header line must also have the same width as body lines.
        if let DocumentLine::Text(header_line) = &doc.lines[0] {
            let hw: usize = header_line.spans.iter().map(|s| s.content.width()).sum();
            assert_eq!(
                hw, first_width,
                "header width {hw} should match body row width {first_width}"
            );
        }

        // First body content line (index 3, after blank sep) should have RGB-colored spans.
        if let DocumentLine::Text(first_row) = &doc.lines[3] {
            let has_rgb = first_row.spans.iter().any(|s| {
                matches!(s.style.fg, Some(Color::Rgb(_, _, _)))
            });
            assert!(has_rgb, "image cell should preserve RGB colors in layout output");
        }
    }

    #[test]
    fn test_layout_table_two_images_different_heights() {
        // Two image cells with different heights in the same row.
        // Shorter image gets padding; all lines must have consistent width.
        let short_img: Vec<Line<'static>> = (0..3)
            .map(|_| Line::from(vec![Span::raw("AB".to_string())]))
            .collect();
        let tall_img: Vec<Line<'static>> = (0..7)
            .map(|_| Line::from(vec![Span::raw("XY".to_string())]))
            .collect();

        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("Short"), make_cell("Tall")],
            alignments: vec![
                pulldown_cmark::Alignment::None,
                pulldown_cmark::Alignment::None,
            ],
            rows: vec![vec![
                TableCell::Block(RenderedBlock::AsciiImage {
                    lines: short_img,
                    alt_text: "s".to_string(),
                }),
                TableCell::Block(RenderedBlock::AsciiImage {
                    lines: tall_img,
                    alt_text: "t".to_string(),
                }),
            ]],
        }];
        let doc = flatten_default(&blocks, 60);
        // header(1) + sep(1) + blank_sep(1) + row(7) = 10
        assert_eq!(doc.total_height, 10, "row height should be max(3,7) = 7");

        // Check width consistency on all body-row lines (skip blank sep at index 2).
        let body_widths: Vec<usize> = (3..10)
            .filter_map(|i| {
                if let DocumentLine::Text(line) = &doc.lines[i] {
                    Some(line.spans.iter().map(|s| s.content.width()).sum())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(body_widths.len(), 7);
        let first = body_widths[0];
        for (i, &w) in body_widths.iter().enumerate() {
            assert_eq!(w, first, "body line {i} has width {w}, expected {first}");
        }

        // Lines 3..6 (indices 5..9 from doc, but line_idx 3..7 in row)
        // should have padding in the short-image column but content in the
        // tall-image column.
        for row_line_idx in 3..7usize {
            let doc_idx = row_line_idx + 2; // offset by header + sep
            if let DocumentLine::Text(line) = &doc.lines[doc_idx] {
                let full: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                // Short image column (left) should be blank spaces.
                // Tall image column (right) should still have "XY".
                assert!(
                    full.contains("XY"),
                    "row line {row_line_idx}: tall image should still have content, got: {full:?}"
                );
            }
        }
    }

    #[test]
    fn test_layout_table_image_fallback_cell() {
        // An ImageFallback cell should render as "[image: alt]" text.
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("Pic"), make_cell("Name")],
            alignments: vec![
                pulldown_cmark::Alignment::None,
                pulldown_cmark::Alignment::None,
            ],
            rows: vec![vec![
                TableCell::Block(RenderedBlock::ImageFallback {
                    alt_text: "photo".to_string(),
                }),
                make_cell("sunset"),
            ]],
        }];
        let doc = flatten_default(&blocks, 60);
        // header(1) + sep(1) + row(1) = 3  (fallback is single-line)
        assert_eq!(doc.total_height, 3);
        if let DocumentLine::Text(row_line) = &doc.lines[2] {
            let text: String = row_line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                text.contains("[image: photo]"),
                "fallback cell should render as '[image: photo]', got: {text:?}"
            );
            assert!(
                text.contains("sunset"),
                "text cell should contain 'sunset', got: {text:?}"
            );
        }
    }

    #[test]
    fn test_layout_table_empty_ascii_image_cell() {
        // An AsciiImage with 0 lines should get the minimum-height guard
        // (1 line of padding) instead of collapsing the row to 0 height.
        let blocks = vec![RenderedBlock::Table {
            headers: vec![make_cell("Img")],
            alignments: vec![pulldown_cmark::Alignment::None],
            rows: vec![vec![TableCell::Block(RenderedBlock::AsciiImage {
                lines: vec![],
                alt_text: "empty".to_string(),
            })]],
        }];
        let doc = flatten_default(&blocks, 40);
        // header(1) + sep(1) + row(1 minimum) = 3
        assert_eq!(
            doc.total_height, 3,
            "empty image should produce minimum 1-line row"
        );
    }

    // ── OSC 8 link layout tests ─────────────────────────────────────────

    #[test]
    fn test_layout_link_url_renders_plain_text() {
        // OSC 8 is disabled (ratatui can't pass through raw escapes),
        // so links render as plain styled text without escape sequences.
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![StyledSpan {
                text: "click".to_string(),
                style: Style::default(),
                url: Some("https://example.com".to_string()),
            }],
        }];
        let doc = flatten_default(&blocks, 80);
        assert_eq!(doc.total_height, 1);
        match &doc.lines[0] {
            DocumentLine::Text(line) => {
                let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                assert!(full_text.contains("click"), "span text should contain visible text");
                assert!(
                    !full_text.contains("\x1b]8"),
                    "OSC 8 sequences should not be present (disabled)"
                );
            }
            _ => panic!("expected Text line"),
        }
    }

    #[test]
    fn test_layout_no_url_no_osc8() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![plain_span("hello")],
        }];
        let doc = flatten_default(&blocks, 80);
        match &doc.lines[0] {
            DocumentLine::Text(line) => {
                let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                assert!(
                    !full_text.contains("\x1b]8"),
                    "plain text should not contain OSC 8 sequences"
                );
            }
            _ => panic!("expected Text line"),
        }
    }

    #[test]
    fn test_layout_url_ignored_does_not_leak_into_text() {
        // With OSC 8 disabled, the URL should not appear in the output at all.
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![StyledSpan {
                text: "x".to_string(),
                style: Style::default(),
                url: Some("https://evil.com/\x1b[31mred".to_string()),
            }],
        }];
        let doc = flatten_default(&blocks, 80);
        match &doc.lines[0] {
            DocumentLine::Text(line) => {
                let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                assert_eq!(full_text, "x", "only visible text should be present");
            }
            _ => panic!("expected Text line"),
        }
    }

    // ── Escape sequence leak prevention tests ───────────────────────────

    /// Helper: asserts that no span in any line contains raw escape sequences.
    fn assert_no_escape_sequences(doc: &PreRenderedDocument) {
        for (i, line) in doc.lines.iter().enumerate() {
            match line {
                DocumentLine::Text(l) | DocumentLine::Code(l) | DocumentLine::AsciiArt(l) => {
                    for span in &l.spans {
                        let text = span.content.as_ref();
                        assert!(
                            !text.contains("\x1b]8"),
                            "line {i}: span contains raw OSC 8 open: {text:?}"
                        );
                        assert!(
                            !text.contains("]8;;"),
                            "line {i}: span contains partial OSC 8 pattern ']8;;': {text:?}"
                        );
                        assert!(
                            !text.contains("\x1b\\"),
                            "line {i}: span contains raw ST (ESC backslash): {text:?}"
                        );
                    }
                }
                DocumentLine::Empty | DocumentLine::Rule => {}
                DocumentLine::ImageStart { .. } | DocumentLine::ImageContinuation => {}
            }
        }
    }

    #[test]
    fn test_escape_no_leak_standalone_link() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![
                plain_span("A standalone link: "),
                StyledSpan {
                    text: "Rust documentation".to_string(),
                    style: Style::default().add_modifier(Modifier::ITALIC),
                    url: Some("https://doc.rust-lang.org".to_string()),
                },
            ],
        }];
        let doc = flatten_default(&blocks, 80);
        assert_no_escape_sequences(&doc);
    }

    #[test]
    fn test_escape_no_leak_multiple_links() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![
                plain_span("Links: "),
                StyledSpan {
                    text: "GitHub".to_string(),
                    style: Style::default().add_modifier(Modifier::ITALIC),
                    url: Some("https://github.com".to_string()),
                },
                plain_span(" and "),
                StyledSpan {
                    text: "crates.io".to_string(),
                    style: Style::default().add_modifier(Modifier::ITALIC),
                    url: Some("https://crates.io".to_string()),
                },
                plain_span(" and "),
                StyledSpan {
                    text: "docs.rs".to_string(),
                    style: Style::default().add_modifier(Modifier::ITALIC),
                    url: Some("https://docs.rs".to_string()),
                },
                plain_span("."),
            ],
        }];
        let doc = flatten_default(&blocks, 80);
        assert_no_escape_sequences(&doc);
    }

    #[test]
    fn test_escape_no_leak_link_with_bold_inside() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![
                StyledSpan {
                    text: "Important".to_string(),
                    style: Style::default()
                        .add_modifier(Modifier::ITALIC)
                        .add_modifier(Modifier::BOLD),
                    url: Some("https://example.com/release".to_string()),
                },
                StyledSpan {
                    text: " release notes".to_string(),
                    style: Style::default().add_modifier(Modifier::ITALIC),
                    url: Some("https://example.com/release".to_string()),
                },
            ],
        }];
        let doc = flatten_default(&blocks, 80);
        assert_no_escape_sequences(&doc);
    }

    #[test]
    fn test_escape_no_leak_link_with_code_inside() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![StyledSpan {
                text: "cargo install".to_string(),
                style: Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::ITALIC),
                url: Some("https://doc.rust-lang.org/cargo/".to_string()),
            }],
        }];
        let doc = flatten_default(&blocks, 80);
        assert_no_escape_sequences(&doc);
    }

    #[test]
    fn test_escape_no_leak_bare_url_as_link_text() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![StyledSpan {
                text: "https://example.com".to_string(),
                style: Style::default().add_modifier(Modifier::ITALIC),
                url: Some("https://example.com".to_string()),
            }],
        }];
        let doc = flatten_default(&blocks, 80);
        assert_no_escape_sequences(&doc);
    }

    #[test]
    fn test_escape_no_leak_link_wrapping_across_lines() {
        let blocks = vec![RenderedBlock::Paragraph {
            content: vec![
                plain_span("Start "),
                StyledSpan {
                    text: "a very long link text that should wrap across lines to verify italic modifier is preserved through wrapping".to_string(),
                    style: Style::default().add_modifier(Modifier::ITALIC),
                    url: Some("https://example.com".to_string()),
                },
                plain_span(" end."),
            ],
        }];
        let doc = flatten_default(&blocks, 40);
        assert!(doc.total_height > 1, "should wrap to multiple lines");
        assert_no_escape_sequences(&doc);
    }

    #[test]
    fn test_escape_no_leak_full_font_slots_document() {
        use std::sync::LazyLock;
        static HL: LazyLock<crate::highlight::Highlighter> =
            LazyLock::new(crate::highlight::Highlighter::new);

        // Integration test: parse the full font-slots.md and verify no
        // escape sequences leak into any span in the layout output.
        let source = include_str!("../../testdata/font-slots.md");
        let theme = crate::theme::default_theme();
        let mut im = crate::images::ImageManager::new(
            std::path::PathBuf::from("testdata"),
            None,
            80,
            true,  // no_images — skip image loading
            false,
        );
        let blocks = crate::parser::parse(source, &HL, &mut im, &theme);
        let doc = crate::layout::flatten(&blocks, 80, &theme);
        assert_no_escape_sequences(&doc);
    }
