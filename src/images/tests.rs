    use super::*;

    #[test]
    fn test_load_image_no_picker_returns_err() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let result = mgr.load_image("anything.png");
        assert!(result.is_err(), "should fail when picker is None");
    }

    #[test]
    fn test_load_image_missing_file_returns_err() {
        // Even with no picker, the "no graphics support" error comes first.
        // With a picker we can't easily construct one in tests (needs terminal).
        // So we just verify the None-picker path.
        let mut mgr = ImageManager::new(PathBuf::from("/nonexistent"), None, 80, false, false, false);
        let result = mgr.load_image("nonexistent.png");
        assert!(result.is_err());
    }

    #[test]
    fn test_image_manager_new_defaults() {
        let mgr = ImageManager::new(PathBuf::from("/tmp"), None, 120, false, false, false);
        assert!(mgr.picker.is_none());
        assert!(mgr.protocols.is_empty());
        assert_eq!(mgr.max_width, 120);
        assert!(!mgr.no_images);
        assert!(!mgr.force_ascii);
    }

    #[test]
    fn test_prefer_ascii_flag() {
        let mgr_off = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        assert!(!mgr_off.prefer_ascii());

        let mgr_on = ImageManager::new(PathBuf::from("."), None, 80, false, true, false);
        assert!(mgr_on.prefer_ascii());
    }

    #[test]
    fn test_images_disabled_flag() {
        let mgr_off = ImageManager::new(PathBuf::from("."), None, 80, true, false, false);
        assert!(mgr_off.images_disabled());
        assert!(!mgr_off.has_graphics_support());

        let mgr_on = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        assert!(!mgr_on.images_disabled());
    }

    #[test]
    fn test_clear_protocols() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        // Simulate protocol accumulation (can't push real protocols without a picker,
        // but we can verify the clear method exists and the vec is empty after).
        mgr.clear_protocols();
        assert!(mgr.protocols.is_empty());
    }

    #[test]
    fn test_max_width_and_update() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        assert_eq!(mgr.max_width, 80);
        mgr.update_max_width(120);
        assert_eq!(mgr.max_width, 120);
    }

    #[test]
    fn test_load_ascii_image_returns_correct_width() {
        // gradient.png is 160×120. With default font (8×16):
        // natural size = 160/8=20 wide × 120/16=8 tall (fits in 80 cols).
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false, false);
        let lines = mgr.load_ascii_image("gradient.png").unwrap();
        assert!(!lines.is_empty(), "should produce lines");
        assert_eq!(lines.len(), 8, "height should be 120/16 = 8 rows");
        for line in &lines {
            assert_eq!(line.spans.len(), 20, "width should be 160/8 = 20 cols");
        }
    }

    #[test]
    fn test_load_ascii_image_missing_file_returns_err() {
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false, false);
        let result = mgr.load_ascii_image("nonexistent.png");
        assert!(result.is_err(), "missing file should return Err");
    }

    #[test]
    fn test_load_ascii_image_spans_have_rgb_foreground() {
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false, false);
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
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 1, false, false, false);
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
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false, false);
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

    // ── Remote URL detection ──────────────────────────────────────────

    #[test]
    fn test_images_is_remote_url_http() {
        assert!(ImageManager::is_remote_url("http://example.com/img.png"));
    }

    #[test]
    fn test_images_is_remote_url_https() {
        assert!(ImageManager::is_remote_url("https://example.com/img.png"));
    }

    #[test]
    fn test_images_is_remote_url_local() {
        assert!(!ImageManager::is_remote_url("images/photo.png"));
        assert!(!ImageManager::is_remote_url("/absolute/path.png"));
        assert!(!ImageManager::is_remote_url("relative.png"));
    }

    // ── Cache operations ──────────────────────────────────────────────

    #[test]
    fn test_images_cache_insert_and_get() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let img = DynamicImage::new_rgb8(100, 50);
        mgr.insert_cache("https://example.com/img.png".to_string(), img.clone());
        let cached = mgr.get_cached("https://example.com/img.png").unwrap();
        assert_eq!(cached.width(), 100);
        assert_eq!(cached.height(), 50);
        assert!(mgr.get_cached("https://other.com/x.png").is_none());
    }

    #[test]
    fn test_images_cache_clear_all_removes_everything() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        mgr.insert_cache("https://example.com/a.png".to_string(), DynamicImage::new_rgb8(10, 10));
        mgr.mark_pending("https://example.com/b.png");
        mgr.mark_failed("https://example.com/c.png");
        mgr.clear_all();
        assert!(mgr.cache.is_empty());
        assert!(mgr.pending_urls.is_empty());
        assert!(mgr.failed_urls.is_empty());
        assert!(mgr.protocols.is_empty());
    }

    #[test]
    fn test_images_clear_protocols_keeps_cache() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        mgr.insert_cache("https://example.com/img.png".to_string(), DynamicImage::new_rgb8(10, 10));
        mgr.clear_protocols();
        assert!(mgr.protocols.is_empty());
        assert!(mgr.get_cached("https://example.com/img.png").is_some(), "cache should survive clear_protocols");
    }

    // ── Pending/failed tracking ───────────────────────────────────────

    #[test]
    fn test_images_pending_url_dedup() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        assert!(mgr.mark_pending("https://example.com/a.png"));
        assert!(!mgr.mark_pending("https://example.com/a.png"), "second mark_pending should return false");
    }

    /// Regression test for the "no network traffic" bug.
    /// mark_pending must happen in queue_pending_fetches (not in the parser),
    /// so that queue_pending_fetches can use mark_pending's return value to
    /// decide whether to send the URL to the fetch thread.
    #[test]
    fn test_images_mark_pending_then_send_pattern() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let (tx, rx) = std::sync::mpsc::channel::<ImageFetchRequest>();

        // Simulate queue_pending_fetches: mark_pending returns true → send.
        let url_a = "https://example.com/a.png";
        let url_b = "https://example.com/b.png";
        assert!(mgr.mark_pending(url_a), "first URL should be markable");
        let _ = tx.send(ImageFetchRequest { url: url_a.to_string() });
        assert!(mgr.mark_pending(url_b), "second URL should be markable");
        let _ = tx.send(ImageFetchRequest { url: url_b.to_string() });

        // Second call for same URL should not re-send.
        assert!(!mgr.mark_pending(url_a), "already pending URL should return false");

        // Verify both were sent.
        let req_a = rx.try_recv().unwrap();
        assert_eq!(req_a.url, url_a);
        let req_b = rx.try_recv().unwrap();
        assert_eq!(req_b.url, url_b);
        assert!(rx.try_recv().is_err(), "no more requests should be queued");
    }

    /// After mark_resolved (image downloaded), the URL is no longer pending.
    /// A re-parse that produces ImagePending for the same URL should find it
    /// in cache (not pending), so it resolves immediately.
    #[test]
    fn test_images_mark_resolved_allows_cache_lookup() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let url = "https://example.com/img.png";
        mgr.mark_pending(url);
        mgr.mark_resolved(url);
        mgr.insert_cache(url.to_string(), DynamicImage::new_rgb8(32, 32));

        // URL is no longer pending — cache should be hit.
        assert!(!mgr.is_pending_or_failed(url), "resolved URL should not be pending");
        assert!(mgr.get_cached(url).is_some(), "cache should contain the image");
    }

    #[test]
    fn test_images_failed_url_not_retried() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        mgr.mark_pending("https://example.com/a.png");
        mgr.mark_failed("https://example.com/a.png");
        assert!(mgr.is_pending_or_failed("https://example.com/a.png"));
        assert!(!mgr.mark_pending("https://example.com/a.png"), "failed URL should not be re-pended");
    }

    // ── load_image_from_memory ────────────────────────────────────────

    #[test]
    fn test_images_load_image_from_memory_no_picker() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let result = mgr.load_image_from_memory(DynamicImage::new_rgb8(100, 50));
        assert!(result.is_err(), "should fail without picker (no graphics support)");
    }

    // ── load_ascii_image_from_memory ──────────────────────────────────

    #[test]
    fn test_images_load_ascii_image_from_memory_gradient() {
        // Load gradient.png from disk, then pass the DynamicImage to load_ascii_image_from_memory.
        // Output should match what load_ascii_image produces.
        let mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false, false);
        let disk_lines = mgr.load_ascii_image("gradient.png").unwrap();

        let dyn_img = ImageReader::open("testdata/gradient.png")
            .unwrap()
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();
        let mem_lines = mgr.load_ascii_image_from_memory(&dyn_img).unwrap();

        assert_eq!(mem_lines.len(), disk_lines.len(), "row count should match");
        for (mem_line, disk_line) in mem_lines.iter().zip(disk_lines.iter()) {
            assert_eq!(mem_line.spans.len(), disk_line.spans.len(), "col count should match");
        }
    }

    // ── fetch_image ───────────────────────────────────────────────────

    #[test]
    fn test_images_fetch_image_invalid_url() {
        let result = super::fetch_image("not-a-url");
        assert!(result.is_err(), "invalid URL should return Err");
    }

    #[test]
    fn test_images_fetch_image_real_github_png() {
        // Integration test: fetch a real PNG from GitHub raw.
        // Verifies ureq + image decode pipeline works end-to-end.
        let url = "https://raw.githubusercontent.com/datawhalechina/Hello-Agents/main/docs/images/1-figures/1757242319667-0.png";
        let result = super::fetch_image(url);
        match &result {
            Ok(img) => {
                assert!(img.width() > 0, "decoded image should have positive width");
                assert!(img.height() > 0, "decoded image should have positive height");
            }
            Err(e) => {
                // Network may be unavailable in CI; log but don't fail.
                eprintln!("warning: fetch skipped (network error): {e}");
            }
        }
    }

    // ── Multi-image fetch simulation ────────────────────────────────

    #[test]
    fn test_images_multiple_fetches_thread_simulation() {
        // Simulate the fetch thread pattern: send URLs, receive results,
        // update cache, verify all are cached.
        use std::sync::mpsc;
        use std::thread;

        let (fetch_tx, fetch_rx) = mpsc::channel::<ImageFetchRequest>();
        let (result_tx, result_rx) = mpsc::channel::<ImageFetchResult>();

        let handle = thread::spawn(move || {
            while let Ok(req) = fetch_rx.recv() {
                let result = match super::fetch_image(&req.url) {
                    Ok(dyn_img) => ImageFetchResult::Ok { url: req.url, dyn_img },
                    Err(e) => ImageFetchResult::Err { url: req.url, error: e.to_string(), expected: e.is_expected() },
                };
                if result_tx.send(result).is_err() { break; }
            }
        });

        // Send 1 valid URL and 1 invalid URL.
        let urls = vec![
            "not-a-valid-url",
            "https://raw.githubusercontent.com/datawhalechina/Hello-Agents/main/docs/images/1-figures/1757242319667-1.png",
        ];
        for url in &urls {
            let _ = fetch_tx.send(ImageFetchRequest { url: url.to_string() });
        }
        drop(fetch_tx); // Signal fetch thread to stop.

        let mut ok_count = 0;
        let mut err_count = 0;
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        // Use blocking recv — fetch thread processes sequentially.
        for _ in 0..urls.len() {
            match result_rx.recv_timeout(std::time::Duration::from_secs(15)) {
                Ok(ImageFetchResult::Ok { url, dyn_img }) => {
                    mgr.insert_cache(url, dyn_img);
                    ok_count += 1;
                }
                Ok(ImageFetchResult::Err { url, error, .. }) => {
                    eprintln!("warning: fetch failed for {url}: {error}");
                    err_count += 1;
                }
                Err(e) => {
                    eprintln!("timeout waiting for result: {e}");
                    break;
                }
            }
        }

        handle.join().unwrap();
        eprintln!("ok_count={ok_count}, err_count={err_count}, cache_len={}", mgr.cache.len());
        // The invalid URL must fail.
        assert!(err_count >= 1, "invalid URL should have failed, got ok={ok_count} err={err_count}");
    }

    // ── fetch_remote flag ─────────────────────────────────────────────

    #[test]
    fn test_images_fetch_remote_default_false() {
        let mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        assert!(!mgr.fetch_remote(), "fetch_remote should default to false");
    }

    #[test]
    fn test_images_fetch_remote_true() {
        let mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        assert!(mgr.fetch_remote(), "fetch_remote should be true when set");
    }

    // ── set_fetch_remote toggle ──────────────────────────────────────

    #[test]
    fn test_images_set_fetch_remote_toggle() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        assert!(!mgr.fetch_remote());
        mgr.set_fetch_remote(true);
        assert!(mgr.fetch_remote());
        mgr.set_fetch_remote(false);
        assert!(!mgr.fetch_remote());
    }

    #[test]
    fn test_images_set_fetch_remote_clears_failed_urls() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        mgr.mark_pending("http://bad.url/img.png");
        mgr.mark_failed("http://bad.url/img.png");
        // mark_failed should prevent re-queueing.
        assert!(!mgr.mark_pending("http://bad.url/img.png"), "failed URL should not be re-queued");
        // Toggling off then on clears failed URLs for retry.
        mgr.set_fetch_remote(false);
        mgr.set_fetch_remote(true);
        assert!(mgr.mark_pending("http://bad.url/img.png"), "after re-enable, previously failed URL should be queueable again");
    }

    #[test]
    fn test_images_is_failed_url() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        assert!(!mgr.is_failed_url("http://example.com/img.png"), "not yet failed");
        mgr.mark_pending("http://example.com/img.png");
        assert!(!mgr.is_failed_url("http://example.com/img.png"), "pending is not failed");
        mgr.mark_failed("http://example.com/img.png");
        assert!(mgr.is_failed_url("http://example.com/img.png"), "should be marked as failed");
    }

    #[test]
    fn test_images_clear_all_clears_failed_urls() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        mgr.mark_pending("http://bad.url/img.png");
        mgr.mark_failed("http://bad.url/img.png");
        assert!(mgr.is_failed_url("http://bad.url/img.png"));
        mgr.clear_all();
        assert!(!mgr.is_failed_url("http://bad.url/img.png"), "clear_all should clear failed_urls");
    }

    #[test]
    fn test_images_fetch_failure_then_reparse_flow() {
        // Simulates the main loop flow:
        // 1. Parse → ImagePending → mark_pending → queue fetch
        // 2. Fetch fails → mark_failed
        // 3. Re-parse → should NOT emit ImagePending (is_failed_url check)
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, true);
        let url = "https://github.com/user/repo/raw/main/broken.png";

        // Step 1: First mark_pending succeeds
        assert!(mgr.mark_pending(url), "first mark_pending should succeed");
        // Step 2: Simulate fetch failure
        mgr.mark_failed(url);
        assert!(mgr.is_failed_url(url));
        // Step 3: Second mark_pending fails (URL in failed_urls)
        assert!(!mgr.mark_pending(url), "failed URL should not be re-queued");
        // Step 4: clear_all on new document clears everything
        mgr.clear_all();
        assert!(!mgr.is_failed_url(url));
        assert!(mgr.mark_pending(url), "after clear_all, URL should be queueable again");
    }

    // ── Missing local file tests ──────────────────────────────────

    #[test]
    fn test_load_image_missing_local_file_returns_err_with_path() {
        let mut mgr = ImageManager::new(PathBuf::from("/nonexistent/dir"), None, 80, false, false, false);
        let result = mgr.load_image("missing.png");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("missing.png") || msg.contains("nonexistent"),
            "error should mention the file path, got: {msg}"
        );
    }

    #[test]
    fn test_load_ascii_image_missing_local_file_returns_err_with_path() {
        let mgr = ImageManager::new(PathBuf::from("/nonexistent/dir"), None, 80, false, false, false);
        let result = mgr.load_ascii_image("missing.png");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("missing.png") || msg.contains("nonexistent"),
            "error should mention the file path, got: {msg}"
        );
    }

    #[test]
    fn test_load_image_non_image_file_returns_err() {
        // A .md file is not a valid image format.
        let mut mgr = ImageManager::new(PathBuf::from("testdata"), None, 80, false, false, false);
        let result = mgr.load_image("basic.md");
        assert!(result.is_err(), "non-image file should fail to decode");
    }

    #[test]
    fn test_load_image_from_memory_no_picker_returns_err() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80, false, false, false);
        let img = DynamicImage::new_rgba8(1, 1);
        let result = mgr.load_image_from_memory(img);
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("no graphics support"),
            "should report no graphics support, got: {msg}"
        );
    }
