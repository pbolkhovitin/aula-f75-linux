//! Off-screen rendering used by `--screenshot`.
//!
//! It runs the real egui UI for 120 frames without creating a window, then
//! rasterizes the last frame on the CPU. This keeps screenshots working on
//! machines without a compositor as well as on a normal desktop.

use eframe::egui;
use eframe::egui::epaint::textures::TexturesDelta;
use eframe::egui::epaint::{ClippedPrimitive, ColorImage, Primitive, TextureId};
use eframe::egui::{Color32, Pos2, Rect};
use std::collections::HashMap;
use std::path::Path;

pub fn render(app: &mut crate::F75App, path: &Path) {
    let ctx = egui::Context::default();
    crate::theme::setup(&ctx);
    ctx.set_pixels_per_point(1.0);

    let mut textures: HashMap<TextureId, ColorImage> = HashMap::new();
    let mut last = None;

    for frame in 0..120u64 {
        let time = frame as f64 / 60.0;
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(crate::model::WINDOW_W, crate::model::WINDOW_H),
        ));
        input.time = Some(time);
        input.predicted_dt = 1.0 / 60.0;
        input.max_texture_side = Some(8192);
        input.focused = true;

        let mut output = ctx.run_ui(input, |ui| app.draw_ui(ui, time));
        apply_textures(&mut textures, &output.textures_delta);
        output.textures_delta.clear();
        last = Some(output);
    }

    let Some(output) = last else {
        eprintln!("no frame was drawn");
        return;
    };

    let primitives = ctx.tessellate(output.shapes, 1.0);
    let width = crate::model::WINDOW_W as usize;
    let height = crate::model::WINDOW_H as usize;
    let pixels = rasterize(width, height, &primitives, &textures);

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(err) = image::save_buffer(
        path,
        &pixels,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    ) {
        eprintln!("could not write screenshot: {err}");
        return;
    }
    eprintln!("saved screenshot {}", path.display());
}

