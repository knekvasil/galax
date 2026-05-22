Status: ready-for-agent

# P2P near-field GPU kernel

## Parent

`.scratch/gpu-architecture/PRD.md`

## What to build

Implement the P2P (Particle-to-Particle) pass as a WGSL compute shader, the CPU-side interaction list compaction that converts the CPU `InteractionLists` (`[u32; 64]` per node) into a compact GPU offset-based format (contiguous `indices: array<u32>` + per-node `offsets: array<u32>`), and the per-body `leaf_id: array<u32>` buffer.

The P2P shader uses one thread per Body. Each thread reads its owning leaf via `leaf_id[gid]`, retrieves the leaf's P2P partner list from the compact offset arrays, loops over the body range of each partner leaf, and accumulates the direct softened `1/r` force into the thread's own acceleration slot. No atomics — each thread writes only to its own Body's acceleration.

The compaction algorithm on the CPU side iterates over the `InteractionLists`, flattens the fixed-size arrays into a contiguous index buffer, and builds a prefix-sum offset array. This runs once when interaction lists are built (which is once per simulation for the static tree model).

Validate by running the GPU P2P shader on a small simulation (N=500), reading back accelerations, and comparing against the CPU `p2p()` function. Max relative error should be within f32 accumulation tolerance (< 1e-4 relative).

## Acceptance criteria

- [ ] `cpu_interaction_lists_to_gpu()` compacts `InteractionLists` into `(indices: Vec<u32>, offsets: Vec<u32>)` for both M2L and P2P lists
- [ ] Compact layout matches sequential CPU iteration: `for off in offsets[node]..offsets[node+1] { let partner = indices[off]; ... }` gives the same partner sequence as looping the CPU fixed-size array
- [ ] P2P WGSL shader reads geometry buffer (positions), mass buffer, leaf_id buffer, and P2P offset/indices buffers; writes to dynamics buffer (accelerations)
- [ ] GPU P2P acceleration for N=500 matches CPU P2P acceleration with max relative error < 1e-4
- [ ] Test passes: `test_gpu_p2p_vs_cpu_p2p` in galax-gpu tests
- [ ] Performance at N=5000 is measured and reported (no target, just a baseline)

## Blocked by

- `.scratch/gpu-architecture/issues/01-gpu-bootstrap.md` (GPU bootstrapping)
