@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let leaf = id.x;
    if (leaf >= params.num_nodes) { return; }
    let p = params.p;
    let stride = num_moments(p);
    let range = leaf_ranges[leaf];
    let n_bodies = range.y - range.x;
    if (n_bodies == 0u) { return; }

    // Compute center of mass for this leaf
    var com_x = 0.0;
    var com_y = 0.0;
    var total_mass = 0.0;
    for (var b = range.x; b < range.y; b = b + 1u) {
        let m = masses[b];
        total_mass += m;
        com_x += positions[b].x * m;
        com_y += positions[b].y * m;
    }
    com_x = com_x / total_mass;
    com_y = com_y / total_mass;

    // Accumulate multipole moments M_{i,j} = sum m * dx^i * dy^j * (-1)^{i+j} / (i! * j!)
    let base = leaf * stride;
    for (var b = range.x; b < range.y; b = b + 1u) {
        let dx = positions[b].x - com_x;
        let dy = positions[b].y - com_y;
        let m = masses[b];
        // Precompute powers
        var pow_x: array<f32, 18>;
        var pow_y: array<f32, 18>;
        pow_x[0] = 1.0; pow_y[0] = 1.0;
        for (var t = 1u; t <= p; t = t + 1u) {
            pow_x[t] = pow_x[t - 1u] * dx;
            pow_y[t] = pow_y[t - 1u] * dy;
        }
        for (var i = 0u; i <= p; i = i + 1u) {
            for (var j = 0u; j <= p - i; j = j + 1u) {
                let mi = moment_index(i, j);
                let sign = select(1.0, -1.0, (i + j) % 2u == 1u);
                let term = m * pow_x[i] * pow_y[j] * sign / (fact(i) * fact(j));
                expansions[base + mi] += term;
            }
        }
    }
}
