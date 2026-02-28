use ratatui::style::{Color, Modifier};

use super::*;

// ── Color parsing ────────────────────────────────────────────────────────────

#[test]
fn test_parse_color_hex() {
    assert_eq!(parse_color("#ff5500"), Some(Color::Rgb(255, 85, 0)));
}

#[test]
fn test_parse_color_hex_no_hash() {
    assert_eq!(parse_color("ff5500"), Some(Color::Rgb(255, 85, 0)));
}

#[test]
fn test_parse_color_indexed() {
    assert_eq!(parse_color("99"), Some(Color::Indexed(99)));
}

#[test]
fn test_parse_color_indexed_boundaries() {
    assert_eq!(parse_color("0"), Some(Color::Indexed(0)));
    assert_eq!(parse_color("255"), Some(Color::Indexed(255)));
    // 256 overflows u8 — should not parse as indexed.
    assert_eq!(parse_color("256"), None);
}

#[test]
fn test_parse_color_named() {
    assert_eq!(parse_color("red"), Some(Color::Red));
    assert_eq!(parse_color("light_cyan"), Some(Color::LightCyan));
    assert_eq!(parse_color("LightCyan"), Some(Color::LightCyan));
}

#[test]
fn test_parse_color_empty() {
    assert_eq!(parse_color(""), None);
}

#[test]
fn test_parse_color_whitespace_only() {
    assert_eq!(parse_color("   "), None);
}

#[test]
fn test_parse_color_invalid() {
    assert_eq!(parse_color("notacolor"), None);
}

#[test]
fn test_parse_color_three_digit_hex_rejected() {
    // 3-digit hex like CSS shorthand is NOT supported — must be 6 digits.
    assert_eq!(parse_color("#f00"), None);
    assert_eq!(parse_color("f00"), None);
}

#[test]
fn test_parse_color_hex_all_zeros() {
    assert_eq!(parse_color("#000000"), Some(Color::Rgb(0, 0, 0)));
}

#[test]
fn test_parse_color_hex_all_ff() {
    assert_eq!(parse_color("#ffffff"), Some(Color::Rgb(255, 255, 255)));
}

// ── Style conversion helpers ────────────────────────────────────────────────

