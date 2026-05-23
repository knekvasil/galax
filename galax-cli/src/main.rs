use clap::Parser;
use galax_core::{
    build_p2p_lists, compute_fmm_force, compute_n2_force,
    InteractionLists, BodiesSoA, Tree,
    n2_regression, validate_fmm, build_constrained,
    allocate_locals, p2m, m2m, m2l, l2l, l2p, p2p,
};
use galax_gpu::{GpuConfig, GpuContext};
use galax_init::{disk, plummer, uniform, galaxy};
use galax_integrate::simulate;
use galax_io::{read_snapshot, write_snapshot};
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "galax-cli")]
struct Args {
    #[arg(long)]
    n: Option<usize>,
    #[arg(long)]
    steps: Option<u64>,
    #[arg(long)]
    t_end: Option<f64>,
    #[arg(long)]
    dt: Option<f64>,
    #[arg(long)]
    softening: Option<f64>,
    #[arg(long, default_value = "32")]
    n_max: usize,
    #[arg(long, default_value = "4")]
    p: usize,
    #[arg(long, default_value = "0.5")]
    theta: f64,
    #[arg(long)]
    validate: bool,
    #[arg(long, default_value = "1000")]
    validate_samples: usize,
    #[arg(long)]
    n2_regression: bool,
    #[arg(long, default_value = "10")]
    energy_every: u64,
    #[arg(long)]
    init: Option<String>,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    snap_every: Option<u64>,
    #[arg(long)]
    load: Option<PathBuf>,
    #[arg(long)]
    convergence: bool,
    #[arg(long)]
    progress: bool,
    #[arg(long)]
    bench: bool,
    #[arg(long)]
    gpu: bool,
    #[arg(long)]
    gpu_crosscheck: bool,
}

fn run_bench(n: usize, p: usize) {
    let softening = 0.1;
    let mut bodies = BodiesSoA::new(n);
    for i in 0..n {
        bodies.x[i] = fastrand::f64() * 100.0 - 50.0;
        bodies.y[i] = fastrand::f64() * 100.0 - 50.0;
        bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
    }

    let start = Instant::now();
    let tree = Tree::build(&mut bodies, 32, -50.0, 50.0, -50.0, 50.0);
    let build_time = start.elapsed();
    let m2l_lists = InteractionLists::build(&tree);
    let p2p_lists = build_p2p_lists(&tree);

    let t = |name: &str, dur: std::time::Duration| {
        println!("  {:20}  N={:>8}  {:>8.3}s", name, n, dur.as_secs_f64());
    };

    t("tree_build", build_time);

    let s = Instant::now(); let _ = p2m(&bodies, &tree, p, softening); t("p2m", s.elapsed());

    let mut multipole = p2m(&bodies, &tree, p, softening);
    let s = Instant::now(); m2m(&mut multipole, &tree, p); t("m2m", s.elapsed());

    let mut locals = allocate_locals(tree.nodes.len(), p);
    let s = Instant::now();
    m2l(&mut locals, &multipole, &m2l_lists, &tree, p, softening);
    t("m2l", s.elapsed());

    let s = Instant::now(); l2l(&mut locals, &tree, p); t("l2l", s.elapsed());

    let mut bb = BodiesSoA::new(n);
    bb.x.copy_from_slice(&bodies.x);
    bb.y.copy_from_slice(&bodies.y);
    bb.mass.copy_from_slice(&bodies.mass);
    bb.ax.iter_mut().for_each(|a| *a = 0.0);
    bb.ay.iter_mut().for_each(|a| *a = 0.0);
    let s = Instant::now(); l2p(&mut bb, &locals, &tree, p); t("l2p", s.elapsed());

    let mut bb2 = BodiesSoA::new(n);
    bb2.x.copy_from_slice(&bodies.x);
    bb2.y.copy_from_slice(&bodies.y);
    bb2.mass.copy_from_slice(&bodies.mass);
    bb2.ax.iter_mut().for_each(|a| *a = 0.0);
    bb2.ay.iter_mut().for_each(|a| *a = 0.0);
    let s = Instant::now(); p2p(&mut bb2, &tree, &p2p_lists, softening); t("p2p", s.elapsed());

    let mut b3 = BodiesSoA::new(n);
    b3.x.copy_from_slice(&bodies.x);
    b3.y.copy_from_slice(&bodies.y);
    b3.mass.copy_from_slice(&bodies.mass);
    let s = Instant::now();
    compute_fmm_force(&mut b3, &tree, &m2l_lists, &p2p_lists, p, softening);
    t("full_fmm", s.elapsed());

    let mut b4 = BodiesSoA::new(n);
    b4.x.copy_from_slice(&bodies.x);
    b4.y.copy_from_slice(&bodies.y);
    b4.mass.copy_from_slice(&bodies.mass);
    let s = Instant::now(); compute_n2_force(&mut b4, softening); t("n2", s.elapsed());
}

