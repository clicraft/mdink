//! Image loading, terminal graphics protocol management, and remote image caching.
//!
//! This is a **leaf module** — it never imports from other mdink modules.
//! The rest of the codebase interacts with images exclusively through
//! `ImageManager`, which owns the `Picker` and `StatefulProtocol` instances.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use color_eyre::eyre::{self, eyre};
use image::imageops::FilterType;
use image::DynamicImage;
use image::ImageReader;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

/// Maximum image file size (10 MB). Prevents OOM during decode.
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

/// Default font cell size in pixels (width, height) when no picker is available.
/// Typical terminal fonts are 8px wide × 16px tall. Used by ASCII art rendering
/// to compute natural image dimensions in cell units.
const DEFAULT_FONT_SIZE: (u16, u16) = (8, 16);

/// Character ramp ordered by visual density (fraction of cell filled).
/// Combines braille, block shades, and ASCII into one 18-level gradient.
const DENSITY_RAMP: &[char] = &[
    ' ', '.', '·', '⠂', ':', '⠒', '-', '░', '=', '+', '⠿', '▒', '*', '#', '⣿', '▓', '@', '█',
];

/// A cached decoded image, keyed by URL or local path.
pub struct CachedImage {
    pub dyn_img: DynamicImage,
}

/// Request sent to the background image fetch thread.
pub struct ImageFetchRequest {
    pub url: String,
}

/// Result sent back from the background fetch thread.
pub enum ImageFetchResult {
    /// Successfully downloaded and decoded.
    Ok {
        url: String,
        dyn_img: DynamicImage,
    },
    /// Download or decode failed.
    #[allow(dead_code)] // error/expected read only in tests
    Err {
        url: String,
        error: String,
        /// True when the failure is expected (e.g. unsupported image format
        /// like SVG). The caller should silently degrade to fallback without
        /// printing a warning.
        expected: bool,
    },
}

/// Owns the terminal graphics picker, loaded image protocols, and remote image cache.
///
/// Created once at startup, passed to the parser (for loading images)
/// and to the renderer (for drawing them). When `picker` is `None` and
/// `no_images` is false, images are rendered as colored ASCII art.
/// When `no_images` is true, all images degrade to alt-text fallback.
pub struct ImageManager {
    picker: Option<Picker>,
    protocols: Vec<StatefulProtocol>,
    base_path: PathBuf,
    max_width: u16,
    no_images: bool,
    force_ascii: bool,
    fetch_remote: bool,
    /// Cache of decoded images keyed by URL/path. Survives re-parse within
    /// the same document (cleared by `clear_all()` on document change).
    cache: HashMap<String, CachedImage>,
    /// URLs currently being fetched by the background thread.
    pending_urls: HashSet<String>,
    /// URLs that failed to fetch/decode (not retried within same document).
    failed_urls: HashSet<String>,
}

impl ImageManager {
    /// Creates a new `ImageManager`.
    ///
    /// - `base_path`: directory of the markdown file (for resolving relative image paths).
    /// - `picker`: terminal graphics picker, or `None` when terminal lacks graphics.
    /// - `max_width`: terminal width in columns (images are scaled to fit).
    /// - `no_images`: when true, all images degrade to alt-text (user passed `--no-images`).
    /// - `force_ascii`: when true, always use ASCII art instead of native graphics.
    pub fn new(
        base_path: PathBuf,
        picker: Option<Picker>,
        max_width: u16,
        no_images: bool,
        force_ascii: bool,
        fetch_remote: bool,
    ) -> Self {
        Self {
            picker,
            protocols: Vec::new(),
            base_path,
            max_width,
            no_images,
            force_ascii,
            fetch_remote,
            cache: HashMap::new(),
            pending_urls: HashSet::new(),
            failed_urls: HashSet::new(),
        }
    }

    /// Returns true if the terminal supports a graphics protocol (Sixel/Kitty/iTerm2/halfblocks).
    pub fn has_graphics_support(&self) -> bool {
        self.picker.is_some()
    }

    /// Returns true if the user explicitly disabled images via `--no-images`.
    pub fn images_disabled(&self) -> bool {
        self.no_images
    }

    /// Returns true if ASCII art rendering should be preferred over native graphics.
    pub fn prefer_ascii(&self) -> bool {
        self.force_ascii
    }

    /// Returns true if remote images should be fetched from the network.
    /// When false, remote URLs produce `ImageFallback` instead of `ImagePending`.
    pub fn fetch_remote(&self) -> bool {
        self.fetch_remote
    }

