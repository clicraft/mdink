    use super::*;
    use crate::layout::{DocumentLine, HeadingEntry, ImageEntry, PreRenderedDocument};

    fn make_doc(line_count: usize) -> PreRenderedDocument {
        let lines = (0..line_count).map(|_| DocumentLine::Empty).collect();
        PreRenderedDocument {
            lines,
            total_height: line_count,
            headings: Vec::new(),
            links: Vec::new(),
            images: Vec::new(),
            inline_images: Vec::new(),
        }
    }

    fn make_app(doc_lines: usize, viewport: usize) -> App {
        let mut app = App::new(
            make_doc(doc_lines),
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true, // fetch_remote_images, fetch_remote_markdown, math_images_enabled
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

    /// Regression test: handle_key processes key events regardless of kind.
    /// The event loop filters to KeyEventKind::Press before calling handle_key,
    /// so handle_key itself does not need to check kind.
    #[test]
    fn test_app_handle_key_quit_q_with_release_kind() {
        let mut app = make_app(10, 5);
        let key = KeyEvent::new_with_kind(KeyCode::Char('q'), KeyModifiers::empty(), KeyEventKind::Release);
        // handle_key processes it — the Press filter is in the event loop, not here.
        app.handle_key(key);
        assert!(app.quit, "handle_key should process Release events too; filtering is the caller's job");
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
            links: Vec::new(),
            images: Vec::new(),
            inline_images: Vec::new(),
        }
    }

    fn make_app_with_headings(doc_lines: usize, viewport: usize, headings: Vec<HeadingEntry>) -> App {
        let mut app = App::new(
            make_doc_with_headings(doc_lines, headings),
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true, // fetch_remote_images, fetch_remote_markdown, math_images_enabled
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
            links: Vec::new(),
            images: Vec::new(),
            inline_images: Vec::new(),
        }
    }

    fn make_searchable_app(texts: &[&str], viewport: usize) -> App {
        let mut app = App::new(
            make_doc_with_text(texts),
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true, // fetch_remote_images, fetch_remote_markdown, math_images_enabled
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

    // ── Link navigation tests ──────────────────────────────────────

    use crate::layout::LinkEntry;

    fn make_doc_with_links(link_specs: Vec<(&str, usize)>) -> PreRenderedDocument {
        let line_count = link_specs.iter().map(|(_, idx)| *idx).max().unwrap_or(0) + 1;
        let lines = (0..line_count).map(|_| DocumentLine::Empty).collect();
        PreRenderedDocument {
            lines,
            total_height: line_count,
            headings: Vec::new(),
            links: link_specs.into_iter().map(|(url, idx)| LinkEntry {
                url: url.to_string(),
                line_index: idx,
            }).collect(),
            images: Vec::new(),
            inline_images: Vec::new(),
        }
    }

    #[test]
    fn test_link_mode_l_enters_when_links_exist() {
        let doc = make_doc_with_links(vec![("https://example.com", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        assert!(app.link_mode);
        assert_eq!(app.link_selected, 0);
    }

    #[test]
    fn test_link_mode_l_noop_when_no_links() {
        let mut app = make_app(10, 5);
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        assert!(!app.link_mode);
    }

    #[test]
    fn test_link_mode_tab_cycles_forward() {
        let doc = make_doc_with_links(vec![
            ("https://a.com", 0),
            ("https://b.com", 5),
            ("https://c.com", 10),
        ]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        assert_eq!(app.link_selected, 0);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.link_selected, 1);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.link_selected, 2);

        // Wraps around.
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.link_selected, 0);
    }

    #[test]
    fn test_link_mode_shift_tab_cycles_backward() {
        let doc = make_doc_with_links(vec![
            ("https://a.com", 0),
            ("https://b.com", 5),
            ("https://c.com", 10),
        ]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        assert_eq!(app.link_selected, 0);

        // Shift+Tab wraps to last.
        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.link_selected, 2);

        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.link_selected, 1);
    }

    #[test]
    fn test_link_mode_enter_sets_follow_flag() {
        let doc = make_doc_with_links(vec![("basic.md", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.link_follow_requested);
    }

    #[test]
    fn test_link_mode_esc_exits() {
        let doc = make_doc_with_links(vec![("https://example.com", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        assert!(app.link_mode);

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert!(!app.link_mode);
        assert!(!app.quit, "Esc in link mode should not quit");
    }

    #[test]
    fn test_link_mode_jk_still_scroll() {
        let doc = make_doc_with_links(vec![("https://example.com", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 5;
        app.document.total_height = 20;
        app.document.lines = (0..20).map(|_| DocumentLine::Empty).collect();
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
        assert_eq!(app.scroll_offset, 1);
        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()));
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_back_requested_when_history_exists() {
        let mut app = make_app(10, 5);
        app.nav_history.push(crate::app::NavHistoryEntry {
            source: String::new(),
            base_path: PathBuf::from("."),
            filename: "prev.md".to_string(),
            scroll_offset: 3,
        });
        app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert!(app.back_requested);
    }

    #[test]
    fn test_back_not_requested_when_history_empty() {
        let mut app = make_app(10, 5);
        app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert!(!app.back_requested);
    }

    #[test]
    fn test_link_mode_scroll_to_selected() {
        // Link at line 15, viewport height 5, start at scroll_offset 0.
        // After entering link mode, should scroll to show link.
        let _doc = make_doc_with_links(vec![("https://example.com", 15)]);
        let doc = make_doc_with_links(vec![("https://example.com", 15)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 5;
        app.document.total_height = 30;
        app.document.links = vec![LinkEntry { url: "https://example.com".to_string(), line_index: 15 }];
        app.document.lines = (0..30).map(|_| DocumentLine::Empty).collect();
        assert_eq!(app.scroll_offset, 0);

        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        // Link at line 15 is below viewport [0..5), so scroll should move.
        assert!(
            app.scroll_offset > 0,
            "should have scrolled to show link at line 15, got offset {}",
            app.scroll_offset
        );
        let range = app.visible_range();
        assert!(range.contains(&15), "link at line 15 should be visible, range={range:?}");
    }

    // ── Image navigation tests ──────────────────────────────────────

    fn make_doc_with_images(image_specs: Vec<(&str, usize)>) -> PreRenderedDocument {
        let line_count = image_specs.iter().map(|(_, idx)| *idx).max().unwrap_or(0) + 1;
        let lines = (0..line_count).map(|_| DocumentLine::Empty).collect();
        PreRenderedDocument {
            lines,
            total_height: line_count,
            headings: Vec::new(),
            links: Vec::new(),
            images: image_specs.into_iter().map(|(url, idx)| ImageEntry {
                url: url.to_string(),
                line_index: idx,
            }).collect(),
            inline_images: Vec::new(),
        }
    }

    #[test]
    fn test_image_mode_i_enters_when_images_exist() {
        let doc = make_doc_with_images(vec![("photo.png", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        assert!(!app.image_mode);
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode);
        assert_eq!(app.image_selected, 0);
    }

    #[test]
    fn test_image_mode_i_noop_when_no_images() {
        let mut app = make_app(10, 5);
        assert!(!app.image_mode);
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(!app.image_mode, "should not enter image mode when no images exist");
    }

    #[test]
    fn test_image_mode_tab_cycles_forward() {
        let doc = make_doc_with_images(vec![("a.png", 0), ("b.png", 1), ("c.png", 2)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert_eq!(app.image_selected, 0);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.image_selected, 1);

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.image_selected, 2);

        // Wrap around.
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.image_selected, 0);
    }

    #[test]
    fn test_image_mode_shift_tab_cycles_backward() {
        let doc = make_doc_with_images(vec![("a.png", 0), ("b.png", 1)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert_eq!(app.image_selected, 0);

        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.image_selected, 1, "backward from 0 should wrap to last");

        app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.image_selected, 0);
    }

    #[test]
    fn test_image_mode_esc_exits() {
        let doc = make_doc_with_images(vec![("a.png", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode);

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert!(!app.image_mode);
        assert!(!app.quit, "Esc in image mode should not quit");
    }

    #[test]
    fn test_image_mode_enter_sets_follow_flag() {
        let doc = make_doc_with_images(vec![("photo.png", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.image_follow_requested);
    }

    #[test]
    fn test_image_mode_enter_stays_in_image_mode() {
        // After pressing Enter to open an image, image mode should remain active.
        let doc = make_doc_with_images(vec![("photo.png", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode);

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.image_mode, "Enter should not exit image mode");
    }

    #[test]
    fn test_image_mode_i_toggles_off() {
        // Pressing 'i' while in image mode should exit it.
        let doc = make_doc_with_images(vec![("photo.png", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode);

        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(!app.image_mode, "pressing 'i' again should exit image mode");
    }

    #[test]
    fn test_image_mode_i_toggle_reenter() {
        // Toggling off with 'i' then pressing 'i' again should re-enter image mode.
        let doc = make_doc_with_images(vec![("photo.png", 0), ("other.png", 1)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;

        // Enter, select second image, toggle off, toggle back on.
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
        assert_eq!(app.image_selected, 1);

        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(!app.image_mode);

        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode, "re-entering image mode should work");
        assert_eq!(app.image_selected, 0, "re-entering should reset selection to 0");
    }

    // ── Link/image mode visible-first entry tests ──────────────────────────

    #[test]
    fn test_link_mode_enters_at_first_visible_link() {
        // Links at lines [0, 5, 10, 15, 20].
        // Viewport at offset 8, height 5 → visible range [8, 13).
        // Line 10 is the first visible link → link_selected = 2.
        let doc = make_doc_with_links(vec![
            ("https://a.com", 0),
            ("https://b.com", 5),
            ("https://c.com", 10),
            ("https://d.com", 15),
            ("https://e.com", 20),
        ]);
        let mut app = App::new(
            doc,
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true,
        );
        app.viewport_height = 5;
        app.document.total_height = 25;
        app.document.lines = (0..25).map(|_| DocumentLine::Empty).collect();
        app.scroll_offset = 8;

        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        assert!(app.link_mode);
        assert_eq!(app.link_selected, 2, "should select first visible link (line 10)");
        assert_eq!(app.scroll_offset, 8, "should not scroll — link already visible");
    }

    #[test]
    fn test_link_mode_picks_nearer_backward_over_forward() {
        // Links at lines [0, 2, 20]. Viewport at offset 5, height 5 → range [5, 10).
        // No links visible. Forward: line 20. Backward: line 2.
        // Forward dist = 20-10 = 10, backward dist = 5-2 = 3 → picks backward (index 1).
        let doc = make_doc_with_links(vec![
            ("https://a.com", 0),
            ("https://b.com", 2),
            ("https://c.com", 20),
        ]);
        let mut app = App::new(
            doc,
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true,
        );
        app.viewport_height = 5;
        app.document.total_height = 25;
        app.document.lines = (0..25).map(|_| DocumentLine::Empty).collect();
        app.scroll_offset = 5;

        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        assert!(app.link_mode);
        assert_eq!(app.link_selected, 1, "should pick nearer backward link (line 2, dist 3) over forward (line 20, dist 10)");
    }

    #[test]
    fn test_link_mode_searches_backward_when_none_visible() {
        // Links at lines [0, 2, 4]. Viewport at offset 10, height 5 → range [10, 15).
        // No links visible. Forward: none. Backward: line 4 (index 2, closest to viewport start).
        let doc = make_doc_with_links(vec![
            ("https://a.com", 0),
            ("https://b.com", 2),
            ("https://c.com", 4),
        ]);
        let mut app = App::new(
            doc,
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true,
        );
        app.viewport_height = 5;
        app.document.total_height = 20;
        app.document.lines = (0..20).map(|_| DocumentLine::Empty).collect();
        app.scroll_offset = 10;

        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        assert!(app.link_mode);
        assert_eq!(app.link_selected, 2, "should pick last link before viewport (line 4)");
    }

    #[test]
    fn test_link_mode_prefers_forward_over_backward_when_equal() {
        // Links at lines [0, 20]. Viewport at offset 9, height 2 → range [9, 11).
        // Forward: line 20 (dist 9). Backward: line 0 (dist 9). Equal → picks forward.
        let doc = make_doc_with_links(vec![
            ("https://a.com", 0),
            ("https://b.com", 20),
        ]);
        let mut app = App::new(
            doc,
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true,
        );
        app.viewport_height = 2;
        app.document.total_height = 25;
        app.document.lines = (0..25).map(|_| DocumentLine::Empty).collect();
        app.scroll_offset = 9;

        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        assert!(app.link_mode);
        assert_eq!(app.link_selected, 1, "equal distance → should pick forward (line 20)");
    }

    #[test]
    fn test_image_mode_enters_at_first_visible_image() {
        // Images at lines [0, 3, 7, 12, 18].
        // Viewport at offset 5, height 5 → visible range [5, 10).
        // Line 7 is the first visible image → image_selected = 2.
        let doc = make_doc_with_images(vec![
            ("a.png", 0),
            ("b.png", 3),
            ("c.png", 7),
            ("d.png", 12),
            ("e.png", 18),
        ]);
        let mut app = App::new(
            doc,
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true,
        );
        app.viewport_height = 5;
        app.document.total_height = 25;
        app.document.lines = (0..25).map(|_| DocumentLine::Empty).collect();
        app.scroll_offset = 5;

        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode);
        assert_eq!(app.image_selected, 2, "should select first visible image (line 7)");
        assert_eq!(app.scroll_offset, 5, "should not scroll — image already visible");
    }

    #[test]
    fn test_image_mode_searches_forward_when_none_visible() {
        // Images at lines [0, 15]. Viewport at offset 5, height 5 → range [5, 10).
        // No images visible. Forward: line 15 (dist 5). Backward: line 0 (dist 5).
        // Equal distance → picks forward (index 1).
        let doc = make_doc_with_images(vec![("a.png", 0), ("b.png", 15)]);
        let mut app = App::new(
            doc,
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true,
        );
        app.viewport_height = 5;
        app.document.total_height = 25;
        app.document.lines = (0..25).map(|_| DocumentLine::Empty).collect();
        app.scroll_offset = 5;

        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode);
        assert_eq!(app.image_selected, 1, "equal distance → should pick forward image (line 15)");
    }

    #[test]
    fn test_image_mode_searches_backward_when_none_visible() {
        // Images at lines [0, 1]. Viewport at offset 10, height 5 → range [10, 15).
        // No images visible. Forward: none. Backward: line 1 (index 1, closest).
        let doc = make_doc_with_images(vec![("a.png", 0), ("b.png", 1)]);
        let mut app = App::new(
            doc,
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true,
        );
        app.viewport_height = 5;
        app.document.total_height = 20;
        app.document.lines = (0..20).map(|_| DocumentLine::Empty).collect();
        app.scroll_offset = 10;

        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode);
        assert_eq!(app.image_selected, 1, "should pick last image before viewport (line 1)");
    }

    #[test]
    fn test_image_mode_toggle_reenter_uses_visible_image() {
        // Images at lines [0, 5, 10].
        // Scroll to line 5, enter image mode → selects index 1 (line 5 is visible).
        // Toggle off, toggle back on → should again select index 1.
        let doc = make_doc_with_images(vec![("a.png", 0), ("b.png", 5), ("c.png", 10)]);
        let mut app = App::new(
            doc,
            "test.md".to_string(),
            crate::theme::default_theme(),
            PathBuf::from("."),
            false, false, true,
        );
        app.viewport_height = 5;
        app.document.total_height = 15;
        app.document.lines = (0..15).map(|_| DocumentLine::Empty).collect();
        app.scroll_offset = 3;

        // Enter: visible range [3, 8) → line 5 is first visible → index 1.
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode);
        assert_eq!(app.image_selected, 1, "should select first visible image (line 5)");

        // Toggle off.
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(!app.image_mode);

        // Re-enter: same viewport → same result.
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode);
        assert_eq!(app.image_selected, 1, "re-entering should still select first visible image");
    }

    // ── Toggle shortcut tests (I/L/T) ──────────────────────────────────────

    #[test]
    fn test_toggle_remote_images_shortcut() {
        let doc = make_doc(5);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), true, false, true);
        app.viewport_height = 10;
        assert!(app.fetch_remote_images);

        // Press 'I' → disables remote images
        app.handle_key(KeyEvent::new(KeyCode::Char('I'), KeyModifiers::NONE));
        assert!(!app.fetch_remote_images);
        assert_eq!(app.status_message.as_deref(), Some("Remote images: disabled"));
        assert!(app.refresh_requested);

        // Press 'I' again → re-enables
        app.refresh_requested = false;
        app.handle_key(KeyEvent::new(KeyCode::Char('I'), KeyModifiers::NONE));
        assert!(app.fetch_remote_images);
        assert_eq!(app.status_message.as_deref(), Some("Remote images: enabled"));
        assert!(app.refresh_requested);
    }

    #[test]
    fn test_toggle_remote_markdown_shortcut() {
        let doc = make_doc(5);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        assert!(!app.fetch_remote_markdown);

        // Press 'L' → enables remote markdown
        app.handle_key(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::NONE));
        assert!(app.fetch_remote_markdown);
        assert_eq!(app.status_message.as_deref(), Some("Remote markdown: enabled"));
        assert!(!app.refresh_requested, "L should NOT trigger re-parse");

        // Press 'L' again → disables
        app.handle_key(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::NONE));
        assert!(!app.fetch_remote_markdown);
        assert_eq!(app.status_message.as_deref(), Some("Remote markdown: disabled"));
    }

    #[test]
    fn test_toggle_math_images_shortcut() {
        let doc = make_doc(5);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 10;
        assert!(app.math_images_enabled);

        // Press 'T' → disables math images
        app.handle_key(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::NONE));
        assert!(!app.math_images_enabled);
        assert_eq!(app.status_message.as_deref(), Some("Math images: disabled"));
        assert!(app.refresh_requested);

        // Press 'T' again → re-enables
        app.refresh_requested = false;
        app.handle_key(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::NONE));
        assert!(app.math_images_enabled);
        assert_eq!(app.status_message.as_deref(), Some("Math images: enabled"));
        assert!(app.refresh_requested);
    }

    #[test]
    fn test_toggle_shortcuts_no_conflict_with_lowercase() {
        let doc = make_doc(5);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), true, false, true);
        app.viewport_height = 10;

        // 'I' should toggle remote images, NOT enter image mode
        app.handle_key(KeyEvent::new(KeyCode::Char('I'), KeyModifiers::NONE));
        assert!(!app.fetch_remote_images);
        assert!(!app.image_mode, "'I' should not enter image mode");

        // 'L' should toggle remote markdown, NOT enter link mode
        app.handle_key(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::NONE));
        assert!(app.fetch_remote_markdown);
        assert!(!app.link_mode, "'L' should not enter link mode");

        // 'T' should toggle math images, NOT cycle theme
        app.handle_key(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::NONE));
        assert!(!app.math_images_enabled);
        assert!(!app.theme_cycle_requested, "'T' should not cycle theme");
    }

    // ── Navigation error handling tests ────────────────────────────

    #[test]
    fn test_nav_history_entry_preserves_scroll_offset() {
        // Verify NavHistoryEntry correctly stores the scroll offset
        // so that back-navigation restores it.
        let entry = crate::app::NavHistoryEntry {
            source: "# Hello".to_string(),
            base_path: PathBuf::from("/some/dir"),
            filename: "prev.md".to_string(),
            scroll_offset: 42,
        };
        assert_eq!(entry.scroll_offset, 42);
        assert_eq!(entry.filename, "prev.md");
        assert_eq!(entry.source, "# Hello");
    }

    #[test]
    fn test_link_follow_requested_set_in_link_mode() {
        // When in link mode and Enter is pressed, link_follow_requested is set.
        // Note: link_mode is cleared by the event loop in main.rs, not by App.
        let doc = make_doc_with_links(vec![("local.md", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 5;

        // Enter link mode
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
        assert!(app.link_mode);

        // Press Enter to follow the link
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.link_follow_requested);
        // link_mode stays true until the event loop clears it
    }

    #[test]
    fn test_image_follow_requested_set_in_image_mode() {
        // When in image mode and Enter is pressed, image_follow_requested is set.
        let doc = make_doc_with_images(vec![("photo.png", 0)]);
        let mut app = App::new(doc, "test.md".to_string(), crate::theme::default_theme(), PathBuf::from("."), false, false, true);
        app.viewport_height = 5;

        // Enter image mode
        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
        assert!(app.image_mode);

        // Press Enter to open the image
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.image_follow_requested);
    }

    // ── Goto-line mode tests ──────────────────────────────────────────────────

    fn goto_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty())
    }

    #[test]
    fn test_goto_colon_enters_mode() {
        let mut app = make_app(100, 10);
        app.handle_key(goto_key(':'));
        assert!(app.goto_input.is_some());
    }

    #[test]
    fn test_goto_accepts_digits() {
        let mut app = make_app(100, 10);
        app.handle_key(goto_key(':'));
        app.handle_key(goto_key('4'));
        app.handle_key(goto_key('2'));
        assert_eq!(app.goto_input.as_deref(), Some("42"));
    }

    #[test]
    fn test_goto_ignores_non_digits() {
        let mut app = make_app(100, 10);
        app.handle_key(goto_key(':'));
        app.handle_key(goto_key('a'));
        app.handle_key(goto_key(' '));
        assert_eq!(app.goto_input.as_deref(), Some(""));
    }

    #[test]
    fn test_goto_esc_cancels() {
        let mut app = make_app(100, 10);
        app.handle_key(goto_key(':'));
        app.handle_key(goto_key('4'));
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert!(app.goto_input.is_none());
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_goto_backspace_on_empty_cancels() {
        let mut app = make_app(100, 10);
        app.handle_key(goto_key(':'));
        app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert!(app.goto_input.is_none());
    }

    #[test]
    fn test_goto_backspace_removes_digit() {
        let mut app = make_app(100, 10);
        app.handle_key(goto_key(':'));
        app.handle_key(goto_key('4'));
        app.handle_key(goto_key('2'));
        app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
        assert_eq!(app.goto_input.as_deref(), Some("4"));
    }

    #[test]
    fn test_goto_enter_scrolls_to_line() {
        let mut app = make_app(100, 10);
        // max_scroll = 100 - 10 = 90
        app.handle_key(goto_key(':'));
        app.handle_key(goto_key('5'));
        app.handle_key(goto_key('0'));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.goto_input.is_none());
        // Line 50 → offset = 49
        assert_eq!(app.scroll_offset, 49);
    }

    #[test]
    fn test_goto_line_1_scrolls_to_top() {
        let mut app = make_app(100, 10);
        app.scroll_offset = 50;
        app.handle_key(goto_key(':'));
        app.handle_key(goto_key('1'));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_goto_line_clamped_to_max_scroll() {
        let mut app = make_app(100, 10);
        // max_scroll = 90
        app.handle_key(goto_key(':'));
        app.handle_key(goto_key('9'));
        app.handle_key(goto_key('9'));
        app.handle_key(goto_key('9'));
        app.handle_key(goto_key('9'));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.goto_input.is_none());
        assert_eq!(app.scroll_offset, 90);
    }

    #[test]
    fn test_goto_empty_enter_does_nothing() {
        let mut app = make_app(100, 10);
        app.scroll_offset = 42;
        app.handle_key(goto_key(':'));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.goto_input.is_none());
        assert_eq!(app.scroll_offset, 42);
    }

    #[test]
    fn test_goto_zero_does_nothing() {
        let mut app = make_app(100, 10);
        app.scroll_offset = 42;
        app.handle_key(goto_key(':'));
        app.handle_key(goto_key('0'));
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert_eq!(app.scroll_offset, 42);
    }
