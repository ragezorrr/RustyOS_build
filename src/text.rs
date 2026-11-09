//! Tiny CPU text renderer used for the Aqua shell (menu bar, dock, labels).
//!
//! We rasterize glyphs with `fontdue` straight into a `tiny-skia` pixmap.
//! Powered by authentic Apple San Francisco (SF Pro Text Regular & Bold).

use fontdue::{Font, FontSettings};
use tiny_skia::Pixmap;

pub struct TextRenderer {
    font: Font,
    font_bold: Font,
}

impl TextRenderer {
    pub fn new() -> Self {
        let regular_data = include_bytes!("../assets/fonts/SF-Pro-Text-Regular.otf");
        let bold_data = include_bytes!("../assets/fonts/SF-Pro-Text-Bold.otf");
        let font = Font::from_bytes(regular_data as &[u8], FontSettings::default())
            .expect("bundled SF Pro Regular font must parse");
        let font_bold = Font::from_bytes(bold_data as &[u8], FontSettings::default())
            .expect("bundled SF Pro Bold font must parse");
        Self { font, font_bold }
    }

    pub fn width(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|c| self.font.metrics(c, size).advance_width)
            .sum()
    }

    pub fn width_bold(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|c| self.font_bold.metrics(c, size).advance_width)
            .sum()
    }

    /// Draw text with real Apple SF Pro Bold.
    pub fn draw_bold(
        &self,
        pixmap: &mut Pixmap,
        text: &str,
        x: f32,
        baseline: f32,
        size: f32,
        color: [u8; 4],
    ) {
        self.draw_with_font(&self.font_bold, pixmap, text, x, baseline, size, color);
    }

    /// Draw `text` with its baseline at `baseline` (in pixels), starting at `x`.
    pub fn draw(
        &self,
        pixmap: &mut Pixmap,
        text: &str,
        x: f32,
        baseline: f32,
        size: f32,
        color: [u8; 4],
    ) {
        self.draw_with_font(&self.font, pixmap, text, x, baseline, size, color);
    }

    fn draw_with_font(
        &self,
        font: &Font,
        pixmap: &mut Pixmap,
        text: &str,
        mut x: f32,
        baseline: f32,
        size: f32,
        color: [u8; 4],
    ) {
        let w = pixmap.width() as i32;
        let h = pixmap.height() as i32;
        let data = pixmap.data_mut();

        for ch in text.chars() {
            let (metrics, bitmap) = font.rasterize(ch, size);
            if metrics.width > 0 && metrics.height > 0 {
                let gx = (x + metrics.xmin as f32).round() as i32;
                let gy = (baseline - (metrics.ymin + metrics.height as i32) as f32).round() as i32;

                for row in 0..metrics.height {
                    for col in 0..metrics.width {
                        let coverage = bitmap[row * metrics.width + col];
                        if coverage == 0 {
                            continue;
                        }
                        let px = gx + col as i32;
                        let py = gy + row as i32;
                        if px < 0 || py < 0 || px >= w || py >= h {
                            continue;
                        }
                        let idx = ((py * w + px) * 4) as usize;
                        let sa = (coverage as f32 / 255.0) * (color[3] as f32 / 255.0);
                        let inv = 1.0 - sa;
                        data[idx] = (color[0] as f32 * sa + data[idx] as f32 * inv)
                            .round()
                            .min(255.0) as u8;
                        data[idx + 1] = (color[1] as f32 * sa + data[idx + 1] as f32 * inv)
                            .round()
                            .min(255.0) as u8;
                        data[idx + 2] = (color[2] as f32 * sa + data[idx + 2] as f32 * inv)
                            .round()
                            .min(255.0) as u8;
                        data[idx + 3] = (255.0 * sa + data[idx + 3] as f32 * inv)
                            .round()
                            .min(255.0) as u8;
                    }
                }
            }
            x += metrics.advance_width;
        }
    }
}
