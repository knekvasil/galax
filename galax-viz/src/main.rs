use clap::Parser;
use galax_core::{
    build_p2p_lists, compute_fmm_force, compute_n2_force, build_constrained,
    InteractionLists, BodiesSoA, Tree,
};
use galax_gpu::{GpuConfig, GpuContext};
use galax_init::{disk, plummer, uniform, galaxy};
use galax_integrate::leapfrog_kdk;
use galax_integrate::leapfrog_kdk_gpu;
use galax_io::read_snapshot;
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "galax-viz")]
struct Args {
    #[arg(long)]
    snap: Option<PathBuf>,

    #[arg(long, default_value = "500")]
    n: usize,

    #[arg(long, default_value = "0")]
    max_render: usize,

    #[arg(long)]
    dt: Option<f64>,

    #[arg(long, default_value = "32")]
    n_max: usize,

    #[arg(long, default_value = "4")]
    p: usize,

    #[arg(long, default_value = "uniform")]
    init: String,

    #[arg(long)]
    softening: Option<f64>,

    /// Force FMM (default: auto-switch at N=5000)
    #[arg(long)]
    fmm: bool,

    /// Force N² even for large N
    #[arg(long)]
    n2: bool,

    #[arg(long, default_value = "30")]
    target_fps: f64,

    /// Target simulation steps per second (lower = slower/smoother animation)
    #[arg(long, default_value = "100")]
    sim_speed: f64,

    /// Maximum physics steps per frame (hard cap to ensure smooth animation)
    #[arg(long, default_value = "5")]
    max_steps_per_frame: usize,

    #[arg(long, default_value = "1280")]
    width: usize,

    #[arg(long, default_value = "720")]
    height: usize,

    /// Use GPU acceleration for FMM
    #[arg(long)]
    gpu: bool,
}

struct Camera {
    offset_x: f64, offset_y: f64, zoom: f64,
    dragging: bool, drag_sx: f64, drag_sy: f64,
}

impl Camera { fn new() -> Self { Self { offset_x: 0.0, offset_y: 0.0, zoom: 1.0, dragging: false, drag_sx: 0.0, drag_sy: 0.0 } } }

fn handle_camera(window: &Window, cam: &mut Camera, w: usize, h: usize) {
    if let Some((_, ys)) = window.get_scroll_wheel() {
        let old = cam.zoom;
        cam.zoom = (cam.zoom * (1.0 + ys as f64 * 0.1)).clamp(0.1, 100.0);
        if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Pass) {
            let rx = (mx as f64 / w as f64) - 0.5 - cam.offset_x;
            let ry = (my as f64 / h as f64) - 0.5 - cam.offset_y;
            cam.offset_x -= rx * (1.0 - old / cam.zoom);
            cam.offset_y -= ry * (1.0 - old / cam.zoom);
        }
    }
    if window.get_mouse_down(MouseButton::Left) {
        if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Pass) {
            if !cam.dragging { cam.dragging = true; cam.drag_sx = mx as f64; cam.drag_sy = my as f64; }
            else {
                cam.offset_x += (mx as f64 - cam.drag_sx) / w as f64;
                cam.offset_y += (my as f64 - cam.drag_sy) / h as f64;
                cam.drag_sx = mx as f64; cam.drag_sy = my as f64;
            }
        }
    } else { cam.dragging = false; }
}

fn render_frame(buf: &mut [u32], w: usize, h: usize, bodies: &BodiesSoA, step: u64, elapsed: Duration, sps: f64, max_render: usize, cam: &Camera, bounds: &WorldBounds, mode_label: &str) {
    for p in buf.iter_mut() { *p = 0; }
    let n = bodies.len();
    if n == 0 { return; }

    let x_min = bounds.x_min; let x_max = bounds.x_max;
    let y_min = bounds.y_min; let y_max = bounds.y_max;
    if x_min >= x_max || y_min >= y_max { return; }
    let xr = (x_max - x_min).max(1.0); let yr = (y_max - y_min).max(1.0);
    let ds = if max_render > 0 && max_render < n { (n / max_render).max(1) } else { 1 };

    for i in (0..n).step_by(ds) {
        let (x, y) = (bodies.x[i], bodies.y[i]);
        if !x.is_finite() || !y.is_finite() { continue; }
        let nx = ((x - x_min) / xr) - 0.5;
        let ny = ((y - y_min) / yr) - 0.5;
        let sx = (nx * cam.zoom + 0.5 + cam.offset_x) * w as f64;
        let sy = (ny * cam.zoom + 0.5 + cam.offset_y) * h as f64;
        if sx >= 0.0 && sx < w as f64 && sy >= 0.0 && sy < h as f64 {
            buf[sy as usize * w + sx as usize] = 0xAA_AA_AA;
        }
    }

    let hud = format!("step {}  N={}  {:.0}s  z={:.1}x  {:.0}st/s  {}x{}  {}", step, n, elapsed.as_secs_f64(), cam.zoom, sps, w, h, mode_label);
    for ci in 0..hud.len().min(w) { buf[ci] = 0x55_AA_55; }
}

