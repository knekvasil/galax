Status: ready-for-agent

## Parent

`.scratch/galax-web/PRD.md`

## What to build

Polish the web demo into a complete static site and set up CI for WASM builds + smoke tests.

Static site structure (`galax-wasm/www/`):
- `index.html`: full-page canvas with overlay controls, WebGPU detection script before loading WASM
- `style.css`: dark theme, control bar styling, responsive layout
- `index.js`: orchestrates WASM init, WebGPU availability check, UI bindings, `requestAnimationFrame` loop — production-grade (error handling, loading state)

WebGPU detection: before loading the WASM module, check `navigator.gpu`. If absent, show a banner/message explaining the browser doesn't support WebGPU and suggesting Chrome/Edge/Firefox Nightly. Do not attempt to load WASM.

Build tooling:
- `wasm-pack build --target web --release` produces `pkg/` with `.wasm` binary, JS glue, `.d.ts`
- Static files from `www/` are copied alongside `pkg/` for deployment
- Output is a standalone static directory ready to serve from any static host (Cloudflare Pages, Vercel, GitHub Pages)

CI (GitHub Actions):
- On push/PR: `cargo build --target wasm32-unknown-unknown -p galax-wasm` as a compilation check
- `wasm-pack test --node` to run the headless smoke test
- Not blocking: full GPU-dispatch tests (headless WebGPU not widely available in CI)

## Acceptance criteria

- [ ] `wasm-pack build --target web --release` produces a deployable `pkg/` directory
- [ ] `www/` files copied alongside `pkg/` produce a working static site when served
- [ ] Opening the site in a browser without WebGPU shows a fallback message (no crash, no blank page)
- [ ] Opening the site in a WebGPU-capable browser loads and runs the simulation
- [ ] CI compiles `wasm32-unknown-unknown` on every push/PR
- [ ] CI runs the headless smoke test via `wasm-bundle-test`
- [ ] Static site can be deployed to Cloudflare Pages / Vercel / GitHub Pages with zero server config

## Blocked by

- `.scratch/galax-web/issues/04-ui-controls.md`
