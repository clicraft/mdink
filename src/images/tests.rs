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
        assert_eq!(mgr.max_width, 80);
        mgr.update_max_width(120);
        assert_eq!(mgr.max_width, 120);
    }

    #[test]
    fn test_load_ascii_image_returns_correct_width() {
        // gradient.png is 160×120. With default font (8×16):
        // natural size = 160/8=20 wide × 120/16=8 tall (fits in 80 cols).
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false);
        let lines = mgr.load_ascii_image("gradient.png").unwrap();
        assert!(!lines.is_empty(), "should produce lines");
        assert_eq!(lines.len(), 8, "height should be 120/16 = 8 rows");
        for line in &lines {
            assert_eq!(line.spans.len(), 20, "width should be 160/8 = 20 cols");
        }
    }

    #[test]
    fn test_load_ascii_image_missing_file_returns_err() {
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false);
        let result = mgr.load_ascii_image("nonexistent.png");
        assert!(result.is_err(), "missing file should return Err");
    }

    #[test]
    fn test_load_ascii_image_spans_have_rgb_foreground() {
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false);
        let lines = mgr.load_ascii_image("gradient.png").unwrap();
        // At least some spans should have an RGB foreground color.
        let has_rgb = lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| matches!(span.style.fg, Some(Color::Rgb(_, _, _))))
        });
        assert!(has_rgb, "ASCII art spans should have RGB foreground colors");
    }

    #[test]
    fn test_load_ascii_image_max_width_1_clamps() {
        // With max_width=1, image should scale down to 1 column.
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 1, false, false);
        let lines = mgr.load_ascii_image("gradient.png").unwrap();
        assert!(!lines.is_empty());
        for line in &lines {
            assert_eq!(line.spans.len(), 1);
        }
    }

    #[test]
    fn test_gamma_expansion_biases_midtones_to_denser_chars() {
        // Reproduce the index formula from load_ascii_image to unit-test the
        // gamma-expansion step without loading a file.
        let ramp_len = DENSITY_RAMP.len();
        let idx_for_luma = |luminance: f64| -> usize {
            ((luminance / 255.0).powf(1.0 / 2.2) * (ramp_len - 1) as f64).round() as usize
        };

        // Fixed points: black and white must not change.
        assert_eq!(idx_for_luma(0.0), 0, "black → space");
        assert_eq!(idx_for_luma(255.0), ramp_len - 1, "white → full block");

        // Mid-gray (luminance 128) with expansion:
        //   0.502^(1/2.2) ≈ 0.731 → index 12 ('⣿').
        // Without expansion it would be: round(0.502 × 17) = 9 ('+').
        // Higher index = denser char = more coverage = less background bleed.
        let mid_idx = idx_for_luma(128.0);
        let uncompanded = ((128.0_f64 / 255.0) * (ramp_len - 1) as f64).round() as usize;
        assert!(
            mid_idx > uncompanded,
            "gamma expansion must raise mid-gray index above linear ({uncompanded}), got {mid_idx}"
        );
        assert!(mid_idx >= 11, "mid-gray index should be ≥ 11, got {mid_idx}");
    }

    #[test]
    fn test_load_ascii_image_rgba_png() {
        // rust-logo.png is 32x32 RGBA — tests that alpha channel images decode
        // without error (image crate composites alpha onto a background).
        // With default font (8×16): 32/8=4 wide × 32/16=2 tall.
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false);
        let lines = mgr.load_ascii_image("rust-logo.png").unwrap();
        assert!(!lines.is_empty(), "RGBA image should produce lines");
        assert_eq!(lines[0].spans.len(), 4, "width should be 32/8 = 4 cols");
        assert_eq!(lines.len(), 2, "height should be 32/16 = 2 rows");

        // Dump the ASCII art to stdout so `cargo test -- --nocapture` shows it.
        eprintln!("\n--- rust-logo.png ASCII art (4×2 natural size) ---");
        for line in &lines {
            let row: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            eprintln!("{row}");
        }
        eprintln!("--- end ---\n");

        // Verify every span holds exactly one character (no empty content).
        for line in &lines {
            for span in &line.spans {
                assert_eq!(
                    span.content.chars().count(),
                    1,
                    "each span should be a single density character"
                );
            }
        }
    }
