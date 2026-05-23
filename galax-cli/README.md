# galax-cli

Command-line simulation runner for galax.

## Usage

```
cargo run -p galax-cli -- [OPTIONS]
```

### Basic simulation

```bash
# Run with 100k bodies, CPU FMM, Plummer initial conditions
cargo run -p galax-cli -- --n 100000 --init plummer --steps 100

# Run with GPU acceleration
cargo run -p galax-cli -- --n 100000 --gpu --steps 200

# Run direct N² for validation
cargo run -p galax-cli -- --n 2000 --n2-regression
```

### Validation

```bash
# Compare GPU FMM against CPU FMM
cargo run -p galax-cli -- --n 10000 --gpu --gpu-crosscheck

# Convergence study (error vs expansion order)
cargo run -p galax-cli -- --n 10000 --convergence

# Sampled N² cross-check during simulation
cargo run -p galax-cli -- --n 100000 --validate
```

### Benchmarking

```bash
cargo run -p galax-cli -- --bench --n 10000
```

Times individual FMM operators at multiple N values.

### Snapshots

```bash
# Save snapshots every 10 steps
cargo run -p galax-cli -- --n 10000 --steps 1000 --snap-every 10 --out snaps/

# Load and continue from a snapshot
cargo run -p galax-cli -- --load snaps/step_000100.galx --steps 500
```

### Key options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 1000 | Number of bodies |
| `--steps` | 100 | Number of integration steps |
| `--t-end` | — | End time (overrides `--steps`) |
| `--dt` | auto | Timestep (auto from max acceleration) |
| `--softening` | auto | Plummer softening length |
| `--init` | uniform | Initial condition preset |
| `--gpu` | — | Use GPU FMM backend |
| `--out` | — | Snapshot output directory |
| `--progress` | — | Show progress bar |
