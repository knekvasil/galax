# ADR-0004: f32 end-to-end on GPU compute

The GPU compute path for FMM force evaluation uses f32 (single-precision) for all positions, masses, velocities, accelerations, and multipole/local expansion coefficients. There is no mixed-precision or f64-on-GPU scheme.

## Context

On Apple M-series GPUs, f64 arithmetic runs at 1/16th the throughput of f32 (2.6 TFLOPS vs 0.16 TFLOPS on M1). The FMM pipeline is compute-bound in the M2L pass — using f64 would reduce peak throughput by 16× for the dominant arithmetic.

The physical error budget is dominated by three larger terms: FMM truncation error (expandable via p), time integration error (O(Δt²) for leapfrog), and softening-scale physics (collisionless dynamics is inherently approximate). f32 machine epsilon (~1e-7 relative) is 4–6 orders of magnitude below these sources.

## Alternatives considered

**Mixed f32/f64 (f64 for accumulation only).** Introduces cross-format writes and conversions in the hottest pass (M2L + P2P), complicates WGSL type layout, and provides no measurable stability gain given the error hierarchy above.

**f64 everywhere on GPU.** Would require accepting the 16× throughput penalty on M1, limiting practical simulation scale to roughly the same as CPU Rayon — defeating the purpose of GPU acceleration.

## Decision

All GPU-side numerical state is f32. The CPU-side diagnostics (energy, momentum, validation) remain f64 after GPU→CPU readback and conversion. No per-field hybrid precision on GPU.
