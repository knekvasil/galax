# galax-integrate

Time integration for N-body simulations using the Leapfrog (KDK) symplectic integrator.

Three backends are provided, all producing identical kick-drift-kick steps:

| Function | Force backend | Use case |
|----------|---------------|----------|
| `leapfrog_kdk` | CPU FMM | General simulation |
| `leapfrog_kdk_n2` | CPU direct N² | Testing / validation |
| `leapfrog_kdk_gpu` | GPU FMM (wgpu) | Performance |

## Simulation runners

Each `simulate_*` function runs a multi-step simulation and returns
`Vec<Diagnostics>` with energy, momentum, and angular momentum snapshots:

- `simulate` — CPU FMM with diagnostics
- `simulate_n2` — Direct N² with diagnostics
- `simulate_gpu` — GPU FMM with diagnostics (uploads tree data once)

## Diagnostics

```rust
pub struct Diagnostics {
    pub step: u64,
    pub kinetic: f64,
    pub potential: f64,
    pub momentum_x: f64,
    pub momentum_y: f64,
    pub angular_momentum: f64,
}
```

## Leapfrog integrator

The KDK (Kick-Drift-Kick) scheme is second-order symplectic:

```
v ← v + a × dt/2     (kick)
x ← x + v × dt       (drift)
a ← compute_forces() (force eval)
v ← v + a × dt/2     (kick)
```
