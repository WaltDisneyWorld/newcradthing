//! Generates the bundled **original** sample artwork and `catalog.json`.
//!
//! Everything here is procedurally drawn from scratch — gradients, soft glows,
//! geometric accents. No brands, logos, characters, or real card designs. Run:
//!
//!     cargo run --release --example gen_art
//!
//! It writes PNGs under `assets/artwork/<pack>/` and an index `catalog.json`.

use std::path::Path;

use image::{Rgb, RgbImage};
use serde::Serialize;

const W: u32 = 1536;
const H: u32 = 969;

#[derive(Serialize)]
struct Art {
    id: String,
    name: String,
    file: String,
}

#[derive(Serialize)]
struct Pack {
    id: String,
    name: String,
    name_it: String,
    accent: String,
    arts: Vec<Art>,
}

#[derive(Serialize)]
struct Catalog {
    schema: u32,
    generated: String,
    packs: Vec<Pack>,
}

type Rgbf = (f32, f32, f32);

fn lerp(a: Rgbf, b: Rgbf, t: f32) -> Rgbf {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

fn put(img: &mut RgbImage, x: u32, y: u32, c: Rgbf) {
    let px = Rgb([
        (c.0.clamp(0.0, 1.0) * 255.0) as u8,
        (c.1.clamp(0.0, 1.0) * 255.0) as u8,
        (c.2.clamp(0.0, 1.0) * 255.0) as u8,
    ]);
    img.put_pixel(x, y, px);
}

/// Cheap value noise (deterministic from a seed).
fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(362437));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) as f32 / u32::MAX as f32).fract()
}

/// Diagonal gradient with a soft off-center radial highlight and fine grain.
fn gradient_card(a: Rgbf, b: Rgbf, glow: Rgbf, seed: u32, stripes: bool) -> RgbImage {
    let mut img = RgbImage::new(W, H);
    let gx = W as f32 * 0.72;
    let gy = H as f32 * 0.22;
    let gr = (W as f32) * 0.65;
    for y in 0..H {
        for x in 0..W {
            let t = (x as f32 / W as f32 * 0.6) + (y as f32 / H as f32 * 0.4);
            let mut c = lerp(a, b, t.clamp(0.0, 1.0));
            // radial glow
            let d = (((x as f32 - gx).powi(2) + (y as f32 - gy).powi(2)).sqrt() / gr).min(1.0);
            let gi = (1.0 - d).powf(2.2) * 0.55;
            c = lerp(c, glow, gi);
            // subtle diagonal sheen lines
            if stripes {
                let s = ((x as f32 + y as f32) * 0.015).sin();
                c = lerp(c, (c.0 + 0.06, c.1 + 0.06, c.2 + 0.06), (s * 0.5 + 0.5) * 0.08);
            }
            // film grain
            let n = (hash2(x as i32, y as i32, seed) - 0.5) * 0.03;
            c = (c.0 + n, c.1 + n, c.2 + n);
            put(&mut img, x, y, c);
        }
    }
    // gentle vignette
    vignette(&mut img, 0.18);
    img
}

/// Concentric soft arcs over a deep base — an "aurora" feel, fully abstract.
fn aurora_card(base: Rgbf, band: Rgbf, seed: u32) -> RgbImage {
    let mut img = RgbImage::new(W, H);
    let cy = H as f32 * 1.35;
    let cx = W as f32 * 0.5;
    for y in 0..H {
        for x in 0..W {
            let mut c = lerp(base, (base.0 * 0.4, base.1 * 0.4, base.2 * 0.5), y as f32 / H as f32);
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            let wave = (d * 0.012 + seed as f32).sin() * 0.5 + 0.5;
            c = lerp(c, band, wave.powf(3.0) * 0.5);
            let n = (hash2(x as i32, y as i32, seed) - 0.5) * 0.025;
            c = (c.0 + n, c.1 + n, c.2 + n);
            put(&mut img, x, y, c);
        }
    }
    vignette(&mut img, 0.22);
    img
}

