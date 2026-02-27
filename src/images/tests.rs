    use super::*;

    #[test]
    fn test_load_image_no_picker_returns_err() {
        let mut mgr = ImageManager::new(PathBuf::from("."), None, 80);
        let result = mgr.load_image("anything.png");
        assert!(result.is_err(), "should fail when picker is None");
    }

    #[test]
    fn test_load_image_missing_file_returns_err() {
        // Even with no picker, the "no graphics support" error comes first.
        // With a picker we can't easily construct one in tests (needs terminal).
        // So we just verify the None-picker path.
        let mut mgr = ImageManager::new(PathBuf::from("/nonexistent"), None, 80);
        let result = mgr.load_image("nonexistent.png");
        assert!(result.is_err());
    }

    #[test]
    fn test_image_manager_new_defaults() {
        let mgr = ImageManager::new(PathBuf::from("/tmp"), None, 120);
        assert!(mgr.picker.is_none());
        assert!(mgr.protocols.is_empty());
        assert_eq!(mgr.max_width, 120);
    }
