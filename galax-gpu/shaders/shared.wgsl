struct SimParams {
    n: u32,
    num_nodes: u32,
    num_leaves: u32,
    p: u32,
    eps: f32,
    m2l_num_indices: u32,
    current_level: u32,
    p2p_num_indices: u32,
}

fn moment_index(i: u32, j: u32) -> u32 {
    let n = i + j;
    return n * (n + 1u) / 2u + j;
}

fn num_moments(p: u32) -> u32 {
    return (p + 1u) * (p + 2u) / 2u;
}

fn fact(n: u32) -> f32 {
    var f = array<f32, 18>(1.0, 1.0, 2.0, 6.0, 24.0, 120.0, 720.0, 5040.0, 40320.0, 362880.0, 3628800.0, 39916800.0, 479001600.0, 6227020800.0, 87178291200.0, 1307674368000.0, 20922789888000.0, 355687428096000.0);
    return f[n];
}

fn binom(n: u32, k: u32) -> f32 {
    return fact(n) / (fact(k) * fact(n - k));
}

@group(0) @binding(0) var<storage, read>     body_data:    array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> accels:      array<vec2<f32>>;
@group(0) @binding(2) var<storage, read_write> expansions:  array<f32>;
@group(0) @binding(3) var<uniform>           params:       SimParams;
@group(0) @binding(4) var<storage, read>     m2l_data:     array<u32>;
@group(0) @binding(5) var<storage, read>     p2p_data:     array<u32>;
@group(0) @binding(6) var<storage, read>     leaf_data:    array<vec4<u32>>;
@group(0) @binding(7) var<storage, read>     centers:      array<vec2<f32>>;
@group(0) @binding(8) var<storage, read>     children:     array<vec4<i32>>;
@group(0) @binding(9) var<storage, read>     deriv_coeffs: array<f32>;
