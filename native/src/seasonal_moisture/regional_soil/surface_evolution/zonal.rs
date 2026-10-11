//! Independent equal-area latitude-band FV reference on p=cos(theta).
//! Backward Euler, damped Picard and a tridiagonal M-matrix. No native mesh.
use super::{AMPLITUDE, MEAN, RADIUS, ROUGHNESS};
use crate::moisture_transport::total_mass;
use serde::Serialize;

const MAX_ITERATIONS: usize = 256;
const RESIDUAL_TOLERANCE: f64 = 1e-12;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Diagnostics {
    pub maximum_iterations: usize,
    pub summed_iterations: usize,
    pub maximum_equation_residual_meters: f64,
    pub maximum_relative_inventory_drift: f64,
}

pub(super) struct Run {
    pub depth: Vec<f64>,
    pub diagnostics: Diagnostics,
}

// Exact band averages of P1 and P2, not native-center samples.
fn initial(bands: usize, degree: u32) -> Vec<f64> {
    let dp = 2. / bands as f64;
    (0..bands)
        .map(|i| {
            let p = -1. + (i as f64 + 0.5) * dp;
            let profile = if degree == 1 {
                p
            } else {
                (3. * p * p - 1.) / 2. + dp * dp / 8.
            };
            MEAN + AMPLITUDE * profile
        })
        .collect()
}

fn conductance(depth: &[f64], cap: f64) -> Result<Vec<f64>, String> {
    let dp = 2. / depth.len() as f64;
    depth
        .windows(2)
        .enumerate()
        .map(|(i, h)| {
            let p = -1. + (i + 1) as f64 * dp;
            let metric = (1. - p * p).max(0.);
            let slope = ((h[1] - h[0]) / dp).abs() * metric.sqrt() / RADIUS;
            let beta = if slope == 0. {
                cap
            } else {
                ((0.5 * h[0] + 0.5 * h[1]).powf(5. / 3.) / (ROUGHNESS * slope.sqrt())).min(cap)
            };
            let k = beta * metric / RADIUS.powi(2);
            if !k.is_finite() || k < 0. {
                return Err("Invalid zonal reference conductance.".into());
            }
            Ok(k)
        })
        .collect()
}

fn solve_linear(old: &[f64], weights: &[f64], seconds: f64) -> Result<Vec<f64>, String> {
    let n = old.len();
    let factor = seconds / (2. / n as f64).powi(2);
    let mut upper = vec![0.; n];
    let mut rhs = vec![0.; n];
    for i in 0..n {
        let left = if i == 0 { 0. } else { factor * weights[i - 1] };
        let right = if i + 1 == n { 0. } else { factor * weights[i] };
        let pivot = 1. + left + right - if i == 0 { 0. } else { left * upper[i - 1] };
        if !pivot.is_finite() || pivot <= 0. {
            return Err("Invalid zonal reference pivot.".into());
        }
        upper[i] = right / pivot;
        rhs[i] = (old[i] + if i == 0 { 0. } else { left * rhs[i - 1] }) / pivot;
    }
    for i in (0..n - 1).rev() {
        rhs[i] += upper[i] * rhs[i + 1];
    }
    Ok(rhs)
}

fn equation_residual(old: &[f64], next: &[f64], cap: f64, seconds: f64) -> Result<f64, String> {
    // Rebuild coefficients at the accepted state, not at a stale Picard guess.
    let k = conductance(next, cap)?;
    let factor = seconds / (2. / old.len() as f64).powi(2);
    let mut maximum: f64 = 0.;
    for i in 0..old.len() {
        let left = if i == 0 {
            0.
        } else {
            factor * k[i - 1] * (next[i - 1] - next[i])
        };
        let right = if i + 1 == old.len() {
            0.
        } else {
            factor * k[i] * (next[i + 1] - next[i])
        };
        let residual = total_mass(&[next[i], -old[i], -left, -right]);
        if !residual.is_finite() {
            return Err("Nonfinite zonal reference residual.".into());
        }
        maximum = maximum.max(residual.abs());
    }
    Ok(maximum)
}