/// Carbon-weave style diagonal mesh in near-monochrome.
fn carbon_card(tint: Rgbf, seed: u32) -> RgbImage {
    let mut img = RgbImage::new(W, H);
    for y in 0..H {
        for x in 0..W {
            let a = (((x + y) / 14) % 2) as f32;
            let b = (((x as i32 - y as i32).rem_euclid(28)) / 14) as f32;
            let base = 0.08 + 0.05 * a + 0.04 * b;
            let mut c = (base + tint.0 * 0.15, base + tint.1 * 0.15, base + tint.2 * 0.15);
            // top sheen
            let sheen = (1.0 - y as f32 / H as f32).powf(2.0) * 0.12;
            c = (c.0 + sheen, c.1 + sheen, c.2 + sheen);
            let n = (hash2(x as i32, y as i32, seed) - 0.5) * 0.02;
            c = (c.0 + n, c.1 + n, c.2 + n);
            put(&mut img, x, y, c);
        }
    }
    vignette(&mut img, 0.25);
    img
}

/// Soft marbled blobs via layered noise.
fn marble_card(a: Rgbf, b: Rgbf, veins: Rgbf, seed: u32) -> RgbImage {
    let mut img = RgbImage::new(W, H);
    for y in 0..H {
        for x in 0..W {
            let fx = x as f32 / W as f32;
            let fy = y as f32 / H as f32;
            let n = (((fx * 6.0 + seed as f32).sin() + (fy * 5.0).cos()) * 0.5
                + (fx * 13.0).sin() * 0.25
                + (fy * 11.0).cos() * 0.25)
                * 0.5
                + 0.5;
            let mut c = lerp(a, b, n.clamp(0.0, 1.0));
            let vein = ((n * 18.0).sin()).abs();
            c = lerp(c, veins, (1.0 - vein).powf(8.0) * 0.4);
            put(&mut img, x, y, c);
        }
    }
    vignette(&mut img, 0.2);
    img
}

fn vignette(img: &mut RgbImage, amount: f32) {
    let cx = W as f32 / 2.0;
    let cy = H as f32 / 2.0;
    let max = (cx * cx + cy * cy).sqrt();
    for y in 0..H {
        for x in 0..W {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / max;
            let f = 1.0 - (d.powf(2.5) * amount);
            let p = img.get_pixel(x, y).0;
            img.put_pixel(
                x,
                y,
                Rgb([
                    (p[0] as f32 * f) as u8,
                    (p[1] as f32 * f) as u8,
                    (p[2] as f32 * f) as u8,
                ]),
            );
        }
    }
}

fn hexf(c: Rgbf) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (c.0 * 255.0) as u8,
        (c.1 * 255.0) as u8,
        (c.2 * 255.0) as u8
    )
}

