Status: completed

## Parent

[fmm-nbody PRD](../PRD.md)

## What to build

Implement binary snapshot writer and reader in `galax-io`. Format: header (magic b"GALX", version u32, N u64, timestamp f64) followed by raw bytes of the 7 SoA arrays. Add `--out`, `--snap-every`, and `--load` CLI flags.

## Acceptance criteria

- [x] Binary format: header with magic, version, N, timestamp; then 7 arrays of f64
- [x] Writer produces correct file; reader restores identical BodiesSoA
- [x] Round-trip test: write → read → every value matches to machine precision
- [x] `--out runs/run1 --snap-every 10`: snapshots written every 10 steps
- [x] `--load runs/run1/snap_0050.bin`: resumes simulation from snapshot
- [x] Graceful error handling: missing file, corrupted header → typed error via SnapshotError (thiserror)

## Completion notes

- SnapshotError enum with Io, BadMagic, UnsupportedVersion variants
- write_snapshot / read_snapshot take impl Write / impl Read for testability
- File I/O via std::fs::File in CLI layer
- 2 unit tests: round-trip, bad magic detection
- CLI --out, --load flags integrated

## Blocked by

- 01-workspace-n2-force.md (needs BodiesSoA)
