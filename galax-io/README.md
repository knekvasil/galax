# galax-io

Binary snapshot I/O for galax simulations.

## Format

Simple binary format (no compression, no schema):

```
[magic: "GALX" (4 bytes)]
[version: u32 LE (4 bytes)]
[count: u64 LE (8 bytes)]      — number of bodies
[timestamp: u64 LE (8 bytes)]  — Unix millis
[x: f64[count] (8×count bytes)]
[y: f64[count] (8×count bytes)]
[vx: f64[count] (8×count bytes)]
[vy: f64[count] (8×count bytes)]
[mass: f64[count] (8×count bytes)]
[ax: f64[count] (8×count bytes)]
[ay: f64[count] (8×count bytes)]
```

## Usage

```rust
use galax_io::{write_snapshot, read_snapshot};
use galax_core::BodiesSoA;

let bodies = BodiesSoA::new(1000);
let mut buf: Vec<u8> = Vec::new();
write_snapshot(&mut buf, &bodies)?;

let loaded = read_snapshot(&mut buf.as_slice())?;
println!("loaded {} bodies", loaded.len());
```

## Errors

| Variant | Cause |
|---------|-------|
| `Io` | Filesystem error |
| `BadMagic` | Invalid header magic bytes |
| `UnsupportedVersion` | Version field not recognized |
| `UnexpectedEof` | Truncated data |