fn main() {
    let args = Args::parse();

    if args.bench {
        println!("\n=== galax-cli --bench ===");
        for &n in &[100, 500, 1000, 5000, args.n.unwrap_or(10000)] {
            run_bench(n, args.p);
            println!();
        }
        return;
    }

    let mut bodies = if let Some(path) = &args.load {
        let mut file = std::fs::File::open(path).expect("failed to open snapshot");
        read_snapshot(&mut file).expect("failed to read snapshot")
    } else {
        let n = args.n.expect("either --n or --load is required");
        let mut b = BodiesSoA::new(n);

        match args.init.as_deref().unwrap_or("uniform") {
            "plummer" => plummer(&mut b, n),
            "disk" => disk(&mut b, n),
            "galaxy" => galaxy(&mut b, n),
            _ => uniform(&mut b, n),
        }

        b
    };

    let n = bodies.len();

    let tree = Tree::build(&mut bodies, args.n_max, -50.0, 50.0, -50.0, 50.0);
    let leaf_count = tree.nodes.iter().filter(|n| n.is_leaf).count();
    let depth = {
        let mut d = 0;
        let mut idx = tree.nodes.len() - 1;
        while !tree.nodes[idx].is_leaf {
            if let Some(child) = tree.nodes[idx].children.iter().flatten().next() {
                idx = *child;
                d += 1;
            } else {
                break;
            }
        }
        d
    };
    println!(
        "Tree: {} nodes, {} leaves, depth={}, n_max={}",
        tree.nodes.len(),
        leaf_count,
        depth,
        args.n_max
    );

    let softening = args.softening.unwrap_or_else(|| {
        let mut sum = 0.0;
        for i in 0..n.min(1000) {
            let dx = bodies.x[i] - bodies.x[(i + 1) % n];
            let dy = bodies.y[i] - bodies.y[(i + 1) % n];
            sum += (dx * dx + dy * dy).sqrt();
        }
        sum / n.min(1000) as f64 * 0.1
    });

    let m2l_lists = InteractionLists::build(&tree);
    let p2p_lists = build_p2p_lists(&tree);

    if args.gpu_crosscheck {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: None, force_fallback_adapter: false,
        })).expect("no GPU adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("galax-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits { max_storage_buffers_per_shader_stage: 16, ..Default::default() },
                ..Default::default()
            },
        )).expect("failed to create GPU device");

        let gpu_config = GpuConfig {
            max_n: n as u32,
            max_nodes: tree.nodes.len().max(1024) as u32,
            max_interactions: (tree.nodes.len() * 64).max(4096) as u32,
            p: args.p as u32,
            eps: softening as f32,
        };
        let mut gpu_ctx = GpuContext::new(Arc::new(device), Arc::new(queue), gpu_config)
            .expect("failed to init GPU context");
        gpu_ctx.upload_tree_data(&tree, &m2l_lists, &p2p_lists);

        println!("\nGPU crosscheck (Layer A):");
        let (max_err, mean_err, n_samp) = gpu_ctx.crosscheck(
            &mut bodies, &tree, &m2l_lists, &p2p_lists,
            args.p, softening, 10.0,
        );
        println!("  max_rel_err={:.2e}, mean_rel_err={:.2e}, samples={}", max_err, mean_err, n_samp);
        return;
    }

    compute_fmm_force(&mut bodies, &tree, &m2l_lists, &p2p_lists, args.p, softening);

    let dt = args.dt.unwrap_or_else(|| {
        let max_accel = bodies
            .ax
            .iter()
            .zip(bodies.ay.iter())
            .map(|(ax, ay)| (ax * ax + ay * ay).sqrt())
            .fold(0.0_f64, f64::max);
        if max_accel > 1e-30 {
            0.5 / max_accel.sqrt()
        } else {
            0.01
        }
    });

    let n_steps = if let Some(s) = args.steps {
        s
    } else if let Some(t) = args.t_end {
        (t / dt).ceil() as u64
    } else {
        10
    };

    println!("Running {} steps, dt={:.6e}, softening={:.6e}", n_steps, dt, softening);

    let start = Instant::now();
    let diags = if args.gpu {
        use galax_integrate::simulate_gpu;

        let instance =         wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("no GPU adapter found");
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("galax-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits {
                    max_storage_buffers_per_shader_stage: 16,
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .expect("failed to create GPU device");

        let gpu_config = GpuConfig {
            max_n: n as u32,
            max_nodes: tree.nodes.len().max(1024) as u32,
            max_interactions: (tree.nodes.len() * 64).max(4096) as u32,
            p: args.p as u32,
            eps: softening as f32,
        };
        let mut gpu_ctx = GpuContext::new(Arc::new(device), Arc::new(queue), gpu_config)
            .expect("failed to init GPU context");

        println!("Using GPU backend (FMM mode)");
        let result = if args.progress {
            let pb = ProgressBar::new(n_steps);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40}] {pos}/{len} steps ({eta})")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            let r = simulate_gpu(
                &mut bodies, &tree, &m2l_lists, &p2p_lists, &mut gpu_ctx,
                args.p, softening, dt, n_steps, args.energy_every,
            );
            pb.finish_with_message("simulation complete");
            r
        } else {
            simulate_gpu(
                &mut bodies, &tree, &m2l_lists, &p2p_lists, &mut gpu_ctx,
                args.p, softening, dt, n_steps, args.energy_every,
            )
        };
        result
    } else if args.progress {
        let pb = ProgressBar::new(n_steps);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40}] {pos}/{len} steps ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );
        let result = simulate(
            &mut bodies, &tree, &m2l_lists, &p2p_lists,
            args.p, softening, dt, n_steps, args.energy_every,
        );
        pb.finish_with_message("simulation complete");
        result
    } else {
        simulate(
            &mut bodies, &tree, &m2l_lists, &p2p_lists,
            args.p, softening, dt, n_steps, args.energy_every,
        )
    };
    let elapsed = start.elapsed();

    println!("Simulation completed in {:.3?}", elapsed);
    for d in &diags {
        println!(
            "step={}: KE={:.6e}, PE={:.6e}, E_total={:.6e}, px={:.6e}, py={:.6e}, Lz={:.6e}",
            d.step,
            d.kinetic,
            d.potential,
            d.kinetic + d.potential,
            d.momentum_x,
            d.momentum_y,
            d.angular_momentum,
        );
    }

    if let Some(out_dir) = &args.out {
        std::fs::create_dir_all(out_dir).expect("failed to create output directory");
        // Write final snapshot
        let snap_path = out_dir.join(format!("snap_{:04}.bin", n_steps));
        let mut file = std::fs::File::create(&snap_path).expect("failed to create snapshot file");
        write_snapshot(&mut file, &bodies).expect("failed to write snapshot");
        println!("Wrote snapshot: {}", snap_path.display());
    }

    if args.convergence {
        // Run validation at multiple p values to show convergence
        // Need to rebuild the tree since bodies are permuted during simulation
        let mut ref_bodies = BodiesSoA::new(n);
        ref_bodies.x.copy_from_slice(&bodies.x);
        ref_bodies.y.copy_from_slice(&bodies.y);
        ref_bodies.mass.copy_from_slice(&bodies.mass);

        let conv_tree = build_constrained(&mut ref_bodies, args.n_max, -50.0, 50.0, -50.0, 50.0);
        let conv_m2l = InteractionLists::build(&conv_tree);
        let conv_p2p = build_p2p_lists(&conv_tree);

        println!("\nConvergence (error vs expansion order):");
        println!("  p   | max_rel_err | mean_rel_err ");
        println!("  ----|-------------|--------------");
        for &cp in &[2, 4, 6, 8] {
            let (sn, max_e, mean_e, _) = validate_fmm(
                &ref_bodies, &conv_tree, &conv_m2l, &conv_p2p,
                cp, softening, args.validate_samples.min(n),
            );
            println!("  {:2}   | {:.2e}    | {:.2e}     ({} samples)", cp, max_e, mean_e, sn);
        }
    }

    if args.validate {
        let (sample_n, max_err, mean_err, _) = validate_fmm(
            &bodies, &tree, &m2l_lists, &p2p_lists, args.p, softening,
            args.validate_samples.min(n),
        );
        println!(
            "Validation (sampled N\u{00b2}, {} samples): max_rel_err={:.2e}, mean_rel_err={:.2e}",
            sample_n, max_err, mean_err,
        );
    }

    if args.n2_regression {
        if n > 2000 {
            eprintln!("n2-regression only valid for N \u{2264} 2000 (got N={})", n);
        } else {
            let max_err = n2_regression(
                &bodies, &tree, &m2l_lists, &p2p_lists, args.p, softening,
            );
            println!("N\u{00b2} regression: max relative error = {:.2e}", max_err);
        }
    }
}