fn main() {
    let out = Path::new("assets/artwork");
    std::fs::create_dir_all(out).unwrap();

    let mut packs: Vec<Pack> = Vec::new();

    // Each pack: (id, name_en, name_it, accent, [(art name, closure)]).
    // Pack 1 — Aurora (abstract light bands)
    packs.push(build_pack(
        out,
        "aurora",
        "Aurora",
        "Aurora",
        (0.36, 0.78, 0.86),
        vec![
            ("Aurora Dawn", aurora_card((0.05, 0.10, 0.22), (0.36, 0.85, 0.80), 1)),
            ("Aurora Dusk", aurora_card((0.12, 0.05, 0.20), (0.85, 0.42, 0.72), 7)),
            ("Aurora Deep", aurora_card((0.03, 0.14, 0.18), (0.30, 0.80, 0.55), 15)),
            ("Aurora Ember", aurora_card((0.16, 0.07, 0.05), (0.95, 0.60, 0.30), 23)),
        ],
    ));

    // Pack 2 — Carbon (near-monochrome weave)
    packs.push(build_pack(
        out,
        "carbon",
        "Carbon",
        "Carbonio",
        (0.55, 0.60, 0.68),
        vec![
            ("Carbon Noir", carbon_card((0.2, 0.2, 0.25), 2)),
            ("Carbon Steel", carbon_card((0.3, 0.4, 0.55), 9)),
            ("Carbon Bronze", carbon_card((0.6, 0.45, 0.25), 18)),
            ("Carbon Jade", carbon_card((0.2, 0.5, 0.42), 31)),
        ],
    ));

    // Pack 3 — Citrus (warm gradients)
    packs.push(build_pack(
        out,
        "citrus",
        "Citrus",
        "Agrumi",
        (0.98, 0.70, 0.25),
        vec![
            ("Citrus Sun", gradient_card((0.98, 0.72, 0.18), (0.92, 0.33, 0.28), (1.0, 0.95, 0.7), 3, true)),
            ("Citrus Lime", gradient_card((0.78, 0.92, 0.30), (0.12, 0.55, 0.40), (0.95, 1.0, 0.8), 11, true)),
            ("Citrus Coral", gradient_card((1.0, 0.55, 0.45), (0.85, 0.25, 0.55), (1.0, 0.9, 0.85), 19, true)),
            ("Citrus Amber", gradient_card((0.95, 0.65, 0.20), (0.45, 0.20, 0.10), (1.0, 0.85, 0.6), 27, true)),
        ],
    ));

    // Pack 4 — Marble (soft stone)
    packs.push(build_pack(
        out,
        "marble",
        "Marble",
        "Marmo",
        (0.80, 0.82, 0.88),
        vec![
            ("Marble Ivory", marble_card((0.92, 0.90, 0.86), (0.72, 0.74, 0.80), (0.55, 0.58, 0.65), 4)),
            ("Marble Onyx", marble_card((0.14, 0.15, 0.18), (0.05, 0.05, 0.07), (0.35, 0.38, 0.45), 12)),
            ("Marble Rose", marble_card((0.95, 0.85, 0.85), (0.80, 0.60, 0.65), (0.60, 0.40, 0.45), 20)),
            ("Marble Azure", marble_card((0.82, 0.90, 0.95), (0.45, 0.65, 0.82), (0.25, 0.40, 0.60), 28)),
        ],
    ));

    // Pack 5 — Nocturne (deep gradients with glow)
    packs.push(build_pack(
        out,
        "nocturne",
        "Nocturne",
        "Notturno",
        (0.52, 0.45, 0.92),
        vec![
            ("Nocturne Violet", gradient_card((0.10, 0.06, 0.24), (0.42, 0.20, 0.70), (0.65, 0.55, 1.0), 5, false)),
            ("Nocturne Teal", gradient_card((0.03, 0.14, 0.18), (0.06, 0.40, 0.48), (0.5, 0.95, 1.0), 13, false)),
            ("Nocturne Crimson", gradient_card((0.16, 0.03, 0.08), (0.55, 0.10, 0.22), (1.0, 0.5, 0.55), 21, false)),
            ("Nocturne Slate", gradient_card((0.08, 0.10, 0.14), (0.24, 0.30, 0.40), (0.7, 0.8, 0.95), 29, false)),
        ],
    ));

    let catalog = Catalog {
        schema: 1,
        generated: "generated by examples/gen_art.rs — original abstract artwork".into(),
        packs,
    };
    let json = serde_json::to_vec_pretty(&catalog).unwrap();
    std::fs::write(out.join("catalog.json"), json).unwrap();
    println!("Wrote {} and PNGs under {}", out.join("catalog.json").display(), out.display());
}

fn build_pack(
    out: &Path,
    id: &str,
    name: &str,
    name_it: &str,
    accent: Rgbf,
    arts: Vec<(&str, RgbImage)>,
) -> Pack {
    let dir = out.join(id);
    std::fs::create_dir_all(&dir).unwrap();
    let mut entries = Vec::new();
    for (i, (art_name, img)) in arts.into_iter().enumerate() {
        let file_rel = format!("{id}/{}.jpg", i + 1);
        img.save(out.join(&file_rel)).unwrap();
        entries.push(Art {
            id: format!("{id}-{}", i + 1),
            name: art_name.to_string(),
            file: file_rel,
        });
    }
    Pack {
        id: id.to_string(),
        name: name.to_string(),
        name_it: name_it.to_string(),
        accent: hexf(accent),
        arts: entries,
    }
}
