Status: ready-for-agent

# GPU bootstrapping and trivial compute

## Parent

`.scratch/gpu-architecture/PRD.md`

## What to build

Create the `galax-gpu` workspace crate with wgpu as a dependency, a `build.rs` that concatenates WGSL shader sources, a stub `GpuContext` struct that initializes wgpu and runs a trivial pass-through compute shader, and wire the `--gpu` flag into `galax-cli` so the GPU path is selectable at the command line.

The `GpuContext` is initialized lazily by the caller (galax-cli or galax-viz), which owns the wgpu `Device` and `Queue`. The `GpuContext` receives these at init time and creates all compute pipelines and preallocated buffers.

The trivial pass-through shader copies a `vec2<f32>` from a storage buffer to another — proving the full GPU pipeline works (buffer allocation, shader compilation, dispatch, readback). The CLI `--gpu` flag creates the wgpu device, initializes `GpuContext`, calls `GpuContext::step()`, and reads back the result.

The Leapfrog KDK integration crate (`galax-integrate`) receives a new `leapfrog_kdk_gpu` that delegates force computation to the `GpuContext` instead of `compute_fmm_force`. For this slice it uses the pass-through shader (forces are wrong, but the integration loop runs).

Buffer preallocation follows the max-size model: `GpuConfig` specifies `max_n`, `max_nodes`, `max_interactions` and all buffers are allocated once at init.

The NaN detection pattern uses an atomic error flag in a small storage buffer — initialized to 0 before dispatch, threads write 1 on NaN/inf, CPU reads after dispatch.

## Acceptance criteria

- [ ] `galax-gpu` crate exists as a workspace member with wgpu dependency, compiles on M1
- [ ] `build.rs` concatenates `shaders/shared.wgsl` with each pass `.wgsl` into standalone modules
- [ ] A pass-through WGSL shader is compiled and dispatched correctly via `GpuContext`
- [ ] `GpuContext::new(device, queue, config)` returns `Result<Self, GpuError>` (handles init failures)
- [ ] Preallocated buffers (geometry, mass, dynamics, expansions, interaction lists, tree nodes, leaf_id, uniform params, error flag) are created at init
- [ ] Buffer round-trip test: write positions to mapped geometry buffer, dispatch pass-through, read back, verify `output[i] == input[i]` within f32 tolerance
- [ ] `galax-cli --gpu --init uniform --n 1000 --steps 1` runs the GPU path and completes without error
- [ ] `leapfrog_kdk_gpu` in `galax-integrate` compiles and calls `GpuContext::step()` per integration step
- [ ] Uniform parameter buffer (`SimParams { n, num_nodes, num_leaves, p, eps }`) is updated and bound before each dispatch
- [ ] Error flag buffer is read after each dispatch; a NaN test (inject NaN position) sets the flag and triggers a panic

## Blocked by

None — can start immediately.
