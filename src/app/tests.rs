    use super::*;
    use crate::layout::{DocumentLine, HeadingEntry, PreRenderedDocument};

    fn make_doc(line_count: usize) -> PreRenderedDocument {
        let lines = (0..line_count).map(|_| DocumentLine::Empty).collect();
        PreRenderedDocument {
            lines,
            total_height: line_count,
            headings: Vec::new(),
        }
    }

    fn make_app(doc_lines: usize, viewport: usize) -> App {
        let mut app = App::new(
            make_doc(doc_lines),
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
        );
        app.viewport_height = viewport;
        app
    }

    #[test]
    fn test_app_scroll_down_clamped() {
        let mut app = make_app(10, 5);
        app.scroll_down(100);
        // max_scroll = 10 - 5 = 5
        assert_eq!(app.scroll_offset, 5);
    }

    #[test]
    fn test_app_scroll_up_floor_at_zero() {
        let mut app = make_app(10, 5);
        app.scroll_offset = 2;
        app.scroll_up(100);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_app_max_scroll_short_document() {
        let app = make_app(3, 10);
        // Document shorter than viewport → max_scroll is 0.
        assert_eq!(app.max_scroll(), 0);
    }

    #[test]
    fn test_app_max_scroll_exact_fit() {
        let app = make_app(10, 10);
        assert_eq!(app.max_scroll(), 0);
    }

    #[test]
    fn test_app_visible_range_at_top() {
        let app = make_app(20, 5);
        assert_eq!(app.visible_range(), 0..5);
    }

    #[test]
    fn test_app_visible_range_at_bottom() {
        let mut app = make_app(20, 5);
        app.scroll_to_bottom();
        assert_eq!(app.visible_range(), 15..20);
    }

    #[test]
    fn test_app_visible_range_short_document() {
        let app = make_app(3, 10);
        // Document is only 3 lines but viewport is 10.
        assert_eq!(app.visible_range(), 0..3);
    }

    #[test]
    fn test_app_scroll_percent_at_top() {
        let app = make_app(20, 5);
        assert_eq!(app.scroll_percent(), 0);
    }

    #[test]
    fn test_app_scroll_percent_at_bottom() {
        let mut app = make_app(20, 5);
        app.scroll_to_bottom();
        assert_eq!(app.scroll_percent(), 100);
    }

    #[test]
    fn test_app_scroll_percent_short_document() {
        let app = make_app(3, 10);
        // Document fits in viewport → 100%.
        assert_eq!(app.scroll_percent(), 100);
    }

    #[test]
    fn test_app_scroll_percent_zero_lines() {
        let app = make_app(0, 10);
        assert_eq!(app.scroll_percent(), 100);
    }

    #[test]
    fn test_app_scroll_percent_one_line() {
        let app = make_app(1, 10);
        assert_eq!(app.scroll_percent(), 100);
    }

    #[test]
    fn test_app_scroll_to_top() {
        let mut app = make_app(20, 5);
        app.scroll_offset = 10;
        app.scroll_to_top();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_app_scroll_to_bottom() {
        let mut app = make_app(20, 5);
        app.scroll_to_bottom();
        assert_eq!(app.scroll_offset, 15);
    }

    #[test]
    fn test_app_handle_key_quit_q() {
        let mut app = make_app(10, 5);
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty());
        app.handle_key(key);
        assert!(app.quit);
    }

    #[test]
    fn test_app_handle_key_quit_esc() {
        let mut app = make_app(10, 5);
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
        app.handle_key(key);
        assert!(app.quit);
    }

    #[test]
    fn test_app_handle_key_scroll_j() {
        let mut app = make_app(20, 5);
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
        app.handle_key(key);
        assert_eq!(app.scroll_offset, 1);
    }

    #[test]
    fn test_app_handle_key_refresh_r() {
        let mut app = make_app(10, 5);
        assert!(!app.refresh_requested);
        let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::empty());
        app.handle_key(key);
        assert!(app.refresh_requested);
        assert!(!app.quit, "r should not quit");
    }

    #[test]
    fn test_app_handle_key_scroll_k() {
        let mut app = make_app(20, 5);
        app.scroll_offset = 5;
        let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
        app.handle_key(key);
        assert_eq!(app.scroll_offset, 4);
    }

    // ── Outline tests ────────────────────────────────────────────────

    fn make_doc_with_headings(line_count: usize, headings: Vec<HeadingEntry>) -> PreRenderedDocument {
        let lines = (0..line_count).map(|_| DocumentLine::Empty).collect();
        PreRenderedDocument {
            lines,
            total_height: line_count,
            headings,
        }
    }

    fn make_app_with_headings(doc_lines: usize, viewport: usize, headings: Vec<HeadingEntry>) -> App {
        let mut app = App::new(
            make_doc_with_headings(doc_lines, headings),
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
        );
        app.viewport_height = viewport;
        app
    }

    fn sample_headings() -> Vec<HeadingEntry> {
        vec![
            HeadingEntry { level: 1, text: "Intro".to_string(), line_index: 0 },
            HeadingEntry { level: 2, text: "Details".to_string(), line_index: 5 },
            HeadingEntry { level: 3, text: "Sub".to_string(), line_index: 10 },
        ]
    }

    #[test]
    fn test_outline_toggle() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        assert!(app.outline.is_none());

        // Toggle on.
        let key = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty());
        app.handle_key(key);
        assert!(app.outline.is_some());
        assert!(app.needs_reflatten);

        // Reset flag, toggle off.
        app.needs_reflatten = false;
        app.handle_key(key);
        assert!(app.outline.is_none());
        assert!(app.needs_reflatten);
    }

    #[test]
    fn test_outline_empty_headings_no_toggle() {
        let mut app = make_app(20, 5);
        let key = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty());
        app.handle_key(key);
        // Should remain None when there are no headings.
        assert!(app.outline.is_none());
        // No state changed, so no wasteful reflatten should be triggered.
        assert!(!app.needs_reflatten);
    }

    #[test]
    fn test_outline_tab_nav() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        // Open outline.
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));
        assert_eq!(app.outline.as_ref().unwrap().selected, 0);

        // Tab forward.
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.outline.as_ref().unwrap().selected, 1);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.outline.as_ref().unwrap().selected, 2);

        // Tab wraps to 0.
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.outline.as_ref().unwrap().selected, 0);
    }

    #[test]
    fn test_outline_shift_tab_nav() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));

        // Shift+Tab wraps to last.
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.outline.as_ref().unwrap().selected, 2);

        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.outline.as_ref().unwrap().selected, 1);
    }

    #[test]
    fn test_outline_jump() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));

        // Navigate to second heading and jump.
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        // pending_jump should be set to the heading index (1), not the line index.
        // main.rs resolves this to document.headings[1].line_index after reflatten.
        assert_eq!(app.pending_jump, Some(1));
    }

    #[test]
    fn test_outline_esc_closes() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));
        assert!(app.outline.is_some());

        // Esc closes outline (does not quit).
        app.needs_reflatten = false;
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert!(app.outline.is_none());
        assert!(app.needs_reflatten);
        assert!(!app.quit, "Esc with outline open should not quit");
    }

    #[test]
    fn test_outline_single_heading_nav_wraps_to_zero() {
        let headings = vec![
            HeadingEntry { level: 1, text: "Only".to_string(), line_index: 0 },
        ];
        let mut app = make_app_with_headings(20, 5, headings);
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));
        assert_eq!(app.outline.as_ref().unwrap().selected, 0);

        // Tab wraps back to 0.
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.outline.as_ref().unwrap().selected, 0);

        // Shift+Tab also wraps back to 0.
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.outline.as_ref().unwrap().selected, 0);
    }

    #[test]
    fn test_outline_jk_still_scroll() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));

        // j/k should still scroll the document.
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
        assert_eq!(app.scroll_offset, 1);
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()));
        assert_eq!(app.scroll_offset, 0);
    }

    // ── Outline resize tests ────────────────────────────────────────

    #[test]
    fn test_outline_grow() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));
        app.needs_reflatten = false;

        app.handle_key(KeyEvent::new(KeyCode::Char('>'), KeyModifiers::empty()));
        assert_eq!(app.outline_width_percent, Some(27)); // 25 default + 2
        assert!(app.needs_reflatten);
    }

    #[test]
    fn test_outline_shrink() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));
        app.needs_reflatten = false;

        app.handle_key(KeyEvent::new(KeyCode::Char('<'), KeyModifiers::empty()));
        assert_eq!(app.outline_width_percent, Some(23)); // 25 default - 2
        assert!(app.needs_reflatten);
    }

    #[test]
    fn test_outline_grow_capped_at_33() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));
        app.outline_width_percent = Some(32);
        app.needs_reflatten = false;

        app.handle_key(KeyEvent::new(KeyCode::Char('>'), KeyModifiers::empty()));
        assert_eq!(app.outline_width_percent, Some(33));
    }

    #[test]
    fn test_outline_shrink_floored_at_10() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));
        app.outline_width_percent = Some(11);
        app.needs_reflatten = false;

        app.handle_key(KeyEvent::new(KeyCode::Char('<'), KeyModifiers::empty()));
        assert_eq!(app.outline_width_percent, Some(10));
    }

    #[test]
    fn test_outline_resize_without_outline_is_noop() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        app.handle_key(KeyEvent::new(KeyCode::Char('>'), KeyModifiers::empty()));
        assert!(app.outline_width_percent.is_none());
        assert!(!app.needs_reflatten);
    }

    #[test]
    fn test_outline_panel_cols() {
        let app = make_app_with_headings(20, 5, sample_headings());
        // Default: 25%. Terminal width 120 -> 30 columns. 120/3 = 40. min(30,40) = 30.
        assert_eq!(app.outline_panel_cols(120), 30);
        // Terminal width 60 -> 15 columns. 60/3 = 20. min(15,20) = 15.
        assert_eq!(app.outline_panel_cols(60), 15);
    }

    #[test]
    fn test_outline_panel_cols_hard_cap() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        // Override to 33%. Terminal width 120 -> 39. 120/3 = 40. min(39,40) = 39.
        app.outline_width_percent = Some(33);
        assert_eq!(app.outline_panel_cols(120), 39);
    }

    #[test]
    fn test_outline_width_persists_across_toggle() {
        let mut app = make_app_with_headings(20, 5, sample_headings());
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));
        app.needs_reflatten = false;

        // Resize.
        app.handle_key(KeyEvent::new(KeyCode::Char('>'), KeyModifiers::empty()));
        assert_eq!(app.outline_width_percent, Some(27));

        // Close and reopen.
        app.needs_reflatten = false;
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));

        // Override should persist.
        assert_eq!(app.outline_width_percent, Some(27));
    }

    // ── Search tests ────────────────────────────────────────────────

    use ratatui::text::{Line, Span};

    /// Creates a document with text lines for search testing.
    fn make_doc_with_text(texts: &[&str]) -> PreRenderedDocument {
        let lines: Vec<DocumentLine> = texts
            .iter()
            .map(|t| DocumentLine::Text(Line::from(Span::raw(t.to_string()))))
            .collect();
        let total_height = lines.len();
        PreRenderedDocument {
            lines,
            total_height,
            headings: Vec::new(),
        }
    }

    fn make_searchable_app(texts: &[&str], viewport: usize) -> App {
        let mut app = App::new(
            make_doc_with_text(texts),
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
        );
        app.viewport_height = viewport;
        app
    }

    #[test]
    fn test_search_slash_enters_search_mode() {
        let mut app = make_app(10, 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        assert!(app.search.is_some());
        assert!(app.search.as_ref().unwrap().active);
        assert!(app.search.as_ref().unwrap().query.is_empty());
        assert!(!app.quit);
    }

    #[test]
    fn test_search_typing_appends_chars() {
        let mut app = make_app(10, 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert_eq!(app.search.as_ref().unwrap().query, "hi");
        assert!(app.search.as_ref().unwrap().active);
    }

    #[test]
    fn test_search_backspace_deletes_char() {
        let mut app = make_app(10, 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert_eq!(app.search.as_ref().unwrap().query, "a");
    }

    #[test]
    fn test_search_backspace_on_empty_cancels() {
        let mut app = make_app(10, 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        // Backspace on empty query cancels search.
        app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert!(app.search.is_none());
    }

    #[test]
    fn test_search_esc_cancels() {
        let mut app = make_app(10, 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert!(app.search.is_none());
        assert!(!app.quit, "Esc in search mode should not quit");
    }

    #[test]
    fn test_search_enter_executes_search() {
        let mut app = make_searchable_app(
            &["Hello world", "hello again", "nothing here"],
            10,
        );
        // Enter search mode, type "hello", press Enter.
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        let search = app.search.as_ref().unwrap();
        assert!(!search.active);
        assert_eq!(search.query, "hello");
        // Case-insensitive: should find "Hello" in line 0 and "hello" in line 1.
        assert_eq!(search.matches.len(), 2);
        assert_eq!(search.matches[0].line_index, 0);
        assert_eq!(search.matches[0].byte_start, 0);
        assert_eq!(search.matches[0].byte_end, 5);
        assert_eq!(search.matches[1].line_index, 1);
    }

    #[test]
    fn test_search_empty_enter_stays_active() {
        let mut app = make_app(10, 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        // Enter with empty query: stays active.
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.search.as_ref().unwrap().active);
    }

    #[test]
    fn test_search_no_matches() {
        let mut app = make_searchable_app(
            &["one", "two", "three"],
            10,
        );
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        let search = app.search.as_ref().unwrap();
        assert!(!search.active);
        assert!(search.matches.is_empty());
        assert_eq!(search.focus, 0);
    }

    #[test]
    fn test_search_n_navigates_next() {
        let mut app = make_searchable_app(
            &["aaa", "aaa", "aaa"],
            10,
        );
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert_eq!(app.search.as_ref().unwrap().focus, 0);

        // n moves to next match.
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
        assert_eq!(app.search.as_ref().unwrap().focus, 1);

        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
        assert_eq!(app.search.as_ref().unwrap().focus, 2);
    }

    #[test]
    fn test_search_n_wraps_around() {
        let mut app = make_searchable_app(
            &["ab", "bb"],
            10,
        );
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        // Only 1 match for "a" on line 0. focus=0.
        assert_eq!(app.search.as_ref().unwrap().matches.len(), 1);
        // n wraps to 0.
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
        assert_eq!(app.search.as_ref().unwrap().focus, 0);
    }

    #[test]
    fn test_search_shift_n_navigates_prev() {
        let mut app = make_searchable_app(
            &["aaa", "aaa", "aaa"],
            10,
        );
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        // N (shift+n) wraps to last match.
        app.handle_key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT));
        let total = app.search.as_ref().unwrap().matches.len();
        assert_eq!(app.search.as_ref().unwrap().focus, total - 1);

        // N again goes to previous.
        app.handle_key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT));
        assert_eq!(app.search.as_ref().unwrap().focus, total - 2);
    }

    #[test]
    fn test_search_esc_in_results_clears() {
        let mut app = make_searchable_app(
            &["hello"],
            10,
        );
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.search.is_some());
        assert!(!app.search.as_ref().unwrap().active);

        // Esc clears search entirely.
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert!(app.search.is_none());
        assert!(!app.quit, "Esc clearing search should not quit");
    }

    #[test]
    fn test_search_slash_in_results_re_enters_input() {
        let mut app = make_searchable_app(
            &["hello"],
            10,
        );
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(!app.search.as_ref().unwrap().active);

        // / re-enters search input mode.
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        assert!(app.search.as_ref().unwrap().active);
        assert_eq!(app.search.as_ref().unwrap().query, "h");
    }

    #[test]
    fn test_search_jk_still_scroll_in_results() {
        let mut app = make_searchable_app(
            &["line1", "line2", "line3", "line4", "line5",
              "line6", "line7", "line8", "line9", "line10"],
            5,
        );
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        // j/k should still scroll the document in results mode.
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
        assert_eq!(app.scroll_offset, 1);
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()));
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_search_n_no_matches_is_noop() {
        let mut app = make_searchable_app(
            &["hello"],
            10,
        );
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.search.as_ref().unwrap().matches.is_empty());

        // n with no matches should not change anything.
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
        assert_eq!(app.search.as_ref().unwrap().focus, 0);
    }

    #[test]
    fn test_search_scrolls_to_first_match() {
        let texts: Vec<&str> = (0..30).map(|i| {
            if i == 25 { "TARGET" } else { "filler" }
        }).collect();
        let mut app = make_searchable_app(&texts, 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        // Type "TARGET"
        for c in "TARGET".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        // Should have scrolled to make line 25 visible.
        assert!(app.scroll_offset > 0, "should have scrolled to match");
        let range = app.visible_range();
        assert!(
            range.contains(&25),
            "match at line 25 should be visible, range={range:?}"
        );
    }

    #[test]
    fn test_search_multiple_matches_per_line() {
        let mut app = make_searchable_app(
            &["aa bb aa cc aa"],
            10,
        );
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        let search = app.search.as_ref().unwrap();
        assert_eq!(search.matches.len(), 3, "should find 3 occurrences of 'aa'");
        assert_eq!(search.matches[0].byte_start, 0);
        assert_eq!(search.matches[1].byte_start, 6);
        assert_eq!(search.matches[2].byte_start, 12);
    }

    #[test]
    fn test_search_input_captures_all_keys() {
        let mut app = make_app(20, 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));

        // 'q' should NOT quit when in search mode — it should append to query.
        app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty()));
        assert!(!app.quit);
        assert_eq!(app.search.as_ref().unwrap().query, "q");

        // 'j' should NOT scroll — it should append.
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.search.as_ref().unwrap().query, "qj");
    }

    #[cfg(unix)]
    #[test]
    fn walk_dir_skips_symlinks() {
        use std::os::unix::fs::symlink;
        let pid = std::process::id();
        let root = std::env::temp_dir().join(format!("mdink_walk_test_{pid}"));
        let outside = std::env::temp_dir().join(format!("mdink_walk_outside_{pid}"));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(root.join("real")).unwrap();
        std::fs::write(root.join("real/in.md"), "# in").unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("leak.md"), "# leak").unwrap();

        // A symlinked .md file and a symlinked directory, both pointing outside.
        symlink(outside.join("leak.md"), root.join("link.md")).unwrap();
        symlink(&outside, root.join("linkdir")).unwrap();

        let mut files = Vec::new();
        walk_dir(&root, &root, 0, &mut files);

        // Only the real in-tree file is found; the symlinks are skipped.
        assert!(files.iter().any(|p| p.ends_with("real/in.md")));
        assert!(!files.iter().any(|p| p.to_string_lossy().contains("link")));
        assert!(!files.iter().any(|p| p.to_string_lossy().contains("leak")));

        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }
