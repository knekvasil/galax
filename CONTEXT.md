# Galax

A 2D gravitational N-body simulation using the Fast Multipole Method.

## Language

**Body**:
A point mass with position (x, y), mass, and velocity.
_Avoid_: Particle, star, point-mass

**Gravitational Interaction**:
The pairwise force between two Bodies, proportional to the product of their masses and inversely proportional to the square of the distance between them. The potential is `1/r`, NOT harmonic in 2D — so the FMM uses Cartesian Taylor expansions, not complex Laurent series.
_Avoid_: Treating the kernel as 2D Laplace (log r) or using complex Laurent FMM

**Softening (Plummer softening)**:
A modification to the gravitational force law. The softened potential is `Φ(r) = -1 / sqrt(r² + ε²)` and the force magnitude is `F = r / (r² + ε²)^(3/2)`. The FMM expands this softened kernel directly — not the unsoftened `1/r`. The same kernel is used for both N² validation and FMM.
_Avoid_: No softening, ε=0, applying softening as a post-correction to unsoftened `1/r`

**Quadtree Cell**:
A node in the spatial decomposition tree built from Morton-sorted bodies via prefix-based hierarchical grouping. Stores only geometry (center, half-width), mass properties (total mass, center of mass), hierarchy (child indices, leaf flag), and a Morton-contiguous body range. Multipole and local expansion coefficients are stored externally, indexed by node ID.
_Avoid_: Storing FMM coefficients in the node struct

**Morton Key**:
A spatial hash encoding a 2D position into a single integer by interleaving the bits of the x and y coordinates. Sorting bodies by Morton key produces a Z-order curve, ensuring spatially close points are contiguous in memory.

**Cartesian Multipole Expansion**:
A Taylor expansion of the gravitational potential field in Cartesian derivatives `∂ˣⁱ∂ʸʲΦ`. For order p, stores all terms `i+j ≤ p` — 45 coefficients for p=8. Used for both multipole moments and local expansions in the FMM tree. Expansions are stored as flat `Vec<f64>` arrays indexed by node ID, not inside node structs.

## Relationships

- A **Gravitational Interaction** involves exactly two **Bodies**
- **Softening** modifies every **Gravitational Interaction**

## Example dialogue

> **Dev:** "When two **Bodies** pass close together, do we compute their **Gravitational Interaction** directly or via the multipole tree?"
> **Domain expert:** "Directly — the FMM uses direct computation for nearby pairs and multipole approximations for faraway clusters."

> **Dev:** "Is this the 2D Laplace FMM with complex numbers?"
> **Domain expert:** "No — the gravitational kernel is 1/r, not log(r). The FMM uses Cartesian tensor expansions, not complex Laurent series."

## Flagged ambiguities

- "validation" was used for both correctness and performance — resolved: unit tests verify exact math; integration tests verify decomposition correctness; regression tests verify physical stability; benchmarks measure performance. Never mixed in the same test.

- "panic is ok" vs "panic is forbidden" — resolved: panic allowed in init/validation/post-kernel checks; forbidden in hot numerical kernels (P2P, M2L, M2M, L2L, L2P). NaN detection happens only at stage boundaries.

- "validation" used to mean both per-body accuracy and energy drift — resolved: these test different failure modes and are not interchangeable

- "interaction list" was used loosely — resolved: M2L uses well-separated parent-neighbor expansion; P2P uses adjacent leaf bodies; these are separate lists with separate computation rules

- "dt" was ambiguous — resolved: user provides an optional cap; internal physics stability bound computes dt_max; actual dt = min(dt_user, 0.5 × dt_max)

- "energy computation" was ambiguous — resolved: two distinct notions coexist. Runtime diagnostic energy (FMM-consistent, cheap, continuous monitoring) lives in galax-core. Validation energy (sampled N², physics-accurate) is gated behind --validate.
