# Feature 9 (Theme Cycling) + Feature 8 (Print Mode) — Implementation Plan

## Feature 9: Theme Cycling

### Overview
Add `t` keybinding to cycle through built-in themes: dark → light → dracula → dark.

### Changes

**`src/app/mod.rs`:**
- Add `theme_index: usize` field to `App` (tracks position in the theme cycle)
- Add `theme_cycle_requested: bool` field to `App`
- Add `t` keybinding in `handle_key()` that sets `theme_cycle_requested = true`
- Initialize `theme_index` based on current theme name in `App::new()`

**`src/main.rs`:**
- After checking `refresh_requested`, check `theme_cycle_requested`
- When true: increment `theme_index`, load next theme via `theme::load_theme()`, update `app.theme`, re-parse and re-flatten

**`src/renderer.rs`:**
- Show current theme name in status bar hints: `"t:theme(<name>)"` or similar

### Theme cycle list
```rust
const THEME_CYCLE: &[&str] = &["dark", "light", "dracula"];
```

### Edge cases
- `theme_index` wraps using modulo arithmetic
- NO_COLOR: theme cycling still works but colors remain stripped (apply `strip_colors()` after loading new theme)
- The `no_color` flag must be accessible in the event loop to know whether to strip

---

## Feature 8: Print Mode

### Overview
Add `--print` flag that renders the document to stdout without entering the TUI.

### Changes

**`src/cli.rs`:**
- Add `#[arg(long)] pub print: bool` field to `Cli`

**`src/main.rs`:**
- After parsing + layout, if `cli.print` is true:
  - Skip `ratatui::init()`, skip event loop
  - Walk `document.lines` and print each line to stdout using crossterm ANSI styling
  - For `DocumentLine::Text(line)` and `DocumentLine::Code(line)`: emit ANSI-styled text
  - For `DocumentLine::Empty`: print empty line
  - For `DocumentLine::Rule`: print dashes
  - For `ImageStart`/`ImageContinuation`: skip
  - For `AsciiArt(line)`: print styled text
  - Use `crossterm::terminal::size()` for width, fallback to 80

### ANSI output approach
Walk each `Line`'s `Span`s, convert ratatui `Style` to crossterm `ContentStyle`, and use `crossterm::style::PrintStyledContent` via `stdout().write()`.

### Edge cases
- No terminal initialization — `TERMINAL_ACTIVE` remains false
- `--no-color` strips styles before printing (already handled by theme.strip_colors())
- Picker query (for images) should be skipped in print mode
- Width detection: use `crossterm::terminal::size()` which works even without raw mode
