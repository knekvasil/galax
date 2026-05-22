fn process_child(node: u32, child_id: i32, stride: u32, p: u32) {
    if (child_id < 0) { return; }
    let child = u32(child_id);
    let nn = params.num_nodes;
    let ctr = centers[node];
    let child_ctr = centers[child];
    let dx = child_ctr.x - ctr.x;
    let dy = child_ctr.y - ctr.y;
    let base_parent = (nn + node) * stride;
    let base_child = (nn + child) * stride;
    for (var i = 0u; i <= p; i = i + 1u) {
        for (var j = 0u; j <= p - i; j = j + 1u) {
            var sum = 0.0;
            for (var I = i; I <= p; I = I + 1u) {
                for (var J = j; J <= p - I; J = J + 1u) {
                    let parent_mi = moment_index(I, J);
                    let parent_val = expansions[base_parent + parent_mi];
                    if (parent_val == 0.0) { continue; }
                    let dxp = pow(dx, f32(I - i));
                    let dyp = pow(dy, f32(J - j));
                    sum += parent_val * dxp * dyp * binom(I, i) * binom(J, j);
                }
            }
            let mi = moment_index(i, j);
            expansions[base_child + mi] += sum;
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
