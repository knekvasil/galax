@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= params.n) { return; }
    let my_pos = positions[i];
    check_nan_val(my_pos.x); check_nan_val(my_pos.y);
    let lid = leaf_id[i];
    let off = p2p_offsets[lid];
    let next_off = p2p_offsets[lid + 1u];
    var accel = vec2(0.0, 0.0);
    let eps2 = params.eps * params.eps;
    for (var k = off; k < next_off; k = k + 1u) {
        let partner = p2p_indices[k];
        let range = leaf_ranges[partner];
        for (var j = range.x; j < range.y; j = j + 1u) {
            if (j == i) { continue; }
            let pj = positions[j];
            let dx = my_pos.x - pj.x;
            let dy = my_pos.y - pj.y;
            let r2 = dx * dx + dy * dy + eps2;
            let inv_r3 = 1.0 / (r2 * sqrt(r2));
            let f = masses[j] * inv_r3;
            accel.x -= f * dx;
            accel.y -= f * dy;
        }
    }
    accels[i] = accel;
}