    /// Toggles remote image fetching at runtime.
    ///
    /// After toggling, callers should re-parse the document so that remote URLs
    /// are reclassified as `ImagePending` (on) or `ImageFallback` (off).
    /// The existing cache is preserved — already-downloaded images remain available.
    pub fn set_fetch_remote(&mut self, enabled: bool) {
        self.fetch_remote = enabled;
        // Clear failed URLs so newly-enabled fetches can retry.
        if enabled {
            self.failed_urls.clear();
        }
    }

    /// Returns true if `src` looks like a remote HTTP/HTTPS URL.
    pub fn is_remote_url(src: &str) -> bool {
        src.starts_with("http://") || src.starts_with("https://")
    }

    /// Updates the maximum width (e.g. after terminal resize or refresh).
    pub fn update_max_width(&mut self, width: u16) {
        self.max_width = width;
    }

    /// Returns the font cell size in pixels (width, height).
    pub fn font_cell_size(&self) -> (u16, u16) {
        self.picker
            .as_ref()
            .map(|p| p.font_size())
            .unwrap_or(DEFAULT_FONT_SIZE)
    }

    /// Clears all loaded protocols so indices start fresh on re-parse.
    ///
    /// Keeps the image cache intact (for same-document re-parse).
    pub fn clear_protocols(&mut self) {
        log::debug!("clearing {} image protocols", self.protocols.len());
        self.protocols.clear();
    }

    /// Returns the number of loaded protocols.
    pub fn protocol_count(&self) -> usize {
        self.protocols.len()
    }

    /// Clears everything: protocols, cache, and pending/failed tracking.
    ///
    /// Called when loading a new document (different source file).
    pub fn clear_all(&mut self) {
        self.protocols.clear();
        self.cache.clear();
        self.pending_urls.clear();
        self.failed_urls.clear();
    }

    // ── Cache operations ──────────────────────────────────────────────

    /// Looks up a cached decoded image by URL.
    pub fn get_cached(&self, url: &str) -> Option<&DynamicImage> {
        self.cache.get(url).map(|c| &c.dyn_img)
    }

    /// Inserts a decoded image into the cache.
    pub fn insert_cache(&mut self, url: String, dyn_img: DynamicImage) {
        log::info!("cached image: {url} ({}x{} px)", dyn_img.width(), dyn_img.height());
        self.cache.insert(url, CachedImage { dyn_img });
    }

    // ── Pending/failed tracking ───────────────────────────────────────

    /// Marks a URL as pending fetch. Returns false if already pending or failed.
    pub fn mark_pending(&mut self, url: &str) -> bool {
        if self.pending_urls.contains(url) || self.failed_urls.contains(url) {
            return false;
        }
        self.pending_urls.insert(url.to_string())
    }

    /// Marks a URL as failed (not retried within same document).
    pub fn mark_failed(&mut self, url: &str) {
        self.pending_urls.remove(url);
        self.failed_urls.insert(url.to_string());
    }

    /// Marks a URL as resolved (successfully fetched).
    pub fn mark_resolved(&mut self, url: &str) {
        self.pending_urls.remove(url);
    }

    /// Returns true if the URL is currently pending or has previously failed.
    #[cfg(test)]
    pub fn is_pending_or_failed(&self, url: &str) -> bool {
        self.pending_urls.contains(url) || self.failed_urls.contains(url)
    }

    /// Returns true if a previous fetch for this URL failed.
    ///
    /// Used by the parser to decide between `ImagePending` and `ImageFallback`:
    /// a failed URL should degrade to fallback immediately rather than emitting
    /// another `ImagePending` that will never be re-queued.
    pub fn is_failed_url(&self, url: &str) -> bool {
        self.failed_urls.contains(url)
    }

    /// Returns true if there are no URLs currently being fetched.
    /// Used by the event loop to decide whether to poll with timeout.
    pub fn pending_urls_is_empty(&self) -> bool {
        self.pending_urls.is_empty()
    }

    // ── Image loading ─────────────────────────────────────────────────

