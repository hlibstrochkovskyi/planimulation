//! Dry geological incision; independent of seasonal water and display.
//! Linear-slope stream power, implicit downstream-first on a frozen DAG.
use serde::Serialize;

pub const MODEL_VERSION: &str = "stream-power-incision-1";
pub const MAX_NODES: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Effective uniform runoff and rock response, units 1/year for m=.5,n=1.
    pub erodibility_per_year: f64,
    pub step_years: f64,
    /// Optional fixed base level. It stops incision, never raises existing bed.
    pub base_level_meters: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Incision {
    pub elevation_meters: Vec<f64>,
    pub removed_thickness_meters: Vec<f64>,
    pub contributing_area_square_meters: Vec<f64>,
    /// Explicit off-grid material reservoir increment, not deposition geometry.
    pub removed_material_cubic_meters: f64,
    pub changed_nodes: usize,
    pub unresolved_positive_incision_nodes: usize,
}

fn add_positive(sum: &mut f64, correction: &mut f64, value: f64) -> Result<(), String> {
    if !value.is_finite() || value < 0. {
        return Err("Invalid landscape area or material contribution.".into());
    }
    let adjusted = value - *correction;
    let next = *sum + adjusted;
    *correction = (next - *sum) - adjusted;
    if !next.is_finite() || !correction.is_finite() {
        return Err("Landscape accumulation overflow.".into());
    }
    *sum = next;
    Ok(())
}

