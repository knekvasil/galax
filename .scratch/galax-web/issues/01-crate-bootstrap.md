Status: ready-for-agent

## Parent

`.scratch/galax-web/PRD.md`

## What to build

Create the `galax-wasm` crate as a new workspace member and prepare `galax-gpu` for cross-compilation to `wasm32-unknown-unknown`.

The crate should:
- Declare `crate-type = ["cdylib"]` with `wasm-bindgen` as a dependency
- Depend on `galax-core`, `galax-gpu`, `galax-integrate`, `galax-init` via path deps
- NOT depend on `galax-cli`, `galax-viz`, `galax-io`, or `galax-bench`

In `galax-gpu/Cargo.toml`, gate `pollster` behind `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]`. In `galax-gpu/src/lib.rs`:

- Add `#[cfg(not(target_arch = "wasm32"))]` to the `create_gpu_context` test helper and any other test-only code that uses `pollster::block_on`
- The `#[cfg(test)] mod tests` block itself is fine — individual test functions that use `create_gpu_context` are gated by it being absent

Add a minimal `lib.rs` stub with a `#[wasm_bindgen]` placeholder function so the build can be verified end-to-end.

## Acceptance criteria

- [ ] `cargo build --target wasm32-unknown-unknown -p galax-wasm` succeeds
- [ ] `cargo build -p galax-gpu` still succeeds on native (tests excluded)
- [ ] `cargo test -p galax-gpu` still passes on native
- [ ] The workspace `Cargo.toml` lists `galax-wasm` in `members`

## Blocked by

None - can start immediately
