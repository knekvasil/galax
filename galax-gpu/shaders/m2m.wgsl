fn process_child(node: u32, child_id: i32, stride: u32, p: u32) {
    if (child_id < 0) { return; }
    let child = u32(child_id);
    let ctr = centers[node];
    let child_ctr = centers[child];
    let dx = ctr.x - child_ctr.x;
    let dy = ctr.y - child_ctr.y;
    let base_parent = node * stride;
    let base_child = child * stride;
    for (var I = 0u; I <= p; I = I + 1u) {
        for (var J = 0u; J <= p - I; J = J + 1u) {
            var sum = 0.0;
            for (var i = 0u; i <= I; i = i + 1u) {
                for (var j = 0u; j <= J; j = j + 1u) {
                    let child_mi = moment_index(i, j);
                    let child_val = expansions[base_child + child_mi];
                    if (child_val == 0.0) { continue; }
                    let dxp = pow(dx, f32(I - i));
                    let dyp = pow(dy, f32(J - j));
                    sum += child_val * dxp * dyp / (fact(I - i) * fact(J - j));
                }
            }
            let mi = moment_index(I, J);
            expansions[base_parent + mi] += sum;
        }
    }
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let node = id.x;
    if (node >= params.num_nodes) { return; }
    if (node_levels[node] != params.current_level) { return; }
    let c = children[node];
    if (c.x < 0) { return; }
    let p = params.p;
    let stride = num_moments(p);
    process_child(node, c.x, stride, p);
    process_child(node, c.y, stride, p);
    process_child(node, c.z, stride, p);
    process_child(node, c.w, stride, p);
}
