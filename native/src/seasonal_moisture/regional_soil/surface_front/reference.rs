//! Dry-capable independent band FV reference, not a native mesh operator.
use crate::moisture_transport::total_mass;
use serde::Serialize;

const MAX_ITERATIONS: usize = 256;
const TOLERANCE: f64 = 1e-12;
pub(super) const ROUGHNESS: f64 = 0.04;
const SPEED: f64 = 0.1;
const POWER: f64 = 3. / 7.;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Diagnostics {
    maximum_iterations: usize,
    maximum_equation_residual_meters: f64,
    maximum_relative_inventory_drift: f64,
}

pub(super) struct Run {
    pub depth: Vec<f64>,
    pub diagnostics: Diagnostics,
}

struct Grid {
    width: f64,
    slope_metric: Vec<f64>,
    divergence_metric: Vec<f64>,
    cap: f64,
}

impl Grid {
    fn weights(&self, depth: &[f64]) -> Result<Vec<f64>, String> {
        depth
            .windows(2)
            .enumerate()
            .map(|(i, h)| {
                let face_depth = 0.5 * h[0] + 0.5 * h[1];
                let slope = (h[1] - h[0]).abs() * self.slope_metric[i] / self.width;
                // No film: a completely dry interface has zero mobility. For
                // positive depth and zero slope use the declared capped limit.
                let beta = if face_depth == 0. {
                    0.
                } else if slope == 0. {
                    self.cap
                } else {
                    (face_depth.powf(5. / 3.) / (ROUGHNESS * slope.sqrt())).min(self.cap)
                };
                let k = beta * self.divergence_metric[i];
                if !k.is_finite() || k < 0. {
                    return Err("Invalid dry reference conductance.".into());
                }
                Ok(k)
            })
            .collect()
    }

    fn linear(&self, rhs: &[f64], weights: &[f64], dt: f64) -> Result<Vec<f64>, String> {
        let factor = dt / self.width.powi(2);
        let mut upper = vec![0.; rhs.len()];
        let mut next = vec![0.; rhs.len()];
        for i in 0..rhs.len() {
            let left = if i == 0 { 0. } else { factor * weights[i - 1] };
            let right = if i + 1 == rhs.len() {
                0.
            } else {
                factor * weights[i]
            };
            let pivot = 1. + left + right - if i == 0 { 0. } else { left * upper[i - 1] };
            if !pivot.is_finite() || pivot <= 0. {
                return Err("Invalid dry reference pivot.".into());
            }
            upper[i] = right / pivot;
            next[i] = (rhs[i] + if i == 0 { 0. } else { left * next[i - 1] }) / pivot;
        }
        for i in (0..rhs.len() - 1).rev() {
            next[i] += upper[i] * next[i + 1];
        }
        Ok(next)
    }

    fn residual(&self, rhs: &[f64], next: &[f64], dt: f64) -> Result<f64, String> {
        let weights = self.weights(next)?;
        let factor = dt / self.width.powi(2);
        let mut maximum: f64 = 0.;
        for i in 0..next.len() {
            let left = if i == 0 {
                0.
            } else {
                factor * weights[i - 1] * (next[i - 1] - next[i])
            };
            let right = if i + 1 == next.len() {
                0.
            } else {
                factor * weights[i] * (next[i + 1] - next[i])
            };
            let residual = total_mass(&[next[i], -rhs[i], -left, -right]).abs();
            if !residual.is_finite() {
                return Err("Nonfinite dry reference residual.".into());
            }
            maximum = maximum.max(residual);
        }
        Ok(maximum)
    }
}

fn validate(bands: usize, steps: usize, seconds: f64) -> Result<(), String> {
    if !(16..=4096).contains(&bands)
        || steps == 0
        || steps > 16384
        || !seconds.is_finite()
        || !(0. ..=1800.).contains(&seconds)
        || seconds == 0.
    {
        return Err("Unsupported bounded dry reference controls.".into());
    }
    Ok(())
}

