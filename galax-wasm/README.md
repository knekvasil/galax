# galax-wasm

WebAssembly build target for running galax in the browser.

Compiles the simulation engine + WGSL shaders to `wasm32-unknown-unknown`
and provides a JS-interop API via `wasm-bindgen`. Renders bodies to an
HTML Canvas via `putImageData`. No server backend is needed — the browser
ships only the `.wasm` binary and a thin JS shell.

## Building

```bash
cd galax-wasm
./deploy.sh
# Output in pkg/ — serve with any static file server
npx serve pkg/
```

## Architecture

```
galax-wasm/
├── Cargo.toml       # cdylib, wasm-bindgen deps
├── src/
│   ├── lib.rs       # #[wasm_bindgen(start)] entry, panic hook
│   ├── simulation.rs # SimState, GPU init, KDK step, resize
│   └── renderer.rs  # Camera, CanvasRenderingContext2d → putImageData
├── www/
│   ├── index.html   # Full-screen canvas, glass control bar
│   └── style.css    # Dark theme, responsive
└── deploy.sh        # wasm-pack build + static file copy
```

## JS API

```javascript
import init, { create_sim_state } from './galax_wasm.js';
await init();

let sim = await create_sim_state(10000, 'uniform');

// In requestAnimationFrame:
await sim.step();
sim.render(ctx, width, height, elapsed, stepsPerSec);
```

## GPU backend

Uses `wgpu` with `webgpu` feature for WebGPU compute. On devices that don't
meet the 10-storage-buffer requirement, silently falls back to CPU FMM.
Bind group layout uses 9 storage buffers (down from the original 15) by
combining positions+masses, indices+offsets, and leaf metadata into
packed `vec4` buffers.

## Deployment

Deploy the `pkg/` directory to any static host:

```bash
npx surge pkg/ galax-nbody.surge.sh
```
