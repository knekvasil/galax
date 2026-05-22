Status: ready-for-agent

# GPU validation harness (three-layer)

## Parent

`.scratch/gpu-architecture/PRD.md`

## What to build

Implement the three-layer validation system for the GPU FMM pipeline:

**Layer A (development — `--gpu-crosscheck`):** After each GPU step, run the CPU `compute_fmm_force()` on the same Bodies and tree, compare per-body accelerations against the GPU result, and report max/mean relative error. Panics if error exceeds configurable threshold (default 1e-4 relative). Gated behind `--gpu-crosscheck` flag which implies `--gpu`. The CPU FMM runs in parallel via Rayon on a separate step — this doubles wall-clock time but is only used during development.

**Layer B (diagnostic — N² debug kernel):** An optional WGSL compute shader `n2_debug.wgsl` that computes direct O(N²) softened forces on GPU. One thread per body, inner loop over all other bodies. Dispatched via `GpuContext::n2_debug()` — returns `Vec<(f32, f32)>` of accelerations. Used only during manual debugging sessions (not part of the hot path). Invoked via `galax-cli --gpu-n2-diagnostic`.

**Layer C (production — `--validate` for GPU):** A `validate_fmm_gpu()` function that builds the tree, runs the full GPU FMM, runs a GPU N² on a sampled subset of bodies (up to `--validate-samples`), and reports RMS/max relative error. This is the GPU analogue of the existing CPU `--validate` flag. No CPU involvement. The error tolerance is looser (target < 1% RMS at p=4) — f32 precision is dominated by FMM truncation error.

All three layers share a common reporting format: `max_rel_err`, `mean_rel_err`, `sample_count`, with per-body error diagnostics optionally written to stderr.

## Acceptance criteria

- [ ] `--gpu-crosscheck` flag runs GPU FMM + CPU FMM on the same state and reports max/mean per-body relative error
- [ ] Layer A panics if error exceeds configurable threshold (default 1e-4)
- [ ] `n2_debug.wgsl` shader compiles and produces correct O(N²) forces (verified against CPU N² at N=200)
- [ ] `--gpu-n2-diagnostic` flag runs GPU N² and prints timing and sample of force values
- [ ] `--validate` (without `--gpu`) continues to use CPU FMM/N² as before
- [ ] `--gpu --validate` uses GPU FMM + GPU sampled N² (Layer C)
- [ ] All three layers report in the same format: max error, mean error, sample count
- [ ] Test: `test_crosscheck_layer_a_matches` — GPU FMM matches CPU FMM within 1e-3 relative at N=500

## Blocked by

- `.scratch/gpu-architecture/issues/04-full-fmm-pipeline.md` (full FMM pipeline)