fn solve(
    grid: Grid,
    initial: Vec<f64>,
    steps: usize,
    seconds: f64,
    cumulative_inflow: impl Fn(f64) -> f64,
) -> Result<Run, String> {
    validate(initial.len(), steps, seconds)?;
    let mut depth = initial;
    let initial_inventory = total_mass(&depth) * grid.width;
    let dt = seconds / steps as f64;
    let mut diagnostics = Diagnostics::default();
    for step in 0..steps {
        let before_time = step as f64 * dt;
        let next_time = (step + 1) as f64 * dt;
        let inflow = cumulative_inflow(next_time) - cumulative_inflow(before_time);
        let mut rhs = depth.clone();
        rhs[0] += inflow / grid.width;
        // Start every Picard iterate with the already accounted boundary input.
        // The linear solve and convex relaxation then share the same inventory,
        // rather than leaving part of the input pending at residual convergence.
        depth.clone_from(&rhs);
        let bound = rhs.iter().copied().fold(0., f64::max);
        let mut accepted = false;
        for iteration in 1..=MAX_ITERATIONS {
            let linear = grid.linear(&rhs, &grid.weights(&depth)?, dt)?;
            for (h, x) in depth.iter_mut().zip(linear) {
                *h = 0.5 * *h + 0.5 * x;
            }
            if depth
                .iter()
                .any(|h| !h.is_finite() || *h < 0. || *h > bound + 64. * f64::EPSILON)
            {
                return Err("Dry reference violates its M-matrix depth bounds.".into());
            }
            let residual = grid.residual(&rhs, &depth, dt)?;
            if residual <= TOLERANCE {
                diagnostics.maximum_iterations = diagnostics.maximum_iterations.max(iteration);
                diagnostics.maximum_equation_residual_meters =
                    diagnostics.maximum_equation_residual_meters.max(residual);
                accepted = true;
                break;
            }
        }
        if !accepted {
            return Err(format!("Dry reference did not converge at step {step}."));
        }
        let expected = initial_inventory + cumulative_inflow(next_time);
        let drift = (total_mass(&depth) * grid.width - expected).abs() / expected.max(1.);
        diagnostics.maximum_relative_inventory_drift =
            diagnostics.maximum_relative_inventory_drift.max(drift);
        if !drift.is_finite() || drift > 1e-9 {
            return Err("Dry reference inventory drift exceeds its separate bound.".into());
        }
    }
    Ok(Run { depth, diagnostics })
}

fn wave_coefficient() -> f64 {
    (7. / 3.) * (SPEED * ROUGHNESS).powi(2)
}

pub(super) fn wave_average(a: f64, b: f64, seconds: f64) -> f64 {
    let primitive = |x: f64| (SPEED * seconds - x).max(0.).powf(POWER + 1.);
    wave_coefficient().powf(POWER) * (primitive(a) - primitive(b)) / ((POWER + 1.) * (b - a))
}

pub(super) fn planar(bands: usize, steps: usize) -> Result<Run, String> {
    validate(bands, steps, 100.)?;
    let width = 200. / bands as f64;
    let initial = (0..bands)
        .map(|i| {
            let a = -100. + i as f64 * width;
            wave_average(a, a + width, 0.)
        })
        .collect();
    // The exact wave's beta=(7/3)*c*(ct-x) stays below 26 m^2/s;
    // cap=100 is inactive throughout this analytic certificate's wet domain.
    let grid = Grid {
        width,
        slope_metric: vec![1.; bands - 1],
        divergence_metric: vec![1.; bands - 1],
        cap: 100.,
    };
    let inflow = |t: f64| {
        wave_coefficient().powf(POWER)
            * ((SPEED * t + 100.).powf(POWER + 1.) - 100_f64.powf(POWER + 1.))
            / (POWER + 1.)
    };
    solve(grid, initial, steps, 100., inflow)
}

pub(super) fn spherical(bands: usize, steps: usize) -> Result<Run, String> {
    validate(bands, steps, super::END_SECONDS)?;
    let width = 2. / bands as f64;
    let initial = (0..bands)
        .map(|i| {
            let a = -1. + i as f64 * width;
            ((a + width).max(0.).powf(POWER + 1.) - a.max(0.).powf(POWER + 1.))
                / ((POWER + 1.) * width)
        })
        .collect();
    let metric: Vec<_> = (1..bands)
        .map(|i| {
            let p = -1. + i as f64 * width;
            1. - p * p
        })
        .collect();
    let grid = Grid {
        width,
        slope_metric: metric.iter().map(|m| m.sqrt() / super::RADIUS).collect(),
        divergence_metric: metric.iter().map(|m| m / super::RADIUS.powi(2)).collect(),
        cap: super::CAP,
    };
    solve(grid, initial, steps, super::END_SECONDS, |_| 0.)
}

