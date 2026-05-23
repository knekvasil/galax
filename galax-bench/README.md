# galax-bench

Micro-benchmark suite for the galax CPU FMM pipeline.

Times each FMM operator individually and reports elapsed time and
bodies/second throughput across multiple N values.

## Usage

```bash
cargo run -p galax-bench
# or with custom max N:
cargo run -p galax-bench -- 50000
```

## Benchmarked operations

| # | Operation | Description |
|---|-----------|-------------|
| 1 | Tree build | Morton-sorted quadtree + 2:1 balancing |
| 2 | Interaction lists | M2L and P2P neighbour list construction |
| 3 | P2M | Particle → multipole moments |
| 4 | M2M | Multipole → multipole (upward pass) |
| 5 | M2L | Multipole → local translation |
| 6 | L2L | Local → local (downward pass) |
| 7 | L2P | Local → particle gradient evaluation |
| 8 | P2P | Direct particle → particle (near field) |
| 9 | Full FMM | Pipeline end-to-end (P2M→M2M→M2L→L2L→L2P→P2P) |
| 10 | N² | Direct all-pairs (Rayon parallel) |

## Default N levels

100, 500, 1000, 5000, max_n (default 10000).

## Criterion benchmarks

```bash
cargo bench -p galax-core
```

Runs Criterion benchmarks for `full_fmm` and `n2_force` at N=200
(defined in `galax-core/benches/fmm.rs`).
