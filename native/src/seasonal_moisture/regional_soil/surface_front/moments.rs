//! Two adjacent mean constraints for a clipped-affine pressure ansatz.
//! These fits are not a shared face flux, limiter or time-evolution operator.
use super::reconstruction::{POWER, fit_peak, mean_from_peak, pressure_flux};

const MOMENT_LIMIT: f64 = 256. * f64::EPSILON;

/// Relative decrease between adjacent normalized means. The positive series
/// avoids subtracting nearly equal power differences near a uniform state.
fn contrast(t: f64) -> f64 {
    let p = POWER + 1.;
    if t == 0. {
        return 0.;
    }
    if t <= 1e-3 {
        let mut coefficient = p * (p - 1.) / 2.;
        let mut power = 1.;
        let mut numerator = 0.;
        let mut denominator = p;
        for k in 2..=9 {
            numerator += coefficient * ((1u32 << k) as f64 - 2.) * power;
            denominator -= coefficient * power * t;
            coefficient *= (k as f64 - p) / (k + 1) as f64;
            power *= t;
        }
        return t * numerator / denominator;
    }
    let upstream = -(p * (-t).ln_1p()).exp_m1();
    let downstream = (1. - t).powf(p) * (-(p * (-t / (1. - t)).ln_1p()).exp_m1());
    (upstream - downstream) / upstream
}

#[derive(Clone, Debug)]
pub(super) struct Pair {
    pub slope_magnitude: f64,
    pub upstream_peak: f64,
    pub downstream_peak: f64,
    pub maximum_relative_residual: f64,
}

pub(super) fn pair(upstream: f64, downstream: f64, width: f64) -> Result<Pair, String> {
    if !upstream.is_finite()
        || !downstream.is_finite()
        || !width.is_finite()
        || upstream < 0.
        || downstream < 0.
        || downstream > upstream
        || width <= 0.
    {
        return Err("Invalid ordered adjacent moment inputs.".into());
    }
    if downstream == 0. {
        return Err("Dry adjacent moment does not identify a unique slope.".into());
    }
    if upstream == downstream {
        let peak = upstream.powf(1. / POWER);
        if !peak.is_finite() || peak <= 0. {
            return Err("Unrepresentable constant moment pressure.".into());
        }
        let residual = (peak.powf(POWER) - upstream).abs() / upstream;
        if !residual.is_finite() || residual > MOMENT_LIMIT {
            return Err("Constant moment fit exceeds its separate residual bound.".into());
        }
        return Ok(Pair {
            slope_magnitude: 0.,
            upstream_peak: peak,
            downstream_peak: peak,
            maximum_relative_residual: residual,
        });
    }
    let ratio = downstream / upstream;
    if ratio == 0. || !ratio.is_finite() {
        return Err("Unrepresentable positive adjacent moment ratio.".into());
    }
    let p = POWER + 1.;
    let partial_boundary = 0.5_f64.powf(p) / (1. - 0.5_f64.powf(p));
    let (t, remaining) = if ratio <= partial_boundary {
        let log_remaining = (ratio.ln() - ratio.ln_1p()) / p;
        (-log_remaining.exp_m1(), log_remaining.exp())
    } else {
        let observed = (upstream - downstream) / upstream;
        let mut low = 0.;
        let mut high = 0.5;
        for _ in 0..96 {
            let mid = low + 0.5 * (high - low);
            if mid == low || mid == high {
                break;
            }
            if contrast(mid) < observed {
                low = mid;
            } else {
                high = mid;
            }
        }
        let t = if (contrast(low) - observed).abs() <= (contrast(high) - observed).abs() {
            low
        } else {
            high
        };
        (t, 1. - t)
    };
    if t <= 0. || t > 1. || remaining <= 0. {
        return Err("Unrepresentable moment pressure shape.".into());
    }
    let upstream_peak = (upstream / mean_from_peak(1., t)?).powf(1. / POWER);
    // Do not recover a tiny positive downstream pressure as P - P*t.
    let downstream_peak = upstream_peak * remaining;
    let span = upstream_peak * t;
    let magnitude = span / width;
    if !magnitude.is_finite() || magnitude <= 0. || !upstream_peak.is_finite() {
        return Err("Unrepresentable moment pressure magnitude.".into());
    }
    let residual = ((mean_from_peak(upstream_peak, span)? - upstream).abs() / upstream)
        .max((mean_from_peak(downstream_peak, span)? - downstream).abs() / downstream);
    if !residual.is_finite() || residual > MOMENT_LIMIT {
        return Err("Adjacent moment fit exceeds its separate residual bound.".into());
    }
    Ok(Pair {
        slope_magnitude: magnitude,
        upstream_peak,
        downstream_peak,
        maximum_relative_residual: residual,
    })
}

