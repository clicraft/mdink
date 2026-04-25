# Feature: Terminal Diagnostics & Halfblocks Fallback

> **Prerequisites:** Phase 4 (images) complete, Feature 7 (LaTeX math) complete
> **Standards:** All code must follow [standards.md](standards.md)

**Goal:** Make the terminal→math rendering decision chain observable via logging,
and disable math pixel rendering when only Halfblocks protocol is available.

---

## Problem

Different terminal emulators display LaTeX formula images with different quality:

| Terminal | Protocol | Math result |
|----------|----------|-------------|
| WezTerm | Sixel/Kitty | Crisp pixel images |
| Kitty | Kitty | Crisp pixel images |
| iTerm2 | iTerm2 | Crisp pixel images |
| lxterminal | Halfblocks | Blurry 2-vertical-pixel images |
| screen/tmux (no graphics) | None | Unicode text (correct fallback) |

The decision chain was completely invisible — no logging existed for:
- Which terminal is running
- Whether the picker query succeeded or failed
- Which protocol was detected
- Font cell size
- Why MathEngine was enabled or disabled
- Whether background color was detected

---

## Solution

### 1. Diagnostic Logging

All new log statements use `info` level (invisible at default `Warn` level).

**Terminal identity** (`detect_terminal_identity()` in `main.rs`):
Checks `TERM_PROGRAM`, `KITTY_WINDOW_ID`, `KITTY_PID`, `WEZTERM_PANE`,
`WEZTERM_EXECUTABLE`, `GHOSTTY_RESOURCES_DIR`, `ALACRITTY_WINDOW_ID`, `WT_SESSION`,
`TERM`. Returns a human-readable string like `"wezterm (WEZTERM_PANE)"`.

**Picker result logging:** The `from_query_stdio()` call now logs its result:
- `Ok(picker)` → logs protocol type and font size
- `Err(e)` → logs the failure reason

**Protocol quality classification:** After picker creation, classifies quality:
- Sixel/Kitty/iTerm2 → "high quality"
- Halfblocks → "reduced quality"
- None → "no graphics support"

**Background color:** Logs detected RGB or "not detected (using black)".

**MathEngine state:** `MathEngine::new()` logs `user_enabled`, `graphics_available`,
and the computed `enabled` state.

### 2. Halfblocks Fallback

Halfblocks protocol renders images using `▀` (upper half) and `▄` (lower half)
characters with foreground/background colors. This gives only **2 vertical pixels
per terminal cell** — adequate for photos but illegible for mathematical notation.

When Halfblocks is the only available protocol, math pixel rendering is disabled
(`graphics_available = false` for MathEngine). Formulas fall back to Unicode text,
which is sharper and more readable than blurry halfblock images.

Regular images (via ImageManager) are **not** affected — they still use whatever
protocol is available, including Halfblocks.

### 3. Files Changed

| File | Change |
|------|--------|
| `src/main.rs` | `detect_terminal_identity()`, picker logging, protocol quality classification, bg color logging, `math_has_high_quality` computation |
| `src/math/mod.rs` | `log::info!` in `MathEngine::new()` |
| `src/math/tests.rs` | Protocol quality tests (Halfblocks/Sixel/Kitty/iTerm2/None) |

### 4. Tests

| Test | What it verifies |
|------|-----------------|
| `test_halfblocks_is_not_high_quality` | Halfblocks → not high quality |
| `test_sixel_is_high_quality` | Sixel → high quality |
| `test_kitty_is_high_quality` | Kitty → high quality |
| `test_iterm2_is_high_quality` | iTerm2 → high quality |
| `test_no_protocol_is_not_high_quality` | No picker → not high quality |
| `test_halfblocks_disables_math_engine` | `MathEngine::new(true, false)` → disabled |
| `test_high_quality_enables_math_engine` | `MathEngine::new(true, true)` → enabled |
