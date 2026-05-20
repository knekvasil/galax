#[cfg(test)]
mod tests {
    use super::*;
    use galax_core::{
        build_p2p_lists, compute_kinetic_energy, compute_momentum, compute_angular_momentum, Tree,
    };

    #[test]
    fn test_single_body_drift() {
        let mut bodies = BodiesSoA::new(1);
        bodies.x[0] = 0.0;
        bodies.y[0] = 0.0;
        bodies.vx[0] = 10.0;
        bodies.vy[0] = 5.0;
        bodies.mass[0] = 1.0;
        bodies.ax[0] = 0.0;
        bodies.ay[0] = 0.0;

        let tree = Tree::build(&mut bodies, 32, -100.0, 100.0, -100.0, 100.0);
        let m2l = InteractionLists::build(&tree);
        let p2p = build_p2p_lists(&tree);

        let dt = 0.1;
        leapfrog_kdk(&mut bodies, &tree, &m2l, &p2p, 2, 0.1, dt);

        assert_eq!(bodies.vx[0], 10.0, "velocity x should be unchanged");
        assert_eq!(bodies.vy[0], 5.0, "velocity y should be unchanged");
        assert!((bodies.x[0] - 10.0 * dt).abs() < 1e-14, "position x drift: got {}", bodies.x[0]);
        assert!((bodies.y[0] - 5.0 * dt).abs() < 1e-14, "position y drift: got {}", bodies.y[0]);
    }

    #[test]
    fn test_energy_diagnostics_known_values() {
        let mut bodies = BodiesSoA::new(2);
        bodies.x = vec![0.0, 3.0];
        bodies.y = vec![0.0, 0.0];
        bodies.vx = vec![2.0, -1.0];
        bodies.vy = vec![1.0, 2.0];
        bodies.mass = vec![5.0, 3.0];
        bodies.ax = vec![0.5, -0.3];
        bodies.ay = vec![0.0, 0.1];

        let ke = compute_kinetic_energy(&bodies);
        let expected_ke = 0.5 * (5.0 * (4.0 + 1.0) + 3.0 * (1.0 + 4.0));
        assert!((ke - expected_ke).abs() < 1e-14, "KE: expected {}, got {}", expected_ke, ke);

        let (px, py) = compute_momentum(&bodies);
        assert!((px - (5.0 * 2.0 + 3.0 * (-1.0))).abs() < 1e-14);
        assert!((py - (5.0 * 1.0 + 3.0 * 2.0)).abs() < 1e-14);

        let lz = compute_angular_momentum(&bodies);
        let expected_lz = 5.0 * (0.0 * 1.0 - 0.0 * 2.0) + 3.0 * (3.0 * 2.0 - 0.0 * (-1.0));
        assert!((lz - expected_lz).abs() < 1e-14, "Lz: expected {}, got {}", expected_lz, lz);
    }

    #[test]
    fn test_kepler_one_orbit() {
        // Two bodies in circular orbit: verify energy and period over 1 orbit
        let mut bodies = BodiesSoA::new(2);
        let a = 10.0; // separation
        let m = 1.0;  // each body mass
        let G = 1.0;
        let softening = 1e-8;

        // Center of mass at origin
        bodies.x = vec![-a / 2.0, a / 2.0];
        bodies.y = vec![0.0, 0.0];
        bodies.mass = vec![m, m];

        // Circular orbital velocity
        let v_circ = (G * (m + m) / a).sqrt();
        let v1 = v_circ * m / (m + m); // v = v_circ * m_other / M
        let v2 = v_circ * m / (m + m);
        bodies.vx = vec![0.0, 0.0];
        bodies.vy = vec![v1, -v2];

        // Compute initial total energy
        let init_ke = compute_kinetic_energy(&bodies);
        let init_pe = -G * m * m / a;
        let init_E = init_ke + init_pe;

        // Kepler period
        let T = 2.0 * std::f64::consts::PI * (a * a * a / (G * (m + m))).sqrt();

        // Run for 1 orbit with small dt
        let n_steps = 500;
        let dt = T / n_steps as f64;
        let diags = simulate_n2(&mut bodies, softening, dt, n_steps, n_steps);

        // Energy should be conserved
        assert!(!diags.is_empty(), "should at least have final diagnostics");
        let final_E = diags.last().unwrap().kinetic + diags.last().unwrap().potential;
        let drift = (final_E - init_E).abs() / init_E.abs().max(1e-30);
        assert!(
            drift < 0.01,
            "energy drift {:.2e} over 1 orbit exceeds 1%", drift
        );

        // Period check: body 1 should return near starting position
        let period_error = (bodies.x[0] + a / 2.0).abs().max((bodies.y[0]).abs()) / a;
        assert!(
            period_error < 0.02,
            "position error {:.2e} after 1 orbit exceeds 2%", period_error
        );
    }

    #[test]
    fn test_kepler_100_orbits() {
        // Two-body Kepler: energy drift < 1% over 100 orbits
        let mut bodies = BodiesSoA::new(2);
        let a = 10.0;
        let m = 1.0;
        let G = 1.0;
        let softening = 1e-8;

        bodies.x = vec![-a / 2.0, a / 2.0];
        bodies.y = vec![0.0, 0.0];
        bodies.mass = vec![m, m];

        let v_circ = (G * (m + m) / a).sqrt();
        let v = v_circ * m / (m + m);
        bodies.vx = vec![0.0, 0.0];
        bodies.vy = vec![v, -v];

        let init_ke = compute_kinetic_energy(&bodies);
        let init_pe = -G * m * m / a;
        let init_E = init_ke + init_pe;

        let T = 2.0 * std::f64::consts::PI * (a * a * a / (G * (m + m))).sqrt();
        let dt = T / 50.0; // 50 steps per orbit
        let n_orbits = 100;
        let n_steps = (n_orbits as f64 * T / dt) as u64;

        let diags = simulate_n2(&mut bodies, softening, dt, n_steps, n_steps);

        let final_E = diags.last().unwrap().kinetic + diags.last().unwrap().potential;
        let drift = (final_E - init_E).abs() / init_E.abs().max(1e-30);

        assert!(
            drift < 0.01,
            "energy drift {:.2e} over 100 orbits exceeds 1%", drift
        );
    }
}

