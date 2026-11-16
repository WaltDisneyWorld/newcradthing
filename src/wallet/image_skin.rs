//! Card-skin image pipeline.
//!
//! A Wallet card face is a 1536×969 asset trio on the device:
//!   cardBackgroundCombined@3x.png   (full resolution)
//!   cardBackgroundCombined@2x.png   (two-thirds)
//!   cardBackgroundCombined.pdf      (vector container holding the same art)
//!
//! Given any PNG/JPG/WebP and a crop offset, we "cover"-fit it to the canvas and
//! render all three.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use image::{imageops::FilterType, DynamicImage, RgbImage, RgbaImage};

use crate::model::{CARD_CANVAS_H, CARD_CANVAS_W};

#[allow(dead_code)] // source_name/preview are retained for callers and tooling
pub struct PreparedSkin {
    pub source_name: String,
    /// Cropped canvas as RGBA (used for the on-screen preview).
    pub preview: RgbaImage,
    pub png_3x: Vec<u8>,
    pub png_2x: Vec<u8>,
    pub pdf: Vec<u8>,
}

/// Load an image file into memory (for repeated re-cropping as the user drags).
pub fn load_source(path: &Path) -> Result<DynamicImage> {
    let img = image::ImageReader::open(path)
        .with_context(|| format!("Could not open image: {}", path.display()))?
        .with_guessed_format()?
        .decode()
        .with_context(|| "Image format not supported by this loader")?;
    Ok(img)
}

/// Cover-fit `src` to the card canvas, biased by `offset` (each component 0..=1).
pub fn crop_to_canvas(src: &DynamicImage, offset: (f32, f32)) -> RgbaImage {
    let (sw, sh) = (src.width().max(1), src.height().max(1));
    let scale = (CARD_CANVAS_W as f32 / sw as f32).max(CARD_CANVAS_H as f32 / sh as f32);
    let scaled_w = (sw as f32 * scale).ceil() as u32;
    let scaled_h = (sh as f32 * scale).ceil() as u32;
    let scaled = src.resize_exact(scaled_w, scaled_h, FilterType::Lanczos3);

    let extra_x = scaled_w.saturating_sub(CARD_CANVAS_W);
    let extra_y = scaled_h.saturating_sub(CARD_CANVAS_H);
    let ox = (extra_x as f32 * offset.0.clamp(0.0, 1.0)).round() as u32;
    let oy = (extra_y as f32 * offset.1.clamp(0.0, 1.0)).round() as u32;

    image::imageops::crop_imm(&scaled, ox, oy, CARD_CANVAS_W, CARD_CANVAS_H)
        .to_image()
}

/// Build the full asset trio for a cropped canvas.
pub fn prepare(src: &DynamicImage, offset: (f32, f32), source_name: String) -> Result<PreparedSkin> {
    let canvas = crop_to_canvas(src, offset);

    let png_3x = encode_png(&canvas)?;
    let two_x = image::imageops::resize(
        &canvas,
        (CARD_CANVAS_W * 2 / 3).max(1),
        (CARD_CANVAS_H * 2 / 3).max(1),
        FilterType::Lanczos3,
    );
    let png_2x = encode_png(&two_x)?;
    let pdf = encode_pdf(&canvas)?;

    Ok(PreparedSkin {
        source_name,
        preview: canvas,
        png_3x,
        png_2x,
        pdf,
    })
}

fn encode_png(img: &RgbaImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
        .context("Could not encode prepared PNG")?;
    Ok(buf)
}

use image::ImageEncoder;

/// Hand-rolled single-page PDF that draws `img` full-bleed. The pixels are
/// embedded as a Flate-compressed DeviceRGB image XObject — no external PDF
/// dependency, and it renders identically to the PNG art.
fn encode_pdf(img: &RgbaImage) -> Result<Vec<u8>> {
    let w = img.width();
    let h = img.height();
    let rgb: RgbImage = DynamicImage::ImageRgba8(img.clone()).to_rgb8();

    let mut raw = Vec::with_capacity((w * h * 3) as usize);
    raw.extend_from_slice(rgb.as_raw());
    let compressed = flate_compress(&raw);

    // Page is the card's physical pass size in points (72 dpi logical).
    let page_w = 360.0_f32; // 1536 / 4.266…  — matches the on-device PDF box
    let page_h = page_w * (h as f32 / w as f32);

    let mut pdf = Vec::new();
    let mut offsets = Vec::new();
    macro_rules! obj {
        ($n:expr, $body:expr) => {{
            offsets.push(($n, pdf.len()));
            let _ = write!(pdf, "{} 0 obj\n{}\nendobj\n", $n, $body);
        }};
    }

    let _ = write!(pdf, "%PDF-1.5\n%\u{00e2}\u{00e3}\u{00cf}\u{00d3}\n");
    obj!(1, "<< /Type /Catalog /Pages 2 0 R >>");
    obj!(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    obj!(
        3,
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
             /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>",
            page_w, page_h
        )
    );
    // Image XObject (object 4) with binary stream.
    offsets.push((4, pdf.len()));
    let _ = write!(
        pdf,
        "4 0 obj\n<< /Type /XObject /Subtype /Image /Width {w} /Height {h} \
         /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
        compressed.len()
    );
    pdf.extend_from_slice(&compressed);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    // Content stream places the image across the full page.
    let content = format!("q\n{:.2} 0 0 {:.2} 0 0 cm\n/Im0 Do\nQ\n", page_w, page_h);
    obj!(
        5,
        format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content)
    );

    let xref_pos = pdf.len();
    let count = offsets.len() + 1;
    let _ = write!(pdf, "xref\n0 {count}\n0000000000 65535 f \n");
    offsets.sort_by_key(|(n, _)| *n);
    for (_, off) in &offsets {
        let _ = write!(pdf, "{:010} 00000 n \n", off);
    }
    let _ = write!(
        pdf,
        "trailer\n<< /Size {count} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF"
    );
    Ok(pdf)
}

fn flate_compress(data: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    let _ = enc.write_all(data);
    enc.finish().unwrap_or_default()
}
