use galax_core::{BodiesSoA, BodyTracker};
use wasm_bindgen::{Clamped, JsValue};
use web_sys::{CanvasRenderingContext2d, ImageData};

pub struct Camera {
    pub offset_x: f64,
    pub offset_y: f64,
    pub zoom: f64,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            zoom: 1.0,
        }
    }

    pub fn zoom_at(&mut self, delta: f64, mx: f64, my: f64, w: f64, h: f64) {
        let old = self.zoom;
        let new = (self.zoom * delta).clamp(0.1, 100.0);
        let rx = (mx / w) - 0.5 - self.offset_x;
        let ry = (my / h) - 0.5 - self.offset_y;
        self.offset_x -= rx * (1.0 - old / new);
        self.offset_y -= ry * (1.0 - old / new);
        self.zoom = new;
    }

    pub fn pan(&mut self, dx: f64, dy: f64, w: f64, h: f64) {
        self.offset_x += dx / w;
        self.offset_y += dy / h;
    }
}

pub struct WorldBounds {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl WorldBounds {
    pub fn new() -> Self {
        Self {
            x_min: -50.0,
            x_max: 50.0,
            y_min: -50.0,
            y_max: 50.0,
        }
    }
}

fn world_to_screen(
    x: f64,
    y: f64,
    cam: &Camera,
    bounds: &WorldBounds,
    width: f64,
    height: f64,
) -> Option<(f64, f64)> {
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    let xr = (bounds.x_max - bounds.x_min).max(1.0);
    let yr = (bounds.y_max - bounds.y_min).max(1.0);
    let nx = ((x - bounds.x_min) / xr) - 0.5;
    let ny = ((y - bounds.y_min) / yr) - 0.5;
    let sx = (nx * cam.zoom + 0.5 + cam.offset_x) * width;
    let sy = (ny * cam.zoom + 0.5 + cam.offset_y) * height;
    if sx >= 0.0 && sx < width && sy >= 0.0 && sy < height {
        Some((sx, sy))
    } else {
        None
    }
}

pub fn render_frame(
    ctx: &CanvasRenderingContext2d,
    width: u32,
    height: u32,
    bodies: &BodiesSoA,
    cam: &Camera,
    bounds: &WorldBounds,
    step: u64,
    elapsed_ms: f64,
    steps_per_sec: f64,
    tracker: Option<&BodyTracker>,
) -> Result<(), JsValue> {
    let pixel_count = (width * height * 4) as usize;
    let mut buf = vec![0u8; pixel_count];

    let n = bodies.len();
    if n > 0 {
        let w = width as f64;
        let h = height as f64;

        // Draw trail first
        if let Some(tr) = tracker {
            if tr.tracked_index.is_some() {
                draw_trail_rgba(&mut buf, width, height, tr, cam, bounds, 0, 136, 204);
            }
        }

        let tracked_idx = tracker.and_then(|t| t.tracked_index);

        for i in 0..n {
            if tracked_idx == Some(i) { continue; }
            if let Some((sx, sy)) = world_to_screen(bodies.x[i], bodies.y[i], cam, bounds, w, h) {
                let idx = (sy as u32 * width + sx as u32) as usize * 4;
                if idx + 2 < pixel_count {
                    buf[idx] = 170;
                    buf[idx + 1] = 170;
                    buf[idx + 2] = 170;
                    buf[idx + 3] = 255;
                }
            }
        }

        // Draw tracked body highlighted
        if let Some(idx) = tracked_idx {
            if idx < n {
                if let Some((sx, sy)) = world_to_screen(bodies.x[idx], bodies.y[idx], cam, bounds, w, h) {
                    let cx = sx as u32;
                    let cy = sy as u32;
                    let set_pixel = |buf: &mut [u8], px: u32, py: u32| {
                        let idx = (py * width + px) as usize * 4;
                        if idx + 2 < pixel_count {
                            buf[idx] = 0;
                            buf[idx + 1] = 255;
                            buf[idx + 2] = 255;
                            buf[idx + 3] = 255;
                        }
                    };
                    set_pixel(&mut buf, cx, cy);
                    if cx > 0 { set_pixel(&mut buf, cx - 1, cy); }
                    if cx + 1 < width { set_pixel(&mut buf, cx + 1, cy); }
                    if cy > 0 { set_pixel(&mut buf, cx, cy - 1); }
                    if cy + 1 < height { set_pixel(&mut buf, cx, cy + 1); }
                }
            }
        }
    }

    let hud = format!(
        "step {}  N={}  {:.1}s  z={:.1}x  {:.0}st/s",
        step, n, elapsed_ms / 1000.0, cam.zoom, steps_per_sec,
    );
    for (ci, _byte) in hud.bytes().enumerate() {
        if ci >= width as usize {
            break;
        }
        let idx = ci * 4;
        buf[idx] = 85;
        buf[idx + 1] = 170;
        buf[idx + 2] = 85;
        buf[idx + 3] = 255;
    }

    let image_data = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&buf), width, height)?;
    ctx.put_image_data(&image_data, 0.0, 0.0)?;
    Ok(())
}

