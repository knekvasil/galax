Status: completed

## Parent

[scale-benchmark PRD](../PRD.md)

## What to build

Real-time visualization crate using minifb.

## Acceptance criteria

- [x] `galax-viz` crate exists, builds with `cargo build`
- [x] Reads snapshot file via galax-io and renders Bodies as colored dots
- [x] Color coding by velocity magnitude (blue=slow → red=fast)
- [x] Window closes on ESC
- [x] Decimates rendering for large N (renders 1 in every N/10000 bodies)

## Completion notes

- Uses minifb (lightweight, cross-platform, no GPU dependency)
- 800×600 window
- No physics logic — pure rendering
- Designed for debug/demo use

## Blocked by

None — can start immediately.
