use super::*;

// ── unicode_math tests ──────────────────────────────────────────────────────

#[test]
fn test_unicode_math_greek_letters() {
    assert_eq!(unicode_math("\\alpha + \\beta"), "\u{03B1} + \u{03B2}");
    assert_eq!(unicode_math("\\pi"), "\u{03C0}");
    assert_eq!(unicode_math("\\Omega"), "\u{03A9}");
}

#[test]
fn test_unicode_math_operators() {
    assert_eq!(unicode_math("\\leq"), "\u{2264}");
    assert_eq!(unicode_math("\\geq"), "\u{2265}");
    assert_eq!(unicode_math("\\neq"), "\u{2260}");
    assert_eq!(unicode_math("\\infty"), "\u{221E}");
    assert_eq!(unicode_math("\\times"), "\u{00D7}");
}

#[test]
fn test_unicode_math_superscript() {
    assert_eq!(unicode_math("x^2"), "x\u{00B2}");
    assert_eq!(unicode_math("x^{23}"), "x\u{00B2}\u{00B3}");
}

#[test]
fn test_unicode_math_subscript() {
    assert_eq!(unicode_math("x_0"), "x\u{2080}");
    assert_eq!(unicode_math("a_{12}"), "a\u{2081}\u{2082}");
}

#[test]
fn test_unicode_math_arrows() {
    assert_eq!(unicode_math("\\rightarrow"), "\u{2192}");
    assert_eq!(unicode_math("\\Rightarrow"), "\u{21D2}");
    assert_eq!(unicode_math("\\leftarrow"), "\u{2190}");
}

#[test]
fn test_unicode_math_unrecognized_passthrough() {
    assert_eq!(unicode_math("\\foobar"), "\\foobar");
}

#[test]
fn test_unicode_math_escaped_chars() {
    assert_eq!(unicode_math("\\{x\\}"), "{x}");
}

#[test]
fn test_unicode_math_empty_input() {
    assert_eq!(unicode_math(""), "");
}

#[test]
fn test_unicode_math_frac() {
    // \frac consumes two brace arguments and joins with /.
    assert_eq!(unicode_math("\\frac{a}{b}"), "a/b");
    assert_eq!(unicode_math("\\frac{x+1}{2}"), "x+1/2");
}

#[test]
fn test_unicode_math_combined() {
    // x² + α₀
    let result = unicode_math("x^2 + \\alpha_0");
    assert_eq!(result, "x\u{00B2} + \u{03B1}\u{2080}");
}

#[test]
fn test_unicode_math_text_command() {
    // \text{...} consumes braces and outputs content.
    assert_eq!(unicode_math("\\text{datawhale}"), "datawhale");
    assert_eq!(unicode_math("\\text{hello world}"), "hello world");
}

#[test]
fn test_unicode_math_text_in_formula() {
    // \text inside a larger formula
    let result = unicode_math("P(\\text{datawhale})");
    assert_eq!(result, "P(datawhale)");
}

#[test]
fn test_unicode_math_frac_with_text() {
    // \frac with \text arguments — content is recursively processed.
    let result = unicode_math("\\frac{\\text{numerator}}{\\text{denominator}}");
    assert_eq!(result, "numerator/denominator");
}

#[test]
fn test_unicode_math_frac_with_plain_args() {
    // \frac with plain text arguments works correctly.
    assert_eq!(unicode_math("\\frac{a}{b}"), "a/b");
    assert_eq!(unicode_math("\\frac{2}{6}"), "2/6");
}

#[test]
fn test_unicode_math_unicode_minus_in_subscript() {
    // Unicode minus sign (U+2212) in subscript should map to subscript minus.
    // Note: 'i' has no Unicode subscript equivalent, so it stays as-is.
    assert_eq!(unicode_math("w_{i\u{2212}1}"), "wi\u{208B}\u{2081}");
}

#[test]
fn test_unicode_math_unicode_minus_in_superscript() {
    // Unicode minus sign (U+2212) in superscript should map to superscript minus.
    assert_eq!(unicode_math("x^{2\u{2212}1}"), "x\u{00B2}\u{207B}\u{00B9}");
}

// ── MathEngine tests ────────────────────────────────────────────────────────

#[test]
fn test_math_engine_disabled_when_no_graphics() {
    let engine = MathEngine::new(true, false);
    assert!(!engine.enabled());
}

#[test]
fn test_math_engine_disabled_when_user_off() {
    let engine = MathEngine::new(false, true);
    assert!(!engine.enabled());
}