    /// Loads a local image and returns its protocol index and cell dimensions.
    ///
    /// The image is decoded, scaled to fit `max_width` columns (maintaining
    /// aspect ratio), and a `StatefulProtocol` is created for terminal rendering.
    ///
    /// Returns `(protocol_index, width_cells, height_cells, px_width, px_height)` on success.
    /// Returns `Err` if the image can't be loaded, is too large, or there
    /// is no graphics support.
    pub fn load_image(&mut self, src: &str) -> eyre::Result<(usize, u16, u16, u32, u32)> {
        let path = self.base_path.join(src);

        // Size guard: reject images that could OOM during decode.
        let file_size = fs::metadata(&path)
            .map_err(|e| eyre!("cannot read image '{}': {}", path.display(), e))?
            .len();
        if file_size > MAX_IMAGE_BYTES {
            log::warn!("rejecting local image {}: too large ({} bytes)", path.display(), file_size);
            return Err(eyre!(
                "image '{}' too large ({} bytes; limit is {} bytes)",
                path.display(),
                file_size,
                MAX_IMAGE_BYTES
            ));
        }

        let dyn_img = ImageReader::open(&path)
            .map_err(|e| eyre!("cannot open image '{}': {}", path.display(), e))?
            .with_guessed_format()
            .map_err(|e| eyre!("cannot detect format for '{}': {}", path.display(), e))?
            .decode()
            .map_err(|e| eyre!("cannot decode image '{}': {}", path.display(), e))?;

        self.load_image_from_memory(dyn_img)
    }

    /// Creates a protocol from a pre-decoded `DynamicImage` (from cache or fetch).
    ///
    /// Returns `(protocol_index, width_cells, height_cells, px_width, px_height)`.
    pub fn load_image_from_memory(
        &mut self,
        dyn_img: DynamicImage,
    ) -> eyre::Result<(usize, u16, u16, u32, u32)> {
        let picker = self.picker.as_ref().ok_or_else(|| eyre!("no graphics support"))?;

        let (px_w, px_h) = (dyn_img.width(), dyn_img.height());
        let (font_w, font_h) = picker.font_size();
        let font_w = font_w.max(1) as u32;
        let font_h = font_h.max(1) as u32;

        let mut w_cells = px_w.div_ceil(font_w) as u16;
        let mut h_cells = px_h.div_ceil(font_h) as u16;

        // Scale down to fit terminal width, maintaining aspect ratio.
        if w_cells > self.max_width && w_cells > 0 {
            let scale = self.max_width as f64 / w_cells as f64;
            w_cells = self.max_width;
            h_cells = (h_cells as f64 * scale).ceil() as u16;
        }
        h_cells = h_cells.max(1);

        let protocol = picker.new_resize_protocol(dyn_img);
        let index = self.protocols.len();
        self.protocols.push(protocol);

        Ok((index, w_cells, h_cells, px_w, px_h))
    }

    /// Loads a local image and converts it to colored ASCII art lines.
    ///
    /// Each pixel is mapped to a density character colored with its RGB value.
    /// Returns `Vec<Line<'static>>` on success, one `Line` per row of the image.
    pub fn load_ascii_image(&self, src: &str) -> eyre::Result<Vec<Line<'static>>> {
        let path = self.base_path.join(src);

        let file_size = fs::metadata(&path)
            .map_err(|e| eyre!("cannot read image '{}': {}", path.display(), e))?
            .len();
        if file_size > MAX_IMAGE_BYTES {
            return Err(eyre!(
                "image '{}' too large ({} bytes; limit is {} bytes)",
                path.display(),
                file_size,
                MAX_IMAGE_BYTES
            ));
        }

        let dyn_img = ImageReader::open(&path)
            .map_err(|e| eyre!("cannot open image '{}': {}", path.display(), e))?
            .with_guessed_format()
            .map_err(|e| eyre!("cannot detect format for '{}': {}", path.display(), e))?
            .decode()
            .map_err(|e| eyre!("cannot decode image '{}': {}", path.display(), e))?;

