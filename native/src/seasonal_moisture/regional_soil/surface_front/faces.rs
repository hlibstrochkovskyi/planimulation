//! Bounded flat-line shared-flux hypotheses and one frozen explicit transfer.
//! Neither operator is selected for native or seasonal evolution.
use super::{moments::Cell, reconstruction::POWER};
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock as Stock};
use serde::Serialize;

#[derive(Clone, Copy)]
pub(super) struct Parameters {
    pub roughness: f64,
    pub cap: f64,
}
impl Parameters {
    fn validate(self) -> Result<(), String> {
        if !self.roughness.is_finite()
            || self.roughness <= 0.
            || !self.cap.is_finite()
            || self.cap <= 0.
        {
            return Err("Invalid frozen face physical parameters.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Face {
    pub flux: f64,
    pub gradient: f64,
    pub donor: Option<&'static str>,
    pub donor_blocked_positive_opposite_trace: bool,
}

fn pressure_flux(v: f64, gradient: f64, parameters: Parameters) -> Result<f64, String> {
    parameters.validate()?;
    if !v.is_finite() || v < 0. || !gradient.is_finite() {
        return Err("Invalid physical pressure face trace or gradient.".into());
    }
    if v == 0. || gradient == 0. {
        return Ok(0.);
    }
    let depth = v.powf(POWER);
    let magnitude = gradient.abs();
    // The cap and Manning branches are flux magnitudes, not singular
    // mobilities subsequently multiplied by a potentially zero gradient.
    let capped = POWER * parameters.cap * depth * (magnitude / v);
    let manning = POWER.sqrt() / parameters.roughness * depth * magnitude.sqrt();
    if !capped.is_finite() || !manning.is_finite() || capped <= 0. || manning <= 0. {
        return Err("Unrepresentable positive pressure flux branch.".into());
    }
    let q = -gradient.signum() * capped.min(manning);
    if !q.is_finite() {
        return Err("Unrepresentable frozen pressure face flux.".into());
    }
    Ok(q)
}

fn trace(cell: &Cell, x: f64, width: f64) -> Result<(f64, f64), String> {
    let raw = cell
        .pressure(x, width)
        .ok_or("Unresolved frozen face owner.")?;
    if !raw.is_finite() {
        return Err("Nonfinite frozen latent pressure trace.".into());
    }
    let v = raw.max(0.);
    let slope = match cell {
        Cell::Fit { slope, .. } if v > 0. => *slope,
        _ => 0.,
    };
    if !v.is_finite() || !slope.is_finite() {
        return Err("Nonfinite frozen face reconstruction.".into());
    }
    Ok((v, slope))
}

pub(super) fn pressure_face(
    left: &Cell,
    right: &Cell,
    width: f64,
    penalty: f64,
    parameters: Parameters,
) -> Result<Face, String> {
    parameters.validate()?;
    if !width.is_finite() || width <= 0. || !penalty.is_finite() || penalty < 0. {
        return Err("Invalid frozen pressure face geometry or penalty.".into());
    }
    let (vl, sl) = trace(left, width, width)?;
    let (vr, sr) = trace(right, 0., width)?;
    let gradient = 0.5 * sl + 0.5 * sr + penalty * ((vr - vl) / width);
    let (v, opposite, donor) = if gradient < 0. {
        (vl, vr, Some("left"))
    } else if gradient > 0. {
        (vr, vl, Some("right"))
    } else {
        (0., 0., None)
    };
    Ok(Face {
        flux: pressure_flux(v, gradient, parameters)?,
        gradient,
        donor,
        donor_blocked_positive_opposite_trace: donor.is_some() && v == 0. && opposite > 0.,
    })
}

pub(super) fn stock_face(
    left: f64,
    right: f64,
    width: f64,
    parameters: Parameters,
) -> Result<Face, String> {
    parameters.validate()?;
    if !left.is_finite()
        || left < 0.
        || !right.is_finite()
        || right < 0.
        || !width.is_finite()
        || width <= 0.
    {
        return Err("Invalid frozen stock face inputs.".into());
    }
    let gradient = (right - left) / width;
    let depth = left.max(right);
    let magnitude = gradient.abs();
    let flux = if gradient == 0. {
        0.
    } else {
        let capped = parameters.cap * magnitude;
        let manning = depth.powf(5. / 3.) / parameters.roughness * magnitude.sqrt();
        if !capped.is_finite() || !manning.is_finite() || capped <= 0. || manning <= 0. {
            return Err("Unrepresentable positive stock flux branch.".into());
        }
        -gradient.signum() * capped.min(manning)
    };
    if !gradient.is_finite() || !flux.is_finite() {
        return Err("Unrepresentable frozen stock face flux.".into());
    }
    Ok(Face {
        flux,
        gradient,
        donor: if gradient < 0. {
            Some("left")
        } else if gradient > 0. {
            Some("right")
        } else {
            None
        },
        donor_blocked_positive_opposite_trace: false,
    })
}

pub(super) fn rhs(fluxes: &[f64], width: f64) -> Result<Vec<f64>, String> {
    if !(3..=1025).contains(&fluxes.len())
        || fluxes.iter().any(|q| !q.is_finite())
        || !width.is_finite()
        || width <= 0.
    {
        return Err("Invalid bounded frozen flux divergence.".into());
    }
    let result: Vec<_> = fluxes.windows(2).map(|q| (q[0] - q[1]) / width).collect();
    if result.iter().any(|r| !r.is_finite()) {
        return Err("Unrepresentable frozen divergence.".into());
    }
    Ok(result)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Step {
    pub volume_high: Vec<f64>,
    pub volume_low: Vec<f64>,
    pub boundary_input: f64,
    pub boundary_output: f64,
    pub maximum_local_relative_ledger_residual: f64,
    pub relative_inventory_residual: f64,
}

/// All fluxes are frozen. Donor checks use the old stock, not incoming credit.
/// No request is resized: failure discards the local proposed result.
pub(super) fn step(means: &[f64], width: f64, fluxes: &[f64], dt: f64) -> Result<Step, String> {
    rhs(fluxes, width)?;
    if means.len() + 1 != fluxes.len()
        || means.iter().any(|h| !h.is_finite() || *h < 0.)
        || !dt.is_finite()
        || dt <= 0.
    {
        return Err("Invalid bounded frozen explicit step.".into());
    }
    let initial: Vec<_> = means.iter().map(|h| h * width).collect();
    if !total_mass(&initial).is_finite() {
        return Err("Unrepresentable total frozen initial inventory.".into());
    }
    let mut stocks: Vec<_> = initial
        .iter()
        .map(|&v| Stock::new(v, 0., f64::MAX))
        .collect::<Result<_, _>>()?;
    if means.iter().zip(&initial).any(|(&h, &v)| h > 0. && v == 0.) {
        return Err("Positive frozen initial volume underflow.".into());
    }
    let amounts: Vec<_> = fluxes.iter().map(|q| q * dt).collect();
    if amounts
        .iter()
        .zip(fluxes)
        .any(|(&a, &q)| !a.is_finite() || q != 0. && a == 0.)
    {
        return Err("Frozen face transfer overflow or underflow.".into());
    }
    let mut incoming = vec![Vec::new(); means.len()];
    let mut outgoing = vec![Vec::new(); means.len()];
    for (i, a) in amounts.iter().copied().enumerate() {
        if i > 0 {
            if a > 0. {
                outgoing[i - 1].push(a);
            } else {
                incoming[i - 1].push(-a);
            }
        }
        if i < means.len() {
            if a > 0. {
                incoming[i].push(a);
            } else {
                outgoing[i].push(-a);
            }
        }
    }
    for (i, values) in outgoing.iter().enumerate() {
        // Count both outgoing faces before any credit or withdrawal.
        if total_mass(values) > stocks[i].available() {
            return Err(format!("Frozen donor budget exceeded at cell {i}."));
        }
    }
    for (i, stock) in stocks.iter_mut().enumerate() {
        for &a in &outgoing[i] {
            if stock.withdraw(a) != a {
                return Err("Frozen request cannot be represented without resizing.".into());
            }
        }
        for &a in &incoming[i] {
            stock.credit(a)?;
        }
    }
    let boundary_input = total_mass(&[amounts[0].max(0.), (-amounts[means.len()]).max(0.)]);
    let boundary_output = total_mass(&[(-amounts[0]).max(0.), amounts[means.len()].max(0.)]);
    let mut local: f64 = 0.;
    let mut ledger = Vec::new();
    for (i, stock) in stocks.iter().enumerate() {
        let mut terms = vec![stock.high, stock.low, -initial[i]];
        terms.extend(incoming[i].iter().map(|a| -a));
        terms.extend(outgoing[i].iter().copied());
        let scale = initial[i]
            .max(total_mass(&incoming[i]))
            .max(total_mass(&outgoing[i]));
        let residual = total_mass(&terms).abs();
        local = local.max(if scale == 0. {
            residual
        } else {
            residual / scale
        });
        ledger.extend([stock.high, stock.low, -initial[i]]);
    }
    ledger.extend([-boundary_input, boundary_output]);
    let scale = total_mass(&initial)
        .max(boundary_input)
        .max(boundary_output);
    let residual = total_mass(&ledger).abs();
    let relative = if scale == 0. {
        residual
    } else {
        residual / scale
    };
    if !local.is_finite()
        || !relative.is_finite()
        || local > 256. * f64::EPSILON
        || relative > 256. * f64::EPSILON
    {
        return Err("Frozen explicit ledger exceeds its separate arithmetic bound.".into());
    }
    Ok(Step {
        volume_high: stocks.iter().map(|s| s.high).collect(),
        volume_low: stocks.iter().map(|s| s.low).collect(),
        boundary_input,
        boundary_output,
        maximum_local_relative_ledger_residual: local,
        relative_inventory_residual: relative,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parameters(cap: f64) -> Parameters {
        Parameters {
            roughness: 0.04,
            cap,
        }
    }
    fn constant(v: f64, slope: f64) -> Cell {
        Cell::Fit {
            peak: v,
            slope,
            neighbor: None,
            residual: 0.,
            pair_residual: 0.,
            pair_peak_difference: 0.,
        }
    }
    #[test]
    fn face_law_has_original_cap_sign_and_zero_limits() {
        for v in [1e-12_f64, 0.001, 1.] {
            for s in [-0.1_f64, -1e-7, 1e-7, 0.1] {
                for cap in [0.01, 100.] {
                    let depth = v.powf(POWER);
                    let gradient = POWER * v.powf(POWER - 1.) * s;
                    let expected =
                        -(depth.powf(5. / 3.) / (0.04 * gradient.abs().sqrt())).min(cap) * gradient;
                    let q = pressure_flux(v, s, parameters(cap)).unwrap();
                    assert!((q - expected).abs() / expected.abs() < 1e-13);
                }
            }
        }
        assert_eq!(pressure_flux(0., 1., parameters(100.)).unwrap(), 0.);
        assert_eq!(pressure_flux(1., 0., parameters(100.)).unwrap(), 0.);
        let jump = pressure_face(
            &constant(1., 0.),
            &constant(0.5, 0.),
            1.,
            1.,
            parameters(100.),
        )
        .unwrap();
        assert!(jump.flux > 0.);
        assert_eq!(jump.donor, Some("left"));
        let reflected = pressure_face(
            &constant(0.5, 0.),
            &constant(1., 0.),
            1.,
            1.,
            parameters(100.),
        )
        .unwrap();
        assert_eq!(jump.flux, -reflected.flux);
        for cap in [0.01, 100.] {
            let continuous = pressure_face(
                &constant(0.2, -0.1),
                &constant(0.1, -0.1),
                1.,
                1.,
                parameters(cap),
            )
            .unwrap();
            assert_eq!(
                continuous.flux,
                pressure_flux(0.1, -0.1, parameters(cap)).unwrap()
            );
        }
        assert!(pressure_flux(1., f64::NAN, parameters(100.)).is_err());
        assert!(pressure_flux(1e-300, 1e10, parameters(f64::from_bits(1))).is_err());
        assert!(stock_face(1e-300, 0., 1., parameters(100.)).is_err());
        assert!(
            pressure_face(
                &Cell::Unresolved("test".into()),
                &Cell::Dry,
                1.,
                1.,
                parameters(100.)
            )
            .is_err()
        );
    }
    #[test]
    fn frozen_step_pairs_transfers_and_refuses_combined_donor_overdraw() {
        let means = [0., 1., 0.];
        let original = means;
        let fluxes = [0., -0.4, 0.4, 0.];
        let accepted = step(&means, 1., &fluxes, 1.).unwrap();
        assert_eq!(accepted.volume_high, vec![0.4, 1. - 2. * 0.4, 0.4]);
        assert!(accepted.maximum_local_relative_ledger_residual < 1e-14);
        assert!(step(&means, 1., &[0., -0.6, 0.6, 0.], 1.).is_err());
        assert_eq!(means, original);
        assert!(step(&[0., 0.], 1., &[-0.1, 0., 0.], 1.).is_err());
        assert!(step(&means, 1., &fluxes, 0.).is_err());
        assert!(step(&means, 1., &[0., f64::NAN, 0., 0.], 1.).is_err());
        assert!(rhs(&[0., f64::MAX, -f64::MAX], 1.).is_err());
    }
    #[test]
    fn frozen_boundary_signs_and_sub_ulp_pair_transfers_keep_their_owners() {
        let inflow = step(&[1., 2.], 1., &[0.1, 0., -0.2], 1.).unwrap();
        assert_eq!(inflow.volume_high, vec![1.1, 2.2]);
        assert_eq!(inflow.boundary_input, 0.1 + 0.2);
        assert_eq!(inflow.boundary_output, 0.);
        let outflow = step(&[1., 2.], 1., &[-0.1, 0., 0.2], 1.).unwrap();
        assert_eq!(outflow.volume_high, vec![0.9, 1.8]);
        assert_eq!(outflow.boundary_input, 0.);
        assert_eq!(outflow.boundary_output, 0.1 + 0.2);
        let large = 2_f64.powi(54);
        let tiny = step(&[large, large], 1., &[0., 1., 0.], 1.).unwrap();
        assert_eq!(tiny.volume_high, vec![large, large]);
        assert_eq!(tiny.volume_low, vec![-1., 1.]);
        assert_eq!(tiny.relative_inventory_residual, 0.);
        assert!(step(&[f64::MAX, f64::MAX], 1., &[0.; 3], 1.).is_err());
    }
}
