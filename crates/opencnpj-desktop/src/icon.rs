//! Application icon, drawn procedurally (no binary assets): a blue rounded square with three
//! "record" bars and a magnifying glass. `assets/opencnpj-admin.svg` is the same design.

use eframe::egui::IconData;

/// Signed distance to a rounded rectangle centered at the origin.
fn sd_round_rect(x: f32, y: f32, half_w: f32, half_h: f32, r: f32) -> f32 {
    let qx = x.abs() - half_w + r;
    let qy = y.abs() - half_h + r;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

/// Signed distance to a segment with thickness `2 * half_t`.
fn sd_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, half_t: f32) -> f32 {
    let (pax, pay, bax, bay) = (px - ax, py - ay, bx - ax, by - ay);
    let h = ((pax * bax + pay * bay) / (bax * bax + bay * bay)).clamp(0.0, 1.0);
    ((pax - bax * h).powi(2) + (pay - bay * h).powi(2)).sqrt() - half_t
}

/// Coverage in `[0, 1]` for a signed distance in pixels (1-pixel anti-aliasing).
fn cover(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

fn over(dst: [f32; 4], rgb: [f32; 3], a: f32) -> [f32; 4] {
    let out_a = a + dst[3] * (1.0 - a);
    if out_a <= 0.0 {
        return [0.0; 4];
    }
    let mix = |i: usize| (rgb[i] * a + dst[i] * dst[3] * (1.0 - a)) / out_a;
    [mix(0), mix(1), mix(2), out_a]
}

/// Straight-alpha RGBA pixels of a `size`×`size` icon.
pub fn rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let u = s / 64.0; // design grid is 64×64
    let top = [0.27, 0.56, 0.98];
    let bottom = [0.11, 0.31, 0.85];
    let white = [1.0, 1.0, 1.0];
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for j in 0..size {
        for i in 0..size {
            let (x, y) = (i as f32 + 0.5, j as f32 + 0.5);
            let t = y / s;
            let bg = [top[0] + (bottom[0] - top[0]) * t, top[1] + (bottom[1] - top[1]) * t, top[2] + (bottom[2] - top[2]) * t];
            let mut px = [0.0f32; 4];
            px = over(px, bg, cover(sd_round_rect(x - 32.0 * u, y - 32.0 * u, 30.0 * u, 30.0 * u, 13.0 * u)));
            // Three record lines (left), shorter as they go down.
            for (k, len) in [(0.0, 26.0), (1.0, 20.0), (2.0, 14.0)] {
                let cy = (18.0 + 9.0 * k) * u;
                let d = sd_segment(x, y, 13.0 * u, cy, (13.0 + len) * u, cy, 2.6 * u);
                px = over(px, white, cover(d) * 0.85);
            }
            // Magnifier ring and handle.
            let (cx, cy, r) = (38.0 * u, 37.0 * u, 11.0 * u);
            let ring = (((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r).abs() - 3.2 * u;
            px = over(px, white, cover(ring));
            let handle = sd_segment(x, y, 46.5 * u, 45.5 * u, 53.0 * u, 52.0 * u, 3.6 * u);
            px = over(px, white, cover(handle));
            out.extend(px.iter().map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
    }
    out
}

/// Window icon for the viewport builder.
pub fn icon_data() -> IconData {
    let size = 128;
    IconData { rgba: rgba(size), width: size, height: size }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_expected_shape() {
        let size = 64u32;
        let px = rgba(size);
        assert_eq!(px.len(), (size * size * 4) as usize);
        let at = |x: u32, y: u32| &px[((y * size + x) * 4) as usize..((y * size + x) * 4 + 4) as usize];
        // Rounded corners are transparent, the body is opaque blue, the ring is white.
        assert_eq!(at(0, 0)[3], 0);
        assert_eq!(at(size - 1, size - 1)[3], 0);
        let body = at(8, 56);
        assert_eq!(body[3], 255);
        assert!(body[2] > body[0]);
        let ring = at(38, 26);
        assert!(ring[0] > 240 && ring[1] > 240 && ring[2] > 240, "{ring:?}");
        assert_eq!(icon_data().rgba.len(), 128 * 128 * 4);
    }
}
