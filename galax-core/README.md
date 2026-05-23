# galax-core

Core library for 2D gravitational N-body simulations using the Fast Multipole Method.

## Features

- **BodiesSoA** — Structure-of-Arrays container for body positions, velocities, masses, and accelerations
- **Quadtree** — Adaptive Morton-sorted quadtree with 2:1 balancing constraint
- **Fast Multipole Method** — Cartesian multipole expansions with configurable order `p`:
  - P2M (particle → multipole)
  - M2M (multipole → multipole, bottom-up)
  - M2L (multipole → local translation)
  - L2L (local → local, top-down)
  - L2P (local → particle gradient)
  - P2P (direct particle → particle for near neighbours)
- **Direct N²** — Parallel (Rayon) O(N²) softened force for validation
- **Plummer softening** — Kernel `Φ = -1/√(r² + ε²)`, expanded directly in FMM
- **Interaction lists** — Well-separated (M2L) and adjacent-leaf (P2P) neighbour lists
- **Diagnostics** — Kinetic energy, potential energy, momentum, angular momentum
- **Validation** — Sampled N² cross-check with configurable tolerance

## Key Types

| Type | Purpose |
|------|---------|
| `BodiesSoA` | Position, velocity, mass, acceleration arrays |
| `Tree` | Morton-sorted adaptive quadtree |
| `Node` | Cell geometry, hierarchy, body range |
| `InteractionLists` | Per-node neighbour lists (compact storage) |
| `SpatialIndex` | Level-based spatial lookup for tree building |

## Usage

```rust
use galax_core::*;

let mut bodies = BodiesSoA::new(1000);
// fill with initial conditions...
let tree = build_constrained(&mut bodies, 32, -50.0, 50.0, -50.0, 50.0);
let m2l = InteractionLists::build(&tree);
let p2p = build_p2p_lists(&tree);

// GPU FMM force
compute_fmm_force(&mut bodies, &tree, &m2l, &p2p, 4, 0.1);

// Or direct N² (for validation)
compute_n2_force(&mut bodies, 0.1);
```

## Performance

At `p=4`, the FMM achieves ~0.26% RMS force error relative to N². Throughput is
O(N) vs O(N²) for direct summation — the crossover is around 5,000–20,000 bodies
depending on hardware.
