Status: ready-for-agent

## Parent

`.scratch/galax-web/PRD.md`

## What to build

Implement the core simulation loop for WASM: `SimState` struct that owns `BodiesSoA`, `GpuContext`, `Tree`, and interaction lists, and exposes `#[wasm_bindgen]` entry points.

Key design:
- `GpuContext::new` already accepts `Arc<wgpu::Device>` + `Arc<wgpu::Queue>` — the WASM init path discovers the adapter in WASM via `wgpu::Instance::request_adapter` (which works on `wasm32`) or receives adapter/device/queue from JS, then passes them in
- Init is async: use `wasm-bindgen-futures::spawn_local` — no `pollster::block_on`
- `SimState::new(n, init_preset)` creates Bodies with the chosen initial condition generator, builds the tree, computes interaction lists, creates `GpuContext`, uploads tree data
- `SimState::step()` calls `leapfrog_kdk_gpu` (one full KDK leapfrog step with GPU FMM force eval), returns the updated `BodiesSoA`
- `SimState::resize(n, init_preset)` re-initialises with new body count

Also include a headless smoke test using `wasm-bindgen-test`: instantiate `SimState`, call `step()` for 5 iterations, verify all accelerations are finite and non-zero.

## Acceptance criteria

- [ ] `wasm-pack test --node -- --test smoke` passes (headless smoke test)
- [ ] `SimState::new(1000, "plummer")` succeeds in WASM without panicking
- [ ] `SimState::step()` returns finite accelerations for all Bodies
- [ ] `SimState::resize(5000, "disk")` re-initialises correctly

## Blocked by

- `.scratch/galax-web/issues/01-crate-bootstrap.md`