        self.load_ascii_image_from_memory(&dyn_img)
    }

    /// Converts a pre-decoded `DynamicImage` to colored ASCII art lines.
    pub fn load_ascii_image_from_memory(
        &self,
        dyn_img: &DynamicImage,
    ) -> eyre::Result<Vec<Line<'static>>> {
        let (px_w, px_h) = (dyn_img.width(), dyn_img.height());
        let (font_w, font_h) = self.font_cell_size();
        let font_w = font_w.max(1) as u32;
        let font_h = font_h.max(1) as u32;

        let mut w = px_w.div_ceil(font_w);
        let mut h = px_h.div_ceil(font_h);

        // Scale down to fit terminal width, maintaining aspect ratio.
        let max_w = self.max_width.max(1) as u32;
        if w > max_w && w > 0 {
            let scale = max_w as f64 / w as f64;
            w = max_w;
            h = (h as f64 * scale).ceil() as u32;
        }
        let w = w.max(1);
        let h = h.max(1);

        let rgb = dyn_img.resize_exact(w, h, FilterType::Triangle).to_rgb8();

        let ramp_len = DENSITY_RAMP.len();
        let mut lines = Vec::with_capacity(h as usize);
        for y in 0..h {
            let mut spans = Vec::with_capacity(w as usize);
            for x in 0..w {
                let pixel = rgb.get_pixel(x, y);
                let [r, g, b] = pixel.0;
                let luminance = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
                // Gamma-expand before ramp lookup. A linear luma→index mapping causes
                // mid-tones to select sparse characters whose black background bleed
                // darkens the image; raising to 1/2.2 biases toward denser characters
                // and recovers perceived brightness.
                let idx = ((luminance / 255.0).powf(1.0 / 2.2) * (ramp_len - 1) as f64).round()
                    as usize;
                let ch = DENSITY_RAMP[idx.min(ramp_len - 1)];
                spans.push(Span::styled(
                    ch.to_string(),
                    Style::default().fg(Color::Rgb(r, g, b)),
                ));
            }
            lines.push(Line::from(spans));
        }

        Ok(lines)
    }

    // ── Protocol access ───────────────────────────────────────────────

    /// Returns a mutable reference to a protocol by index.
    ///
    /// Used by the renderer at draw time. `StatefulProtocol` needs `&mut`
    /// because it lazily encodes the image on first render.
    pub fn get_protocol(&mut self, index: usize) -> &mut StatefulProtocol {
        &mut self.protocols[index]
    }
}

/// Image MIME types that the `image` crate can decode.
const SUPPORTED_IMAGE_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/tiff",
    "image/bmp",
    "image/x-tga",
];

/// Downloads and decodes a remote image via HTTP.
///
/// Runs on the background fetch thread. Enforces the `MAX_IMAGE_BYTES`
/// size guard at the system boundary.
///
/// Returns `Ok(dyn_img)` on success, or `Err` with `expected = true`
/// when the server returned an unsupported format (e.g. SVG).
/// Timeout for remote image fetches (connect + transfer).
const FETCH_TIMEOUT_SECS: u64 = 10;

pub fn fetch_image(url: &str) -> Result<DynamicImage, ImageFetchError> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(FETCH_TIMEOUT_SECS)))
        .build();
    let agent = ureq::Agent::new_with_config(config);

    let mut response = agent
        .get(url)
        .call()
        .map_err(|e| ImageFetchError::new(format!("HTTP error for {url}: {e}"), false))?;

    // Check Content-Type: skip known non-raster formats early.
    if let Some(ct) = response.headers().get("content-type").and_then(|v| v.to_str().ok()) {
        let ct_lower = ct.to_ascii_lowercase();
        let is_supported = SUPPORTED_IMAGE_TYPES
            .iter()
            .any(|t| ct_lower.starts_with(t));
        if !is_supported {
            log::warn!("skipping remote image {url}: unsupported format {ct}");
            return Err(ImageFetchError::new(
                format!("unsupported format for {url}: {ct}"),
                true, // expected — SVG, HTML, etc.
            ));
        }
    }

    let mut body = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|e| ImageFetchError::new(format!("read error for {url}: {e}"), false))?;

    if body.len() as u64 > MAX_IMAGE_BYTES {
        log::warn!("rejecting remote image {url}: too large ({} bytes)", body.len());
        return Err(ImageFetchError::new(
            format!("image '{url}' too large ({} bytes; limit is {MAX_IMAGE_BYTES} bytes)", body.len()),
            false,
        ));
    }

    image::load_from_memory(&body).map_err(|e| {
        log::warn!("failed to decode remote image {url}: {e}");
        ImageFetchError::new(
            format!("decode error for {url}: {e}"),
            // Decode errors for "could not be determined" are almost always
            // an unsupported format (SVG, ICO, etc.) — treat as expected.
            e.to_string().contains("could not be determined"),
        )
    })
}

/// Error from `fetch_image`. Carries an `expected` flag indicating whether
/// the failure is an anticipated condition (unsupported format) rather than
/// a network or server error.
pub struct ImageFetchError {
    message: String,
    expected: bool,
}

impl ImageFetchError {
    fn new(message: String, expected: bool) -> Self {
        Self { message, expected }
    }

    pub fn is_expected(&self) -> bool {
        self.expected
    }
}

impl std::fmt::Display for ImageFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::fmt::Debug for ImageFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ImageFetchError({})", self.message)
    }
}

impl std::error::Error for ImageFetchError {}

#[cfg(test)]
mod tests;
