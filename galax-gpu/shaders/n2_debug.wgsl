@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= params.n) { return; }
    let my_pos = body_data[i].xy;
    var accel = vec2(0.0, 0.0);
    let eps2 = params.eps * params.eps;
    for (var j = 0u; j < params.n; j = j + 1u) {
        if (j == i) { continue; }
        let pj = body_data[j].xy;
        let dx = my_pos.x - pj.x;
        let dy = my_pos.y - pj.y;
        let r2 = dx * dx + dy * dy + eps2;
        let inv_r3 = 1.0 / (r2 * sqrt(r2));
        let f = body_data[j].z * inv_r3;
        accel.x -= f * dx;
        accel.y -= f * dy;
    }
    accels[i] = accel;
}
