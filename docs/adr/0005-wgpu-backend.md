# ADR-0005: wgpu as GPU backend

The GPU compute path uses wgpu (WebGPU implementation) as the sole GPU backend, not Metal directly, CUDA, Vulkan, or rust-gpu.

## Context

The target hardware is Apple M1 (8 GB unified memory). The codebase is Rust. There are two GPU consumers: galax-gpu (FMM compute) and galax-viz (rendering via minifb). They must share the same wgpu Device and Queue for zero-copy buffer sharing between compute and render passes.

## Alternatives considered

**metal-rs (direct Metal bindings).** Would give the lowest overhead and most control on Apple Silicon. However, it is Apple-only, has a complex retain-count memory model in Rust, and prevents the project from running on non-Apple hardware. The application has a CLI path (galax-cli) that should also work on Linux/NVIDIA hardware for validation.

**rust-gpu (Rust→SPIR-V).** Experimental and unstable. Not ready for production numerical kernels. Would couple WGSL code to SPIR-V which has limited Metal support.

**CUDA via rust-cuda bindings.** Requires NVIDIA hardware. The project's primary and current-only hardware is M1. CUDA is not an option without separate hardware.

**Vulkan compute shaders (ash crate).** Portable to NVIDIA/AMD/Apple (via MoltenVK). Significant API complexity in Rust, requires manual memory management that wgpu handles. No benefit over wgpu for this workload.

## Decision

wgpu provides a Metal backend on Apple Silicon (zero overhead vs raw Metal for what we use), a Vulkan backend for Linux/Windows, and a DX12 backend — all from a single Rust API. The wgpu compute model (storage buffers, compute pipelines, indirect dispatch) maps directly to WGSL's shader model. The application layer (galax-cli, galax-viz) owns the Device and Queue, sharing them between compute and render pipelines.
