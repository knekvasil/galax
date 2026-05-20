use galax_io::read_snapshot;
use minifb::{Key, Window, WindowOptions};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let snap_path = args.get(1).expect("Usage: galax-viz <snapshot.bin>");
    let path = PathBuf::from(snap_path);

    let mut file = std::fs::File::open(&path).expect("failed to open snapshot");
    let bodies = read_snapshot(&mut file).expect("failed to read snapshot");

    let n = bodies.len();
    let width = 800usize;
    let height = 600usize;
    let mut buffer = vec![0u32; width * height];

    // Map simulation bounds to pixel coordinates
    let mut x_min = f64::MAX;
    let mut x_max = f64::MIN;
    let mut y_min = f64::MAX;
    let mut y_max = f64::MIN;
    for i in 0..n {
        x_min = x_min.min(bodies.x[i]);
        x_max = x_max.max(bodies.x[i]);
        y_min = y_min.min(bodies.y[i]);
        y_max = y_max.max(bodies.y[i]);
    }
    let x_range = (x_max - x_min).max(1.0);
    let y_range = (y_max - y_min).max(1.0);

    let mut window = Window::new(
        &format!("galax-viz — {} bodies", n),
        width,
        height,
        WindowOptions::default(),
    )
    .expect("failed to create window");

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Clear
        for pixel in buffer.iter_mut() {
            *pixel = 0x00_00_00; // black
        }

        // Draw bodies
        let max_vel: f64 = bodies
            .vx
            .iter()
            .zip(bodies.vy.iter())
            .map(|(vx, vy)| (vx * vx + vy * vy).sqrt())
            .fold(0.0_f64, f64::max)
            .max(1.0);

        // For large N, decimate
        let step = (n / 10000).max(1);
        for i in (0..n).step_by(step) {
            let px = ((bodies.x[i] - x_min) / x_range * (width - 1) as f64) as usize;
            let py = ((bodies.y[i] - y_min) / y_range * (height - 1) as f64) as usize;
            if px < width && py < height {
                let speed = (bodies.vx[i] * bodies.vx[i] + bodies.vy[i] * bodies.vy[i])
                    .sqrt()
                    / max_vel;
                // Color: blue (slow) → red (fast)
                let r = (speed * 255.0) as u32;
                let b = ((1.0 - speed) * 255.0) as u32;
                let color = (r << 16) | (b);
                buffer[py * width + px] = color;
            }
        }

        window
            .update_with_buffer(&buffer, width, height)
            .expect("failed to update window");
    }
}
