Status: ready-for-agent

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Implement binary snapshot writer and reader in `galax-io`. Format: header (N, timestamp, version magic) followed by raw bytes of the 7 SoA arrays in fixed order (x, y, vx, vy, mass, ax, ay). Add `--out <path>` for output directory, `--snap-every N` for snapshot frequency, and `--load <path>` to resume from a snapshot. Write round-trip test: BodiesSoA → buffer → read → data matches exactly.

## Acceptance criteria

- [ ] Binary format: header with magic, version, N, timestamp; then 7 arrays of f64
- [ ] Writer produces correct file; reader restores identical BodiesSoA
- [ ] Round-trip test: write → read → every value matches to machine precision
- [ ] `--out runs/run1 --snap-every 10`: snapshots written every 10 steps
- [ ] `--load runs/run1/snap_0050.bin`: resumes simulation from snapshot
- [ ] Graceful error handling: missing file, corrupted header → typed error via thiserror

## Blocked by

- 01-workspace-n2-force.md (needs BodiesSoA)