pub(super) fn solve(
    bands: usize,
    steps: usize,
    seconds: f64,
    degree: u32,
    cap: f64,
) -> Result<Run, String> {
    if !(16..=4096).contains(&bands)
        || steps == 0
        || steps > 16384
        || !seconds.is_finite()
        || !(0. ..=1800.).contains(&seconds)
        || seconds == 0.
        || !(1..=2).contains(&degree)
        || !cap.is_finite()
        || !(0. ..=1e6).contains(&cap)
    {
        return Err("Unsupported bounded zonal reference settings.".into());
    }
    let mut depth = initial(bands, degree);
    let initial_sum = total_mass(&depth);
    let dt = seconds / steps as f64;
    let mut diagnostics = Diagnostics::default();
    for step in 0..steps {
        let old = depth.clone();
        let mut accepted = false;
        for iteration in 1..=MAX_ITERATIONS {
            let linear = solve_linear(&old, &conductance(&depth, cap)?, dt)?;
            for (h, x) in depth.iter_mut().zip(linear) {
                *h = 0.5 * *h + 0.5 * x;
            }
            if depth.iter().any(|h| {
                !h.is_finite()
                    || *h < MEAN - AMPLITUDE - 64. * f64::EPSILON
                    || *h > MEAN + AMPLITUDE + 64. * f64::EPSILON
            }) {
                return Err("Zonal reference violates its positive wet depth bounds.".into());
            }
            let residual = equation_residual(&old, &depth, cap, dt)?;
            if residual <= RESIDUAL_TOLERANCE {
                diagnostics.maximum_iterations = diagnostics.maximum_iterations.max(iteration);
                diagnostics.summed_iterations += iteration;
                diagnostics.maximum_equation_residual_meters =
                    diagnostics.maximum_equation_residual_meters.max(residual);
                accepted = true;
                break;
            }
        }
        if !accepted {
            return Err(format!(
                "Zonal nonlinear reference did not converge at step {step}."
            ));
        }
        let drift = (total_mass(&depth) - initial_sum).abs() / initial_sum;
        diagnostics.maximum_relative_inventory_drift =
            diagnostics.maximum_relative_inventory_drift.max(drift);
        if drift > 1e-9 {
            return Err("Zonal reference inventory drift exceeds its separate bound.".into());
        }
    }
    Ok(Run { depth, diagnostics })
}

