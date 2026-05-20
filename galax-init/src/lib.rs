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
        let m = fastrand::f64(); // uniform in [0, 1)
        let r = (m.powf(-2.0 / 3.0) - 1.0).sqrt();

        // Reject if radius exceeds bounds
        if r > 10.0 {
            continue;
        }

        let angle = fastrand::f64() * std::f64::consts::TAU;

        bodies.x[i] = r * angle.cos();
        bodies.y[i] = r * angle.sin();
        bodies.mass[i] = 1.0 / bodies.len() as f64;

        // Velocities from isotropic distribution
        // v² ∝ 1 / sqrt(1 + r²)
        let v_esc = (2.0 / (1.0 + r * r).sqrt()).sqrt();
        let v = v_esc * fastrand::f64().sqrt();
        let v_angle = fastrand::f64() * std::f64::consts::TAU;
        bodies.vx[i] = v * v_angle.cos();
        bodies.vy[i] = v * v_angle.sin();

        i += 1;
    }
}

/// Generate a disk galaxy with exponential surface density.
pub fn disk(bodies: &mut BodiesSoA, n: usize) {
    let _ = n;
    let scale_length = 5.0;
    let n_bodies = bodies.len();

    for i in 0..n_bodies {
        // Sample radius from exponential distribution: ρ(r) ∝ exp(-r/r_d)
        // using inverse CDF: r = -r_d · ln(1 - u)
        let r = -scale_length * (1.0 - fastrand::f64()).ln();

        // Random angle
        let angle = fastrand::f64() * std::f64::consts::TAU;
        bodies.x[i] = r * angle.cos();
        bodies.y[i] = r * angle.sin();
        bodies.mass[i] = 1.0 / n_bodies as f64;

        // Circular velocity for exponential disk (approximate)
        // v_c² = (G · M(r)) / r, where M(r) ≈ total_mass for r >> r_d
        // Simplified: v_c ≈ constant
        let v_c = 1.0;
        // Add some velocity dispersion
        let v_angle = angle + std::f64::consts::FRAC_PI_2;
        let disp = fastrand::f64() * 0.1 * v_c;
        bodies.vx[i] = v_c * v_angle.cos() + disp * (fastrand::f64() - 0.5);
        bodies.vy[i] = v_c * v_angle.sin() + disp * (fastrand::f64() - 0.5);
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