fn draw_line_rgba(buf: &mut [u8], width: u32, height: u32, x0: f64, y0: f64, x1: f64, y1: f64, r: u8, g: u8, b: u8) {
    let mut x = x0.round() as i32;
    let mut y = y0.round() as i32;
    let xe = x1.round() as i32;
    let ye = y1.round() as i32;
    let dx = (xe - x).abs();
    let dy = -(ye - y).abs();
    let sx = if x < xe { 1 } else { -1 };
    let sy = if y < ye { 1 } else { -1 };
    let mut err = dx + dy;
    let w = width as i32;
    let h = height as i32;
    loop {
        if x >= 0 && x < w && y >= 0 && y < h {
            let idx = (y as u32 * width + x as u32) as usize * 4;
            buf[idx] = r;
            buf[idx + 1] = g;
            buf[idx + 2] = b;
            buf[idx + 3] = 255;
        }
        if x == xe && y == ye { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

fn draw_trail_rgba(buf: &mut [u8], width: u32, height: u32, tracker: &BodyTracker, cam: &Camera, bounds: &WorldBounds, r: u8, g: u8, b: u8) {
    let w = width as f64;
    let h = height as f64;
    let pts: Vec<(f64, f64)> = tracker.history().iter().filter_map(|&(wx, wy)| {
        world_to_screen(wx, wy, cam, bounds, w, h)
    }).collect();
    for pair in pts.windows(2) {
        draw_line_rgba(buf, width, height, pair[0].0, pair[0].1, pair[1].0, pair[1].1, r, g, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use galax_core::BodiesSoA;

    fn default_cam() -> Camera {
        Camera { offset_x: 0.0, offset_y: 0.0, zoom: 1.0 }
    }

    fn default_bounds() -> WorldBounds {
        WorldBounds { x_min: -50.0, x_max: 50.0, y_min: -50.0, y_max: 50.0 }
    }

    #[test]
    fn test_world_to_screen_center() {
        let cam = default_cam();
        let bounds = default_bounds();
        let result = world_to_screen(0.0, 0.0, &cam, &bounds, 1280.0, 720.0);
        assert!(result.is_some(), "center body should be on screen");
        let (sx, sy) = result.unwrap();
        assert!((sx - 640.0).abs() < 1.0, "sx={} expected ~640", sx);
        assert!((sy - 360.0).abs() < 1.0, "sy={} expected ~360", sy);
    }

    #[test]
    fn test_world_to_screen_zoomed() {
        let mut cam = default_cam();
        cam.zoom = 2.0;
        let bounds = default_bounds();
        let result = world_to_screen(10.0, 0.0, &cam, &bounds, 1280.0, 720.0);
        assert!(result.is_some());
        let (sx, sy) = result.unwrap();
        assert!(sx > 640.0, "zoomed body should shift right, got sx={}", sx);
    }

    #[test]
    fn test_world_to_screen_off_screen() {
        let cam = default_cam();
        let bounds = default_bounds();
        let result = world_to_screen(1000.0, 0.0, &cam, &bounds, 1280.0, 720.0);
        assert!(result.is_none(), "far body should be off-screen");
    }

    #[test]
    fn test_world_to_screen_non_finite() {
        let cam = default_cam();
        let bounds = default_bounds();
        assert!(world_to_screen(f64::NAN, 0.0, &cam, &bounds, 1280.0, 720.0).is_none());
        assert!(world_to_screen(f64::INFINITY, 0.0, &cam, &bounds, 1280.0, 720.0).is_none());
    }

    #[test]
    fn test_camera_pan() {
        let mut cam = default_cam();
        cam.pan(100.0, 50.0, 1280.0, 720.0);
        assert!((cam.offset_x - (100.0 / 1280.0)).abs() < 1e-10);
        assert!((cam.offset_y - (50.0 / 720.0)).abs() < 1e-10);
    }

    #[test]
    fn test_camera_zoom_clamp() {
        let mut cam = default_cam();
        cam.zoom_at(0.001, 640.0, 360.0, 1280.0, 720.0);
        assert!((cam.zoom - 0.1).abs() < 1e-10, "zoom should clamp to 0.1, got {}", cam.zoom);
        cam.zoom = 1.0;
        cam.zoom_at(1000.0, 640.0, 360.0, 1280.0, 720.0);
        assert!((cam.zoom - 100.0).abs() < 1e-10, "zoom should clamp to 100, got {}", cam.zoom);
    }
}
