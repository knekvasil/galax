use galax_core::{
    build_p2p_lists, compute_fmm_force, compute_n2_force, InteractionLists, BodiesSoA, Tree,
    allocate_expansions, allocate_locals, p2m, m2m, m2l, l2l, l2p, p2p,
};
use std::time::Instant;

fn bench<F>(name: &str, n: usize, f: F)
where
    F: FnOnce(),
{
    let start = Instant::now();
    f();
    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64();
    println!("  {:20}  N={:>8}  {:>8.3}s  {:>10.0} bodies/s",
        name, n, secs, n as f64 / secs.max(1e-12));
}

fn main() {
    let max_n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10000);

    println!("\n=== galax-bench ===");
    println!("{:20}  {:>8}  {:>10}  {:>12}", "Pass", "N", "Time (s)", "Bodies/s");
    println!("{}", "-".repeat(60));

    for n in &[100usize, 500, 1000, 5000, max_n] {
        let n = *n;
        if n > max_n { break; }

        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 100.0 - 50.0;
            bodies.y[i] = fastrand::f64() * 100.0 - 50.0;
            bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
        }

        // Tree construction
        let (tree, _sorted_bodies, m2l_lists, p2p_lists) = {
            let mut b = BodiesSoA::new(n);
            b.x.copy_from_slice(&bodies.x);
            b.y.copy_from_slice(&bodies.y);
            b.mass.copy_from_slice(&bodies.mass);

            let start = Instant::now();
            let tree = Tree::build(&mut b, 32, -50.0, 50.0, -50.0, 50.0);
            let build_time = start.elapsed().as_secs_f64();

            let start2 = Instant::now();
            let m2l = InteractionLists::build(&tree);
            let m2l_time = start2.elapsed().as_secs_f64();

            let start3 = Instant::now();
            let p2p_l = build_p2p_lists(&tree);
            let p2p_time = start3.elapsed().as_secs_f64();

            println!("  {:20}  N={:>8}  {:>8.3}s  {:>10.0} (build)",
                "tree_build", n, build_time, n as f64 / build_time.max(1e-12));
            println!("  {:20}  N={:>8}  {:>8.3}s  {:>10.0}",
                "interaction_lists", n, m2l_time + p2p_time, n as f64 / (m2l_time + p2p_time).max(1e-12));
            (tree, b, m2l, p2p_l)
        };

        // Combined body for FMM + N² timing
        let softening = 0.1;
        let p = 4;

        // P2M
        bench("p2m", n, || {
            let _ = p2m(&bodies, &tree, p, softening);
        });

        // M2M
        let mut multipole = allocate_expansions(tree.nodes.len(), p);
        let tmp_mult = p2m(&bodies, &tree, p, softening);
        multipole.copy_from_slice(&tmp_mult);
        bench("m2m", n, || {
            let mut m = multipole.clone();
            m2m(&mut m, &tree, p);
        });

        // M2L
        let locals = allocate_locals(tree.nodes.len(), p);
        bench("m2l", n, || {
            let mut l = locals.clone();
            m2l(&mut l, &multipole, &m2l_lists, &tree, p, softening);
        });

        // L2L
        let mut loc_copy = locals.clone();
        m2l(&mut loc_copy, &multipole, &m2l_lists, &tree, p, softening);
        bench("l2l", n, || {
            let mut l = loc_copy.clone();
            l2l(&mut l, &tree, p);
        });

        // L2P
        bench("l2p", n, || {
            let mut b2 = BodiesSoA::new(n);
            b2.x.copy_from_slice(&bodies.x);
            b2.y.copy_from_slice(&bodies.y);
            b2.mass.copy_from_slice(&bodies.mass);
            l2p(&mut b2, &loc_copy, &tree, p);
        });

        // P2P
        bench("p2p", n, || {
            let mut b2 = BodiesSoA::new(n);
            b2.x.copy_from_slice(&bodies.x);
            b2.y.copy_from_slice(&bodies.y);
            b2.mass.copy_from_slice(&bodies.mass);
            p2p(&mut b2, &tree, &p2p_lists, softening);
        });

        // Full FMM
        bench("full_fmm", n, || {
            let mut b2 = BodiesSoA::new(n);
            b2.x.copy_from_slice(&bodies.x);
            b2.y.copy_from_slice(&bodies.y);
            b2.mass.copy_from_slice(&bodies.mass);
            compute_fmm_force(&mut b2, &tree, &m2l_lists, &p2p_lists, p, softening);
        });

        // N² (parallel)
        bench("n2", n, || {
            let mut b2 = BodiesSoA::new(n);
            b2.x.copy_from_slice(&bodies.x);
            b2.y.copy_from_slice(&bodies.y);
            b2.mass.copy_from_slice(&bodies.mass);
            compute_n2_force(&mut b2, softening);
        });

        println!();
    }
}