#[derive(Clone, Debug)]
pub(super) enum Cell {
    Dry,
    Fit {
        peak: f64,
        slope: f64,
        neighbor: Option<usize>,
        residual: f64,
        pair_residual: f64,
        pair_peak_difference: f64,
    },
    Unresolved(String),
}

impl Cell {
    pub fn pressure(&self, x: f64, width: f64) -> Option<f64> {
        match self {
            Self::Dry => Some(0.),
            Self::Fit { peak, slope, .. } => Some(
                peak + if *slope <= 0. {
                    slope * x
                } else {
                    slope * (x - width)
                },
            ),
            Self::Unresolved(_) => None,
        }
    }
    pub fn depth(&self, x: f64, width: f64) -> Option<f64> {
        self.pressure(x, width).map(|p| p.max(0.).powf(POWER))
    }
    pub fn flux(&self, x: f64, width: f64) -> Option<f64> {
        match self {
            Self::Dry => Some(0.),
            Self::Fit { slope, .. } => Some(pressure_flux(self.pressure(x, width)?, *slope)),
            Self::Unresolved(_) => None,
        }
    }
    /// Return isolated in-cell affine roots, not extrapolated contours. A
    /// constant equal to the threshold has a non-isolated level set, not a root.
    pub fn root(&self, threshold: f64, width: f64) -> Option<f64> {
        let Self::Fit { peak, slope, .. } = self else {
            return None;
        };
        if *slope == 0. {
            return None;
        }
        let wet_endpoint = if *slope < 0. { 0. } else { width };
        let x = wet_endpoint + (threshold.powf(1. / POWER) - peak) / slope;
        (x >= 0. && x <= width).then_some(x)
    }
}

