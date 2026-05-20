use criterion::{black_box, criterion_group, criterion_main, Criterion};
use galax_core::{
    build_p2p_lists, compute_fmm_force, compute_n2_force, allocate_expansions,
    p2m, m2m, m2l, l2l, l2p, p2p, allocate_locals,
    InteractionLists, BodiesSoA, Tree,
};

fn setup(n: usize, n_max: usize) -> (BodiesSoA, Tree, InteractionLists, InteractionLists) {
    let mut bodies = BodiesSoA::new(n);
    for i in 0..n {
        bodies.x[i] = fastrand::f64() * 100.0 - 50.0;
        bodies.y[i] = fastrand::f64() * 100.0 - 50.0;
        bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
    }
    let tree = Tree::build(&mut bodies, n_max, -50.0, 50.0, -50.0, 50.0);
    let m2l_lists = InteractionLists::build(&tree);
    let p2p_lists = build_p2p_lists(&tree);
    (bodies, tree, m2l_lists, p2p_lists)
}

fn bench_fmm(c: &mut Criterion) {
    let (mut bodies, tree, m2l_lists, p2p_lists) = setup(200, 16);
    let p = 4;
    let softening = 0.1;

    c.bench_function("full_fmm", |b| {
        b.iter(|| {
            compute_fmm_force(black_box(&mut bodies), &tree, &m2l_lists, &p2p_lists, p, softening);
        })
    });
}

fn bench_n2(c: &mut Criterion) {
    let mut bodies = BodiesSoA::new(200);
    for i in 0..200 {
        bodies.x[i] = fastrand::f64() * 100.0 - 50.0;
        bodies.y[i] = fastrand::f64() * 100.0 - 50.0;
        bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
    }

    c.bench_function("n2_force", |b| {
        b.iter(|| {
            compute_n2_force(black_box(&mut bodies), 0.1);
        })
    });
}

criterion_group!(benches, bench_fmm, bench_n2);
criterion_main!(benches);
