# galax-init

Initial condition generators for N-body simulations.

## Generators

| Function | Description |
|----------|-------------|
| `uniform` | Random positions in `[-50, 50]²`, random masses `[0.1, 10.1]`, zero velocities |
| `plummer` | Plummer sphere (scale radius = 1, truncated at 10), isotropic velocity dispersion, equal masses |
| `disk` | Exponential disk (scale length = 5), circular velocity from spherical enclosed-mass approximation, 5% velocity dispersion, equal masses |
| `galaxy` | Central SMBH (M = 1000) at origin, test particles in `Σ(r) ∝ 1/r` between r = 2 and 50, Keplerian circular velocities, equal masses |

## Usage

All generators fill a pre-allocated `BodiesSoA`:

```rust
use galax_init::{uniform, plummer, disk, galaxy};
use galax_core::BodiesSoA;

let mut bodies = BodiesSoA::new(10000);
uniform(&mut bodies, 10000);
// or: plummer(&mut bodies, 10000);
// or: disk(&mut bodies, 10000);
// or: galaxy(&mut bodies, 10000);
```
