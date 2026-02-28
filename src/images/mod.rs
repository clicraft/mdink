//! Image loading and terminal graphics protocol management.
//!
//! This is a **leaf module** — it never imports from other mdink modules.
//! The rest of the codebase interacts with images exclusively through
//! `ImageManager`, which owns the `Picker` and `StatefulProtocol` instances.

use std::fs;
use std::path::PathBuf;

use color_eyre::eyre::{self, eyre};
use image::imageops::FilterType;
use image::ImageReader;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

/// Maximum image file size (10 MB). Prevents OOM during decode.
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

/// Character ramp ordered by visual density (fraction of cell filled).
/// Combines braille, block shades, and ASCII into one 18-level gradient.
const DENSITY_RAMP: &[char] = &[
    ' ', '.', '·', '⠂', ':', '⠒', '-', '░', '=', '+', '⠿', '▒', '*', '#', '⣿', '▓', '@', '█',
];

/// Owns the terminal graphics picker and all loaded image protocols.
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
}

impl ImageManager {
    /// Creates a new `ImageManager`.
    ///
    /// - `base_path`: directory of the markdown file (for resolving relative image paths).
    /// - `picker`: terminal graphics picker, or `None` when terminal lacks graphics.
    /// - `max_width`: terminal width in columns (images are scaled to fit).
    /// - `no_images`: when true, all images degrade to alt-text (user passed `--no-images`).
    pub fn new(base_path: PathBuf, picker: Option<Picker>, max_width: u16, no_images: bool) -> Self {
        Self {
            picker,
            protocols: Vec::new(),
            base_path,
            max_width,
            no_images,
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

    /// Returns the maximum image width in terminal columns.
    pub fn max_width(&self) -> u16 {
        self.max_width
    }

    /// Updates the maximum width (e.g. after terminal resize or refresh).
    pub fn update_max_width(&mut self, width: u16) {
        self.max_width = width;
    }

    /// Clears all loaded protocols so indices start fresh on re-parse.
    ///
    /// Must be called before re-parsing to avoid unbounded growth of the
    /// protocols vec (each `load_image` call pushes a new entry).
    pub fn clear_protocols(&mut self) {
        self.protocols.clear();
    }

    /// Loads an image and returns its protocol index and cell dimensions.
    ///
    /// The image is decoded, scaled to fit `max_width` columns (maintaining
    /// aspect ratio), and a `StatefulProtocol` is created for terminal rendering.
    ///
    /// Returns `(protocol_index, width_cells, height_cells)` on success.
    /// Returns `Err` if the image can't be loaded, is too large, or there
    /// is no graphics support.
    pub fn load_image(&mut self, src: &str) -> eyre::Result<(usize, u16, u16)> {
        let picker = self.picker.as_ref().ok_or_else(|| eyre!("no graphics support"))?;

        let path = self.base_path.join(src);

        // Size guard: reject images that could OOM during decode.
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

        // Compute cell dimensions from pixel dimensions and font size.
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

        Ok((index, w_cells, h_cells))
    }

    /// Loads an image and converts it to colored ASCII art lines.
    ///
    /// Each pixel is mapped to a density character colored with its RGB value.
    /// The image is resized to fit `width` columns with aspect-ratio correction
    /// (terminal cells are ~2x taller than wide).
    ///
    /// Returns `Vec<Line<'static>>` on success, one `Line` per row of the image.
    pub fn load_ascii_image(&self, src: &str, width: u16) -> eyre::Result<Vec<Line<'static>>> {
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

        let w = width.max(1) as u32;
        let aspect = dyn_img.height() as f64 / dyn_img.width().max(1) as f64;
        // Divide by 2 because terminal cells are ~2x taller than wide.
        let h = ((w as f64 * aspect) / 2.0).ceil().max(1.0) as u32;

        let rgb = dyn_img.resize_exact(w, h, FilterType::Triangle).to_rgb8();

        let ramp_len = DENSITY_RAMP.len();
        let mut lines = Vec::with_capacity(h as usize);
        for y in 0..h {
            let mut spans = Vec::with_capacity(w as usize);
            for x in 0..w {
                let pixel = rgb.get_pixel(x, y);
                let [r, g, b] = pixel.0;
                let luminance = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
                let idx = ((luminance / 255.0) * (ramp_len - 1) as f64).round() as usize;
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

    /// Returns a mutable reference to a protocol by index.
    ///
    /// Used by the renderer at draw time. `StatefulProtocol` needs `&mut`
    /// because it lazily encodes the image on first render.
    pub fn get_protocol(&mut self, index: usize) -> &mut StatefulProtocol {
        &mut self.protocols[index]
    }
}

#[cfg(test)]
mod tests;
