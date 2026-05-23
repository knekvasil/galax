Status: ready-for-agent

# PRD: WASM + WebGPU deployment for galax n-body simulation

## Problem Statement

galax is a high-performance 2D gravitational N-body simulation using the Fast Multipole Method, built in Rust with wgpu GPU compute. It currently runs only as a native binary (galax-cli, galax-viz). There is no way to share the simulation as an interactive web demo — a significant gap for a visually compelling project that would benefit from a shareable URL.

A naive WASM port would degrade to CPU-only performance (losing the wgpu GPU compute path) or require a costly server-side rendering architecture. Neither is acceptable: the core value of galax is that it runs the entire FMM pipeline on the GPU at 100k Body scale, and a web port must preserve that.

## Solution

A new `galax-wasm` crate that compiles the existing simulation engine + WGSL shaders to WASM (`wasm32-unknown-unknown`), reuses the exact same `galax-gpu` wgpu compute pipeline in-browser via WebGPU, and renders Bodies to an HTML Canvas via `putImageData`. The browser ships only the `.wasm` binary and a thin JS shell — no server backend, no video streaming, no backend compute.

The same WGSL shaders, the same `GpuContext::step()` dispatch, the same `leapfrog_kdk_gpu` integration loop. Only the rendering layer and the GPU init path change.

## User Stories

1. As a visitor, I want to open a URL and see a live n-body simulation running in my browser, so that I can explore the project without installing anything.

2. As a visitor, I want the simulation to use my GPU (WebGPU), so that it runs at interactive frame rates with 100k+ Bodies — not stuck on CPU.

3. As a visitor, I want to pan and zoom the camera with mouse/trackpad, so that I can inspect different regions of the simulation.

4. As a visitor, I want to see a HUD showing step count, Body count, elapsed time, and simulation speed, so that I understand the simulation state.

5. As a visitor, I want to choose the initial condition preset (uniform, Plummer sphere, disk galaxy) from a UI control, so that I can compare different dynamical scenarios.

6. As a visitor, I want to adjust the number of Bodies via a slider, so that I can trade performance for scale on my device.

7. As a visitor, I want the simulation to auto-detect WebGPU availability and show a fallback message if unsupported, so that I understand why it won't load on my browser.

8. As a visitor, I want the page to be performant on a mid-range laptop (2020+ MacBook, modern Windows laptop), so that the simulation feels responsive.

9. As a developer, I want every WGSL shader change to be automatically reflected in the WASM build, so that I don't maintain two shader source trees.

10. As a developer, I want to deploy by pushing static files to a CDN (no server, no Docker), so that the deployment cost is zero and maintenance is minimal.

11. As a developer, I want the WASM crate to share dependencies with the existing workspace via path deps, so that changes to galax-core or galax-gpu don't drift.

12. As a maintainer, I want the web build CI to compile on `wasm32-unknown-unknown` and run a headless smoke test, so that regressions are caught before deployment.

13. As a contributor, I want the codebase's domain vocabulary (Body, Gravitational Interaction, Plummer softening, Quadtree Cell, Cartesian Multipole Expansion) to carry into the web rendering code, so that the mental model is consistent across native and web.

## Implementation Decisions

### New crate: `galax-wasm`

A new workspace member with `crate-type = ["cdylib"]` for `wasm-bindgen`. Depends on `galax-core`, `galax-gpu`, `galax-integrate`, `galax-init` via path. Does NOT depend on `galax-cli`, `galax-viz`, `galax-io`, or `galax-bench`.

```
/galax-wasm/
├── Cargo.toml
├── src/
│   ├── lib.rs           # #[wasm_bindgen] entry points
│   ├── simulation.rs    # SimState: owns BodiesSoA, GpuContext, Tree, loop
│   └── renderer.rs      # Camera, pixel buffer → Canvas putImageData
└── www/
    ├── index.html       # Full-page canvas + overlay controls
    ├── index.js         # Orchestrates WASM init, UI bindings, requestAnimationFrame
    └── style.css        # Dark theme, control bar styling
```

### GPU init for WASM

`GpuContext::new` already accepts an `Arc<wgpu::Device>` and `Arc<wgpu::Queue>` — it does not own adapter discovery. The WASM init path will:

1. Call `navigator.gpu.requestAdapter()` from JS
2. Pass the adapter to WASM via `wasm-bindgen` (or discover entirely in WASM via `wgpu::Instance::request_adapter` which works on wasm32)
3. Create `device` + `queue` via `adapter.requestDevice()`
4. Pass both into `GpuContext::new`

No `pollster::block_on` — use `wasm-bindgen-futures::spawn_local` and callback-based initialization. The existing `galax-gpu` tests use `pollster` (dev-dependency only); WASM build will not compile those tests.

### Rendering

Replaces `minifb` pixel-buffer display with `web-sys::CanvasRenderingContext2d::put_image_data()`. The pixel buffer remains a `Vec<u32>` (BGRA/xRGB). The projection logic (world coords → screen coords) is identical to `galax-viz/src/main.rs:render_frame`.

### Camera

Reimplements the same scroll-zoom + drag-pan from `galax-viz`, wired to mouse wheel `onwheel` and pointer `onmousedown`/`onmousemove`/`onmouseup` events in JS, forwarded to WASM.

### UI controls

Implement in HTML/CSS/JS, not in WASM. JS reads control values and calls `#[wasm_bindgen]` functions on the WASM side to reconfigure:

- Body count slider (triggers re-init)
- Init preset dropdown (uniform / plummer / disk / galaxy)
- Pause/resume button
- Quality adjustment (p expansion order or max-render cap)
- Monitor info display (step, N, FPS, st/s)

### Build tooling

- `wasm-pack build --target web` for development
- `wasm-pack build --target web --release` for deployment
- Output `pkg/` directory: `.wasm` binary, JS glue, type declarations (`.d.ts`)
- Static files (`www/`) copied alongside `pkg/` for deployment
- Static host: Cloudflare Pages, Vercel, or GitHub Pages (no server needed)

### Integration with galax-gpu

`galax-gpu` will need a minor `Cargo.toml` change: conditionally compile `#[cfg(test)]` only modules behind `cfg_attr`. Specifically:

- `pollster` becomes `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]`
- No `include!` changes needed — WGSL shaders already embed via `include_str!`
- The `create_gpu_context` test helper uses `pollster::block_on` — gate it behind `#[cfg(not(target_arch = "wasm32"))]`

### Threading

No changes. The GPU compute pipeline is single-threaded (one command encoder, sequential dispatch). Host-side per-frame work (reading accelerations, stepping integrator) is single-threaded WASM. No rayon, no SharedArrayBuffer.

### Error handling

- Missing WebGPU: detect in JS before loading WASM, show a banner
- GPU device lost: currently panics (same behavior as native); could add resilience later
- NaN in GPU kernel: `error_flag_buffer` check in `GpuContext::step` panics in WASM the same as native — acceptable

### Domain vocabulary adherence

Uses existing CONTEXT.md terms. The rendering code refers to Bodies (not particles/stars), Gravitational Interaction, Plummer softening, Quadtree Cell where applicable. Camera and HUD code is not domain-loaded and uses generic terms.

### Performance expectations

- 100k Bodies on a base M1 Mac: 30-60 fps (vs ~60 fps native). The ~5-15% wgpu WASM overhead and CPU-side f64→f32 conversions in the step loop are the main costs.
- Bottleneck shifts from GPU compute to CPU-side per-frame work (integration, render). This is acceptable — GPU still does the heavy FMM compute.
- Mobile GPU (recent phone): 10-20k Bodies at 30 fps is a reasonable expectation.
- Auto-downgrade: future enhancement — for now, the user adjusts N manually.

### Out of scope for this crate (may be future issues)

- File snapshot load/save (galax-io depends on std::fs)
- Keyboard shortcuts beyond Escape
- Touch-input mobile camera controls
- Resizable canvas (fixed 1280×720 for MVP, or match window size)
- In-browser benchmark/validation suite
- Safari support (requires WebGPU fallback or WebGL compute — explicitly deferred)
- Performance auto-tuning (body count / theta / p adaptation)
- Multiple simultaneous simulation instances

## Testing Decisions

A good test verifies external behavior — the WASM crate is tested primarily by its dependencies (galax-core, galax-gpu, galax-integrate already have extensive tests). The WASM layer itself is thin orchestration.

### galax-wasm

- **Headless smoke test**: Instantiate SimState in a headless WASM context, call `step()` for 5 iterations, verify accelerations are finite and non-zero. Run via `wasm-bindgen-test` (runs in a headless browser or Node with `--target nodejs`).

### galax-gpu (modified)

- Existing `test_gpu_full_fmm_vs_cpu_fmm` and `test_crosscheck_layer_a_matches` remain unchanged on native.
- No WASM-specific GPU tests (headless WebGPU is not widely available in CI).

### galax-integrate

- No changes needed. Tests remain native-only.

### Prior art

- Existing test patterns in `galax-core`: unit tests verify math invariants; integration tests run small N full FMM vs N²; regression tests verify physical stability over many steps.
- `galax-gpu/tests`: `test_gpu_full_fmm_vs_cpu_fmm` is the model — run GPU FMM, compare against CPU FMM, assert RMS error < threshold.
- Follow same pattern: new tests verify WASM-orchestrated simulation against CPU FMM reference, not against native binary behavior.

## Out of Scope

- Native-to-WASM performance parity (some overhead is inherent; 2× is acceptable)
- File I/O and snapshot loading in-browser
- Safari / WebGL fallback
- Server-side rendering or cloud streaming
- Mobile touch controls
- Multi-window or multi-instance
- PWA / offline support
- Advanced FMM visualization (potential field overlay, particle trails)
- Embedding galax in third-party websites via npm package

## Further Notes

- ADR-0005 (wgpu as GPU backend) already chose wgpu for cross-platform portability — this WASM path is a natural extension of that decision, not a departure.
- ADR-0004 (f32 GPU compute) stands unchanged: the same f32 GPU kernels run in-browser.
- The `www/` directory is deliberately simplistic — it's a demo page, not a full web app. No framework, no bundler beyond `wasm-pack`.
- `wasm-pack` outputs a native ES module — importable directly from `<script type="module">`.
- No `rayon` in WASM. If CPU-side parallelism becomes a bottleneck in the future, `wasm-bindgen-rayon` + `SharedArrayBuffer` is an option (requires COOP/COEP headers).
- The `target/wasm32-unknown-unknown` compilation should be checked in CI, but full GPU-dispatch tests require a WebGPU-capable headless browser (not yet widely available in GitHub Actions).
