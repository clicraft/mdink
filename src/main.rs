//! Entry point: wires CLI → parser → layout → TUI event loop.
//!
//! This is the thin orchestrator that connects all pipeline stages.
//! It handles CLI argument parsing, file I/O, terminal initialization,
//! the event loop, and graceful shutdown.

mod app;
mod cli;
mod config;
mod font_detect;
mod highlight;
mod images;
mod layout;
mod parser;
mod pdf;
mod renderer;
mod theme;

use std::fs;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use clap::Parser;
use ratatui::crossterm::event::{self, Event};
use ratatui::crossterm::style::{
    Attribute, Color as CtColor, SetAttribute, SetForegroundColor,
    SetBackgroundColor, ResetColor, Print,
};

use crate::app::{App, THEME_CYCLE};
use crate::cli::Cli;
use crate::images::ImageManager;
use crate::layout::DocumentLine;
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

/// Maximum file size in bytes (100 MB). Checked before terminal init and
/// used as a cap for streaming input.
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

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

    // Read source from stdin, URL, or file, with appropriate guards.

    // Detect streaming mode: stdin is piped (not a TTY) and file is "-".
    let streaming = file == "-" && !std::io::stdin().is_terminal();

    let (mut source, display_name, base_path) = if file == "-" && !streaming {
        // Interactive stdin: read all at once (user typing then Ctrl+D).
        let mut buf = String::new();
        std::io::stdin()
            .take(MAX_FILE_BYTES + 1)
            .read_to_string(&mut buf)?;
        if buf.len() as u64 > MAX_FILE_BYTES {
            eprintln!("<stdin>: input too large (limit is {MAX_FILE_BYTES} bytes)");
            process::exit(EX_DATAERR);
        }
        (buf, "<stdin>".to_string(), PathBuf::from("."))
    } else if file == "-" {
        // Streaming stdin: start with empty content; reader thread will feed data.
        (String::new(), "<stdin> (streaming)".to_string(), PathBuf::from("."))
    } else if file.starts_with("http://") || file.starts_with("https://") {
        fetch_url(file, MAX_FILE_BYTES)
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
    // In print mode, fall back to 80 columns if terminal size detection fails.
    let cols = if cli.print {
        ratatui::crossterm::terminal::size().map(|(c, _)| c).unwrap_or(80)
    } else {
        ratatui::crossterm::terminal::size()?.0
    };

    // Skip image picker query in print mode (no TUI rendering).
    let picker = if no_images || cli.print {
        None
    } else {
        ratatui::crossterm::terminal::enable_raw_mode().ok();
        let p = ratatui_image::picker::Picker::from_query_stdio().ok();
        let _ = ratatui::crossterm::terminal::disable_raw_mode();
        p
    };

    let mut image_manager = ImageManager::new(base_path.clone(), picker, cols, no_images, ascii_images);

    // Parse markdown into IR blocks. Kept mutable so refresh can re-parse.
    let mut blocks = parser::parse(&source, &highlighter, &mut image_manager, &theme);

    // Flatten blocks into document lines at the current width.
    let document = layout::flatten(&blocks, cols, &theme);

    // Print mode: render to stdout without entering the TUI.
    if cli.print {
        return print_document(&document.lines, cols, &theme, no_color);
    }

    // Create the application state.
    let mut app = App::new(document, display_name, theme, base_path);

    // Initialize the terminal (enters raw mode + alternate screen).
    // TERMINAL_ACTIVE must be set immediately after so the panic hook is correct.
    let mut terminal = ratatui::init();
    TERMINAL_ACTIVE.store(true, Ordering::SeqCst);

    // Spawn streaming reader thread if in streaming mode.
    let stream_rx = if streaming {
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            use std::io::BufRead;
            let stdin = std::io::stdin();
            let reader = stdin.lock();
            for line in reader.lines() {
                match line {
                    Ok(text) => {
                        // Send line + newline to maintain markdown structure.
                        if tx.send(format!("{text}\n")).is_err() {
                            break; // Receiver dropped (main thread exited).
                        }
                    }
                    Err(_) => break, // Read error (e.g. broken pipe).
                }
            }
            // Sender drops here, signaling EOF to the main thread.
        });
        Some(rx)
    } else {
        None
    };

    // Resolve PDF font override: --pdf-font > config.pdf_font.
    let pdf_font_override = cli.pdf_font.or(config.pdf_font);

    // Main event loop.
    let result = run_event_loop(
        &mut terminal,
        &mut app,
        &mut blocks,
        &mut image_manager,
        &mut source,
        &highlighter,
        no_color,
        stream_rx,
        pdf_font_override,
    );

    // Always restore the terminal, even if the loop returned an error.
    ratatui::restore();

    result
}

