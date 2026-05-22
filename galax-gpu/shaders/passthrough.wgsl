struct SimParams {
    n: u32,
    num_nodes: u32,
    num_leaves: u32,
    p: u32,
    eps: f32,
    _pad: f32,
}

struct ErrFlag {
    value: atomic<u32>,
}

@group(0) @binding(0) var<storage, read> input_positions: array<vec2<f32>>;
@group(0) @binding(1) var<storage, read_write> output_accelerations: array<vec2<f32>>;
@group(0) @binding(2) var<uniform> params: SimParams;
@group(0) @binding(3) var<storage, read_write> error_flag: ErrFlag;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= params.n) { return; }
    let pos = input_positions[i];
    if (pos.x != pos.x || pos.y != pos.y) {
        atomicMax(&error_flag.value, 1u);
        return;
    }
    output_accelerations[i] = pos;
}
