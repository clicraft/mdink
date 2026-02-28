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
