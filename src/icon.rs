use eframe::egui;

const ORANGE: (f32, f32, f32) = (194.0, 65.0, 12.0);
const WHITE: (f32, f32, f32) = (255.0, 255.0, 255.0);

/// Rounded orange tile with three white status bars, shared by the window and the tray.
pub fn rgba(size: u32) -> Vec<u8> {
    let s = size as f32;
    let margin = s * 0.08;
    let radius = s * 0.24;
    let bars = [
        (0.24, 0.76, 0.29, 0.40),
        (0.24, 0.66, 0.44, 0.55),
        (0.24, 0.56, 0.59, 0.70),
    ];
    let mut pixels = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let tile = rounded_box_alpha(px, py, margin, margin, s - margin, s - margin, radius);
            if tile <= 0.0 {
                continue;
            }

            let mut color = ORANGE;
            for (left, right, top, bottom) in bars {
                let (x0, x1) = (left * s, right * s);
                let (y0, y1) = (top * s, bottom * s);
                let bar = rounded_box_alpha(px, py, x0, y0, x1, y1, (y1 - y0) * 0.5);
                if bar > 0.0 {
                    color.0 += (WHITE.0 - color.0) * bar;
                    color.1 += (WHITE.1 - color.1) * bar;
                    color.2 += (WHITE.2 - color.2) * bar;
                }
            }

            let index = ((y * size + x) * 4) as usize;
            pixels[index] = color.0.round() as u8;
            pixels[index + 1] = color.1.round() as u8;
            pixels[index + 2] = color.2.round() as u8;
            pixels[index + 3] = (tile * 255.0).round() as u8;
        }
    }
    pixels
}

pub fn window_icon() -> egui::IconData {
    let size = 64;
    egui::IconData {
        rgba: rgba(size),
        width: size,
        height: size,
    }
}

fn rounded_box_alpha(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, radius: f32) -> f32 {
    let cx = (x0 + x1) * 0.5;
    let cy = (y0 + y1) * 0.5;
    let hx = (x1 - x0) * 0.5 - radius;
    let hy = (y1 - y0) * 0.5 - radius;
    let qx = (px - cx).abs() - hx;
    let qy = (py - cy).abs() - hy;
    let distance = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius;
    (0.5 - distance).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_is_opaque_inside_and_transparent_at_the_corner() {
        let size = 64;
        let pixels = rgba(size);
        assert_eq!(pixels.len(), (size * size * 4) as usize);

        let center = ((32 * size + 32) * 4) as usize;
        assert_eq!(pixels[center + 3], 255);
        assert_eq!(&pixels[center..center + 3], &[255, 255, 255]);

        let corner = 0;
        assert_eq!(pixels[corner + 3], 0);
    }

    #[test]
    fn icon_tile_is_orange_near_the_edge() {
        let size = 64;
        let pixels = rgba(size);
        let index = ((32 * size + 8) * 4) as usize;
        assert_eq!(pixels[index + 3], 255);
        assert_eq!(&pixels[index..index + 3], &[194, 65, 12]);
    }
}