/// Returns true if N² should be used for the given N.
/// Auto-switches at N=5000. Can be overridden with --fmm or --n2 flags.
fn use_n2(n: usize, force_fmm: bool, force_n2: bool) -> bool {
    if force_n2 { return true; }
    if force_fmm { return false; }
    // N² is exact and fast enough for N < 20000. FMM accuracy (~0.26% RMS at p=4)
    // degrades at small N where force gradients are steep (e.g. galaxy core).
    n < 20000
}

struct WorldBounds { x_min: f64, x_max: f64, y_min: f64, y_max: f64, frame: u64 }

impl WorldBounds {
    fn new() -> Self { Self { x_min: f64::MAX, x_max: f64::MIN, y_min: f64::MAX, y_max: f64::MIN, frame: 0 } }
    fn get(&mut self, bodies: &BodiesSoA, frame: u64) -> (f64, f64, f64, f64) {
        // Recompute every 10 frames (bounds change slowly)
        if frame - self.frame > 10 || self.frame == 0 {
            let n = bodies.len(); let ss = (n / 5000).max(1);
            let mut x_min = f64::MAX; let mut x_max = f64::MIN;
            let mut y_min = f64::MAX; let mut y_max = f64::MIN;
            for i in (0..n).step_by(ss) {
                let (x, y) = (bodies.x[i], bodies.y[i]);
                if x.is_finite() && y.is_finite() { x_min = x_min.min(x); x_max = x_max.max(x); y_min = y_min.min(y); y_max = y_max.max(y); }
            }
            self.x_min = x_min; self.x_max = x_max; self.y_min = y_min; self.y_max = y_max;
            self.frame = frame;
        }
        (self.x_min, self.x_max, self.y_min, self.y_max)
    }
}

fn physics_step(bodies: &mut BodiesSoA, tree: &Tree, m2l: &InteractionLists, p2p: &InteractionLists, p: usize, softening: f64, dt: f64, no_fmm: bool) {
    if no_fmm {
        let h = 0.5 * dt;
        for i in 0..bodies.len() { bodies.vx[i] += bodies.ax[i] * h; bodies.vy[i] += bodies.ay[i] * h; }
        for i in 0..bodies.len() { bodies.x[i] += bodies.vx[i] * dt; bodies.y[i] += bodies.vy[i] * dt; }
        compute_n2_force(bodies, softening);
        for i in 0..bodies.len() { bodies.vx[i] += bodies.ax[i] * h; bodies.vy[i] += bodies.ay[i] * h; }
    } else {
        leapfrog_kdk(bodies, tree, m2l, p2p, p, softening, dt);
    }
}