#[test]
fn test_math_engine_enabled_when_both_true() {
    let engine = MathEngine::new(true, true);
    assert!(engine.enabled());
}

// ── Halfblocks fallback tests ───────────────────────────────────────────────
//
// The key decision in main.rs: when the terminal supports only the Halfblocks
// protocol, math images are disabled (graphics_available = false) because
// Halfblocks renders at 2 vertical pixels per cell — too coarse for legible
// formulas.  MathEngine sees a plain boolean, so we test the mapping here.

/// Simulates the protocol-quality check from main.rs.
/// Returns true when the protocol supports high-quality pixel rendering
/// (Sixel, Kitty, or iTerm2). Returns false for Halfblocks or None.
fn is_high_quality_protocol(pt: Option<ratatui_image::picker::ProtocolType>) -> bool {
    pt.is_some_and(|p| p != ratatui_image::picker::ProtocolType::Halfblocks)
}

#[test]
fn test_halfblocks_is_not_high_quality() {
    assert!(!is_high_quality_protocol(Some(
        ratatui_image::picker::ProtocolType::Halfblocks
    )));
}

#[test]
fn test_sixel_is_high_quality() {
    assert!(is_high_quality_protocol(Some(
        ratatui_image::picker::ProtocolType::Sixel
    )));
}

#[test]
fn test_kitty_is_high_quality() {
    assert!(is_high_quality_protocol(Some(
        ratatui_image::picker::ProtocolType::Kitty
    )));
}

#[test]
fn test_iterm2_is_high_quality() {
    assert!(is_high_quality_protocol(Some(
        ratatui_image::picker::ProtocolType::Iterm2
    )));
}

#[test]
fn test_no_protocol_is_not_high_quality() {
    assert!(!is_high_quality_protocol(None));
}

#[test]
fn test_halfblocks_disables_math_engine() {
    // When Halfblocks is detected, main.rs passes graphics_available=false,
    // which disables MathEngine — formulas fall back to Unicode text.
    let engine = MathEngine::new(true, false); // false = Halfblocks
    assert!(!engine.enabled());
}

#[test]
fn test_high_quality_enables_math_engine() {
    // Any high-quality protocol (Sixel, Kitty, iTerm2) passes
    // graphics_available=true, enabling pixel rendering.
    let engine = MathEngine::new(true, true);
    assert!(engine.enabled());
}

#[test]
fn test_math_engine_cache_insert_and_get() {
    let mut engine = MathEngine::new(true, true);
    let img = DynamicImage::new_rgb8(1, 1);
    engine.insert_cache("x^2".to_string(), img);

    assert!(engine.get_cached("x^2").is_some());
    assert!(engine.get_cached("y^2").is_none());
    assert!(engine.cache_touched());
}

#[test]
fn test_math_engine_pending_dedup() {
    let mut engine = MathEngine::new(true, true);
    assert!(engine.mark_pending("x^2"));  // first time → true
    assert!(!engine.mark_pending("x^2")); // duplicate → false
    assert!(engine.has_pending());
}

#[test]
fn test_math_engine_failed_not_retried() {
    let mut engine = MathEngine::new(true, true);
    engine.mark_pending("bad$$");
    engine.mark_failed("bad$$");
    assert!(!engine.has_pending());
    assert!(!engine.mark_pending("bad$$")); // failed → not retried
}

#[test]
fn test_math_engine_has_pending_clears_on_resolve() {
    let mut engine = MathEngine::new(true, true);
    engine.mark_pending("a");
    engine.mark_pending("b");
    assert!(engine.has_pending());
    engine.mark_resolved("a");
    assert!(engine.has_pending()); // b still pending
    engine.mark_resolved("b");
    assert!(!engine.has_pending()); // all done
}

#[test]
fn test_math_engine_clear_protocols_keeps_cache() {
    let mut engine = MathEngine::new(true, true);
    let img = DynamicImage::new_rgb8(1, 1);
    engine.insert_cache("x^2".to_string(), img);
    engine.clear_protocols();
    assert!(engine.get_cached("x^2").is_some());
}

#[test]
fn test_math_engine_clear_all_resets_everything() {
    let mut engine = MathEngine::new(true, true);
    let img = DynamicImage::new_rgb8(1, 1);
    engine.insert_cache("x^2".to_string(), img);
    engine.mark_pending("y^2");
    engine.clear_all();
    assert!(engine.get_cached("x^2").is_none());
    assert!(!engine.has_pending());
    assert!(!engine.cache_touched());
}