fn apply_textures(textures: &mut HashMap<TextureId, ColorImage>, delta: &TexturesDelta) {
    for (id, updates) in &delta.set {
        for update in updates {
            let egui::ImageData::Color(source) = &update.image;
            match update.pos {
                None => {
                    textures.insert(*id, (**source).clone());
                }
                Some([x0, y0]) => {
                    if let Some(dest) = textures.get_mut(id) {
                        for y in 0..source.size[1] {
                            for x in 0..source.size[0] {
                                let dx = x0 + x;
                                let dy = y0 + y;
                                if dx < dest.size[0] && dy < dest.size[1] {
                                    dest.pixels[dy * dest.size[0] + dx] =
                                        source.pixels[y * source.size[0] + x];
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for id in &delta.free {
        textures.remove(id);
    }
}

/// A tiny software triangle rasterizer for egui meshes: clipped, textured,
/// premultiplied-alpha blending. It is intentionally simple; the UI only needs
/// one still frame per screenshot.
fn rasterize(
    width: usize,
    height: usize,
    primitives: &[ClippedPrimitive],
    textures: &HashMap<TextureId, ColorImage>,
) -> Vec<u8> {
    let bg = crate::theme::BG;
    let mut buffer = vec![0u8; width * height * 4];
    for px in buffer.chunks_exact_mut(4) {
        px.copy_from_slice(&bg.to_array());
    }

    let white = ColorImage::filled([1, 1], Color32::WHITE);

    for primitive in primitives {
        let Primitive::Mesh(mesh) = &primitive.primitive else {
            continue;
        };
        let texture = textures.get(&mesh.texture_id).unwrap_or(&white);
        let clip = primitive.clip_rect;

        for tri in mesh.indices.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let (Some(v0), Some(v1), Some(v2)) = (
                mesh.vertices.get(i0),
                mesh.vertices.get(i1),
                mesh.vertices.get(i2),
            ) else {
                continue;
            };

            let area = edge(v0.pos, v1.pos, v2.pos);
            if area.abs() < 1e-6 {
                continue;
            }

            let min_x = v0.pos.x.min(v1.pos.x).min(v2.pos.x).floor().max(0.0) as usize;
            let min_y = v0.pos.y.min(v1.pos.y).min(v2.pos.y).floor().max(0.0) as usize;
            let max_x = (v0.pos.x.max(v1.pos.x).max(v2.pos.x).ceil() as isize)
                .min(width as isize - 1)
                .max(0) as usize;
            let max_y = (v0.pos.y.max(v1.pos.y).max(v2.pos.y).ceil() as isize)
                .min(height as isize - 1)
                .max(0) as usize;

            let cx0 = clip.min.x.floor().max(0.0) as usize;
            let cy0 = clip.min.y.floor().max(0.0) as usize;
            let cx1 = (clip.max.x.ceil() as isize).min(width as isize).max(0) as usize;
            let cy1 = (clip.max.y.ceil() as isize).min(height as isize).max(0) as usize;

            let x_start = min_x.max(cx0);
            let y_start = min_y.max(cy0);
            let x_end = max_x.min(cx1.saturating_sub(1));
            let y_end = max_y.min(cy1.saturating_sub(1));
            if x_start > x_end || y_start > y_end {
                continue;
            }

            let c0 = v0.color.to_array();
            let c1 = v1.color.to_array();
            let c2 = v2.color.to_array();

            for y in y_start..=y_end {
                for x in x_start..=x_end {
                    let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = edge(v1.pos, v2.pos, p) / area;
                    let w1 = edge(v2.pos, v0.pos, p) / area;
                    let w2 = edge(v0.pos, v1.pos, p) / area;
                    if w0 < -0.0001 || w1 < -0.0001 || w2 < -0.0001 {
                        continue;
                    }

                    let u = w0 * v0.uv.x + w1 * v1.uv.x + w2 * v2.uv.x;
                    let v = w0 * v0.uv.y + w1 * v1.uv.y + w2 * v2.uv.y;
                    let texel = sample(texture, u, v);

                    let r = (w0 * c0[0] as f32 + w1 * c1[0] as f32 + w2 * c2[0] as f32) / 255.0;
                    let g = (w0 * c0[1] as f32 + w1 * c1[1] as f32 + w2 * c2[1] as f32) / 255.0;
                    let b = (w0 * c0[2] as f32 + w1 * c1[2] as f32 + w2 * c2[2] as f32) / 255.0;
                    let a = (w0 * c0[3] as f32 + w1 * c1[3] as f32 + w2 * c2[3] as f32) / 255.0;

                    let sr = r * texel[0];
                    let sg = g * texel[1];
                    let sb = b * texel[2];
                    let sa = a * texel[3];

                    let idx = (y * width + x) * 4;
                    let inv = 1.0 - sa;
                    buffer[idx] =
                        ((sr + buffer[idx] as f32 / 255.0 * inv) * 255.0).clamp(0.0, 255.0) as u8;
                    buffer[idx + 1] = ((sg + buffer[idx + 1] as f32 / 255.0 * inv) * 255.0)
                        .clamp(0.0, 255.0) as u8;
                    buffer[idx + 2] = ((sb + buffer[idx + 2] as f32 / 255.0 * inv) * 255.0)
                        .clamp(0.0, 255.0) as u8;
                    buffer[idx + 3] = 255;
                }
            }
        }
    }
    buffer
}

fn edge(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x)
}

/// Bilinear sample, returning premultiplied RGBA in 0..1.
fn sample(image: &ColorImage, u: f32, v: f32) -> [f32; 4] {
    let w = image.size[0] as f32;
    let h = image.size[1] as f32;
    if w <= 0.0 || h <= 0.0 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let x = (u * w - 0.5).clamp(0.0, w - 1.0);
    let y = (v * h - 0.5).clamp(0.0, h - 1.0);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(image.size[0] - 1);
    let y1 = (y0 + 1).min(image.size[1] - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let p00 = image.pixels[y0 * image.size[0] + x0].to_array();
    let p10 = image.pixels[y0 * image.size[0] + x1].to_array();
    let p01 = image.pixels[y1 * image.size[0] + x0].to_array();
    let p11 = image.pixels[y1 * image.size[0] + x1].to_array();

    let mut out = [0.0f32; 4];
    for c in 0..4 {
        let top = p00[c] as f32 * (1.0 - fx) + p10[c] as f32 * fx;
        let bottom = p01[c] as f32 * (1.0 - fx) + p11[c] as f32 * fx;
        out[c] = (top * (1.0 - fy) + bottom * fy) / 255.0;
    }
    out
}
