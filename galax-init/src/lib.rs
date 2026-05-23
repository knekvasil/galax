use galax_core::BodiesSoA;

/// Generate Bodies uniformly distributed in a square `[-size/2, size/2]²`.
pub fn uniform(bodies: &mut BodiesSoA, _n: usize) {
    let n = bodies.len();
    for i in 0..n {
        bodies.x[i] = fastrand::f64() * 100.0 - 50.0;
        bodies.y[i] = fastrand::f64() * 100.0 - 50.0;
        bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
    }
}

/// Generate a Plummer sphere with N bodies, total mass M, and scale radius a.
pub fn plummer(bodies: &mut BodiesSoA, n: usize) {
    let _ = n;
    let mut i = 0;
    while i < bodies.len() {
        // Sample radius from Plummer density profile using inverse transform
        let m = fastrand::f64();
        let r = (m.powf(-2.0 / 3.0) - 1.0).sqrt();

        if r > 10.0 {
            continue;
        }

        let angle = fastrand::f64() * std::f64::consts::TAU;

        bodies.x[i] = r * angle.cos();
        bodies.y[i] = r * angle.sin();
        bodies.mass[i] = 1.0 / bodies.len() as f64;

        // Velocities from isotropic equilibrium distribution.
        // For a Plummer model the 3D velocity dispersion at radius r is
        // σ²(r) = 1 / (2 * sqrt(1 + r²)), and the escape velocity is
        // v_esc² = 2 / sqrt(1 + r²).  The equilibrium distribution has
        // ⟨v²⟩ = σ² = (1/2) / sqrt(1 + r²).
        // The naive v = v_esc * sqrt(u) gives ⟨v²⟩ = 1/sqrt(1+r²) which
        // is 2× too high — scale by 1/√2 to match equilibrium.
        let v_esc = (2.0 / (1.0 + r * r).sqrt()).sqrt();
        let v = v_esc * fastrand::f64().sqrt() / std::f64::consts::SQRT_2;
        let v_angle = fastrand::f64() * std::f64::consts::TAU;
        bodies.vx[i] = v * v_angle.cos();
        bodies.vy[i] = v * v_angle.sin();

        i += 1;
    }
}

/// Generate a galaxy with a massive central body and orbiting test particles.
/// Body 0 is a supermassive central mass (SMBH). Bodies 1..N-1 are test
/// particles in circular orbits with Σ(r) ∝ 1/r surface density.
pub fn galaxy(bodies: &mut BodiesSoA, n: usize) {
    let _ = n;
    let n_bodies = bodies.len();
    if n_bodies == 0 { return; }

    // Central massive body
    bodies.x[0] = 0.0;
    bodies.y[0] = 0.0;
    bodies.vx[0] = 0.0;
    bodies.vy[0] = 0.0;
    bodies.mass[0] = 1000.0; // dominates the potential

    if n_bodies == 1 { return; }

    // Orbiting bodies: surface density Σ(r) ∝ 1/r between r_in and r_out
    // CDF: r^2, so r ∝ sqrt(u) for uniform u
    let r_in = 2.0;
    let r_out = 50.0;
    let g = 1.0;

    for i in 1..n_bodies {
        let u = fastrand::f64();
        let r = (r_in * r_in + u * (r_out * r_out - r_in * r_in)).sqrt();

        let angle = fastrand::f64() * std::f64::consts::TAU;
        bodies.x[i] = r * angle.cos();
        bodies.y[i] = r * angle.sin();
        bodies.mass[i] = 1.0 / n_bodies as f64;

        // Circular velocity around central mass
        let v_circ = (g * bodies.mass[0] / r).sqrt();
        bodies.vx[i] = -v_circ * angle.sin();
        bodies.vy[i] = v_circ * angle.cos();
    }
}

/// Generate a disk galaxy with exponential surface density.
pub fn disk(bodies: &mut BodiesSoA, n: usize) {
    let _ = n;
    let scale_length = 5.0;
    let n_bodies = bodies.len();
    let total_mass = 1.0;

    for i in 0..n_bodies {
        // Sample radius from exponential distribution: ρ(r) ∝ exp(-r/r_d)
        let r = -scale_length * (1.0 - fastrand::f64()).ln();

        let angle = fastrand::f64() * std::f64::consts::TAU;
        bodies.x[i] = r * angle.cos();
        bodies.y[i] = r * angle.sin();
        bodies.mass[i] = 1.0 / n_bodies as f64;

        // Circular velocity from spherical enclosed-mass approximation:
        //   M(<r) = M_total × [1 − (1 + r/r_d) × exp(−r/r_d)]
        //   v_circ² = G × M(<r) / r   (G = 1)
        let x = r / scale_length;
        let enclosed = total_mass * (1.0 - (1.0 + x) * (-x).exp());
        let v_circ = if r > 1e-10 && enclosed > 0.0 {
            (enclosed / r).sqrt()
        } else {
            // Solid-body rotation in the core
            (total_mass / (2.0 * scale_length * scale_length)).sqrt() * r.sqrt()
        };
        // Velocity perpendicular to radius (counter-clockwise)
        let v_angle = angle + std::f64::consts::FRAC_PI_2;
        // Add small velocity dispersion (~5%)
        let disp = fastrand::f64() * 0.05 * v_circ;
        bodies.vx[i] = v_circ * v_angle.cos() + disp * (fastrand::f64() - 0.5);
        bodies.vy[i] = v_circ * v_angle.sin() + disp * (fastrand::f64() - 0.5);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_generates_correct_count() {
        let n = 100;
        let mut bodies = BodiesSoA::new(n);
        uniform(&mut bodies, n);
        assert_eq!(bodies.len(), n);
        for i in 0..n {
            assert!(bodies.x[i].is_finite());
            assert!(bodies.y[i].is_finite());
            assert!(bodies.mass[i] > 0.0);
        }
    }

    #[test]
    fn test_plummer_generates_correct_count() {
        let n = 100;
        let mut bodies = BodiesSoA::new(n);
        plummer(&mut bodies, n);
        assert_eq!(bodies.len(), n);
        for i in 0..n {
            assert!(bodies.x[i].is_finite());
            assert!(bodies.y[i].is_finite());
            assert!(bodies.mass[i] > 0.0);
        }
    }

    #[test]
    fn test_galaxy_generates_correct_count() {
        let n = 100;
        let mut bodies = BodiesSoA::new(n);
        galaxy(&mut bodies, n);
        assert_eq!(bodies.len(), n);
        // Central body should be massive and at origin
        assert!((bodies.x[0]).abs() < 1e-15);
        assert!((bodies.y[0]).abs() < 1e-15);
        assert_eq!(bodies.mass[0], 1000.0);
        // Orbiting bodies should be finite
        for i in 1..n {
            assert!(bodies.x[i].is_finite());
            assert!(bodies.y[i].is_finite());
            assert!(bodies.mass[i] > 0.0);
        }
    }

    #[test]
    fn test_disk_generates_correct_count() {
        let n = 100;
        let mut bodies = BodiesSoA::new(n);
        disk(&mut bodies, n);
        assert_eq!(bodies.len(), n);
        for i in 0..n {
            assert!(bodies.x[i].is_finite());
            assert!(bodies.y[i].is_finite());
            assert!(bodies.mass[i] > 0.0);
        }
    }
}
