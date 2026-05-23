@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= params.n) { return; }
    let my_pos = body_data[i].xy;
    let lid = leaf_data[i].x;
    let p = params.p;
    let stride = num_moments(p);
    let nn = params.num_nodes;
    let base_local = (nn + lid) * stride;
    let leaf_ctr = centers[lid];
    let dx = my_pos.x - leaf_ctr.x;
    let dy = my_pos.y - leaf_ctr.y;
    var ax = 0.0;
    var ay = 0.0;
    for (var I = 0u; I <= p; I = I + 1u) {
        for (var J = 0u; J <= p - I; J = J + 1u) {
            let mi = moment_index(I, J);
            let L = expansions[base_local + mi];
            if (L == 0.0) { continue; }
            if (I > 0u) {
                let term_x = -L * f32(I) * pow(dx, f32(I - 1u)) * pow(dy, f32(J));
                ax += term_x;
            }
            if (J > 0u) {
                let term_y = -L * f32(J) * pow(dx, f32(I)) * pow(dy, f32(J - 1u));
                ay += term_y;
            }
        }
    }
    accels[i] += vec2(ax, ay);
}