// Linear interpolation/extrapolation of band-center averages. Its spatial
// uncertainty must be measured by reference refinement, not hidden by fitting.
pub(super) fn sample(depth: &[f64], p: f64) -> Result<f64, String> {
    if depth.len() < 2 || !p.is_finite() || !(-1. - 1e-14..=1. + 1e-14).contains(&p) {
        return Err("Invalid zonal sampling location.".into());
    }
    let at = (p.clamp(-1., 1.) + 1.) * depth.len() as f64 / 2. - 0.5;
    let left = (at.floor().max(0.) as usize).min(depth.len() - 2);
    let fraction = at - left as f64;
    let value = depth[left] + fraction * (depth[left + 1] - depth[left]);
    if !value.is_finite() {
        return Err("Invalid zonal sampled depth.".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tridiagonal_step_matches_an_independent_dyadic_three_band_oracle() {
        let solution = solve_linear(&[1., 0., 0.], &[1., 1.], (2_f64 / 3.).powi(2)).unwrap();
        for (a, b) in solution.iter().zip([0.625, 0.25, 0.125]) {
            assert!((a - b).abs() < 1e-14);
        }
        assert!((total_mass(&solution) - 1.).abs() < 1e-14);
        let uniform = solve_linear(&[1.; 3], &[1., 1.], (2_f64 / 3.).powi(2)).unwrap();
        assert!(uniform.iter().all(|h| (h - 1.).abs() < 1e-14));
    }

    #[test]
    fn exact_band_initialization_matches_independent_polynomial_integrals() {
        for degree in 1..=2 {
            let bands = 64;
            let dp = 2. / bands as f64;
            for (i, h) in initial(bands, degree).iter().enumerate() {
                let a = -1. + i as f64 * dp;
                let b = a + dp;
                let primitive = |p: f64| {
                    if degree == 1 {
                        p * p / 2.
                    } else {
                        (p * p * p - p) / 2.
                    }
                };
                let exact = MEAN + AMPLITUDE * (primitive(b) - primitive(a)) / dp;
                assert!((h - exact).abs() < 1e-14);
            }
            assert!((total_mass(&initial(bands, degree)) / bands as f64 - MEAN).abs() < 1e-14);
        }
    }

    #[test]
    fn reference_time_halving_converges_to_independent_continuous_capped_modes() {
        for degree in 1..=2 {
            let exact_factor =
                (-f64::from(degree * (degree + 1)) * 10_000. * 1000. / RADIUS.powi(2)).exp();
            let mut errors = Vec::new();
            for steps in [10, 20, 40] {
                let r = solve(64, steps, 1000., degree, 10_000.).unwrap();
                let error = r
                    .depth
                    .iter()
                    .zip(initial(64, degree))
                    .map(|(a, h)| (a - MEAN - (h - MEAN) * exact_factor).powi(2))
                    .sum::<f64>()
                    .sqrt();
                errors.push(error);
            }
            for pair in errors.windows(2) {
                assert!((0.45..0.55).contains(&(pair[1] / pair[0])));
            }
        }
    }
    #[test]
    fn capped_reference_matches_two_exact_discrete_spherical_eigenmodes() {
        for degree in 1..=2 {
            let bands = 64;
            let steps = 20;
            let seconds = 100.;
            let cap = 100.;
            let r = solve(bands, steps, seconds, degree, cap).unwrap();
            let factor = (1.
                + f64::from(degree * (degree + 1)) * cap * seconds
                    / (RADIUS.powi(2) * steps as f64))
                .powi(-(steps as i32));
            for (actual, initial) in r.depth.iter().zip(initial(bands, degree)) {
                let exact = MEAN + (initial - MEAN) * factor;
                assert!((actual - exact).abs() < 5e-11);
            }
            assert!(r.diagnostics.maximum_equation_residual_meters <= RESIDUAL_TOLERANCE);
            assert!(r.diagnostics.maximum_relative_inventory_drift < 1e-12);
        }
    }
    #[test]
    fn nonlinear_reference_repeats_and_obeys_separate_inventory_and_residual_bounds() {
        for degree in 1..=2 {
            let a = solve(128, 20, 100., degree, 1e6).unwrap();
            let b = solve(128, 20, 100., degree, 1e6).unwrap();
            assert_eq!(a.depth, b.depth);
            assert!(a.diagnostics.maximum_relative_inventory_drift < 1e-11);
            assert!(
                a.depth
                    .iter()
                    .zip(initial(128, degree))
                    .any(|(a, b)| (a - b).abs() > 1e-5)
            );
        }
    }
    #[test]
    fn zero_cap_is_motionless_and_sampling_preserves_affine_fields() {
        let r = solve(64, 10, 100., 1, 0.).unwrap();
        assert_eq!(r.depth, initial(64, 1));
        for p in [-1., -0.8, 0., 0.7, 1.] {
            assert!((sample(&r.depth, p).unwrap() - MEAN - AMPLITUDE * p).abs() < 1e-14);
        }
        assert!(sample(&[], 0.).is_err());
        assert!(sample(&r.depth, f64::NAN).is_err());
    }
    #[test]
    fn invalid_reference_settings_refuse_before_solving() {
        for (bands, steps, seconds, degree, cap) in [
            (0, 1, 1., 1, 100.),
            (64, 0, 1., 1, 100.),
            (64, 1, f64::NAN, 1, 100.),
            (64, 1, 1801., 1, 100.),
            (64, 1, 1., 3, 100.),
            (64, 1, 1., 1, -1.),
        ] {
            assert!(solve(bands, steps, seconds, degree, cap).is_err());
        }
    }
}