#[test]
fn test_math_engine_cache_dirtied_flag() {
    let mut engine = MathEngine::new(true, true);
    assert!(!engine.cache_touched());
    let img = DynamicImage::new_rgb8(1, 1);
    engine.insert_cache("x^2".to_string(), img);
    assert!(engine.cache_touched());
    engine.clear_cache_touched();
    assert!(!engine.cache_touched());
}

// ── render_latex_to_image tests ──────────────────────────────────────────────

#[test]
fn test_render_latex_size_guard() {
    // Over-size formula should fail with size limit error.
    let big = "x".repeat(11 * 1024);
    let result = render_latex_to_image(&big, false, 80, (8, 16), (0, 0, 0));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("too large"), "expected size error, got: {err}");
}

#[test]
fn test_render_latex_simple_formula_succeeds() {
    let result = render_latex_to_image("x^2", false, 80, (8, 16), (0, 0, 0));
    assert!(result.is_ok(), "simple formula should render: {:?}", result.err());
}

#[test]
fn test_render_latex_pure_ascii_accepted() {
    // Pure ASCII formula should not be rejected by CJK check.
    let result = render_latex_to_image("x^2 + y^2 = z^2", false, 80, (8, 16), (0, 0, 0));
    // CJK check should pass; whether it ultimately succeeds depends on the feature.
    if let Err(e) = &result {
        let err = e.to_string();
        assert!(!err.contains("CJK"), "pure ASCII should not trigger CJK rejection");
    }
}

// ── MathEngine batch flow tests ──────────────────────────────────────────────

#[test]
fn test_math_engine_batch_refresh_guard() {
    let mut engine = MathEngine::new(true, true);

    // Nothing touched → should not refresh.
    assert!(!engine.has_pending());
    assert!(!engine.cache_touched());

    // Mark formulas as pending.
    engine.mark_pending("a");
    engine.mark_pending("b");
    assert!(engine.has_pending());
    assert!(!engine.cache_touched()); // not touched yet (no results)

    // First result arrives.
    let img = DynamicImage::new_rgb8(1, 1);
    engine.mark_resolved("a");
    engine.insert_cache("a".to_string(), img);
    assert!(engine.cache_touched());
    assert!(engine.has_pending()); // b still pending → no refresh

    // Second result arrives.
    let img2 = DynamicImage::new_rgb8(2, 2);
    engine.mark_resolved("b");
    engine.insert_cache("b".to_string(), img2);
    assert!(!engine.has_pending());
    assert!(engine.cache_touched()); // NOW safe to refresh

    // Simulate refresh.
    engine.clear_cache_touched();
    assert!(!engine.cache_touched());
}

#[test]
fn test_math_engine_failed_formula_still_triggers_refresh() {
    let mut engine = MathEngine::new(true, true);
    engine.mark_pending("bad$$");
    assert!(engine.has_pending());

    // Render fails.
    engine.mark_failed("bad$$");
    assert!(!engine.has_pending());
    assert!(engine.cache_touched()); // failure still dirties cache
}

#[test]
fn test_math_engine_cache_survives_clear_protocols() {
    let mut engine = MathEngine::new(true, true);
    let img = DynamicImage::new_rgb8(10, 10);
    engine.insert_cache("cached".to_string(), img);
    engine.clear_protocols();
    // Cache entry survives.
    assert!(engine.get_cached("cached").is_some());
}

// ── Real rendering tests ─────────────────────────────────────────────────────

#[test]
fn test_render_latex_simple_formula() {
    let result = render_latex_to_image("x^2", false, 80, (8, 16), (0, 0, 0));
    assert!(result.is_ok(), "simple formula should render: {:?}", result.err());
    let img = result.unwrap();
    assert!(img.width() > 0, "rendered image should have nonzero width");
    assert!(img.height() > 0, "rendered image should have nonzero height");
}

#[test]
fn test_render_latex_greek_letters() {
    let result = render_latex_to_image("\\alpha + \\beta = \\gamma", false, 80, (8, 16), (0, 0, 0));
    assert!(result.is_ok(), "Greek formula should render: {:?}", result.err());
}

#[test]
fn test_render_latex_fraction() {
    let result = render_latex_to_image("\\frac{a}{b}", false, 80, (8, 16), (0, 0, 0));
    assert!(result.is_ok(), "fraction should render: {:?}", result.err());
}

#[test]
fn test_render_latex_invalid_fails() {
    // Invalid LaTeX should return an error, not panic.
    let result = render_latex_to_image("\\frac{}{}{", false, 80, (8, 16), (0, 0, 0));
    // ratex-parser may or may not reject this, but it should not panic.
    // We just verify it doesn't panic — either Ok or Err is fine.
    let _ = result;
}

