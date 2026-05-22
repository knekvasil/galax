# ADR-0006: Body buffer layout by access domain

GPU body data is split into three buffer groups by kernel access pattern — geometry (vec2 positions), mass (f32 masses), dynamics (vec2 velocities + vec2 accelerations) — rather than a single canonical representation.

## Context

The CPU model stores all body state as seven separate `Vec<f64>` arrays (x, y, vx, vy, mass, ax, ay) — a pure SoA layout that works well for CPU cache and SIMD. On GPU, different FMM passes access different subsets of body fields:

- M2L and L2P read only positions
- P2P reads positions and mass
- Integration (KDK, CPU-side) reads positions, velocities, and accelerations
- Diagnostics read accelerations

A single SoA layout with seven independent buffers means every kernel fetches all buffers through address arithmetic, wasting bandwidth on fields it never reads. A single AoS layout (interleaved struct) over-fetches — dragging velocity and mass through L1 cache for M2L which wants only positions.

## Alternatives considered

**Seven separate f32 buffers (GPU SoA).** Causes excessive buffer indirection and descriptor pressure for kernels that need only a few fields. Each binding slot consumed per buffer increases pipeline complexity.

**Interleaved body struct (AoS).** Every position read pulls velocity, mass, and acceleration through the cache. For M2L (the dominant pass), 5/7 of every cache line is wasted.

## Decision

Three buffer groups matching kernel access coherence domains:

| Domain | Fields | GPU type | Consumers |
|--------|--------|----------|-----------|
| Geometry | positions | `array<vec2<f32>>` | M2L, P2P, L2P, N² |
| Mass | masses | `array<f32>` | P2P, P2M, N² |
| Dynamics | velocities + accelerations | `array<vec2<f32>>` × 2 | Integration (CPU), L2P (accel write), diagnostics (CPU readback) |

Each kernel binds only the domains it needs. This minimizes bandwidth, reduces descriptor pressure, and aligns with the GPU design principle that layout follows access coherence, not logical entity structure.