/// Prints the pre-rendered document to stdout with ANSI styling.
///
/// Walks each `DocumentLine`, converting ratatui `Span` styles to crossterm
/// ANSI sequences. This bypasses the TUI entirely — no `ratatui::init()` is called.
fn print_document(
    lines: &[DocumentLine],
    width: u16,
    theme_ref: &theme::MarkdownTheme,
    no_color: bool,
) -> color_eyre::Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in lines {
        match line {
            DocumentLine::Text(l) | DocumentLine::Code(l) | DocumentLine::AsciiArt(l) => {
                print_styled_line(&mut out, l, no_color)?;
            }
            DocumentLine::Empty => {
                writeln!(out)?;
            }
            DocumentLine::Rule => {
                let char_ = &theme_ref.thematic_break.char_;
                let char_width = unicode_width::UnicodeWidthStr::width(char_.as_str()).max(1);
                let rule = char_.repeat(width as usize / char_width);
                if no_color {
                    writeln!(out, "{rule}")?;
                } else {
                    let style = theme::rule_style(&theme_ref.thematic_break);
                    print_with_style(&mut out, &rule, style)?;
                    writeln!(out)?;
                }
            }
            // Images cannot be rendered to stdout — skip silently.
            DocumentLine::ImageStart { .. } | DocumentLine::ImageContinuation => {}
        }
    }

    out.flush()?;
    Ok(())
}

/// Prints a single ratatui `Line` to the writer with ANSI escape codes.
fn print_styled_line(
    out: &mut impl Write,
    line: &ratatui::text::Line<'_>,
    no_color: bool,
) -> color_eyre::Result<()> {
    use ratatui::crossterm::execute;

    if no_color {
        for span in &line.spans {
            write!(out, "{}", span.content)?;
        }
        writeln!(out)?;
        return Ok(());
    }

    for span in &line.spans {
        print_with_style(out, &span.content, span.style)?;
    }
    // Reset after each line to prevent style bleed.
    execute!(out, ResetColor)?;
    writeln!(out)?;
    Ok(())
}

/// Writes a string to the writer with the given ratatui Style converted to crossterm ANSI.
fn print_with_style(
    out: &mut impl Write,
    text: &str,
    style: ratatui::style::Style,
) -> color_eyre::Result<()> {
    use ratatui::crossterm::execute;
    use ratatui::style::Modifier;

    // Reset before applying new style.
    execute!(out, ResetColor)?;

    if let Some(fg) = style.fg {
        if let Some(ct_fg) = ratatui_to_crossterm_color(fg) {
            execute!(out, SetForegroundColor(ct_fg))?;
        }
    }
    if let Some(bg) = style.bg {
        if let Some(ct_bg) = ratatui_to_crossterm_color(bg) {
            execute!(out, SetBackgroundColor(ct_bg))?;
        }
    }

    let mods = style.add_modifier;
    if mods.contains(Modifier::BOLD) {
        execute!(out, SetAttribute(Attribute::Bold))?;
    }
    if mods.contains(Modifier::DIM) {
        execute!(out, SetAttribute(Attribute::Dim))?;
    }
    if mods.contains(Modifier::ITALIC) {
        execute!(out, SetAttribute(Attribute::Italic))?;
    }
    if mods.contains(Modifier::UNDERLINED) {
        execute!(out, SetAttribute(Attribute::Underlined))?;
    }
    if mods.contains(Modifier::CROSSED_OUT) {
        execute!(out, SetAttribute(Attribute::CrossedOut))?;
    }

    execute!(out, Print(text))?;
    Ok(())
}

/// Converts a ratatui `Color` to a crossterm `Color`.
fn ratatui_to_crossterm_color(color: ratatui::style::Color) -> Option<CtColor> {
    use ratatui::style::Color;
    match color {
        Color::Reset => None,
        Color::Black => Some(CtColor::Black),
        Color::Red => Some(CtColor::DarkRed),
        Color::Green => Some(CtColor::DarkGreen),
        Color::Yellow => Some(CtColor::DarkYellow),
        Color::Blue => Some(CtColor::DarkBlue),
        Color::Magenta => Some(CtColor::DarkMagenta),
        Color::Cyan => Some(CtColor::DarkCyan),
        Color::Gray => Some(CtColor::Grey),
        Color::DarkGray => Some(CtColor::DarkGrey),
        Color::LightRed => Some(CtColor::Red),
        Color::LightGreen => Some(CtColor::Green),
        Color::LightYellow => Some(CtColor::Yellow),
        Color::LightBlue => Some(CtColor::Blue),
        Color::LightMagenta => Some(CtColor::Magenta),
        Color::LightCyan => Some(CtColor::Cyan),
        Color::White => Some(CtColor::White),
        Color::Rgb(r, g, b) => Some(CtColor::Rgb { r, g, b }),
        Color::Indexed(idx) => Some(CtColor::AnsiValue(idx)),
    }
}

