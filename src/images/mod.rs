//! Image loading and terminal graphics protocol management.
//!
//! This is a **leaf module** — it never imports from other mdink modules.
//! The rest of the codebase interacts with images exclusively through
//! `ImageManager`, which owns the `Picker` and `StatefulProtocol` instances.

use std::fs;
use std::path::PathBuf;

use color_eyre::eyre::{self, eyre};
use image::ImageReader;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

/// Maximum image file size (10 MB). Prevents OOM during decode.
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

/// Owns the terminal graphics picker and all loaded image protocols.
///
/// Created once at startup, passed to the parser (for loading images)
/// and to the renderer (for drawing them). When `picker` is `None`,
/// all images degrade to alt-text fallback.
pub struct ImageManager {
    picker: Option<Picker>,
    protocols: Vec<StatefulProtocol>,
    base_path: PathBuf,
    max_width: u16,
}

impl ImageManager {
    /// Creates a new `ImageManager`.
    ///
    /// - `base_path`: directory of the markdown file (for resolving relative image paths).
    /// - `picker`: terminal graphics picker, or `None` to disable images.
    /// - `max_width`: terminal width in columns (images are scaled to fit).
    pub fn new(base_path: PathBuf, picker: Option<Picker>, max_width: u16) -> Self {
        Self {
            picker,
            protocols: Vec::new(),
            base_path,
            max_width,
        }
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
