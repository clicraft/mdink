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
#[cfg(feature = "logging")]
mod logging;
mod math;
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
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::crossterm::style::{
    Attribute, Color as CtColor, SetAttribute, SetForegroundColor,
    SetBackgroundColor, ResetColor, Print,
};
use ratatui_image::picker::ProtocolType;

use crate::app::{App, NavHistoryEntry, THEME_CYCLE};
use crate::cli::Cli;
use crate::images::{ImageFetchRequest, ImageFetchResult, ImageManager};
use crate::layout::DocumentLine;
use crate::parser::{RenderedBlock, TableCell};

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

/// Timeout for remote URL fetches (connect + transfer).
const FETCH_TIMEOUT_SECS: u64 = 10;

/// Tracks a local file's modification time to detect on-disk changes.
///
/// When the source is stdin or a remote URL, `path` is `None` (not watchable).
/// For local files, `path` holds `(canonical_path, last_known_mtime)`.
/// On each `check()`, the current mtime is compared against the stored value;
/// a change triggers a reload. If the file is deleted, `check()` returns `false`
/// so the viewer keeps showing the last successfully loaded content.
struct FileWatcher {
    path: Option<(PathBuf, std::time::SystemTime)>,
}

impl FileWatcher {
    /// Creates a watcher for a local file.
    ///
    /// Returns a non-watching instance (`path: None`) for stdin (`-`),
    /// URLs, or files whose mtime cannot be determined.
    fn new(file: &str) -> Self {
        if file == "-" || file.starts_with("http://") || file.starts_with("https://") {
            return Self { path: None };
        }
        let full_path = PathBuf::from(file);
        let mtime = fs::metadata(&full_path).ok().and_then(|m| m.modified().ok());
        Self {
            path: mtime.map(|t| (full_path, t)),
        }
    }

    /// Returns `true` when the watched file's mtime has changed since the last call.
    ///
    /// Returns `false` when: not watching, mtime unchanged, or the file is
    /// currently deleted (graceful degradation — keep showing existing content).
    fn check(&mut self) -> bool {
        let Some((ref path, ref last_mtime)) = self.path else {
            return false;
        };
        match fs::metadata(path).and_then(|m| m.modified()) {
            Ok(new_mtime) if new_mtime != *last_mtime => {
                self.path = Some((path.clone(), new_mtime));
                true
            }
            Ok(_) => false,
            Err(_) => false,
        }
    }

    /// Updates the tracked file after navigation (link follow, back, file browser).
    ///
    /// Sets `path` to `None` for stdin or URLs.
    fn set_file(&mut self, file: &str) {
        if file == "-" || file.starts_with("http://") || file.starts_with("https://") {
            self.path = None;
            return;
        }
        let full_path = PathBuf::from(file);
        let mtime = fs::metadata(&full_path).ok().and_then(|m| m.modified().ok());
        self.path = mtime.map(|t| (full_path, t));
    }