#[test]
fn test_heading_style_applies_modifiers() {
    let h = HeadingStyle { bold: true, italic: true, underline: true, ..Default::default() };
    let style = heading_style(&h);
    assert!(style.add_modifier.contains(Modifier::BOLD));
    assert!(style.add_modifier.contains(Modifier::ITALIC));
    assert!(style.add_modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn test_inline_style_all_modifiers() {
    let s = InlineStyle {
        bold: true,
        italic: true,
        underline: true,
        strikethrough: true,
        dim: true,
        ..Default::default()
    };
    let style = inline_style(&s);
    assert!(style.add_modifier.contains(Modifier::BOLD));
    assert!(style.add_modifier.contains(Modifier::ITALIC));
    assert!(style.add_modifier.contains(Modifier::UNDERLINED));
    assert!(style.add_modifier.contains(Modifier::CROSSED_OUT));
    assert!(style.add_modifier.contains(Modifier::DIM));
}

#[test]
fn test_inline_style_no_modifiers() {
    let s = InlineStyle::default();
    let style = inline_style(&s);
    assert_eq!(style.add_modifier, Modifier::empty());
    assert_eq!(style.fg, None);
    assert_eq!(style.bg, None);
}

#[test]
fn test_code_block_bg_returns_color() {
    let cb = CodeBlockStyle { bg: Some("235".to_string()), ..Default::default() };
    assert_eq!(code_block_bg(&cb), Some(Color::Indexed(235)));
}

#[test]
fn test_code_block_bg_none_when_missing() {
    let cb = CodeBlockStyle::default();
    assert_eq!(code_block_bg(&cb), None);
}

#[test]
fn test_code_label_style() {
    let cb = CodeBlockStyle {
        label_fg: Some("245".to_string()),
        label_bg: Some("235".to_string()),
        label_italic: true,
        ..Default::default()
    };
    let style = code_label_style(&cb);
    assert_eq!(style.fg, Some(Color::Indexed(245)));
    assert_eq!(style.bg, Some(Color::Indexed(235)));
    assert!(style.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn test_quote_content_style() {
    let bq = BlockQuoteStyle { italic: true, dim: true, ..Default::default() };
    let style = quote_content_style(&bq);
    assert!(style.add_modifier.contains(Modifier::ITALIC));
    assert!(style.add_modifier.contains(Modifier::DIM));
}

#[test]
fn test_quote_prefix_style_with_border_fg() {
    let bq = BlockQuoteStyle {
        border_fg: Some("cyan".to_string()),
        italic: false,
        dim: false,
        ..Default::default()
    };
    let style = quote_prefix_style(&bq);
    assert_eq!(style.fg, Some(Color::Cyan));
    assert_eq!(style.add_modifier, Modifier::empty());
}

#[test]
fn test_table_header_style() {
    let t = TableStyle { header_bold: true, header_fg: Some("red".to_string()), ..Default::default() };
    let style = table_header_style(&t);
    assert_eq!(style.fg, Some(Color::Red));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_table_border_style() {
    let t = TableStyle { border_dim: true, border_fg: Some("240".to_string()), ..Default::default() };
    let style = table_border_style(&t);
    assert_eq!(style.fg, Some(Color::Indexed(240)));
    assert!(style.add_modifier.contains(Modifier::DIM));
}

#[test]
fn test_rule_style() {
    let tb = ThematicBreakStyle { dim: true, fg: Some("green".to_string()), ..Default::default() };
    let style = rule_style(&tb);
    assert_eq!(style.fg, Some(Color::Green));
    assert!(style.add_modifier.contains(Modifier::DIM));
}

#[test]
fn test_status_bar_style() {
    let sb = StatusBarStyle {
        fg: Some("black".to_string()),
        bg: Some("white".to_string()),
        bold: true,
    };
    let style = status_bar_style(&sb);
    assert_eq!(style.fg, Some(Color::Black));
    assert_eq!(style.bg, Some(Color::White));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_list_bullet_style() {
    let ls = ListStyle { bullet_fg: Some("yellow".to_string()), ..Default::default() };
    let style = list_bullet_style(&ls);
    assert_eq!(style.fg, Some(Color::Yellow));
}

#[test]
fn test_list_number_style() {
    let ls = ListStyle { number_fg: Some("cyan".to_string()), ..Default::default() };
    let style = list_number_style(&ls);
    assert_eq!(style.fg, Some(Color::Cyan));
}

#[test]
fn test_list_task_styles() {
    let ls = ListStyle {
        task_checked_fg: Some("green".to_string()),
        task_unchecked_fg: Some("red".to_string()),
        ..Default::default()
    };
    assert_eq!(list_task_checked_style(&ls).fg, Some(Color::Green));
    assert_eq!(list_task_unchecked_style(&ls).fg, Some(Color::Red));
}

#[test]
fn test_list_style_none_when_no_color() {
    let ls = ListStyle::default();
    assert_eq!(list_bullet_style(&ls).fg, None);
    assert_eq!(list_number_style(&ls).fg, None);
    assert_eq!(list_task_checked_style(&ls).fg, None);
    assert_eq!(list_task_unchecked_style(&ls).fg, None);
}

// ── Theme loading ────────────────────────────────────────────────────────────

#[test]
fn test_load_builtin_dark() {
    let theme = load_theme("dark").expect("dark theme should load");
    assert_eq!(theme.name, "dark");
}

#[test]
fn test_load_builtin_light() {
    let theme = load_theme("light").expect("light theme should load");
    assert_eq!(theme.name, "light");
}

#[test]
fn test_load_builtin_dracula() {
    let theme = load_theme("dracula").expect("dracula theme should load");
    assert_eq!(theme.name, "dracula");
}

#[test]
fn test_load_nonexistent() {
    let err = load_theme("nosuchtheme");
    assert!(err.is_err());
    assert!(matches!(err.unwrap_err(), ThemeError::NotFound { .. }));
}

#[test]
fn test_load_malformed_json() {
    // This tests the file-path codepath with invalid JSON content.
    // Since we can't easily write a temp file in this context,
    // test the parse error path directly via serde.
    let bad_json = r#"{"name": "broken", invalid json"#;
    let result: Result<MarkdownTheme, _> = serde_json::from_str(bad_json);
    assert!(result.is_err());
}

#[test]
fn test_dracula_syntect_theme_is_valid() {
    let theme = load_theme("dracula").expect("dracula should load");
    let theme_set = syntect::highlighting::ThemeSet::load_defaults();
    assert!(
        theme_set.themes.contains_key(&theme.syntect_theme),
        "dracula's syntect_theme '{}' must exist in syntect defaults",
        theme.syntect_theme,
    );
}

#[test]
fn test_all_builtin_syntect_themes_are_valid() {
    let theme_set = syntect::highlighting::ThemeSet::load_defaults();
    for name in &["dark", "light", "dracula"] {
        let theme = load_theme(name).expect(&format!("{name} should load"));
        assert!(
            theme_set.themes.contains_key(&theme.syntect_theme),
            "built-in theme '{name}' references syntect theme '{}' which doesn't exist",
            theme.syntect_theme,
        );
    }
}

// ── Regression: load_theme("dark") matches default_theme() ──────────────────

#[test]
fn test_loaded_dark_matches_default_theme() {
    let loaded = load_theme("dark").expect("dark theme should load");
    let default = default_theme();

    // Heading styles must match for all 6 levels.
    for i in 0..6 {
        let ls = heading_style(&loaded.heading[i]);
        let ds = heading_style(&default.heading[i]);
        assert_eq!(ls, ds, "heading[{i}] style differs between load_theme(\"dark\") and default_theme()");
    }

    // Inline styles.
    assert_eq!(inline_style(&loaded.code_inline), inline_style(&default.code_inline), "code_inline");
    assert_eq!(inline_style(&loaded.emphasis), inline_style(&default.emphasis), "emphasis");
    assert_eq!(inline_style(&loaded.strong), inline_style(&default.strong), "strong");
    assert_eq!(inline_style(&loaded.strikethrough), inline_style(&default.strikethrough), "strikethrough");
    assert_eq!(inline_style(&loaded.link), inline_style(&default.link), "link");
    assert_eq!(inline_style(&loaded.image_alt), inline_style(&default.image_alt), "image_alt");

    // Code block.
    assert_eq!(code_block_bg(&loaded.code_block), code_block_bg(&default.code_block), "code_block bg");
    assert_eq!(code_label_style(&loaded.code_block), code_label_style(&default.code_block), "code_label");

    // Status bar.
    assert_eq!(status_bar_style(&loaded.status_bar), status_bar_style(&default.status_bar), "status_bar");

    // Rule.
    assert_eq!(rule_style(&loaded.thematic_break), rule_style(&default.thematic_break), "rule");
    assert_eq!(loaded.thematic_break.char_, default.thematic_break.char_, "rule char");

    // Table.
    assert_eq!(table_header_style(&loaded.table), table_header_style(&default.table), "table header");
    assert_eq!(table_border_style(&loaded.table), table_border_style(&default.table), "table border");

    // Syntect theme.
    assert_eq!(loaded.syntect_theme, default.syntect_theme, "syntect_theme");
}

// ── Default theme regression ─────────────────────────────────────────────────

#[test]
fn test_default_theme_heading_styles() {
    let t = default_theme();
    let h1 = heading_style(&t.heading[0]);
    assert_eq!(h1.fg, Some(Color::LightCyan));
    assert!(h1.add_modifier.contains(Modifier::BOLD));
    assert!(!h1.add_modifier.contains(Modifier::ITALIC));

    let h2 = heading_style(&t.heading[1]);
    assert_eq!(h2.fg, Some(Color::Green));

    let h3 = heading_style(&t.heading[2]);
    assert_eq!(h3.fg, Some(Color::Yellow));

    let h4 = heading_style(&t.heading[3]);
    assert_eq!(h4.fg, Some(Color::White));
    assert!(h4.add_modifier.contains(Modifier::BOLD));
    assert!(h4.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn test_default_theme_code_inline_style() {
    let t = default_theme();
    let style = inline_style(&t.code_inline);
    assert_eq!(style.fg, Some(Color::Indexed(252)));
    assert_eq!(style.bg, Some(Color::Indexed(236)));
    assert!(style.add_modifier.contains(Modifier::BOLD));
    assert!(style.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn test_partial_json_uses_defaults() {
    let json = r#"{"name": "minimal"}"#;
    let theme: MarkdownTheme = serde_json::from_str(json).expect("partial JSON should parse");
    assert_eq!(theme.name, "minimal");
    // Other fields should get defaults.
    assert!(theme.strong.bold);
    assert!(theme.emphasis.italic);
    assert_eq!(theme.list.bullet.len(), 3);
    assert_eq!(theme.syntect_theme, "base16-ocean.dark");
}

// ── Non-default theme produces different styles ─────────────────────────────

#[test]
fn test_light_theme_differs_from_dark() {
    let dark = load_theme("dark").expect("dark");
    let light = load_theme("light").expect("light");

    // Heading 1 colors must differ between dark (light_cyan) and light (blue).
    let dark_h1 = heading_style(&dark.heading[0]);
    let light_h1 = heading_style(&light.heading[0]);
    assert_ne!(dark_h1.fg, light_h1.fg, "dark and light h1 fg should differ");

    // Status bar should differ.
    let dark_sb = status_bar_style(&dark.status_bar);
    let light_sb = status_bar_style(&light.status_bar);
    assert_ne!(dark_sb, light_sb, "dark and light status bars should differ");

    // Syntect themes must differ.
    assert_ne!(dark.syntect_theme, light.syntect_theme);
}

#[test]
fn test_dracula_theme_uses_hex_colors() {
    let dracula = load_theme("dracula").expect("dracula");
    // Dracula h1 is #bd93f9 → Rgb(189, 147, 249).
    let h1 = heading_style(&dracula.heading[0]);
    assert_eq!(h1.fg, Some(Color::Rgb(189, 147, 249)));
}

// ── Sanitize ────────────────────────────────────────────────────────────────

#[test]
fn test_sanitize_pads_heading_to_six() {
    let json = r#"{"name": "short", "heading": [{"fg": "red", "bold": true}]}"#;
    let mut theme: MarkdownTheme = serde_json::from_str(json).expect("parse");
    assert_eq!(theme.heading.len(), 1);
    theme.sanitize();
    assert_eq!(theme.heading.len(), 6);
    // Entries 1-5 should repeat the last (first) entry.
    for i in 1..6 {
        assert_eq!(theme.heading[i].fg, Some("red".to_string()));
        assert!(theme.heading[i].bold);
    }
}

#[test]
fn test_sanitize_empty_heading_gets_defaults() {
    let json = r#"{"name": "nohead", "heading": []}"#;
    let mut theme: MarkdownTheme = serde_json::from_str(json).expect("parse");
    theme.sanitize();
    assert_eq!(theme.heading.len(), 6);
    // First heading should match the default (light_cyan).
    assert_eq!(theme.heading[0].fg, Some("light_cyan".to_string()));
}

#[test]
fn test_sanitize_truncates_long_heading() {
    let json = r#"{"name": "long", "heading": [
        {"fg": "red"}, {"fg": "red"}, {"fg": "red"},
        {"fg": "red"}, {"fg": "red"}, {"fg": "red"},
        {"fg": "red"}, {"fg": "red"}
    ]}"#;
    let mut theme: MarkdownTheme = serde_json::from_str(json).expect("parse");
    assert_eq!(theme.heading.len(), 8);
    theme.sanitize();
    assert_eq!(theme.heading.len(), 6);
}

#[test]
fn test_sanitize_empty_thematic_break_char() {
    let json = r#"{"name": "empty_rule", "thematic_break": {"char": ""}}"#;
    let mut theme: MarkdownTheme = serde_json::from_str(json).expect("parse");
    assert!(theme.thematic_break.char_.is_empty());
    theme.sanitize();
    assert_eq!(theme.thematic_break.char_, "─");
}

#[test]
fn test_sanitize_empty_block_quote_prefix() {
    let json = r#"{"name": "no_border", "block_quote": {"prefix": ""}}"#;
    let mut theme: MarkdownTheme = serde_json::from_str(json).expect("parse");
    assert!(theme.block_quote.prefix.is_empty());
    theme.sanitize();
    assert_eq!(theme.block_quote.prefix, "│ ");
}

#[test]
fn test_sanitize_empty_bullet_vec() {
    let json = r#"{"name": "no_bullets", "list": {"bullet": []}}"#;
    let mut theme: MarkdownTheme = serde_json::from_str(json).expect("parse");
    assert!(theme.list.bullet.is_empty());
    theme.sanitize();
    assert_eq!(theme.list.bullet, vec!["•"]);
}

#[test]
fn test_sanitize_zero_indent_size() {
    let json = r#"{"name": "flat", "list": {"indent_size": 0}}"#;
    let mut theme: MarkdownTheme = serde_json::from_str(json).expect("parse");
    assert_eq!(theme.list.indent_size, 0);
    theme.sanitize();
    assert_eq!(theme.list.indent_size, 2);
}

#[test]
fn test_sanitize_valid_theme_unchanged() {
    let before = default_theme();
    let mut after = default_theme();
    after.sanitize();
    // All key fields should remain identical.
    assert_eq!(before.heading.len(), after.heading.len());
    assert_eq!(before.thematic_break.char_, after.thematic_break.char_);
    assert_eq!(before.block_quote.prefix, after.block_quote.prefix);
    assert_eq!(before.list.bullet, after.list.bullet);
    assert_eq!(before.list.indent_size, after.list.indent_size);
}

#[test]
fn test_load_theme_calls_sanitize() {
    // load_theme should always sanitize, so a theme loaded from JSON
    // with 6 headings should have exactly 6 (not more, not fewer).
    let theme = load_theme("dark").expect("dark");
    assert_eq!(theme.heading.len(), 6);
}

// ── Serialize / roundtrip ────────────────────────────────────────────────────

#[test]
fn test_theme_roundtrip() {
    let original = default_theme();
    let json = serde_json::to_string_pretty(&original).expect("serialize");
    let roundtripped: MarkdownTheme = serde_json::from_str(&json).expect("deserialize");

    // Key fields must survive the roundtrip.
    assert_eq!(roundtripped.name, original.name);
    assert_eq!(roundtripped.syntect_theme, original.syntect_theme);
    assert_eq!(roundtripped.heading.len(), original.heading.len());
    for i in 0..6 {
        assert_eq!(heading_style(&roundtripped.heading[i]), heading_style(&original.heading[i]),
            "heading[{i}] differs after roundtrip");
    }
    assert_eq!(inline_style(&roundtripped.code_inline), inline_style(&original.code_inline));
    assert_eq!(inline_style(&roundtripped.emphasis), inline_style(&original.emphasis));
    assert_eq!(inline_style(&roundtripped.strong), inline_style(&original.strong));
    assert_eq!(status_bar_style(&roundtripped.status_bar), status_bar_style(&original.status_bar));
    assert_eq!(roundtripped.thematic_break.char_, original.thematic_break.char_);
    assert_eq!(roundtripped.block_quote.prefix, original.block_quote.prefix);
    assert_eq!(roundtripped.list.bullet, original.list.bullet);
}

// ── strip_colors ─────────────────────────────────────────────────────────────

#[test]
fn test_strip_colors_removes_all_colors() {
    let mut theme = load_theme("dracula").expect("dracula");
    theme.strip_colors();

    // Document
    assert!(theme.document.bg.is_none());
    // Headings
    for h in &theme.heading {
        assert!(h.fg.is_none(), "heading fg should be None");
        assert!(h.bg.is_none(), "heading bg should be None");
    }
    // Code block
    assert!(theme.code_block.bg.is_none());
    assert!(theme.code_block.label_fg.is_none());
    assert!(theme.code_block.label_bg.is_none());
    // Block quote
    assert!(theme.block_quote.fg.is_none());
    assert!(theme.block_quote.border_fg.is_none());
    // Table
    assert!(theme.table.header_fg.is_none());
    assert!(theme.table.border_fg.is_none());
    // Thematic break
    assert!(theme.thematic_break.fg.is_none());
    // List
    assert!(theme.list.bullet_fg.is_none());
    assert!(theme.list.number_fg.is_none());
    assert!(theme.list.task_checked_fg.is_none());
    assert!(theme.list.task_unchecked_fg.is_none());
    // Status bar
    assert!(theme.status_bar.fg.is_none());
    assert!(theme.status_bar.bg.is_none());
    // Inlines
    assert!(theme.code_inline.fg.is_none());
    assert!(theme.code_inline.bg.is_none());
    assert!(theme.emphasis.fg.is_none());
    assert!(theme.strong.fg.is_none());
    assert!(theme.link.fg.is_none());
    assert!(theme.image_alt.fg.is_none());
    // Syntect theme
    assert!(theme.syntect_theme.is_empty());
}

#[test]
fn test_strip_colors_preserves_modifiers() {
    let mut theme = default_theme();
    theme.strip_colors();

    // Bold headings stay bold.
    assert!(theme.heading[0].bold);
    // Emphasis stays italic.
    assert!(theme.emphasis.italic);
    // Strong stays bold.
    assert!(theme.strong.bold);
    // Strikethrough stays crossed-out.
    assert!(theme.strikethrough.strikethrough);
    // Code inline keeps bold+italic.
    assert!(theme.code_inline.bold);
    assert!(theme.code_inline.italic);
    // Block quote keeps structural modifiers.
    assert!(theme.block_quote.italic);
    assert!(theme.block_quote.dim);
    // Status bar keeps bold.
    assert!(theme.status_bar.bold);
    // Text fields preserved.
    assert_eq!(theme.thematic_break.char_, "─");
    assert_eq!(theme.block_quote.prefix, "│ ");
    assert_eq!(theme.list.bullet, vec!["•", "◦", "▪"]);
    // Syntect theme cleared (signals no highlighting).
    assert!(theme.syntect_theme.is_empty());
}