fn main() {
    let args = Args::parse();
    let frame_budget = Duration::from_secs_f64(1.0 / args.target_fps);

    // === Snapshot replay ===
    if let Some(path) = &args.snap {
        let mut f = std::fs::File::open(path).expect("snapshot");
        let bodies = read_snapshot(&mut f).expect("read");
        let mut win = Window::new(&format!("galax-viz — {} bodies", bodies.len()), args.width, args.height, WindowOptions { resize: true, ..Default::default() }).expect("window");
        let mut cam = Camera::new();
        let mut buf = vec![0u32; args.width * args.height];
        while win.is_open() && !win.is_key_down(Key::Escape) {
            let (w, h) = win.get_size(); if w == 0 || h == 0 { continue; }
            if buf.len() != w * h { buf = vec![0u32; w * h]; }
            handle_camera(&win, &mut cam, w, h);
            let snap_bounds = WorldBounds { x_min: -50.0, x_max: 50.0, y_min: -50.0, y_max: 50.0, frame: 1 };
            render_frame(&mut buf, w, h, &bodies, 0, Duration::ZERO, 0.0, args.max_render, &cam, &snap_bounds, "snap");
            win.update_with_buffer(&buf, w, h).expect("update");
        }
        return;
    }

    // === Live simulation ===
    let mut bodies = BodiesSoA::new(args.n);
    match args.init.as_str() { "plummer" => { plummer(&mut bodies, args.n); } "disk" => { disk(&mut bodies, args.n); } "galaxy" => { galaxy(&mut bodies, args.n); } _ => { uniform(&mut bodies, args.n); } }

    let softening = args.softening.unwrap_or_else(|| {
        let mut s = 0.0;
        for i in 0..args.n.min(1000) { let dx = bodies.x[i] - bodies.x[(i + 1) % args.n]; let dy = bodies.y[i] - bodies.y[(i + 1) % args.n]; s += (dx * dx + dy * dy).sqrt(); }
        s / args.n.min(1000) as f64 * 0.1
    });

    // Build tree (unused in --no-fmm mode but kept to simplify code flow)
    let tree = build_constrained(&mut bodies, args.n_max, -50.0, 50.0, -50.0, 50.0);
    let m2l = InteractionLists::build(&tree);
    let p2p = build_p2p_lists(&tree);

    // Compute initial forces and dt
    let using_n2 = use_n2(args.n, args.fmm, args.n2);
    if using_n2 && args.n > 1 { compute_n2_force(&mut bodies, softening); }
    else { compute_fmm_force(&mut bodies, &tree, &m2l, &p2p, args.p, softening); }

    let dt = args.dt.unwrap_or_else(|| {
        let ma = bodies.ax.iter().zip(bodies.ay.iter()).map(|(x,y)| (x*x+y*y).sqrt()).fold(0.0_f64, f64::max);
        if ma > 1e-30 { 0.5 / ma.sqrt() } else { 0.01 }
    });

    // Capture initial world bounds ONCE — camera never auto-adjusts
    let mut init_bounds = WorldBounds::new();
    init_bounds.get(&bodies, 1);

    // ── Optional GPU setup ───────────────────────────────────────
    let gpu_ctx = if args.gpu {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::PRIMARY, ..Default::default() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: None, force_fallback_adapter: false })).expect("no GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("galax-viz"), required_features: wgpu::Features::empty(), required_limits: wgpu::Limits { max_storage_buffers_per_shader_stage: 16, ..Default::default() }, ..Default::default() }, None)).expect("GPU device");
        let cfg = GpuConfig { max_n: args.n.max(args.max_render) as u32, max_nodes: tree.nodes.len().max(1024) as u32, max_interactions: (tree.nodes.len() * 64).max(4096) as u32, p: args.p as u32, eps: softening as f32 };
        let mut ctx = GpuContext::new(Arc::new(device), Arc::new(queue), cfg).expect("init GPU");
        ctx.upload_tree_data(&tree, &m2l, &p2p);
        // Initial GPU force computation
        ctx.step(&bodies);
        ctx.read_accelerations(&mut bodies.ax, &mut bodies.ay);
        Some(ctx)
    } else {
        None
    };

    let mode_label = if args.gpu { "GPU" } else if using_n2 { "N²" } else { "FMM" };

    let mut win = Window::new(&format!("galax-viz — N={} {} [{}]", args.n, args.init, mode_label), args.width, args.height, WindowOptions { resize: true, ..Default::default() }).expect("window");
    let mut cam = Camera::new();
    let mut step: u64 = 0;
    let global_start = Instant::now();
    let mut buf = vec![0u32; args.width * args.height];

    let mut last_render = Instant::now();

    while win.is_open() && !win.is_key_down(Key::Escape) {
        let (w, h) = win.get_size(); if w == 0 || h == 0 { continue; }
        if buf.len() != w * h { buf = vec![0u32; w * h]; }
        handle_camera(&win, &mut cam, w, h);

        let real_elapsed = last_render.elapsed().as_secs_f64();
        let target_steps = (args.sim_speed * real_elapsed).ceil() as u64;
        let steps_to_run = target_steps.min(args.max_steps_per_frame as u64).max(1);

        let mut steps_this_frame = 0u64;
        for _ in 0..steps_to_run {
            let step_start = Instant::now();
            if let Some(ref ctx) = gpu_ctx {
                leapfrog_kdk_gpu(&mut bodies, ctx, dt);
            } else {
                physics_step(&mut bodies, &tree, &m2l, &p2p, args.p, softening, dt, using_n2);
            }
            step += 1;
            steps_this_frame += 1;
            if steps_this_frame >= steps_to_run || step_start.elapsed() > frame_budget {
                break;
            }
        }

        let elapsed_render = last_render.elapsed();
        if elapsed_render < frame_budget {
            std::thread::sleep(frame_budget - elapsed_render);
        }

        let sps = if real_elapsed > 0.0 { steps_this_frame as f64 / real_elapsed } else { 0.0 };
        render_frame(&mut buf, w, h, &bodies, step, global_start.elapsed(), sps, args.max_render, &cam, &init_bounds, mode_label);
        win.update_with_buffer(&buf, w, h).expect("update");
        last_render = Instant::now();
    }
}