    /// Returns `true` if a local file is being watched.
    fn is_watching(&self) -> bool {
        self.path.is_some()
    }
}

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

    // Initialize Phase A logger (writes to stderr before TUI init).
    #[cfg(feature = "logging")]
    let log_config = logging::resolve_log_config(
        cli.log_level.as_deref(),
        cli.log_file.as_deref(),
        config.log_level.as_deref(),
        config.log_file.as_deref(),
    );
    #[cfg(feature = "logging")]
    logging::init_phase_a(&log_config);
    log::info!("mdink starting");
    log::info!("terminal identity: {}", detect_terminal_identity());

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
            log::error!("<stdin>: input too large (limit is {MAX_FILE_BYTES} bytes)");
            process::exit(EX_DATAERR);
        }
        (buf, "<stdin>".to_string(), PathBuf::from("."))
    } else if file == "-" {
        // Streaming stdin: start with empty content; reader thread will feed data.
        (String::new(), "<stdin> (streaming)".to_string(), PathBuf::from("."))
    } else if file.starts_with("http://") || file.starts_with("https://") {
        match fetch_url(file, MAX_FILE_BYTES) {
            Ok(tuple) => tuple,
            Err(e) => {
                log::error!("{e}");
                process::exit(EX_NOINPUT);
            }
        }
    } else {
        // Guard against OOM: reject files that exceed a reasonable size threshold.
        // The check happens before ratatui::init() so the error prints to the normal
        // terminal instead of a raw alternate screen.
        let metadata = match fs::metadata(file) {
            Ok(m) => m,
            Err(e) => {
                log::error!("{file}: {e}");
                process::exit(EX_NOINPUT);
            }
        };
        if metadata.len() > MAX_FILE_BYTES {
            log::error!(
                "{file}: file too large ({} bytes; limit is {MAX_FILE_BYTES} bytes)",
                metadata.len()
            );
            process::exit(EX_DATAERR);
        }

        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                log::error!("{file}: not valid UTF-8 text");
                process::exit(EX_DATAERR);
            }
            Err(e) => {
                log::error!("{file}: {e}");
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

    log::info!("loaded {} ({} bytes)", display_name, source.len());

    // Resolve no_images and ascii_images from CLI flag or config.
    let no_images = cli.no_images || config.no_images.unwrap_or(false);
    let ascii_images = cli.ascii_images || config.ascii_images.unwrap_or(false);
    let fetch_remote_images = cli.fetch_remote_images
        || std::env::var("MDINK_FETCH_REMOTE").is_ok()
        || config.fetch_remote_images.unwrap_or(false);
    let fetch_remote_markdown = cli.fetch_remote_markdown
        || std::env::var("MDINK_FETCH_REMOTE_MD").is_ok()
        || config.fetch_remote_markdown.unwrap_or(false);

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
        log::info!("picker: skipped (no_images={}, print={})", no_images, cli.print);
        None
    } else {
        ratatui::crossterm::terminal::enable_raw_mode().ok();
        let result = ratatui_image::picker::Picker::from_query_stdio();
        let _ = ratatui::crossterm::terminal::disable_raw_mode();
        match &result {
            Ok(p) => {
                let pt = p.protocol_type();
                let (fw, fh) = p.font_size();
                log::info!("picker: detected protocol={pt:?}, font_size=({fw},{fh})");
            }
            Err(e) => {
                log::info!("picker: query failed — {e}");
            }
        }
        result.ok()
    };

    let protocol_quality = match picker.as_ref() {
        None => "none (no graphics support)",
        Some(p) => match p.protocol_type() {
            ProtocolType::Halfblocks => "halfblocks (reduced quality)",
            ProtocolType::Sixel => "sixel (high quality)",
            ProtocolType::Kitty => "kitty (high quality)",
            ProtocolType::Iterm2 => "iterm2 (high quality)",
        },
    };
    log::info!("graphics protocol quality: {protocol_quality}");

    // Halfblocks protocol renders images at poor quality (only 2 vertical pixels
    // per cell). For math formulas, Unicode text is sharper than halfblock images.
    let math_has_high_quality = picker.as_ref().is_some_and(|p| {
        p.protocol_type() != ProtocolType::Halfblocks
    });

    let mut image_manager = ImageManager::new(base_path.clone(), picker, cols, no_images, ascii_images, fetch_remote_images);

    // Detect terminal background color for math image compositing.
    // Must happen before alternate screen; falls back to black if detection fails.
    let math_bg_color_detected = detect_terminal_bg_color();
    let math_bg_color = math_bg_color_detected.unwrap_or((0, 0, 0));
    match &math_bg_color_detected {
        Some((r, g, b)) => log::info!("terminal bg color: rgb({r},{g},{b})"),
        None => log::info!("terminal bg color: not detected (using black)"),
    }

    // Math engine: pixel rendering enabled unless explicitly disabled.
    // Precedence: --no-math-images > MDINK_NO_MATH_IMAGES > config.math_images: false > default (enabled).
    // Requires a high-quality protocol (Sixel, Kitty, iTerm2); Halfblocks degrades
    // to Unicode text because its 2-vertical-pixel-per-cell resolution is too low
    // for legible formulas.
    let math_user_enabled = !cli.no_math_images
        && std::env::var("MDINK_NO_MATH_IMAGES").is_err()
        && config.math_images.unwrap_or(true);
    let mut math_engine = math::MathEngine::new(math_user_enabled, math_has_high_quality);

    // Parse markdown into IR blocks. Kept mutable so refresh can re-parse.
    let mut blocks = parser::parse(&source, &highlighter, &mut image_manager, &mut math_engine, &theme);

    // Flatten blocks into document lines at the current width.
    let document = layout::flatten(&blocks, cols, &theme);

    // Print mode: render to stdout without entering the TUI.
    if cli.print {
        return print_document(&document.lines, cols, &theme, no_color);
    }

    // PDF export mode: export to PDF and exit without entering the TUI.
    if cli.pdf {
        let pdf_font_override = cli.pdf_font.or(config.pdf_font);
        let resolved = font_detect::detect_and_resolve(pdf_font_override.as_deref());
        // Load the print theme for PDF (matches print preview appearance).
        let print_theme = theme::load_theme("print").unwrap_or_else(|_| theme::default_theme());
        let pdf_blocks = parser::parse(&source, &highlighter, &mut image_manager, &mut math_engine, &print_theme);
        let pdf_cols = pdf::usable_columns(resolved.as_ref());
        let pdf_doc = layout::flatten(&pdf_blocks, pdf_cols, &print_theme);
        let source_dir = Path::new(file)
            .parent()
            .unwrap_or(Path::new("."));
        let pdf_path = compute_pdf_path(source_dir, file);
        match pdf::export_pdf(&pdf_doc.lines, &pdf_path, resolved.as_ref()) {
            Ok(()) => {
                log::info!("exported: {}", pdf_path.display());
                let _ = open_with_system_viewer(&pdf_path);
            }
            Err(e) => {
                log::error!("PDF error: {e}");
                process::exit(1);
            }
        }
        return Ok(());
    }

    // Create the application state.
    let mut app = App::new(
        document, display_name, theme, base_path,
        fetch_remote_images, fetch_remote_markdown, math_user_enabled,
    );

    // Initialize the terminal (enters raw mode + alternate screen).
    // TERMINAL_ACTIVE must be set immediately after so the panic hook is correct.
    let mut terminal = ratatui::init();
    TERMINAL_ACTIVE.store(true, Ordering::SeqCst);

    // Switch logger to Phase B: redirect away from stderr to avoid TUI corruption.
    #[cfg(feature = "logging")]
    logging::init_phase_b(&log_config);
    log::info!("terminal initialized");

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

    // Spawn background image fetch thread for remote URLs.
    let (fetch_tx, fetch_rx) = mpsc::channel::<ImageFetchRequest>();
    let (img_result_tx, img_result_rx) = mpsc::channel::<ImageFetchResult>();
    log::info!("spawning image fetch thread (remote_fetch={fetch_remote_images})");
    {
        std::thread::spawn(move || {
            while let Ok(req) = fetch_rx.recv() {
                let result = match crate::images::fetch_image(&req.url) {
                    Ok(dyn_img) => ImageFetchResult::Ok {
                        url: req.url,
                        dyn_img,
                    },
                    Err(e) => ImageFetchResult::Err {
                        url: req.url,
                        error: e.to_string(),
                        expected: e.is_expected(),
                    },
                };
                if img_result_tx.send(result).is_err() {
                    break;
                }
            }
        });
    }

    // Spawn background math render thread (only when enabled and graphics available).
    log::info!("spawning math render thread (enabled={})", math_engine.enabled());
    let (math_tx, math_rx) = mpsc::channel::<math::MathRenderRequest>();
    let (math_result_tx, math_result_rx) = mpsc::channel::<math::MathRenderResult>();
    if math_engine.enabled() {
        std::thread::spawn(move || {
            while let Ok(req) = math_rx.recv() {
                let result = match math::render_latex_to_image(
                    &req.latex, req.display, req.width_cells, req.font_size, req.bg_color,
                ) {
                    Ok(dyn_img) => math::MathRenderResult::Ok {
                        latex: req.latex,
                        dyn_img,
                    },
                    Err(e) => math::MathRenderResult::Err {
                        latex: req.latex,
                        error: e.to_string(),
                    },
                };
                if math_result_tx.send(result).is_err() {
                    break;
                }
            }
        });
    }

    // File watcher: tracks mtime of the local file for auto-reload.
    let mut watcher = FileWatcher::new(cli.file.as_deref().expect("file argument is required here"));

    // Main event loop.
    let result = run_event_loop(
        &mut terminal,
        &mut app,
        &mut blocks,
        &mut image_manager,
        &mut math_engine,
        &mut source,
        &highlighter,
        no_color,
        stream_rx,
        pdf_font_override,
        fetch_tx,
        img_result_rx,
        math_tx,
        math_result_rx,
        math_bg_color,
        &mut watcher,
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
/// Applies the same size guard as local files. Returns a human-readable
/// error string on failure (suitable for TUI status display or CLI stderr).
fn fetch_url(url: &str, max_bytes: u64) -> Result<(String, String, PathBuf), String> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(FETCH_TIMEOUT_SECS)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let mut response = agent.get(url).call().map_err(|e| {
        match &e {
            ureq::Error::StatusCode(code) => format!("{url}: HTTP {code}"),
            _ => format!("{url}: {e}"),
        }
    })?;

    // Read response body with size limit. ureq 3.x uses with_config().limit()
    // to cap the read size, preventing OOM from unbounded responses.
    let body = response
        .body_mut()
        .with_config()
        .limit(max_bytes + 1)
        .read_to_string()
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("limit") || msg.contains("too large") {
                format!("{url}: response too large (limit is {max_bytes} bytes)")
            } else {
                format!("{url}: {e}")
            }
        })?;

    if body.len() as u64 > max_bytes {
        return Err(format!("{url}: response too large (limit is {max_bytes} bytes)"));
    }

    // Sanitize URL for display: strip control characters.
    let safe_name = url
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>();

    Ok((body, safe_name, PathBuf::from(".")))
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
/// On WSL, uses `explorer.exe` with a Windows path converted via `wslpath`.
/// On native Linux, uses `xdg-open`. On macOS, `open`. On Windows, `start`.
/// Errors are silently ignored (best-effort).
fn open_with_system_viewer(path: &Path) -> std::io::Result<()> {
    // Detect WSL: check for WSL-specific interop marker.
    if cfg!(target_os = "linux") && is_wsl() {
        // Convert Linux path to Windows path via wslpath, then open with explorer.exe.
        let wslpath_output = std::process::Command::new("wslpath")
            .arg("-w")
            .arg(path)
            .output()?;
        if wslpath_output.status.success() {
            let win_path = String::from_utf8_lossy(&wslpath_output.stdout)
                .trim()
                .to_string();
            std::process::Command::new("explorer.exe")
                .arg(&win_path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
            return Ok(());
        }
        // Fall through to xdg-open if wslpath fails.
    }

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

/// Detects whether we're running inside WSL (Windows Subsystem for Linux).
///
/// Checks `/proc/sys/fs/binfmt_misc/WSLInterop` which is present only in WSL.
fn is_wsl() -> bool {
    Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists()
        || std::env::var_os("WSL_DISTRO_NAME").is_some()
}

/// Percent-decodes a URL-encoded string (e.g. `"my%20file.md"` → `"my file.md"`).
///
/// Used when following links from Markdown documents where the URL may contain
/// percent-encoded characters. Invalid percent-sequences are passed through as-is.
/// UTF-8 multi-byte characters (CJK, etc.) are preserved intact.
fn percent_decode_str(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = bytes[i + 1];
            let l = bytes[i + 2];
            if let (Some(hv), Some(lv)) = (hex_digit(h), hex_digit(l)) {
                let decoded = hv << 4 | lv;
                // Only decode ASCII printable characters (space, punctuation, etc.).
                // This preserves sequences like %E7%B1%AC which would be UTF-8 bytes
                // that were never intended as percent-encoding.
                if (0x20..0x7F).contains(&decoded) {
                    result.push(char::from(decoded));
                    i += 3;
                    continue;
                }
            }
            // Not a valid ASCII percent-sequence: pass '%' through and advance.
            result.push('%');
            i += 1;
        } else if bytes[i] < 0x80 {
            // ASCII byte: safe to convert directly.
            result.push(char::from(bytes[i]));
            i += 1;
        } else {
            // UTF-8 multi-byte sequence: find its length and copy it intact.
            let start = i;
            let len = utf8_len_from_leader(bytes[i]);
            let end = (start + len).min(bytes.len());
            // to_str here is safe because input is valid UTF-8.
            result.push_str(&input[start..end]);
            i = end;
        }
    }
    result
}