/// Receivers are neighboring downstream nodes; self identifies fixed terminals.
/// The caller owns geometry/connectivity; distances are per donor link in meters.
/// Result owns the accepted material loss; all input slices remain unchanged.
pub fn incise(
    elevation: &[f64],
    areas: &[f64],
    receivers: &[u32],
    distances: &[f64],
    settings: Settings,
) -> Result<Incision, String> {
    let count = elevation.len();
    if !(1..=MAX_NODES).contains(&count)
        || areas.len() != count
        || receivers.len() != count
        || distances.len() != count
        || elevation.iter().any(|h| !h.is_finite())
        || areas.iter().any(|a| !a.is_finite() || *a <= 0.)
        || !settings.erodibility_per_year.is_finite()
        || settings.erodibility_per_year < 0.
        || !settings.step_years.is_finite()
        || settings.step_years <= 0.
        || settings.base_level_meters.is_some_and(|b| !b.is_finite())
    {
        return Err("Invalid landscape field, shape or settings.".into());
    }
    let coefficient = settings.erodibility_per_year * settings.step_years;
    if !coefficient.is_finite() || settings.erodibility_per_year > 0. && coefficient == 0. {
        return Err("Unrepresentable landscape time coefficient.".into());
    }
    let mut incoming = vec![0_u32; count];
    for (i, &r) in receivers.iter().enumerate() {
        let r = r as usize;
        if r >= count || !distances[i].is_finite() || distances[i] < 0. {
            return Err("Invalid landscape receiver or distance.".into());
        }
        if r != i {
            if distances[i] == 0. || elevation[r] > elevation[i] {
                return Err("Landscape receiver must be no higher than its donor.".into());
            }
            incoming[r] = incoming[r]
                .checked_add(1)
                .ok_or("Landscape degree overflow.")?;
        }
    }
    let mut order: Vec<_> = (0..count).filter(|i| incoming[*i] == 0).collect();
    let mut contributing = areas.to_vec();
    let mut corrections = vec![0.; count];
    let mut cursor = 0;
    while cursor < order.len() {
        let i = order[cursor];
        cursor += 1;
        let r = receivers[i] as usize;
        if r == i {
            continue;
        }
        let area = contributing[i];
        add_positive(&mut contributing[r], &mut corrections[r], area)?;
        incoming[r] -= 1;
        if incoming[r] == 0 {
            order.push(r);
        }
    }
    if order.len() != count {
        return Err("Landscape receiver graph contains a cycle.".into());
    }
    let mut result = Incision {
        elevation_meters: elevation.to_vec(),
        removed_thickness_meters: vec![0.; count],
        contributing_area_square_meters: contributing,
        removed_material_cubic_meters: 0.,
        changed_nodes: 0,
        unresolved_positive_incision_nodes: 0,
    };
    let mut material_correction = 0.;
    for &i in order.iter().rev() {
        let r = receivers[i] as usize;
        if r == i || coefficient == 0. {
            continue;
        }
        let receiver = result.elevation_meters[r];
        let base = settings
            .base_level_meters
            .map_or(receiver, |b| receiver.max(b));
        if elevation[i] <= base {
            continue;
        }
        let difference = elevation[i] - base;
        let gamma = coefficient * result.contributing_area_square_meters[i].sqrt() / distances[i];
        if !difference.is_finite() || !gamma.is_finite() || gamma <= 0. {
            return Err("Unrepresentable positive stream-power coefficient or relief.".into());
        }
        // Equivalent to (old + gamma*receiver)/(1+gamma), avoiding overflow
        // and unnecessary cancellation of two absolute elevations.
        let fraction = if gamma <= 1. {
            gamma / (1. + gamma)
        } else {
            1. / (1. + 1. / gamma)
        };
        let requested = difference * fraction;
        if !requested.is_finite() || requested <= 0. {
            return Err("Unrepresentable positive requested landscape incision.".into());
        }
        let updated = if gamma <= 1. {
            elevation[i] - requested
        } else {
            base + difference / (1. + gamma)
        };
        if !updated.is_finite() || updated < base || updated > elevation[i] {
            return Err("Landscape incision left its physical bed/base interval.".into());
        }
        let actual = elevation[i] - updated;
        if actual == 0. {
            result.unresolved_positive_incision_nodes += 1;
        }
        let volume = actual * areas[i];
        if !volume.is_finite() || actual > 0. && volume == 0. {
            return Err("Unrepresentable landscape material volume.".into());
        }
        add_positive(
            &mut result.removed_material_cubic_meters,
            &mut material_correction,
            volume,
        )?;
        result.changed_nodes += usize::from(actual > 0.);
        result.elevation_meters[i] = updated;
        result.removed_thickness_meters[i] = actual;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> Settings {
        Settings {
            erodibility_per_year: 1.,
            step_years: 1.,
            base_level_meters: None,
        }
    }

    #[test]
    fn single_link_and_downstream_first_chain_have_independent_dyadic_results() {
        let r = incise(&[10., 0.], &[1., 1.], &[1, 1], &[1., 0.], settings()).unwrap();
        assert_eq!(r.elevation_meters, [5., 0.]);
        assert_eq!(r.removed_material_cubic_meters, 5.);
        let r = incise(
            &[9., 6., 0.],
            &[4., 5., 1.],
            &[1, 2, 2],
            &[1., 1., 0.],
            settings(),
        )
        .unwrap();
        assert_eq!(r.contributing_area_square_meters, [4., 9., 10.]);
        assert_eq!(r.elevation_meters, [4., 1.5, 0.]);
        assert_eq!(r.removed_material_cubic_meters, 42.5);
    }

    #[test]
    fn base_level_terminal_and_zero_forcing_never_raise_or_erase_bed() {
        let old = [10., -5., 2.];
        let r = incise(
            &old,
            &[1.; 3],
            &[1, 1, 2],
            &[1., 0., 0.],
            Settings {
                base_level_meters: Some(4.),
                ..settings()
            },
        )
        .unwrap();
        assert_eq!(r.elevation_meters, [7., -5., 2.]);
        assert_eq!(old, [10., -5., 2.]);
        let r = incise(
            &old,
            &[1.; 3],
            &[1, 1, 2],
            &[1., 0., 0.],
            Settings {
                erodibility_per_year: 0.,
                ..settings()
            },
        )
        .unwrap();
        assert_eq!(r.elevation_meters, old);
        assert_eq!(r.removed_material_cubic_meters, 0.);
    }

    #[test]
    fn time_refinement_approaches_an_independent_fixed_outlet_exponential() {
        let exact = 10. * (-1_f64).exp();
        let mut previous = f64::INFINITY;
        for steps in [1, 2, 4, 8, 16, 32] {
            let mut bed = vec![10., 0.];
            for _ in 0..steps {
                bed = incise(
                    &bed,
                    &[1., 1.],
                    &[1, 1],
                    &[1., 0.],
                    Settings {
                        step_years: 1. / steps as f64,
                        ..settings()
                    },
                )
                .unwrap()
                .elevation_meters;
            }
            let error = (bed[0] - exact).abs();
            assert!(error < previous);
            previous = error;
        }
    }

    #[test]
    fn cycles_uphill_links_shapes_and_nonfinite_settings_refuse_even_at_zero_forcing() {
        for (bed, area, receivers, distance) in [
            (vec![1., 1.], vec![1.; 2], vec![1, 0], vec![1.; 2]),
            (vec![0., 1.], vec![1.; 2], vec![1, 1], vec![1., 0.]),
            (vec![1., 0.], vec![1.; 2], vec![2, 1], vec![1., 0.]),
            (vec![1., 0.], vec![1.; 2], vec![1, 1], vec![0.; 2]),
            (vec![1., 0.], vec![0.; 2], vec![1, 1], vec![1., 0.]),
        ] {
            assert!(
                incise(
                    &bed,
                    &area,
                    &receivers,
                    &distance,
                    Settings {
                        erodibility_per_year: 0.,
                        ..settings()
                    }
                )
                .is_err()
            );
        }
        assert!(incise(&[], &[], &[], &[], settings()).is_err());
        assert!(
            incise(
                &[1.],
                &[1.],
                &[0],
                &[0.],
                Settings {
                    step_years: f64::NAN,
                    ..settings()
                }
            )
            .is_err()
        );
        assert!(
            incise(
                &[1.],
                &[1.],
                &[0],
                &[0.],
                Settings {
                    base_level_meters: Some(f64::NAN),
                    ..settings()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn repeat_and_relabeling_preserve_fields_and_independent_material_ledger() {
        let bed = [9., 6., 0.];
        let area = [4., 5., 1.];
        let a = incise(&bed, &area, &[1, 2, 2], &[1., 1., 0.], settings()).unwrap();
        let b = incise(&bed, &area, &[1, 2, 2], &[1., 1., 0.], settings()).unwrap();
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
        let reversed = incise(
            &[0., 6., 9.],
            &[1., 5., 4.],
            &[0, 0, 1],
            &[0., 1., 1.],
            settings(),
        )
        .unwrap();
        assert_eq!(
            a.elevation_meters.iter().rev().copied().collect::<Vec<_>>(),
            reversed.elevation_meters
        );
        let volume: f64 = bed
            .iter()
            .zip(&a.elevation_meters)
            .zip(area)
            .map(|((before, after), area)| (before - after) * area)
            .sum();
        assert_eq!(volume, a.removed_material_cubic_meters);
    }

    #[test]
    fn large_acyclic_chain_is_iterative_and_uses_linear_storage() {
        let n = 100_000;
        let bed: Vec<_> = (0..n).map(|i| (n - i) as f64).collect();
        let receivers: Vec<_> = (0..n).map(|i| (i + 1).min(n - 1) as u32).collect();
        let r = incise(&bed, &vec![1.; n], &receivers, &vec![1.; n], settings()).unwrap();
        assert_eq!(r.contributing_area_square_meters[n - 1], n as f64);
        assert!(r.elevation_meters.windows(2).all(|h| h[0] >= h[1]));
        assert_eq!(r.elevation_meters[n - 1], 1.);
    }

    #[test]
    fn large_coefficient_retains_base_and_tiny_unresolved_incision_is_reported() {
        let r = incise(&[1e9, 0.1], &[1., 1.], &[1, 1], &[1e-20, 0.], settings()).unwrap();
        assert!(r.elevation_meters[0] >= 0.1);
        let r = incise(
            &[1e9, 0.],
            &[1., 1.],
            &[1, 1],
            &[1., 0.],
            Settings {
                erodibility_per_year: 1e-30,
                ..settings()
            },
        )
        .unwrap();
        assert_eq!(r.elevation_meters[0], 1e9);
        assert_eq!(r.unresolved_positive_incision_nodes, 1);
        assert_eq!(r.removed_material_cubic_meters, 0.);
    }

    #[test]
    fn overflow_and_incomplete_shapes_refuse_without_mutating_input() {
        let bed = [2., 1.];
        assert!(incise(&bed, &[f64::MAX; 2], &[1, 1], &[1., 0.], settings()).is_err());
        assert!(
            incise(
                &bed,
                &[1.; 2],
                &[1, 1],
                &[1., 0.],
                Settings {
                    erodibility_per_year: f64::MAX,
                    step_years: 2.,
                    base_level_meters: None
                }
            )
            .is_err()
        );
        assert!(incise(&bed, &[1.], &[1, 1], &[1., 0.], settings()).is_err());
        assert_eq!(bed, [2., 1.]);
    }
}
