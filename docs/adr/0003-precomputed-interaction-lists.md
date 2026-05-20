# ADR-0003: Precomputed fixed-size interaction lists

The M2L (far-field) and P2P (near-field) passes of the FMM need to know which cells interact. Two approaches exist: compute interaction lists on-the-fly via dual-tree traversal each time step, or precompute them once after tree construction.

Interaction lists are precomputed during tree build using the standard parent-neighbor expansion rule: for each node at level L, examine the children of the 3×3 neighbor block of its parent at level L−1; non-adjacent children become M2L partners. The near-field list for each leaf is computed from adjacent leaf cells. Storage uses SoA-layout fixed-size arrays: `[[u32; 64]; num_nodes]` for partner indices and `[u8; num_nodes]` for lengths. The 64-entry capacity covers worst-case quadtree node neighbor counts under pathological clustering without branching or heap fallback.

Precomputation eliminates redundant tree traversal at every time step. Fixed-size arrays avoid per-node heap allocations, ensure cache-friendly sequential access during the hot M2L pass, and keep the design portable to GPU where dynamic allocation is unavailable.
