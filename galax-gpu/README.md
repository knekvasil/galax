# galax-gpu

GPU-accelerated Fast Multipole Method via `wgpu` (WebGPU).

Runs the same 6-pass FMM pipeline on the GPU using WGSL compute shaders.
Supports both native (Metal/Vulkan/DX12) and WASM (WebGPU in-browser) targets.

## Architecture

- **Single bind group** with 9 storage buffers + 1 uniform — fits within WebGPU's
  `maxStorageBuffersPerShaderStage` limit of 10
- **WGSL shaders** embed via `include_str!` at compile time, processed by `build.rs`
- **Build script** prepends `shared.wgsl` (common structs and bindings) to each pass shader
- **Async readback** on WASM uses `map_async` with `setTimeout` macrotask yields
  (wgpu's `poll()` is a no-op on the WebGPU backend)

## Key Types

| Type | Purpose |
|------|---------|
| `GpuContext` | Device, queue, buffers, pipelines, bind group |
| `GpuConfig` | Buffer sizing parameters (max N, nodes, interactions) |

## Pipeline (6 passes)

```
P2M → M2M (per-level) → M2L → L2L (per-level) → L2P → P2P
```

## Shaders

All WGSL shaders live in `shaders/`:

| File | Pass | Bindings used |
|------|------|---------------|
| `p2m.wgsl` | Particle → multipole | body_data, expansions, leaf_data, centers |
| `m2m.wgsl` | Multipole → multipole (up) | expansions, leaf_data, centers, children |
| `m2l.wgsl` | Multipole → local | m2l_data, expansions, centers, deriv_coeffs |
| `l2l.wgsl` | Local → local (down) | expansions, leaf_data, centers, children |
| `l2p.wgsl` | Local → particle | body_data, accels, expansions, centers, leaf_data |
| `p2p.wgsl` | Particle → particle | body_data, accels, p2p_data, leaf_data |
| `n2_debug.wgsl` | Brute-force N² (debug) | body_data, accels |

## Usage

```rust
use galax_gpu::*;
use std::sync::Arc;

let (device, queue) = /* wgpu device + queue */;
let cfg = GpuConfig { max_n: 262144, max_nodes: 524288, max_interactions: 1 << 24, p: 4, eps: 0.1 };
let mut ctx = GpuContext::new(Arc::new(device), Arc::new(queue), cfg)?;
ctx.upload_tree_data(&tree, &m2l_lists, &p2p_lists);

// GPU FMM
ctx.step(&bodies);
ctx.read_accelerations(&mut bodies.ax, &mut bodies.ay);

// Cross-check against CPU
let (max_err, mean_err, _) = ctx.crosscheck(&mut bodies, &tree, &m2l_lists, &p2p_lists, 4, 0.1, 1.0);
```
