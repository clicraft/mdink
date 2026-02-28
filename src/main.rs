//! Entry point: wires CLI → parser → layout → TUI event loop.
//!
//! This is the thin orchestrator that connects all pipeline stages.
//! It handles CLI argument parsing, file I/O, terminal initialization,
//! the event loop, and graceful shutdown.

mod app;
mod cli;
mod config;
mod highlight;
mod images;
mod layout;
mod parser;
mod renderer;
mod theme;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;
use ratatui::crossterm::event::{self, Event};

use crate::app::App;
use crate::cli::Cli;
use crate::images::ImageManager;
use crate::parser::RenderedBlock;

/// Set to `true` immediately after `ratatui::init()` so the panic hook knows
/// whether the terminal has been initialised and needs restoring.
///
/// Calling `ratatui::restore()` before `ratatui::init()` sends spurious
/// escape sequences to the terminal, which can corrupt the calling shell's
/// display on some terminals and multiplexers.
static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

// BSD-style exit codes for pre-terminal-init errors.
const EX_DATAERR: i32 = 65; // file too large
const EX_NOINPUT: i32 = 66; // file not found / unreadable

fn main() -> color_eyre::Result<()> {
    // Install color_eyre error/panic hooks for pretty backtraces.
    color_eyre::install()?;

    // Chain our panic hook to restore the terminal before printing the backtrace.
    // The restore is guarded by TERMINAL_ACTIVE so it only runs after ratatui::init().
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if TERMINAL_ACTIVE.load(Ordering::SeqCst) {
            ratatui::restore();
        }
        original_hook(info);
    }));

    // Parse CLI arguments.
    let cli = Cli::parse();

    // Load config file (fail-safe: defaults on any error).
    let config = config::load_config();

    // Handle --list-themes early: print to stdout before terminal init, then exit.
    if cli.list_themes {
        println!("Built-in themes:");
        println!("  dark      Dark background with bright colors (default)");
        println!("  light     Light background with muted colors");
        println!("  dracula   Dracula color palette");
        println!();
        println!("Custom themes: place .json files in ~/.config/mdink/themes/");
        return Ok(());
    }

    // Resolve theme: --style flag > MDINK_STYLE env var > config.style > default.
    let theme_name = cli
        .style
        .clone()
        .or_else(|| std::env::var("MDINK_STYLE").ok())
        .or(config.style);
    let mut theme = match theme_name {
        Some(name) => theme::load_theme(&name)?,
        None => theme::default_theme(),
    };

    // Handle --dump-theme: print the resolved theme as-is, before NO_COLOR
    // stripping. This ensures the exported theme retains all color definitions
    // even when NO_COLOR is set in the environment.
    if cli.dump_theme {
        println!("{}", serde_json::to_string_pretty(&theme)?);
        return Ok(());
    }

    // NO_COLOR: strip colors if any source requests it.
    let no_color = cli.no_color
        || std::env::var_os("NO_COLOR").is_some()
        || config.no_color.unwrap_or(false);
    if no_color {
        theme.strip_colors();
    }

    // From here on, `file` is required (enforced by clap's required_unless_present_any).
    let file = cli.file.as_deref().expect("file argument is required here");

    // Read source from stdin or file, with appropriate guards.
    const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024; // 100 MB
    let (source, display_name, base_path) = if file == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .take(MAX_FILE_BYTES + 1)
            .read_to_string(&mut buf)?;
        if buf.len() as u64 > MAX_FILE_BYTES {
            eprintln!("<stdin>: input too large (limit is {MAX_FILE_BYTES} bytes)");
            process::exit(EX_DATAERR);
        }
        (buf, "<stdin>".to_string(), PathBuf::from("."))
    } else {
        // Guard against OOM: reject files that exceed a reasonable size threshold.
        // The check happens before ratatui::init() so the error prints to the normal
        // terminal instead of a raw alternate screen.
        let metadata = match fs::metadata(file) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{file}: {e}");
                process::exit(EX_NOINPUT);
            }
        };
        if metadata.len() > MAX_FILE_BYTES {
            eprintln!(
                "{file}: file too large ({} bytes; limit is {MAX_FILE_BYTES} bytes)",
                metadata.len()
            );
            process::exit(EX_DATAERR);
        }

        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                eprintln!("{file}: not valid UTF-8 text");
                process::exit(EX_DATAERR);
            }
            Err(e) => {
                eprintln!("{file}: {e}");
                process::exit(EX_NOINPUT);
            }
        };

        // Sanitize filename for display: strip control characters and ANSI escape
        // sequences so a crafted filename cannot inject terminal escape codes into
        // the status bar output.
        let safe_filename = file
            .chars()
            .filter(|c| !c.is_control())
            .collect::<String>();

        let bp = Path::new(file)
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();

        (content, safe_filename, bp)
    };

    // Resolve no_images and ascii_images from CLI flag or config.
    let no_images = cli.no_images || config.no_images.unwrap_or(false);
    let ascii_images = cli.ascii_images || config.ascii_images.unwrap_or(false);

    // Load syntax highlighting resources (expensive, done once).
    let highlighter = highlight::Highlighter::new();

    // Get terminal size early — needed for image cell dimension computation.
    let (cols, _rows) = ratatui::crossterm::terminal::size()?;

    // Query terminal for graphics protocol support (Sixel/Kitty/iTerm2/halfblocks).
    // from_query_stdio() requires raw mode, so we enable it briefly and disable
    // before ratatui::init() takes over. Failure is non-fatal: images fall back to alt text.
    let picker = if no_images {
        None
    } else {
        ratatui::crossterm::terminal::enable_raw_mode().ok();
        let p = ratatui_image::picker::Picker::from_query_stdio().ok();
        let _ = ratatui::crossterm::terminal::disable_raw_mode();
        p
    };

    let mut image_manager = ImageManager::new(base_path, picker, cols, no_images, ascii_images);

    // Parse markdown into IR blocks. Kept mutable so refresh can re-parse.
    let mut blocks = parser::parse(&source, &highlighter, &mut image_manager, &theme);

    // Flatten blocks into document lines at the current width.
    let document = layout::flatten(&blocks, cols, &theme);

    // Create the application state.
    let mut app = App::new(document, display_name, theme);

    // Initialize the terminal (enters raw mode + alternate screen).
    // TERMINAL_ACTIVE must be set immediately after so the panic hook is correct.
    let mut terminal = ratatui::init();
    TERMINAL_ACTIVE.store(true, Ordering::SeqCst);

    // Main event loop.
    let result = run_event_loop(
        &mut terminal,
        &mut app,
        &mut blocks,
        &mut image_manager,
        &source,
        &highlighter,
    );

    // Always restore the terminal, even if the loop returned an error.
    ratatui::restore();

    result
}