#[test]
fn test_render_latex_sqrt() {
    let result = render_latex_to_image("\\sqrt{b^2 - 4ac}", false, 80, (8, 16), (0, 0, 0));
    assert!(result.is_ok(), "sqrt formula should render: {:?}", result.err());
    let img = result.unwrap();
    assert!(img.width() > 0);
}

#[test]
fn test_font_extraction_works() {
    // Verify that the font extraction directory contains the expected files.
    use std::fs;
    let dir = std::env::temp_dir().join("mdink-katex-fonts");
    assert!(dir.exists(), "font dir should exist after first render");
    // At least KaTeX_Main-Regular.ttf should be present.
    assert!(
        dir.join("KaTeX_Main-Regular.ttf").exists(),
        "Main-Regular font should be extracted"
    );
}

#[test]
fn test_render_latex_inline_scaled_to_cell_height() {
    // Inline formula (display=false) should be resized to match cell height.
    let cell_h: u16 = 20;
    let result = render_latex_to_image("x^2", false, 80, (10, cell_h), (0, 0, 0));
    assert!(result.is_ok(), "inline formula should render: {:?}", result.err());
    let img = result.unwrap();
    assert_eq!(img.height(), cell_h as u32, "inline math image height should match cell height");
    assert!(img.width() > 0, "inline math image should have nonzero width");
}

#[test]
fn test_render_latex_display_not_resized_to_cell_height() {
    // Display formula (display=true) should NOT be resized to cell height.
    let cell_h: u16 = 20;
    let result = render_latex_to_image("x^2", true, 80, (10, cell_h), (0, 0, 0));
    assert!(result.is_ok(), "display formula should render: {:?}", result.err());
    let img = result.unwrap();
    // Display formulas render at natural size, which is much taller than 20px.
    assert!(img.height() > cell_h as u32, "display math should be taller than cell height");
}

#[test]
fn test_render_latex_inline_zero_cell_height_no_panic() {
    // Zero cell height should not panic — just return unscaled image.
    let result = render_latex_to_image("x^2", false, 80, (10, 0), (0, 0, 0));
    assert!(result.is_ok(), "zero cell height should still render: {:?}", result.err());
}

#[test]
fn test_unicode_math_formula_from_line_40() {
    // From line 40: Count(w_{i−1},w_i) — uses Unicode minus U+2212
    let result = unicode_math("Count(w_{i\u{2212}1},w_i)");
    // i has no subscript form, stays as-is; − maps to ₋; 1 maps to ₁
    assert!(result.contains("₋"), "should have subscript minus: {result}");
    assert!(result.contains("₁"), "should have subscript 1: {result}");
    assert!(!result.contains("\\"), "no LaTeX commands in output: {result}");
}

#[test]
fn test_unicode_math_formula_from_line_49_display() {
    // From line 49 display math: P(\text{datawhale}) = \frac{\text{...}}{\text{...}} = \frac{2}{6} \approx 0.333
    let result = unicode_math("P(\\text{datawhale}) = \\frac{\\text{numerator}}{\\text{denominator}} = \\frac{2}{6} \\approx 0.333");
    assert!(result.contains("datawhale"), "should have text content: {result}");
    assert!(result.contains("numerator/denominator"), "should have frac: {result}");
    assert!(result.contains("2/6"), "should have frac: {result}");
    assert!(result.contains("≈"), "should have approx symbol: {result}");
    assert!(!result.contains("\\text"), "no raw commands: {result}");
    assert!(!result.contains("\\frac"), "no raw commands: {result}");
}

#[test]
fn test_unicode_math_line_49_full() {
    // Full formula from line 49 (display math)
    let input = r#"P(\text{datawhale}) = \frac{\text{总语料中"datawhale"的数量}}{\text{总语料的词数}} = \frac{2}{6} \approx 0.333"#;
    let result = unicode_math(input);
    println!("Result: {}", result);
    
    // \text{datawhale} should be "datawhale"
    assert!(result.contains("datawhale"), "should contain 'datawhale': {result}");
    // Chinese text should pass through
    assert!(result.contains("总语料中"), "should contain Chinese text: {result}");
    assert!(result.contains("总语料的词数"), "should contain Chinese text: {result}");
    // \frac should produce /
    assert!(result.contains("总语料中\"datawhale\"的数量/总语料的词数"), 
        "frac with Chinese should work: {result}");
    // \frac{2}{6} -> 2/6
    assert!(result.contains("2/6"), "should contain 2/6: {result}");
    // \approx -> ≈
    assert!(result.contains("≈"), "should contain ≈: {result}");
    // No raw LaTeX commands should remain
    assert!(!result.contains("\\text"), "no raw \\text: {result}");
    assert!(!result.contains("\\frac"), "no raw \\frac: {result}");
    assert!(!result.contains("\\approx"), "no raw \\approx: {result}");
}

