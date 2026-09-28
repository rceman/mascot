//! PNG decode/encode (straight RGBA8) and pixel-format helpers.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    /// Straight (non-premultiplied) RGBA8, row-major, tightly packed.
    pub data: Vec<u8>,
}

impl RgbaImage {
    pub fn new(width: u32, height: u32) -> Self {
        RgbaImage { width, height, data: vec![0; (width * height * 4) as usize] }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut dec = png::Decoder::new(BufReader::new(file));
        dec.set_transformations(png::Transformations::normalize_to_color8() | png::Transformations::ALPHA);
        let mut reader = dec.read_info().map_err(|e| format!("{}: {e}", path.display()))?;
        let size = reader.output_buffer_size().ok_or_else(|| format!("{}: image too large", path.display()))?;
        let mut buf = vec![0; size];
        let info = reader.next_frame(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
        buf.truncate(info.buffer_size());
        let (w, h) = (info.width, info.height);
        let data = match info.color_type {
            png::ColorType::Rgba => buf,
            png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
            other => return Err(format!("{}: unsupported colour type {other:?}", path.display())),
        };
        Ok(RgbaImage { width: w, height: h, data })
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut enc = png::Encoder::new(BufWriter::new(file), self.width, self.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&self.data).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Straight RGBA -> premultiplied BGRA (Direct2D's native bitmap layout).
    pub fn to_premultiplied_bgra(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.data.len()];
        for (o, p) in out.chunks_exact_mut(4).zip(self.data.chunks_exact(4)) {
            let a = p[3] as u32;
            let pm = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            o[0] = pm(p[2]);
            o[1] = pm(p[1]);
            o[2] = pm(p[0]);
            o[3] = p[3];
        }
        out
    }

    /// Premultiplied BGRA rows (with `pitch`) -> straight RGBA.
    pub fn from_premultiplied_bgra(width: u32, height: u32, pitch: usize, bits: &[u8]) -> Self {
        let mut img = RgbaImage::new(width, height);
        for y in 0..height as usize {
            let row = &bits[y * pitch..y * pitch + width as usize * 4];
            for (x, p) in row.chunks_exact(4).enumerate() {
                let o = &mut img.data[(y * width as usize + x) * 4..][..4];
                let a = p[3] as u32;
                let un = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a / 2) / a).min(255) as u8 };
                o[0] = un(p[2]);
                o[1] = un(p[1]);
                o[2] = un(p[0]);
                o[3] = p[3];
            }
        }
        img
    }

    pub fn alpha(&self) -> Vec<u8> {
        self.data.chunks_exact(4).map(|p| p[3]).collect()
    }

    /// Composites over an opaque colour.
    pub fn over(&self, bg: [u8; 3]) -> RgbaImage {
        let mut o = self.clone();
        for p in o.data.chunks_exact_mut(4) {
            let a = p[3] as u32;
            for c in 0..3 {
                p[c] = ((p[c] as u32 * a + bg[c] as u32 * (255 - a) + 127) / 255) as u8;
            }
            p[3] = 255;
        }
        o
    }

    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
        let mut o = RgbaImage::new(w, h);
        for row in 0..h {
            let sy = y + row;
            if sy >= self.height {
                break;
            }
            for col in 0..w {
                let sx = x + col;
                if sx >= self.width {
                    break;
                }
                let s = ((sy * self.width + sx) * 4) as usize;
                let d = ((row * w + col) * 4) as usize;
                o.data[d..d + 4].copy_from_slice(&self.data[s..s + 4]);
            }
        }
        o
    }

    /// Box-filter downscale by an integer factor.
    pub fn downscale(&self, f: u32) -> RgbaImage {
        let (w, h) = (self.width / f, self.height / f);
        let mut o = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0u32; 4];
                for dy in 0..f {
                    for dx in 0..f {
                        let s = (((y * f + dy) * self.width + x * f + dx) * 4) as usize;
                        let a = self.data[s + 3] as u32;
                        for c in 0..3 {
                            acc[c] += self.data[s + c] as u32 * a;
                        }
                        acc[3] += a;
                    }
                }
                let d = ((y * w + x) * 4) as usize;
                let n = f * f;
                for c in 0..3 {
                    o.data[d + c] = if acc[3] == 0 { 0 } else { (acc[c] / acc[3]) as u8 };
                }
                o.data[d + 3] = (acc[3] / n) as u8;
            }
        }
        o
    }

    /// Pastes `src` (opaque or not, copied verbatim) at (x, y).
    pub fn blit(&mut self, src: &RgbaImage, x: u32, y: u32) {
        for row in 0..src.height {
            let dy = y + row;
            if dy >= self.height {
                break;
            }
            let n = src.width.min(self.width.saturating_sub(x)) as usize * 4;
            let s = (row * src.width * 4) as usize;
            let d = ((dy * self.width + x) * 4) as usize;
            self.data[d..d + n].copy_from_slice(&src.data[s..s + n]);
        }
    }

    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, c: [u8; 4]) {
        for yy in y..(y + h).min(self.height) {
            for xx in x..(x + w).min(self.width) {
                let d = ((yy * self.width + xx) * 4) as usize;
                self.data[d..d + 4].copy_from_slice(&c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiply_round_trip() {
        let img = RgbaImage { width: 2, height: 1, data: vec![255, 128, 0, 255, 200, 100, 50, 128] };
        let bgra = img.to_premultiplied_bgra();
        assert_eq!(&bgra[0..4], &[0, 128, 255, 255]);
        let back = RgbaImage::from_premultiplied_bgra(2, 1, 8, &bgra);
        for (a, b) in back.data.iter().zip(&img.data) {
            assert!((*a as i32 - *b as i32).abs() <= 2);
        }
    }
}