/// Fetches markdown content from a remote URL.
///
/// Applies the same size guard as local files. Errors are printed to stderr
/// and the process exits with the appropriate BSD-style exit code.
fn fetch_url(url: &str, max_bytes: u64) -> (String, String, PathBuf) {
    let mut response = match ureq::get(url).call() {
        Ok(resp) => resp,
        Err(ureq::Error::StatusCode(code)) => {
            eprintln!("{url}: HTTP {code}");
            process::exit(EX_NOINPUT);
        }
        Err(e) => {
            eprintln!("{url}: {e}");
            process::exit(EX_NOINPUT);
        }
    };

    // Read response body with size limit. ureq 3.x uses with_config().limit()
    // to cap the read size, preventing OOM from unbounded responses.
    let body = match response
        .body_mut()
        .with_config()
        .limit(max_bytes + 1)
        .read_to_string()
    {
        Ok(s) => s,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("limit") || msg.contains("too large") {
                eprintln!("{url}: response too large (limit is {max_bytes} bytes)");
                process::exit(EX_DATAERR);
            }
            eprintln!("{url}: {e}");
            process::exit(EX_NOINPUT);
        }
    };

    if body.len() as u64 > max_bytes {
        eprintln!("{url}: response too large (limit is {max_bytes} bytes)");
        process::exit(EX_DATAERR);
    }

    // Sanitize URL for display: strip control characters.
    let safe_name = url
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>();

    (body, safe_name, PathBuf::from("."))
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

/// Computes the PDF output path from the source directory and display filename.
///
/// For regular files: replaces the `.md` extension with `.pdf`.
/// For stdin/URL inputs: falls back to `./output.pdf`.
fn compute_pdf_path(source_dir: &Path, display_name: &str) -> PathBuf {
    if display_name.starts_with('<') || display_name.starts_with("http") {
        return PathBuf::from("output.pdf");
    }
    let stem = Path::new(display_name)
        .file_stem()
        .unwrap_or(std::ffi::OsStr::new("output"));
    let mut name = stem.to_os_string();
    name.push(".pdf");
    source_dir.join(name)
}