pub(super) fn sample(depth: &[f64], p: f64) -> Result<f64, String> {
    if depth.len() < 2 || !p.is_finite() || !(-1. - 1e-14..=1. + 1e-14).contains(&p) {
        return Err("Invalid dry reference sample.".into());
    }
    // Constant extension at the poles prevents extrapolation of a zero front
    // into negative water. Interior interpolation has no floor or clipping.
    let at = ((p.clamp(-1., 1.) + 1.) * depth.len() as f64 / 2. - 0.5)
        .clamp(0., (depth.len() - 1) as f64);
    let left = (at.floor() as usize).min(depth.len() - 2);
    let fraction = at - left as f64;
    Ok((1. - fraction) * depth[left] + fraction * depth[left + 1])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completely_dry_interfaces_have_no_film_or_spurious_conductance() {
        let grid = Grid {
            width: 1.,
            slope_metric: vec![1.; 2],
            divergence_metric: vec![1.; 2],
            cap: 100.,
        };
        assert_eq!(grid.weights(&[0.; 3]).unwrap(), [0.; 2]);
        assert_eq!(grid.linear(&[0.; 3], &[0.; 2], 1.).unwrap(), [0.; 3]);
        assert_eq!(grid.weights(&[1.; 3]).unwrap(), [100.; 2]);
    }
    #[test]
    fn wave_flux_and_time_derivative_satisfy_the_uncapped_equation_independently() {
        for x in [-100., -10., 2.] {
            let z: f64 = SPEED * 50. - x;
            let h = (wave_coefficient() * z).powf(POWER);
            let gradient = -POWER * h / z;
            let flux = -h.powf(5. / 3.) * (-gradient).sqrt() / ROUGHNESS;
            assert!((flux + SPEED * h).abs() < 1e-14);
            let beta = h.powf(5. / 3.) / (ROUGHNESS * (-gradient).sqrt());
            assert!((beta - (7. / 3.) * SPEED * z).abs() < 1e-12);
            assert!(beta < 100.);
        }
        assert_eq!(wave_average(10., 20., 100.), 0.);
    }
    #[test]
    fn exact_wave_cell_averages_integrate_mass_and_have_a_moving_dry_front() {
        for n in [64, 128, 256] {
            let width = 200. / n as f64;
            let sum: f64 = (0..n)
                .map(|i| {
                    let a = -100. + i as f64 * width;
                    wave_average(a, a + width, 100.) * width
                })
                .sum();
            assert!((sum - wave_average(-100., 100., 100.) * 200.).abs() < 1e-12);
        }
    }
    #[test]
    fn planar_dry_reference_converges_to_exact_unforced_wave_with_accounted_boundary_input() {
        let mut errors = Vec::new();
        for n in [64, 128, 256] {
            let r = planar(n, 400).unwrap();
            let width = 200. / n as f64;
            let rms = (r
                .depth
                .iter()
                .enumerate()
                .map(|(i, h)| {
                    let a = -100. + i as f64 * width;
                    (h - wave_average(a, a + width, 100.)).powi(2)
                })
                .sum::<f64>()
                / n as f64)
                .sqrt();
            errors.push(rms);
            assert!(r.diagnostics.maximum_relative_inventory_drift < 1e-10);
        }
        assert!(errors[1] < errors[0] && errors[2] < errors[1]);
    }
    #[test]
    fn invalid_controls_and_samples_reject() {
        for (n, steps) in [(0, 1), (64, 0), (4097, 1), (64, 16385)] {
            assert!(planar(n, steps).is_err());
        }
        assert!(sample(&[], 0.).is_err());
        assert!(sample(&[0., 1.], f64::NAN).is_err());
        for p in [-1., 0., 1.] {
            assert!(sample(&[0., 1.], p).unwrap() >= 0.);
        }
    }
}