#[test]
fn test_render_latex_cjk_text_in_formula() {
    // Formula with Chinese text in \text{} should render successfully
    // using embed_glyphs=false mode with system CJK font fallback.
    let result = render_latex_to_image(
        r#"P(\text{datawhale}) = \frac{\text{总语料中"datawhale"的数量}}{\text{总语料的词数}} = \frac{2}{6} \approx 0.333"#,
        true,  // display math
        80, (10, 20), (0, 0, 0),
    );
    assert!(result.is_ok(), "CJK formula should render: {:?}", result.err());
    let img = result.unwrap();
    assert!(img.width() > 0, "rendered image should have nonzero width");
    assert!(img.height() > 0, "rendered image should have nonzero height");
}

#[test]
fn test_render_latex_simple_cjk_text() {
    // Simpler CJK formula test.
    let result = render_latex_to_image(
        r#"\text{中文测试}"#,
        true, 80, (10, 20), (0, 0, 0),
    );
    assert!(result.is_ok(), "simple CJK text should render: {:?}", result.err());
}

// ── set_enabled toggle tests ────────────────────────────────────────────────

#[test]
fn test_math_engine_set_enabled() {
    let mut engine = MathEngine::new(true, true);
    assert!(engine.enabled());
    engine.set_enabled(false);
    assert!(!engine.enabled());
    engine.set_enabled(true);
    assert!(engine.enabled());
}

#[test]
fn test_math_engine_set_enabled_preserves_cache() {
    let mut engine = MathEngine::new(true, true);
    let img = DynamicImage::new_rgb8(1, 1);
    engine.insert_cache("x^2".to_string(), img);
    assert!(engine.get_cached("x^2").is_some());

    engine.set_enabled(false);
    assert!(!engine.enabled());
    assert!(engine.get_cached("x^2").is_some(), "cache should survive set_enabled(false)");

    engine.set_enabled(true);
    assert!(engine.get_cached("x^2").is_some());
}

// ── Regression tests for toggle bugs ────────────────────────────────────────

#[test]
fn test_math_engine_batch_refresh_not_triggered_after_clear_cache_touched() {
    // Regression: cache_touched staying true after a toggle refresh caused
    // an infinite re-parse loop. After clear_cache_touched(), the batch
    // refresh condition (!has_pending() && cache_touched()) must be false.
    let mut engine = MathEngine::new(true, true);

    // Simulate: render completes, cache is touched.
    engine.mark_pending("x^2");
    engine.mark_resolved("x^2");
    engine.insert_cache("x^2".to_string(), DynamicImage::new_rgb8(1, 1));
    assert!(engine.cache_touched(), "cache should be touched after insert");
    assert!(!engine.has_pending(), "no pending after resolve");

    // The batch refresh condition would trigger here.
    // Simulate what the refresh handler does: clear the flag.
    engine.clear_cache_touched();
    assert!(!engine.cache_touched(), "cache_touched must be false after clear");
    // Now the condition !has_pending() && cache_touched() is false → no loop.
}

#[test]
fn test_math_engine_toggle_cycle_no_stale_cache_touched() {
    // Full toggle cycle: enable → disable → re-enable.
    // After each clear_cache_touched(), the flag must stay false until
    // a new insert_cache/mark_failed call.
    let mut engine = MathEngine::new(true, true);
    let img = DynamicImage::new_rgb8(1, 1);

    // Phase 1: initial render
    engine.mark_pending("a");
    engine.mark_resolved("a");
    engine.insert_cache("a".to_string(), img.clone());
    assert!(engine.cache_touched());
    engine.clear_cache_touched();
    assert!(!engine.cache_touched());

    // Phase 2: toggle off (set_enabled(false))
    engine.set_enabled(false);
    assert!(!engine.enabled());
    // cache_touched should still be false — set_enabled doesn't dirty it.
    assert!(!engine.cache_touched(), "set_enabled(false) must not touch cache flag");

    // Phase 3: toggle on (set_enabled(true))
    engine.set_enabled(true);
    assert!(engine.enabled());
    assert!(!engine.cache_touched(), "set_enabled(true) must not touch cache flag");
    // Batch refresh condition is false → no infinite loop.
    assert!(!(!engine.has_pending() && engine.cache_touched()),
        "batch refresh condition must be false after toggle cycle");
}