/// Opens a file with the system's default viewer.
///
/// Uses `xdg-open` on Linux, `open` on macOS, `start` on Windows.
/// Errors are silently ignored (best-effort).
fn open_with_system_viewer(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "start";
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let cmd = "xdg-open";

    std::process::Command::new(cmd)
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

/// Runs the TUI event loop until the user quits or an error occurs.
///
/// Separated from `main()` so that `ratatui::restore()` always runs
/// regardless of how this function exits. Takes mutable blocks so
/// refresh can re-parse, and source/highlighter for re-parsing.
#[allow(clippy::too_many_arguments)]
fn run_event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    blocks: &mut Vec<RenderedBlock>,
    image_manager: &mut ImageManager,
    source: &mut String,
    highlighter: &highlight::Highlighter,
    no_color: bool,
    stream_rx: Option<mpsc::Receiver<String>>,
    pdf_font_override: Option<String>,
) -> color_eyre::Result<()> {
    let mut cols = terminal.size()?.width;
    let mut streaming_done = stream_rx.is_none();

    loop {
        // Update viewport height from current terminal size.
        app.viewport_height = terminal.size()?.height.saturating_sub(1) as usize;

        // Draw the current frame.
        terminal.draw(|frame| renderer::draw(frame, app, image_manager))?;
        // Clear transient status message after it has been rendered once.
        app.status_message = None;

        // When streaming, poll with timeout so we can check for new data;
        // otherwise block until the next event arrives.
        let maybe_event = if !streaming_done {
            if event::poll(Duration::from_millis(50))? {
                Some(event::read()?)
            } else {
                None
            }
        } else {
            Some(event::read()?)
        };

        // Drain streaming data from the channel.
        if let Some(rx) = &stream_rx {
            if !streaming_done {
                let at_bottom = app.scroll_offset >= app.max_scroll();
                let mut got_data = false;

                loop {
                    match rx.try_recv() {
                        Ok(chunk) => {
                            // Memory guard: cap accumulated content at 100 MB.
                            if source.len() as u64 + chunk.len() as u64 > MAX_FILE_BYTES {
                                streaming_done = true;
                                break;
                            }
                            source.push_str(&chunk);
                            got_data = true;
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            streaming_done = true;
                            break;
                        }
                    }
                }

                if got_data {
                    // Re-parse and re-flatten with new content.
                    *blocks = parser::parse(source, highlighter, image_manager, &app.theme);
                    let w = compute_content_width(cols, app);
                    app.document = layout::flatten(blocks, w, &app.theme);

                    // Auto-scroll: if user was at the bottom, keep them there.
                    if at_bottom {
                        app.scroll_offset = app.max_scroll();
                    }
                }
            }
        }

        let Some(event) = maybe_event else {
            continue;
        };

        match event {
            Event::Key(key) => {
                app.handle_key(key);
                // Print preview toggle: load/unload the "print" theme.
                if app.print_preview_changed {
                    app.print_preview_changed = false;
                    let theme_name = if app.print_preview {
                        "print"
                    } else {
                        app.theme_index = app.saved_theme_index;
                        THEME_CYCLE[app.saved_theme_index]
                    };
                    if let Ok(mut new_theme) = theme::load_theme(theme_name) {
                        if no_color {
                            new_theme.strip_colors();
                        }
                        app.theme = new_theme;
                        let size = terminal.size()?;
                        cols = size.width;
                        app.viewport_height = size.height.saturating_sub(1) as usize;
                        image_manager.update_max_width(cols);
                        image_manager.clear_protocols();
                        *blocks = parser::parse(source, highlighter, image_manager, &app.theme);
                        let w = compute_content_width(cols, app);
                        app.document = layout::flatten(blocks, w, &app.theme);
                        let max = app.max_scroll();
                        if app.scroll_offset > max {
                            app.scroll_offset = max;
                        }
                    }
                }
                // PDF export (only meaningful in print preview).
                if app.pdf_export_requested {
                    app.pdf_export_requested = false;
                    // Resolve terminal font for PDF embedding.
                    let resolved = font_detect::detect_and_resolve(
                        pdf_font_override.as_deref(),
                    );
                    let ratio = resolved
                        .as_ref()
                        .and_then(|r| font_detect::monospace_width_ratio(&r.regular))
                        .unwrap_or(0.6);
                    // Re-flatten at the PDF's column width so lines wrap to fit the page.
                    let pdf_cols = pdf::usable_columns_for_ratio(ratio);
                    let pdf_doc = layout::flatten(blocks, pdf_cols, &app.theme);
                    let pdf_path = compute_pdf_path(&app.source_path, &app.filename);
                    match pdf::export_pdf(&pdf_doc.lines, &pdf_path, resolved.as_ref(), ratio) {
                        Ok(()) => {
                            app.status_message =
                                Some(format!("Exported: {} | o:open", pdf_path.display()));
                            app.last_exported_pdf = Some(pdf_path);
                        }
                        Err(e) => {
                            app.status_message =
                                Some(format!("PDF error: {e}"));
                        }
                    }
                }
                // Open the last exported PDF with the system viewer.
                if app.open_pdf_requested {
                    app.open_pdf_requested = false;
                    if let Some(pdf_path) = &app.last_exported_pdf {
                        let _ = open_with_system_viewer(pdf_path);
                    }
                }
                if app.theme_cycle_requested {
                    app.theme_cycle_requested = false;
                    let next_name = app.next_theme_name();
                    if let Ok(mut new_theme) = theme::load_theme(next_name) {
                        if no_color {
                            new_theme.strip_colors();
                        }
                        app.advance_theme_index();
                        app.theme = new_theme;
                        let size = terminal.size()?;
                        cols = size.width;
                        app.viewport_height = size.height.saturating_sub(1) as usize;
                        image_manager.update_max_width(cols);
                        image_manager.clear_protocols();
                        *blocks = parser::parse(source, highlighter, image_manager, &app.theme);
                        let w = compute_content_width(cols, app);
                        app.document = layout::flatten(blocks, w, &app.theme);
                        let max = app.max_scroll();
                        if app.scroll_offset > max {
                            app.scroll_offset = max;
                        }
                    }
                }
                if app.refresh_requested {
                    let size = terminal.size()?;
                    cols = size.width;
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

        // Handle file browser selection: load the new file.
        if let Some(path) = app.file_selected.take() {
            match load_file_for_browser(&path) {
                Ok(new_source) => {
                    *source = new_source;
                    let size = terminal.size()?;
                    cols = size.width;
                    app.viewport_height = size.height.saturating_sub(1) as usize;
                    image_manager.update_max_width(cols);
                    image_manager.clear_protocols();
                    *blocks = parser::parse(source, highlighter, image_manager, &app.theme);
                    let w = compute_content_width(cols, app);
                    app.document = layout::flatten(blocks, w, &app.theme);
                    app.scroll_offset = 0;
                    app.filename = path.display().to_string();
                    app.outline = None;
                    app.search = None;
                }
                Err(msg) => {
                    // Show error in the filename field briefly; the file stays unchanged.
                    app.filename = format!("[error: {}]", msg);
                }
            }
        }

        if app.quit {
            break;
        }
    }

    Ok(())
}

/// Loads a file for the file browser with the same guards as the initial load.
fn load_file_for_browser(path: &std::path::Path) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(format!("file too large ({} bytes)", metadata.len()));
    }
    fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::InvalidData {
            "not valid UTF-8 text".to_string()
        } else {
            e.to_string()
        }
    })
}