/// Returns the expected byte length of a UTF-8 character from its leader byte.
fn utf8_len_from_leader(leader: u8) -> usize {
    if leader < 0xC0 { 1 }       // continuation or ASCII (shouldn't happen)
    else if leader < 0xE0 { 2 }  // 2-byte sequence
    else if leader < 0xF0 { 3 }  // 3-byte sequence
    else { 4 }                   // 4-byte sequence
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
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
    math_engine: &mut math::MathEngine,
    source: &mut String,
    highlighter: &highlight::Highlighter,
    no_color: bool,
    stream_rx: Option<mpsc::Receiver<String>>,
    pdf_font_override: Option<String>,
    fetch_tx: mpsc::Sender<ImageFetchRequest>,
    img_result_rx: mpsc::Receiver<ImageFetchResult>,
    math_tx: mpsc::Sender<math::MathRenderRequest>,
    math_result_rx: mpsc::Receiver<math::MathRenderResult>,
    math_bg_color: (u8, u8, u8),
    watcher: &mut FileWatcher,
) -> color_eyre::Result<()> {
    let mut cols = terminal.size()?.width;
    let mut streaming_done = stream_rx.is_none();

    // Queue any remote image fetches from the initial parse.
    queue_pending_fetches(blocks, &fetch_tx, image_manager);

    // Queue any LaTeX formulas for async rendering.
    let font_size = image_manager.font_cell_size();
    queue_pending_math_renders(blocks, &math_tx, math_engine, cols, font_size, math_bg_color);

    // Frame counter for diagnosing rendering loops.
    let mut frame_counter: u64 = 0;
    let mut last_batch_refresh_frame: u64 = 0;
    // Tracks whether the document or UI state changed since the last draw.
    // Only re-draw when true; set by events, file changes, async results.
    let mut needs_redraw = true; // draw the first frame

    loop {
        // Update viewport height from current terminal size.
        app.viewport_height = terminal.size()?.height.saturating_sub(1) as usize;

        // Draw the current frame only when something changed.
        if needs_redraw {
            frame_counter += 1;
            let draw_start = std::time::Instant::now();
            terminal.draw(|frame| renderer::draw(frame, app, image_manager))?;
            let draw_elapsed = draw_start.elapsed();
            if draw_elapsed.as_millis() > 16 {
                log::debug!("slow frame {frame_counter}: draw={:?}", draw_elapsed);
            }
            // Clear transient status message after it has been rendered once.
            app.status_message = None;
            needs_redraw = false;
        }

        // When streaming, poll with timeout so we can check for new data;
        // otherwise block until the next event arrives.
        // When streaming, or when remote images/math formulas are pending,
        // poll with timeout so results can be drained even without user input.
        // Otherwise (streaming done, nothing pending) block for the next key.
        let has_pending_images = !image_manager.pending_urls_is_empty();
        let has_pending_math = math_engine.has_pending();
        let watching_file = watcher.is_watching();
        let maybe_event = if !streaming_done || has_pending_images || has_pending_math || watching_file {
            if event::poll(Duration::from_millis(200))? {
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
                    needs_redraw = true;
                    // Re-parse and re-flatten with new content.
                    *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
                    let w = compute_content_width(cols, app);
                    app.document = layout::flatten(blocks, w, &app.theme);

                    // Auto-scroll: if user was at the bottom, keep them there.
                    if at_bottom {
                        app.scroll_offset = app.max_scroll();
                    }
                }
            }
        }

        // Check if the watched file has changed on disk and reload if so.
        if watcher.check() {
            needs_redraw = true;
            let file_path = watcher.path.as_ref().unwrap().0.clone();
            log::debug!("file changed, reloading {}", file_path.display());
            match load_file_for_browser(&file_path) {
                Ok(new_source) => {
                    let at_bottom = app.scroll_offset >= app.max_scroll();
                    *source = new_source;
                    image_manager.clear_protocols();
                    math_engine.clear_protocols();
                    *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
                    queue_pending_fetches(blocks, &fetch_tx, image_manager);
                    let fs = image_manager.font_cell_size();
                    queue_pending_math_renders(blocks, &math_tx, math_engine, cols, fs, math_bg_color);
                    let w = compute_content_width(cols, app);
                    app.document = layout::flatten(blocks, w, &app.theme);
                    if at_bottom {
                        app.scroll_offset = app.max_scroll();
                    } else {
                        let max = app.max_scroll();
                        if app.scroll_offset > max {
                            app.scroll_offset = max;
                        }
                    }
                }
                Err(_) => {
                    // File deleted or unreadable: keep showing current content.
                    // The watcher did not update mtime on error, so it will
                    // detect when the file reappears.
                }
            }
        }

        // Drain completed remote image fetches.
        let mut images_arrived = false;
        loop {
            match img_result_rx.try_recv() {
                Ok(ImageFetchResult::Ok { url, dyn_img }) => {
                    image_manager.mark_resolved(&url);
                    image_manager.insert_cache(url, dyn_img);
                    images_arrived = true;
                }
                Ok(ImageFetchResult::Err { url, .. }) => {
                    image_manager.mark_failed(&url);
                    images_arrived = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
        if images_arrived {
            needs_redraw = true;
            image_manager.clear_protocols();
            *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
            queue_pending_fetches(blocks, &fetch_tx, image_manager);
            let fs = image_manager.font_cell_size();
            queue_pending_math_renders(blocks, &math_tx, math_engine, cols, fs, math_bg_color);
            let w = compute_content_width(cols, app);
            app.document = layout::flatten(blocks, w, &app.theme);
        }

        // Drain completed math renders into cache. Do NOT re-parse yet —
        // batch refresh fires only when ALL formulas are done rendering.
        let mut drained_ok = 0usize;
        let mut drained_err = 0usize;
        loop {
            match math_result_rx.try_recv() {
                Ok(math::MathRenderResult::Ok { latex, dyn_img }) => {
                    math_engine.mark_resolved(&latex);
                    math_engine.insert_cache(latex, dyn_img);
                    drained_ok += 1;
                }
                Ok(math::MathRenderResult::Err { latex, .. }) => {
                    math_engine.mark_failed(&latex);
                    drained_err += 1;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
        if drained_ok + drained_err > 0 {
            log::debug!("math drain: {drained_ok} ok, {drained_err} err, pending={}, cache_touched={}",
                math_engine.has_pending(), math_engine.cache_touched());
        }

        // Batch refresh: only when ALL formulas finished rendering AND cache was touched.
        if !math_engine.has_pending() && math_engine.cache_touched() {
            needs_redraw = true;
            let protocol_count_before = image_manager.protocol_count();
            let frames_since_last = frame_counter - last_batch_refresh_frame;
            let batch_start = std::time::Instant::now();
            log::debug!("math batch refresh at frame {frame_counter} ({frames_since_last} frames since last): clearing {} protocols, re-parsing", protocol_count_before);
            last_batch_refresh_frame = frame_counter;
            math_engine.clear_cache_touched();
            image_manager.clear_protocols();
            let parse_start = std::time::Instant::now();
            *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
            let parse_elapsed = parse_start.elapsed();
            let math_image_count = blocks.iter().filter(|b| matches!(b, RenderedBlock::MathImage { .. })).count();
            let math_unicode_count = blocks.iter().filter(|b| matches!(b, RenderedBlock::MathUnicode { .. })).count();
            log::debug!("math batch refresh: {} MathImage, {} MathUnicode, {} protocols after parse (parse={:?})",
                math_image_count, math_unicode_count, image_manager.protocol_count(), parse_elapsed);
            queue_pending_fetches(blocks, &fetch_tx, image_manager);
            let fs = image_manager.font_cell_size();
            queue_pending_math_renders(blocks, &math_tx, math_engine, cols, fs, math_bg_color);
            let layout_start = std::time::Instant::now();
            let w = compute_content_width(cols, app);
            app.document = layout::flatten(blocks, w, &app.theme);
            let layout_elapsed = layout_start.elapsed();
            let batch_elapsed = batch_start.elapsed();
            log::debug!("math batch refresh: {} lines, {} inline_images (layout={:?}, total={:?})",
                app.document.lines.len(), app.document.inline_images.len(), layout_elapsed, batch_elapsed);
        }

        let Some(event) = maybe_event else {
            // Poll timeout with no event. Only redraw if async data actually arrived.
            // If nothing changed, skip the expensive draw to avoid StatefulImage flicker.
            continue;
        };

        // Any real event (key, resize, etc.) requires a redraw.
        needs_redraw = true;

        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                app.handle_key(key);
                if app.print_preview_changed {
                    app.print_preview_changed = false;
                    let theme_name = if app.print_preview {
                        "print"
                    } else {
                        app.theme_index = app.saved_theme_index;
                        THEME_CYCLE[app.saved_theme_index]
                    };
                    log::debug!("switching theme to {theme_name}");
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
                        math_engine.clear_protocols();
                        *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
                        queue_pending_fetches(blocks, &fetch_tx, image_manager);
                        let fs = image_manager.font_cell_size();
                        queue_pending_math_renders(blocks, &math_tx, math_engine, cols, fs, math_bg_color);
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
                    // Re-flatten at the PDF's column width so lines wrap to fit the page.
                    let pdf_cols = pdf::usable_columns(resolved.as_ref());
                    let pdf_doc = layout::flatten(blocks, pdf_cols, &app.theme);
                    let pdf_path = compute_pdf_path(&app.source_path, &app.filename);
                    match pdf::export_pdf(&pdf_doc.lines, &pdf_path, resolved.as_ref()) {
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
                // Link following: load local/remote .md or open URL in system browser.
                if app.link_follow_requested {
                    app.link_follow_requested = false;
                    app.link_mode = false;

                    let url = match app.document.links.get(app.link_selected) {
                        Some(link) => link.url.clone(),
                        None => continue,
                    };

                    // Strip fragment identifier (#section).
                    let url_clean = url.split('#').next().unwrap_or(&url).to_string();

                    // Percent-decode the URL for local file path resolution.
                    // URLs like [link](my%20file.md) should resolve to "my file.md".
                    let url_decoded = percent_decode_str(&url_clean);

                    if url_decoded.starts_with("http://") || url_decoded.starts_with("https://") {
                        if url_decoded.to_lowercase().ends_with(".md") && app.fetch_remote_markdown {
                            follow_remote_md(
                                &url_decoded, app, source, blocks, highlighter, image_manager, math_engine, cols,
                                &fetch_tx, &math_tx, math_bg_color, watcher,
                            );
                        } else {
                            open_url_with_system_browser(&url_decoded);
                            app.status_message = Some(format!("Opened: {}", url_decoded));
                        }
                    } else {
                        let path = app.source_path.join(&url_decoded);
                        let lower = url_decoded.to_lowercase();
                        if lower.ends_with(".md") || lower.ends_with(".markdown") {
                            follow_local_md(
                                &path, app, source, blocks, highlighter, image_manager, math_engine, cols,
                                &fetch_tx, &math_tx, math_bg_color, watcher,
                            );
                        } else {
                            let _ = open_with_system_viewer(&path);
                            app.status_message = Some(format!("Opened: {}", path.display()));
                        }
                    }
                }
                // Image following: open image URL/file with system viewer.
                if app.image_follow_requested {
                    app.image_follow_requested = false;

                    let url = match app.document.images.get(app.image_selected) {
                        Some(img) => img.url.clone(),
                        None => continue,
                    };

                    if url.starts_with("http://") || url.starts_with("https://") {
                        open_url_with_system_browser(&url);
                        app.status_message = Some(format!("Opened: {}", url));
                    } else {
                        let path = app.source_path.join(&url);
                        match open_with_system_viewer(&path) {
                            Ok(()) => app.status_message = Some(format!("Opened: {}", path.display())),
                            Err(e) => app.status_message = Some(format!("Error: {e}")),
                        }
                    }
                }
                // Back navigation: restore previous document from history stack.
                if app.back_requested {
                    app.back_requested = false;
                    app.link_mode = false;

                    if let Some(entry) = app.nav_history.pop() {
                        *source = entry.source;
                        app.source_path = entry.base_path;
                        app.filename = entry.filename;
                        watcher.set_file(&app.filename);
                        image_manager.update_max_width(cols);
                        image_manager.clear_all();
                        math_engine.clear_all();
                        *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
                        queue_pending_fetches(blocks, &fetch_tx, image_manager);
                        let fs = image_manager.font_cell_size();
                        queue_pending_math_renders(blocks, &math_tx, math_engine, cols, fs, math_bg_color);
                        let w = compute_content_width(cols, app);
                        app.document = layout::flatten(blocks, w, &app.theme);
                        app.scroll_offset = entry.scroll_offset.min(app.max_scroll());
                        app.outline = None;
                        app.search = None;
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
                        math_engine.clear_protocols();
                        *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
                        queue_pending_fetches(blocks, &fetch_tx, image_manager);
                        let fs = image_manager.font_cell_size();
                        queue_pending_math_renders(blocks, &math_tx, math_engine, cols, fs, math_bg_color);
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
                    image_manager.set_fetch_remote(app.fetch_remote_images);
                    image_manager.update_max_width(cols);
                    image_manager.clear_protocols();
                    math_engine.set_enabled(app.math_images_enabled);
                    math_engine.clear_protocols();
                    math_engine.clear_cache_touched();
                    *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
                    queue_pending_fetches(blocks, &fetch_tx, image_manager);
                    let fs = image_manager.font_cell_size();
                    queue_pending_math_renders(blocks, &math_tx, math_engine, cols, fs, math_bg_color);
                    app.document = layout::flatten(blocks, cols, &app.theme);
                    let max = app.max_scroll();
                    if app.scroll_offset > max {
                        app.scroll_offset = max;
                    }
                    app.refresh_requested = false;
                }
            }
            Event::Resize(new_cols, _rows) => {
                log::debug!("terminal resized to {new_cols} columns");
                cols = new_cols;
                image_manager.update_max_width(cols);
                image_manager.clear_protocols();
                math_engine.clear_protocols();
                *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
                queue_pending_fetches(blocks, &fetch_tx, image_manager);
                let fs = image_manager.font_cell_size();
                queue_pending_math_renders(blocks, &math_tx, math_engine, cols, fs, math_bg_color);
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
                    image_manager.clear_all();
                    *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
                    queue_pending_fetches(blocks, &fetch_tx, image_manager);
                    let w = compute_content_width(cols, app);
                    app.document = layout::flatten(blocks, w, &app.theme);
                    app.scroll_offset = 0;
                    app.filename = path.display().to_string();
                    watcher.set_file(&app.filename);
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

/// Follows a local markdown link: pushes history, loads file, re-parses.
///
/// On error, pops the history entry to avoid orphan entries.
#[allow(clippy::too_many_arguments)]
fn follow_local_md(
    path: &Path,
    app: &mut App,
    source: &mut String,
    blocks: &mut Vec<RenderedBlock>,
    highlighter: &highlight::Highlighter,
    image_manager: &mut ImageManager,
    math_engine: &mut math::MathEngine,
    cols: u16,
    fetch_tx: &mpsc::Sender<ImageFetchRequest>,
    math_tx: &mpsc::Sender<math::MathRenderRequest>,
    math_bg_color: (u8, u8, u8),
    watcher: &mut FileWatcher,
) {
    app.nav_history.push(NavHistoryEntry {
        source: source.clone(),
        base_path: app.source_path.clone(),
        filename: app.filename.clone(),
        scroll_offset: app.scroll_offset,
    });
    log::debug!("following local link: {}", path.display());

    match load_file_for_browser(path) {
        Ok(new_source) => {
            *source = new_source;
            image_manager.update_max_width(cols);
            image_manager.clear_all();
            math_engine.clear_all();
            *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
            queue_pending_fetches(blocks, fetch_tx, image_manager);
            let fs = image_manager.font_cell_size();
            queue_pending_math_renders(blocks, math_tx, math_engine, cols, fs, math_bg_color);
            let w = compute_content_width(cols, app);
            app.document = layout::flatten(blocks, w, &app.theme);
            app.scroll_offset = 0;
            app.filename = path.display().to_string();
            app.source_path = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            watcher.set_file(&app.filename);
            app.outline = None;
            app.search = None;
        }
        Err(msg) => {
            app.nav_history.pop();
            app.status_message = Some(format!("Error: {}", msg));
        }
    }
}

/// Follows a remote markdown link: pushes history, fetches URL, re-parses.
#[allow(clippy::too_many_arguments)]
fn follow_remote_md(
    url: &str,
    app: &mut App,
    source: &mut String,
    blocks: &mut Vec<RenderedBlock>,
    highlighter: &highlight::Highlighter,
    image_manager: &mut ImageManager,
    math_engine: &mut math::MathEngine,
    cols: u16,
    fetch_tx: &mpsc::Sender<ImageFetchRequest>,
    math_tx: &mpsc::Sender<math::MathRenderRequest>,
    math_bg_color: (u8, u8, u8),
    watcher: &mut FileWatcher,
) {
    app.nav_history.push(NavHistoryEntry {
        source: source.clone(),
        base_path: app.source_path.clone(),
        filename: app.filename.clone(),
        scroll_offset: app.scroll_offset,
    });
    log::debug!("following remote link: {url}");

    match fetch_url(url, MAX_FILE_BYTES) {
        Ok((new_source, display_name, base_path)) => {
            *source = new_source;
            image_manager.update_max_width(cols);
            image_manager.clear_all();
            math_engine.clear_all();
            *blocks = parser::parse(source, highlighter, image_manager, math_engine, &app.theme);
            queue_pending_fetches(blocks, fetch_tx, image_manager);
            let fs = image_manager.font_cell_size();
            queue_pending_math_renders(blocks, math_tx, math_engine, cols, fs, math_bg_color);
            let w = compute_content_width(cols, app);
            app.document = layout::flatten(blocks, w, &app.theme);
            app.scroll_offset = 0;
            app.filename = display_name;
            app.source_path = base_path;
            watcher.set_file(&app.filename);
            app.outline = None;
            app.search = None;
        }
        Err(e) => {
            app.nav_history.pop();
            app.status_message = Some(format!("Error: {e}"));
        }
    }
}

/// Walks the `RenderedBlock` tree and sends any `ImagePending` URLs to the
/// background fetch thread. Marks each URL as pending before sending so
/// it won't be re-sent on subsequent calls (e.g. after re-parse).
fn queue_pending_fetches(
    blocks: &[RenderedBlock],
    tx: &mpsc::Sender<ImageFetchRequest>,
    mgr: &mut ImageManager,
) {
    for block in blocks {
        match block {
            RenderedBlock::ImagePending { url, .. } => {
                if mgr.mark_pending(url) {
                    let _ = tx.send(ImageFetchRequest { url: url.clone() });
                }
            }
            RenderedBlock::List { items, .. } => {
                for item in items {
                    queue_pending_fetches(&item.children, tx, mgr);
                }
            }
            RenderedBlock::BlockQuote { children } => {
                queue_pending_fetches(children, tx, mgr);
            }
            RenderedBlock::Table { headers, rows, .. } => {
                for cell in headers {
                    if let TableCell::Block(b) = cell {
                        queue_pending_fetches(std::slice::from_ref(b), tx, mgr);
                    }
                }
                for row in rows {
                    for cell in row {
                        if let TableCell::Block(b) = cell {
                            queue_pending_fetches(std::slice::from_ref(b), tx, mgr);
                        }
                    }
                }
            }
            // Leaf blocks — no nested RenderedBlocks to traverse.
            RenderedBlock::Heading { .. }
            | RenderedBlock::Paragraph { .. }
            | RenderedBlock::CodeBlock { .. }
            | RenderedBlock::ThematicBreak
            | RenderedBlock::Spacer { .. }
            | RenderedBlock::Image { .. }
            | RenderedBlock::AsciiImage { .. }
            | RenderedBlock::ImageFallback { .. }
            | RenderedBlock::MathUnicode { .. }
            | RenderedBlock::MathImage { .. } => {}
        }
    }
}

/// Walks the `RenderedBlock` tree and sends ALL LaTeX formulas (inline + display)
/// to the background math render thread. No complexity heuristic — every formula
/// is queued. Skips formulas already cached, pending, or failed.
fn queue_pending_math_renders(
    blocks: &[RenderedBlock],
    tx: &mpsc::Sender<math::MathRenderRequest>,
    math: &mut math::MathEngine,
    cols: u16,
    font_size: (u16, u16),
    bg_color: (u8, u8, u8),
) {
    if !math.enabled() {
        return;
    }
    let pending_before = math.pending_count();
    for block in blocks {
        match block {
            // Display math: queued via MathUnicode.raw_latex
            RenderedBlock::MathUnicode { raw_latex, .. } => {
                if math.get_cached(raw_latex).is_some() {
                    continue;
                }
                if math.mark_pending(raw_latex) {
                    let _ = tx.send(math::MathRenderRequest {
                        latex: raw_latex.clone(),
                        display: true,
                        width_cells: cols,
                        font_size,
                        bg_color,
                    });
                }
            }
            // Already rendered — skip
            RenderedBlock::MathImage { .. } => {}

            // Inline math: queued via StyledSpan.math_latex in Paragraphs and Headings
            RenderedBlock::Paragraph { content } => {
                for span in content {
                    if span.math_latex.is_empty() {
                        continue;
                    }
                    if math.get_cached(&span.math_latex).is_some() {
                        continue;
                    }
                    if math.mark_pending(&span.math_latex) {
                        let _ = tx.send(math::MathRenderRequest {
                            latex: span.math_latex.clone(),
                            display: false,
                            width_cells: cols,
                            font_size,
                            bg_color,
                        });
                    }
                }
            }
            RenderedBlock::Heading { content, .. } => {
                for span in content {
                    if span.math_latex.is_empty() {
                        continue;
                    }
                    if math.get_cached(&span.math_latex).is_some() {
                        continue;
                    }
                    if math.mark_pending(&span.math_latex) {
                        let _ = tx.send(math::MathRenderRequest {
                            latex: span.math_latex.clone(),
                            display: false,
                            width_cells: cols,
                            font_size,
                            bg_color,
                        });
                    }
                }
            }

            // Recurse into containers
            RenderedBlock::List { items, .. } => {
                for item in items {
                    // List item inline content may contain inline math spans.
                    for span in &item.content {
                        if span.math_latex.is_empty() {
                            continue;
                        }
                        if math.get_cached(&span.math_latex).is_some() {
                            continue;
                        }
                        if math.mark_pending(&span.math_latex) {
                            let _ = tx.send(math::MathRenderRequest {
                                latex: span.math_latex.clone(),
                                display: false,
                                width_cells: cols,
                                font_size,
                                bg_color,
                            });
                        }
                    }
                    queue_pending_math_renders(&item.children, tx, math, cols, font_size, bg_color);
                }
            }
            RenderedBlock::BlockQuote { children } => {
                queue_pending_math_renders(children, tx, math, cols, font_size, bg_color);
            }
            RenderedBlock::Table { headers, rows, .. } => {
                for cell in headers {
                    if let TableCell::Block(b) = cell {
                        queue_pending_math_renders(
                            std::slice::from_ref(b), tx, math, cols, font_size, bg_color,
                        );
                    }
                }
                for row in rows {
                    for cell in row {
                        if let TableCell::Block(b) = cell {
                            queue_pending_math_renders(
                                std::slice::from_ref(b), tx, math, cols, font_size, bg_color,
                            );
                        }
                    }
                }
            }

            // Leaf blocks with no math content
            RenderedBlock::CodeBlock { .. }
            | RenderedBlock::ThematicBreak
            | RenderedBlock::Spacer { .. }
            | RenderedBlock::Image { .. }
            | RenderedBlock::AsciiImage { .. }
            | RenderedBlock::ImageFallback { .. }
            | RenderedBlock::ImagePending { .. } => {}
        }
    }
    let pending_after = math.pending_count();
    let queued = pending_after.saturating_sub(pending_before);
    if queued > 0 || pending_before != pending_after {
        log::debug!("queue_pending_math: queued {queued} new, pending {pending_after}, failed {}, cache {}",
            math.failed_count(), math.cache_count());
    }
}

/// Opens a URL with the system's default browser.
///
/// Uses the same platform-dispatch pattern as `open_with_system_viewer`.
/// Errors are silently ignored (best-effort).
fn open_url_with_system_browser(url: &str) {
    if cfg!(target_os = "linux") && is_wsl() {
        // On WSL, xdg-open handles URLs directly.
    }
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "windows")]
    let cmd = "start";
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let cmd = "xdg-open";
    let _ = std::process::Command::new(cmd)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percent_decode_space() {
        assert_eq!(percent_decode_str("my%20file.md"), "my file.md");
    }

    #[test]
    fn test_percent_decode_multiple_segments() {
        assert_eq!(
            percent_decode_str("path/to%20/my%20file.md"),
            "path/to /my file.md"
        );
    }

    #[test]
    fn test_percent_decode_no_encoding() {
        assert_eq!(percent_decode_str("basic.md"), "basic.md");
    }

    #[test]
    fn test_percent_decode_full_url() {
        assert_eq!(
            percent_decode_str("https://example.com/my%20page.md"),
            "https://example.com/my page.md"
        );
    }

    #[test]
    fn test_percent_decode_special_chars() {
        assert_eq!(percent_decode_str("%28paren%29.md"), "(paren).md");
    }

    #[test]
    fn test_percent_decode_incomplete_sequence_passthrough() {
        // Incomplete percent-sequences should pass through unchanged.
        assert_eq!(percent_decode_str("file%2.md"), "file%2.md");
        assert_eq!(percent_decode_str("file%.md"), "file%.md");
    }

    #[test]
    fn test_percent_decode_invalid_hex_passthrough() {
        assert_eq!(percent_decode_str("file%GG.md"), "file%GG.md");
    }

    /// Simulates the full link-follow flow: parse a percent-encoded local path,
    /// decode it, join with source_path, and verify the file can be opened.
    #[test]
    fn test_link_follow_percent_encoded_path_opens_file() {
        // Create a temp directory with a subdirectory containing a file whose
        // name has spaces (matching the real-world pattern "第一章 初识智能体.md").
        let dir = std::env::temp_dir().join("mdink_test_link_follow");
        let _ = std::fs::remove_dir_all(&dir);
        let sub = dir.join("docs");
        std::fs::create_dir_all(&sub).unwrap();

        let file_name = "第一章 初识智能体.md";
        let file_path = sub.join(file_name);
        std::fs::write(&file_path, "# Hello\n").unwrap();

        // This is what pulldown-cmark gives us for [text](./docs/第一章%20初识智能体.md)
        let url = "./docs/第一章%20初识智能体.md";

        // Simulate the link-follow code path from main.rs event loop.
        let url_clean = url.split('#').next().unwrap_or(url).to_string();
        let url_decoded = percent_decode_str(&url_clean);

        assert_eq!(url_decoded, "./docs/第一章 初识智能体.md");

        let source_path: PathBuf = dir.clone();
        let path = source_path.join(&url_decoded);

        // The critical assertion: Path::join + fs::read_to_string works.
        assert!(path.exists(), "path should exist: {}", path.display());
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        assert_eq!(content, "# Hello\n");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Same test but with CJK parentheses in the filename (common in Chinese docs).
    #[test]
    fn test_link_follow_cjk_parentheses_opens_file() {
        let dir = std::env::temp_dir().join("mdink_test_cjk_paren");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let file_name = "附录（工具链）.md";
        let file_path = dir.join(file_name);
        std::fs::write(&file_path, "appendix\n").unwrap();

        // pulldown-cmark passes the URL through as-is (no percent-encoding for CJK).
        let url = "附录（工具链）.md";
        let url_decoded = percent_decode_str(url);
        let source_path: PathBuf = dir.clone();
        let path = source_path.join(&url_decoded);

        assert!(path.exists(), "path should exist: {}", path.display());
        assert_eq!(fs::read_to_string(&path).unwrap(), "appendix\n");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Verify that a link with both spaces AND CJK characters resolves correctly
    /// when the markdown is in a subdirectory (relative path with ..).
    #[test]
    fn test_link_follow_relative_subdir_with_spaces() {
        let dir = std::env::temp_dir().join("mdink_test_relative");
        let _ = std::fs::remove_dir_all(&dir);
        let sub = dir.join("chapter1");
        std::fs::create_dir_all(&sub).unwrap();

        let file_name = "first chapter intro.md";
        let file_path = sub.join(file_name);
        std::fs::write(&file_path, "content\n").unwrap();

        // Source file is in dir/; link points into dir/chapter1/
        let url = "./chapter1/first%20chapter%20intro.md";
        let url_decoded = percent_decode_str(url);
        let source_path: PathBuf = dir.clone();
        let path = source_path.join(&url_decoded);

        assert!(path.exists(), "path should exist: {}", path.display());
        assert_eq!(fs::read_to_string(&path).unwrap(), "content\n");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_watcher_not_created_for_stdin() {
        let w = FileWatcher::new("-");
        assert!(!w.is_watching());
    }

    #[test]
    fn test_watcher_not_created_for_url() {
        let w = FileWatcher::new("https://example.com/readme.md");
        assert!(!w.is_watching());
        let w = FileWatcher::new("http://example.com/readme.md");
        assert!(!w.is_watching());
    }

    #[test]
    fn test_watcher_created_for_local_file() {
        // Use the current source file as a guaranteed-to-exist file.
        let w = FileWatcher::new(file!());
        assert!(w.is_watching());
    }

    #[test]
    fn test_watcher_no_change_returns_false() {
        let mut w = FileWatcher::new(file!());
        assert!(!w.check(), "file has not changed since construction");
    }

    #[test]
    fn test_watcher_detects_mtime_change() {
        let dir = std::env::temp_dir().join("mdink_test_watcher_mtime");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.md");
        std::fs::write(&path, "hello").unwrap();

        let path_str = path.to_str().unwrap();
        let mut w = FileWatcher::new(path_str);

        // No change yet.
        assert!(!w.check());

        // Modify the file and force mtime update.
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&path, "world").unwrap();
        let file = std::fs::File::open(&path).unwrap();
        file.set_modified(std::time::SystemTime::now()).unwrap();
        drop(file);

        assert!(w.check(), "should detect mtime change");
        assert!(!w.check(), "second check should return false");

        // Cleanup
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_watcher_file_deleted_returns_false() {
        let dir = std::env::temp_dir().join("mdink_test_watcher_del");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("temp_delete_test.md");
        std::fs::write(&path, "content").unwrap();

        let path_str = path.to_str().unwrap();
        let mut w = FileWatcher::new(path_str);

        assert!(!w.check(), "no change initially");
        std::fs::remove_file(&path).unwrap();
        assert!(!w.check(), "deleted file should not trigger reload");

        // Cleanup
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_watcher_set_file_updates_path() {
        let mut w = FileWatcher::new("-");
        assert!(!w.is_watching());

        // Switch to a local file.
        w.set_file(file!());
        assert!(w.is_watching());

        // Switch to a URL — should stop watching.
        w.set_file("https://example.com");
        assert!(!w.is_watching());

        // Switch back to a local file.
        w.set_file(file!());
        assert!(w.is_watching());
    }

    #[test]
    fn test_watcher_nonexistent_file_not_watching() {
        let w = FileWatcher::new("/nonexistent/path/to/file.md");
        assert!(!w.is_watching(), "nonexistent file should not be watched");
    }
}

// ── Terminal identity detection ───────────────────────────────────────────────

/// Detects the running terminal emulator from environment variables.
///
/// Returns a human-readable string like `"wezterm (WEZTERM_PANE)"` or
/// `"unknown (TERM=xterm-256color)"` for diagnostic logging.
fn detect_terminal_identity() -> String {
    if let Ok(v) = std::env::var("TERM_PROGRAM") {
        return format!("TERM_PROGRAM={v}");
    }
    if std::env::var("KITTY_WINDOW_ID").is_ok() || std::env::var("KITTY_PID").is_ok() {
        return "kitty (KITTY_WINDOW_ID/KITTY_PID)".to_string();
    }
    if std::env::var("WEZTERM_PANE").is_ok() || std::env::var("WEZTERM_EXECUTABLE").is_ok() {
        return "wezterm (WEZTERM_PANE)".to_string();
    }
    if std::env::var("GHOSTTY_RESOURCES_DIR").is_ok() {
        return "ghostty (GHOSTTY_RESOURCES_DIR)".to_string();
    }
    if std::env::var("ALACRITTY_WINDOW_ID").is_ok() {
        return "alacritty (ALACRITTY_WINDOW_ID)".to_string();
    }
    if std::env::var("WT_SESSION").is_ok() {
        return "windows-terminal (WT_SESSION)".to_string();
    }
    if let Ok(v) = std::env::var("TERM") {
        return format!("unknown (TERM={v})");
    }
    "unknown (no terminal env vars)".to_string()
}

// ── Terminal background color detection ─────────────────────────────────────

/// Detects the terminal's background color using the OSC 11 escape sequence.
///
/// Must be called **before** entering alternate screen. Sends `\x1b]11;?\x07`,
/// reads the response, and parses the `rgb:RRRR/GGGG/BBBB` color value.
///
/// Returns `None` if the terminal doesn't respond (unsupported terminal, piped
/// stdout, timeout, etc.).
fn detect_terminal_bg_color() -> Option<(u8, u8, u8)> {
    use std::io::{Read, Write};

    // Don't query if stdout isn't a terminal (piped, redirected).
    if !std::io::stdout().is_terminal() {
        return None;
    }

    let _ = ratatui::crossterm::terminal::enable_raw_mode();

    let result = (|| -> Option<(u8, u8, u8)> {
        // Send OSC 11 query (BEL terminator for maximum compatibility).
        std::io::stdout().write_all(b"\x1b]11;?\x07").ok()?;
        std::io::stdout().flush().ok()?;

        // Read response with timeout. Spawn a reader thread so we can use
        // mpsc::recv_timeout() instead of platform-specific poll().
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 128];
            let n = std::io::stdin().lock().read(&mut buf).unwrap_or(0);
            let _ = tx.send(buf[..n].to_vec());
        });

        let response_bytes = rx.recv_timeout(Duration::from_millis(200)).ok()?;
        let response = String::from_utf8_lossy(&response_bytes);
        parse_osc11_color(&response)
    })();

    let _ = ratatui::crossterm::terminal::disable_raw_mode();
    result
}

/// Parses an OSC 11 terminal response into an RGB color tuple.
///
/// Expected format: `\x1b]11;rgb:RRRR/GGGG/BBBB\x1b\\` or `...BEL`
/// Some terminals report `rgb:RR/GG/BB` (8-bit per channel) instead of 16-bit.
fn parse_osc11_color(response: &str) -> Option<(u8, u8, u8)> {
    // Find the start of the color data after "11;".
    let start = response.find("11;")? + 3;
    let rest = &response[start..];

    // Strip terminator: BEL (\x07) or ST (\x1b\\).
    let end = rest.find('\x07')
        .or_else(|| rest.find("\x1b\\"))
        .unwrap_or(rest.len());
    let color_str = &rest[..end];

    // Parse "rgb:RRRR/GGGG/BBBB" or "rgb:RR/GG/BB".
    let rgb = color_str.strip_prefix("rgb:")?;
    let parts: Vec<&str> = rgb.split('/').collect();
    if parts.len() != 3 {
        return None;
    }

    let parse_channel = |s: &str| -> Option<u8> {
        let raw = u32::from_str_radix(s, 16).ok()?;
        // 16-bit value → 8-bit (scale down).
        Some((raw.min(0xFFFF) >> (4 * (s.len() - 2))) as u8)
    };

    Some((parse_channel(parts[0])?, parse_channel(parts[1])?, parse_channel(parts[2])?))
}
