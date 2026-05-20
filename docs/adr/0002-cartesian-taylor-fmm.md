# ADR-0002: Cartesian Taylor multipole expansions (not complex Laurent)

The gravitational potential in 2D is `Φ = 1/r` (a 3D interaction confined to a plane), which is NOT a harmonic function of the 2D Laplacian. The well-known complex Laurent FMM (Greengard–Rokhlin) requires a harmonic kernel and applies only to the 2D Laplace equation `∇²Φ = 0` (log r potential). Using it for `1/r` gravity would produce incorrect results.

All forces use Plummer-softened kernel `Φ(r) = -1 / sqrt(r² + ε²)` and `F(r) = m₁ m₂ · r / (r² + ε²)^(3/2)`. The FMM expands this softened kernel directly — not as a post-correction to the unsoftened `1/r` form. The same kernel is used for the N² validation baseline.

The FMM uses full symmetric Cartesian tensor expansions for both multipole moments and local expansions of the softened kernel. All partial derivatives `∂ˣⁱ ∂ʸʲ Φ` for `i + j ≤ p` are tracked — 45 coefficients at p=8. The expansion coefficients are stored as flat `Vec<f64>` arrays indexed by node ID (not inside node structs), consistent with the SoA design.

This is the standard formulation for non-harmonic kernels in Cartesian FMM.
