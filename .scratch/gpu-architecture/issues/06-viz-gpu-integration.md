Status: ready-for-agent

# galax-viz GPU integration

## Parent

`.scratch/gpu-architecture/PRD.md`

## What to build

Integrate the GPU FMM pipeline into `galax-viz` so that the visualization window can simulate and render 100k+ Bodies using the GPU.

The key architectural decision: `galax-viz` already uses `minifb` for windowing and pixel buffer rendering. It does NOT currently use wgpu for graphics — rendering is CPU-side pixel pushing. The wgpu device is created solely for compute (galax-gpu). The viz gets the wgpu device from galax-gpu, but renders via minifb as before.

The `--gpu` flag in `galax-viz` creates a wgpu `Device` and `Queue`, passes them to `GpuContext::new()`, and runs the simulation using GPU forces. Bodies are read back from the GPU dynamics buffer each frame for rendering via the existing minifb render path. At 100k Bodies, the force compute is GPU but rendering decimation (`--max-render`) is still needed for the pixel-pushing renderer.

The viz retains the existing CPU FMM path as default; `--gpu` switches to the GPU path.

The shared device pattern means `galax-viz` creates the wgpu device, passes it to `GpuContext`, and owns the device lifecycle. This positions the code for a future upgrade to wgpu-based rendering (compute buffer → render buffer → screen) without requiring device migration.

## Acceptance criteria

- [ ] `galax-viz --gpu --init plummer --n 50000` opens a window and simulates at >1 fps (GPU forces + CPU pixel rendering)
- [ ] The wgpu `Device` is created by `galax-viz` and shared with `GpuContext` — no second device
- [ ] Bodies are read back from the GPU dynamics buffer each frame and rendered via the existing minifb buffer
- [ ] `galax-viz` without `--gpu` uses CPU FMM as before (no regression)
- [ ] `--max-render` works on the GPU path (renders subset of bodies for performance)
- [ ] HUD shows "GPU" or "CPU" status in the overlay when `--gpu` is active

## Blocked by

- `.scratch/gpu-architecture/issues/04-full-fmm-pipeline.md` (full FMM pipeline)
