use std::path::Path;

/// Packed RGBA8 image, row-major top-left origin.
#[derive(Clone)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

impl Image {
    pub fn new(w: u32, h: u32, rgba: [u8; 4]) -> Self {
        let mut data = vec![0u8; (w * h * 4) as usize];
        for px in data.chunks_exact_mut(4) {
            px.copy_from_slice(&rgba);
        }
        Image { w, h, data }
    }

    #[inline]
    pub fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }

    #[inline]
    pub fn set(&mut self, x: u32, y: u32, c: [u8; 4]) {
        let i = ((y * self.w + x) * 4) as usize;
        self.data[i..i + 4].copy_from_slice(&c);
    }

    /// Alpha-composite `src` (straight alpha over existing pixel).
    #[inline]
    pub fn over(&mut self, x: u32, y: u32, c: [u8; 4], cov: f64) {
        if cov <= 0.0 {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let a = cov.min(1.0) * c[3] as f64 / 255.0;
        let ia = self.data[i + 3] as f64 / 255.0;
        let oa = a + ia * (1.0 - a);
        if oa <= 0.0 {
            self.data[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
            return;
        }
        for (ch, sc) in c.iter().take(3).enumerate() {
            let s = *sc as f64 * a;
            let d = self.data[i + ch] as f64 * ia * (1.0 - a);
            self.data[i + ch] = ((s + d) / oa).round().clamp(0.0, 255.0) as u8;
        }
        self.data[i + 3] = (oa * 255.0).round() as u8;
    }
}

/// Decode any colour-type PNG to RGBA8.
pub fn load_png(path: &Path) -> Result<Image, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    decode_png(&bytes)
}

pub fn decode_png(bytes: &[u8]) -> Result<Image, String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().map_err(|e| format!("png header: {e}"))?;
    let out_size = reader
        .output_buffer_size()
        .ok_or_else(|| "png: no output size".to_string())?;
    let mut buf = vec![0u8; out_size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("png decode: {e}"))?;
    let (w, h) = (info.width, info.height);
    let mut out = Image::new(w, h, [0, 0, 0, 0]);
    let bpp = match info.color_type {
        png::ColorType::Rgba => 4usize,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => unreachable!("expanded"),
    };
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize * bpp;
            let c = match info.color_type {
                png::ColorType::Rgba => [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]],
                png::ColorType::Rgb => [buf[i], buf[i + 1], buf[i + 2], 255],
                png::ColorType::GrayscaleAlpha => [buf[i], buf[i], buf[i], buf[i + 1]],
                png::ColorType::Grayscale => [buf[i], buf[i], buf[i], 255],
                png::ColorType::Indexed => unreachable!(),
            };
            out.set(x, y, c);
        }
    }
    Ok(out)
}

/// Deterministic RGBA8 PNG encoding.
pub fn save_png(path: &Path, img: &Image) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.w, img.h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Balanced);
        enc.set_filter(png::Filter::Adaptive);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&img.data).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, &out).map_err(|e| format!("write {}: {e}", path.display()))
}