/// Computes the content width available for document layout.
///
/// When a side panel is active (outline open + wide terminal), the panel
/// and its border consume space from the left. Otherwise, the full
/// terminal width is available.
fn compute_content_width(cols: u16, app: &App) -> u16 {
    if app.outline.is_some() && cols >= renderer::OUTLINE_MIN_COLS {
        let panel_w = app.outline_panel_cols(cols);
        cols.saturating_sub(panel_w + 1 + renderer::OUTLINE_CONTENT_PAD).max(1)
    } else {
        cols
    }
}

/// Runs the TUI event loop until the user quits or an error occurs.
///
/// Separated from `main()` so that `ratatui::restore()` always runs
/// regardless of how this function exits. Takes mutable blocks so
/// refresh can re-parse, and source/highlighter for re-parsing.
fn run_event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    blocks: &mut Vec<RenderedBlock>,
    image_manager: &mut ImageManager,
    source: &str,
    highlighter: &highlight::Highlighter,
) -> color_eyre::Result<()> {
    let mut cols = terminal.size()?.width;

    loop {
        // Update viewport height from current terminal size.
        app.viewport_height = terminal.size()?.height.saturating_sub(1) as usize;

        // Draw the current frame.
        terminal.draw(|frame| renderer::draw(frame, app, image_manager))?;

        // Block until the next event.
        let event = event::read()?;

        match event {
            Event::Key(key) => {
                app.handle_key(key);
                if app.refresh_requested {
                    let size = terminal.size()?;
                    let cols = size.width;
                    app.viewport_height = size.height.saturating_sub(1) as usize;
                    image_manager.update_max_width(cols);
                    image_manager.clear_protocols();
                    *blocks = parser::parse(source, highlighter, image_manager, &app.theme);
                    app.document = layout::flatten(blocks, cols, &app.theme);
                    let max = app.max_scroll();
                    if app.scroll_offset > max {
                        app.scroll_offset = max;
                    }
                    app.refresh_requested = false;
                }
            }
            Event::Resize(new_cols, _rows) => {
                cols = new_cols;
                let w = compute_content_width(cols, app);
                app.document = layout::flatten(blocks, w, &app.theme);
                let max = app.max_scroll();
                if app.scroll_offset > max {
                    app.scroll_offset = max;
                }
            }
            // Ignore mouse, focus, and paste events.
            _ => {}
        }

        // Auto-close outline in dropdown mode when jumping.
        if app.pending_jump.is_some() && cols < renderer::OUTLINE_MIN_COLS {
            app.outline = None;
            app.needs_reflatten = true;
        }

        // Re-flatten if outline was toggled (side panel changes content width).
        if app.needs_reflatten {
            app.needs_reflatten = false;
            let w = compute_content_width(cols, app);
            app.document = layout::flatten(blocks, w, &app.theme);
            let max = app.max_scroll();
            if app.scroll_offset > max {
                app.scroll_offset = max;
            }
        }

        // Resolve pending heading jump against the (possibly re-flattened) document.
        if let Some(heading_idx) = app.pending_jump.take() {
            if let Some(entry) = app.document.headings.get(heading_idx) {
                app.scroll_offset = entry.line_index.min(app.max_scroll());
            } else {
                debug_assert!(
                    false,
                    "pending_jump index {} out of bounds for {} headings",
                    heading_idx,
                    app.document.headings.len()
                );
            }
        }

        if app.quit {
            break;
        }
    }

    Ok(())
}