pub(super) fn cells(means: &[f64], width: f64) -> Result<Vec<Cell>, String> {
    if means.len() < 2
        || means.len() > 1024
        || !width.is_finite()
        || width <= 0.
        || means.iter().any(|x| !x.is_finite() || *x < 0.)
    {
        return Err("Invalid bounded moment grid.".into());
    }
    Ok(means
        .iter()
        .enumerate()
        .map(|(i, &mean)| {
            if mean == 0. {
                return Cell::Dry;
            }
            let adjacent: Vec<_> = [i.checked_sub(1), (i + 1 < means.len()).then_some(i + 1)]
                .into_iter()
                .flatten()
                .collect();
            let equal = adjacent.iter().all(|&j| means[j] == mean);
            if equal {
                return match pair(mean, mean, width) {
                    Ok(p) => Cell::Fit {
                        peak: p.upstream_peak,
                        slope: 0.,
                        neighbor: None,
                        residual: p.maximum_relative_residual,
                        pair_residual: p.maximum_relative_residual,
                        pair_peak_difference: 0.,
                    },
                    Err(e) => Cell::Unresolved(e),
                };
            }
            let wetter: Vec<_> = adjacent
                .iter()
                .copied()
                .filter(|&j| means[j] > mean)
                .collect();
            let neighbor = if adjacent.len() == 1 {
                adjacent[0]
            } else if wetter.len() == 1 && adjacent.iter().all(|&j| means[j] != mean) {
                wetter[0]
            } else {
                return Cell::Unresolved("Ambiguous extremum or plateau stencil.".into());
            };
            let (u, d) = if means[neighbor] > mean {
                (neighbor, i)
            } else {
                (i, neighbor)
            };
            let p = match pair(means[u], means[d], width) {
                Ok(p) => p,
                Err(e) => return Cell::Unresolved(e),
            };
            let signed = if u < d {
                -p.slope_magnitude
            } else {
                p.slope_magnitude
            };
            let peak = match fit_peak(mean, signed, width) {
                Ok(Some(peak)) => peak,
                Ok(None) => return Cell::Dry,
                Err(e) => return Cell::Unresolved(e),
            };
            let residual = match mean_from_peak(peak, signed.abs() * width) {
                Ok(x) => (x - mean).abs() / mean,
                Err(e) => return Cell::Unresolved(e),
            };
            Cell::Fit {
                peak,
                slope: signed,
                neighbor: Some(neighbor),
                residual,
                pair_residual: p.maximum_relative_residual,
                pair_peak_difference: peak
                    - if i == u {
                        p.upstream_peak
                    } else {
                        p.downstream_peak
                    },
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adjacent_means_recover_full_partial_and_almost_uniform_pressure() {
        for t in [
            1e-14,
            1e-7,
            0.000999,
            0.001001,
            0.1,
            0.49,
            0.5,
            0.9,
            1. - 1. / 1048576.,
        ] {
            let a = mean_from_peak(1., t).unwrap();
            let b = mean_from_peak(1. - t, t).unwrap();
            let p = pair(a, b, 1.).unwrap();
            assert!(p.maximum_relative_residual <= MOMENT_LIMIT);
            // Near a uniform state, relative slope recovery is conditioned by
            // the small difference of rounded inputs, not by either stock alone.
            assert!((p.slope_magnitude - t).abs() < 2e-13);
            assert!((p.upstream_peak - 1.).abs() < 2e-13);
            assert!((p.downstream_peak - (1. - t)).abs() < 2e-13);
        }
    }
    #[test]
    fn dry_pairs_and_ambiguous_stencils_do_not_borrow_an_exact_slope() {
        assert!(pair(1., 0., 1.).is_err());
        assert!(pair(0., 0., 1.).is_err());
        assert!(pair(1., 2., 1.).is_err());
        assert!(pair(1., 1., 0.).is_err());
        assert!(pair(f64::NAN, 1., 1.).is_err());
        assert!(pair(1e-135, 1e-135, 1.).is_err());
        assert!(matches!(cells(&[0., 0., 0.], 1.).unwrap()[1], Cell::Dry));
        assert!(matches!(
            cells(&[1., 1., 1.], 1.).unwrap()[1],
            Cell::Fit { slope: 0., .. }
        ));
        assert!(matches!(
            cells(&[1., 2., 1.], 1.).unwrap()[1],
            Cell::Unresolved(_)
        ));
        assert!(matches!(
            cells(&[2., 1., 2.], 1.).unwrap()[1],
            Cell::Unresolved(_)
        ));
        assert!(matches!(
            cells(&[0., 1., 0.], 1.).unwrap()[1],
            Cell::Unresolved(_)
        ));
        assert!(cells(&[1., -1.], 1.).is_err());
        let constant = cells(&[1., 1.], 1.).unwrap();
        assert_eq!(constant[0].root(1., 1.), None);
        assert_eq!(constant[0].flux(0.5, 1.), Some(0.));
        assert!(cells(&[1.; 1025], 1.).is_err());
        assert!(matches!(
            cells(&[2., 2., 1.], 1.).unwrap()[1],
            Cell::Unresolved(_)
        ));
        assert!(matches!(
            cells(&[2., 1., 0.], 1.).unwrap()[1],
            Cell::Fit {
                neighbor: Some(0),
                ..
            }
        ));
        assert!(matches!(
            cells(&[1., 0.], 1.).unwrap()[0],
            Cell::Unresolved(_)
        ));
    }
}
