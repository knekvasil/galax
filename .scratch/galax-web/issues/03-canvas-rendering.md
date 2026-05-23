Status: ready-for-agent

## Parent

`.scratch/galax-web/PRD.md`

## What to build

Render Bodies to an HTML Canvas and add camera controls, replacing `minifb` pixel-buffer display with `web-sys::CanvasRenderingContext2d::put_image_data()`.

Components:
- **`renderer.rs`**: `Camera` struct with scroll-zoom + drag-pan (same projection logic as `galax-viz/src/main.rs:render_frame`). `render_frame()` takes Bodies, Camera, and writes to a `Vec<u32>` pixel buffer (BGRA/xRGB). Same Body-to-pixel projection as native.
- **JS side**: `requestAnimationFrame` loop that calls WASM step + render, then pushes pixel buffer to canvas via `putImageData`. Wires `onwheel` → zoom, `onpointerdown/move/up` → drag-pan, forwarding event data to WASM.
- **HUD**: Overlay text on pixel buffer showing step count, Body count, elapsed time, simulation speed (st/s), and zoom level — same format as native viz.
- **Canvas sizing**: fixed 1280×720 for MVP (or match available window size).

## Acceptance criteria

- [ ] Bodies render as grey dots on a dark canvas at correct world-coordinate positions
- [ ] Scroll-wheel zooms in/out anchored to cursor position
- [ ] Left-click drag pans the view
- [ ] HUD displays: step, N, elapsed time, st/s, zoom level
- [ ] `requestAnimationFrame` loop maintains smooth animation (~30-60 fps at 100k Bodies)

## Blocked by

- `.scratch/galax-web/issues/02-gpu-init-simulation.md`