use galax_core::{
    compute_fmm_force, compute_fmm_potential_energy, compute_kinetic_energy,
    compute_momentum, compute_angular_momentum, compute_n2_force,
    InteractionLists, BodiesSoA, Tree,
};

/// Run one Leapfrog Kick-Drift-Kick step.
///
/// Assumes bodies.ax/ay contain the current accelerations (from previous step
/// or initialization). After the step, bodies.ax/ay are updated with the new
/// accelerations for the next step's first kick.
pub fn leapfrog_kdk(
    bodies: &mut BodiesSoA,
    tree: &Tree,
    m2l_lists: &InteractionLists,
    p2p_lists: &InteractionLists,
    p: usize,
    softening: f64,
    dt: f64,
) {
    let half_dt = 0.5 * dt;

    // Kick (first half): v += a * dt/2
    for i in 0..bodies.len() {
        bodies.vx[i] += bodies.ax[i] * half_dt;
        bodies.vy[i] += bodies.ay[i] * half_dt;
    }

    // Drift: x += v * dt
    for i in 0..bodies.len() {
        bodies.x[i] += bodies.vx[i] * dt;
        bodies.y[i] += bodies.vy[i] * dt;
    }

    // Compute new forces
    compute_fmm_force(bodies, tree, m2l_lists, p2p_lists, p, softening);

    // Kick (second half): v += a_new * dt/2
    for i in 0..bodies.len() {
        bodies.vx[i] += bodies.ax[i] * half_dt;
        bodies.vy[i] += bodies.ay[i] * half_dt;
    }
}

/// Energy diagnostics snapshot.
pub struct Diagnostics {
    pub step: u64,
    pub kinetic: f64,
    pub potential: f64,
    pub momentum_x: f64,
    pub momentum_y: f64,
    pub angular_momentum: f64,
}

/// Run simulation for a given number of steps.
///
/// Returns diagnostics at each step where (step % energy_every == 0).
pub fn simulate(
    bodies: &mut BodiesSoA,
    tree: &Tree,
    m2l_lists: &InteractionLists,
    p2p_lists: &InteractionLists,
    p: usize,
    softening: f64,
    dt: f64,
    n_steps: u64,
    energy_every: u64,
) -> Vec<Diagnostics> {
    let mut diags = Vec::new();

    for step in 0..n_steps {
        leapfrog_kdk(bodies, tree, m2l_lists, p2p_lists, p, softening, dt);

        if energy_every > 0 && (step + 1) % energy_every == 0 {
            let ke = compute_kinetic_energy(bodies);
            let pe = compute_fmm_potential_energy(bodies);
            let (px, py) = compute_momentum(bodies);
            let lz = compute_angular_momentum(bodies);
            diags.push(Diagnostics {
                step: step + 1,
                kinetic: ke,
                potential: pe,
                momentum_x: px,
                momentum_y: py,
                angular_momentum: lz,
            });
        }
    }

    diags
}

/// Leapfrog KDK using direct N² forces (bypasses FMM).
/// Used for testing where exact forces are needed.
pub fn leapfrog_kdk_n2(bodies: &mut BodiesSoA, softening: f64, dt: f64) {
    let half_dt = 0.5 * dt;
    for i in 0..bodies.len() {
        bodies.vx[i] += bodies.ax[i] * half_dt;
        bodies.vy[i] += bodies.ay[i] * half_dt;
    }
    for i in 0..bodies.len() {
        bodies.x[i] += bodies.vx[i] * dt;
        bodies.y[i] += bodies.vy[i] * dt;
    }
    compute_n2_force(bodies, softening);
    for i in 0..bodies.len() {
        bodies.vx[i] += bodies.ax[i] * half_dt;
        bodies.vy[i] += bodies.ay[i] * half_dt;
    }
}

/// Simulate using direct N² forces.
pub fn simulate_n2(
    bodies: &mut BodiesSoA, softening: f64, dt: f64, n_steps: u64, energy_every: u64,
) -> Vec<Diagnostics> {
    // Compute initial forces
    compute_n2_force(bodies, softening);
    let mut diags = Vec::new();
    for step in 0..n_steps {
        leapfrog_kdk_n2(bodies, softening, dt);
        if energy_every > 0 && (step + 1) % energy_every == 0 {
            let ke = compute_kinetic_energy(bodies);
            let mut pe = 0.0_f64;
            for i in 0..bodies.len() {
                for j in (i + 1)..bodies.len() {
                    let dx = bodies.x[j] - bodies.x[i];
                    let dy = bodies.y[j] - bodies.y[i];
                    pe -= bodies.mass[i] * bodies.mass[j]
                        / (dx * dx + dy * dy + softening * softening).sqrt();
                }
            }
            let (px, py) = compute_momentum(bodies);
            let lz = compute_angular_momentum(bodies);
            diags.push(Diagnostics {
                step: step + 1, kinetic: ke, potential: pe,
                momentum_x: px, momentum_y: py, angular_momentum: lz,
            });
        }
    }
    diags
}
