    use super::*;

    #[test]
    fn test_load_image_no_picker_returns_err() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false);
        let result = mgr.load_image("anything.png");
        assert!(result.is_err(), "should fail when picker is None");
    }

    #[test]
    fn test_load_image_missing_file_returns_err() {
        // Even with no picker, the "no graphics support" error comes first.
        // With a picker we can't easily construct one in tests (needs terminal).
        // So we just verify the None-picker path.
        let mut mgr = ImageManager::new(PathBuf::from("/nonexistent"), None, 80, false, false);
        let result = mgr.load_image("nonexistent.png");
        assert!(result.is_err());
    }

    #[test]
    fn test_image_manager_new_defaults() {
        let mgr = ImageManager::new(PathBuf::from("/tmp"), None, 120, false, false);
        assert!(mgr.picker.is_none());
        assert!(mgr.protocols.is_empty());
        assert_eq!(mgr.max_width, 120);
        assert!(!mgr.no_images);
        assert!(!mgr.force_ascii);
    }

    #[test]
    fn test_prefer_ascii_flag() {
        let mgr_off = ImageManager::new(PathBuf::from("."), None, 80, false, false);
        assert!(!mgr_off.prefer_ascii());

        let mgr_on = ImageManager::new(PathBuf::from("."), None, 80, false, true);
        assert!(mgr_on.prefer_ascii());
    }

    #[test]
    fn test_images_disabled_flag() {
        let mgr_off = ImageManager::new(PathBuf::from("."), None, 80, true, false);
        assert!(mgr_off.images_disabled());
        assert!(!mgr_off.has_graphics_support());

        let mgr_on = ImageManager::new(PathBuf::from("."), None, 80, false, false);
        assert!(!mgr_on.images_disabled());
    }

    #[test]
    fn test_clear_protocols() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false);
        // Simulate protocol accumulation (can't push real protocols without a picker,
        // but we can verify the clear method exists and the vec is empty after).
        mgr.clear_protocols();
        assert!(mgr.protocols.is_empty());
    }

    #[test]
    fn test_max_width_and_update() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false);
        assert_eq!(mgr.max_width(), 80);
        mgr.update_max_width(120);
        assert_eq!(mgr.max_width(), 120);
    }

    #[test]
    fn test_load_ascii_image_returns_correct_width() {
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false);
        let lines = mgr.load_ascii_image("gradient.png", 40).unwrap();
        assert!(!lines.is_empty(), "should produce lines");
        // Every line should have exactly 40 spans (one per column).
        for line in &lines {
            assert_eq!(line.spans.len(), 40, "each line should have width spans");
        }
    }

    #[test]
    fn test_load_ascii_image_missing_file_returns_err() {
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false);
        let result = mgr.load_ascii_image("nonexistent.png", 40);
        assert!(result.is_err(), "missing file should return Err");
    }

    #[test]
    fn test_load_ascii_image_spans_have_rgb_foreground() {
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false);
        let lines = mgr.load_ascii_image("gradient.png", 20).unwrap();
        // At least some spans should have an RGB foreground color.
        let has_rgb = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| matches!(span.style.fg, Some(Color::Rgb(_, _, _))))
        });
        assert!(has_rgb, "ASCII art spans should have RGB foreground colors");
    }

    #[test]
    fn test_load_ascii_image_width_1_no_panic() {
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false);
        let lines = mgr.load_ascii_image("gradient.png", 1).unwrap();
        assert!(!lines.is_empty());
        for line in &lines {
            assert_eq!(line.spans.len(), 1);
        }
    }

    #[test]
    fn test_load_ascii_image_rgba_png() {
        // rust-logo.png is 32x32 RGBA — tests that alpha channel images decode
        // without error (image crate composites alpha onto a background).
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false);
        let lines = mgr.load_ascii_image("rust-logo.png", 40).unwrap();
        assert!(!lines.is_empty(), "RGBA image should produce lines");
        assert_eq!(lines[0].spans.len(), 40, "width should match requested cols");

        // Dump the ASCII art to stdout so `cargo test -- --nocapture` shows it.
        eprintln!("\n--- rust-logo.png ASCII art (40 cols) ---");
        for line in &lines {
            let row: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            eprintln!("{row}");
        }
        eprintln!("--- end ---\n");

        // Verify the image has both dark and bright regions (not a solid block).
        let chars: Vec<char> = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.chars().next().unwrap()))
            .collect();
        let unique: std::collections::HashSet<char> = chars.iter().copied().collect();
        assert!(
            unique.len() >= 3,
            "logo should use at least 3 distinct density characters, got {}: {:?}",
            unique.len(),
            unique
        );
    }
