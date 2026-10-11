//! Flat closed-land gradient-side coefficient assembly ablation.
//! Edge depth is retained; this is not elementwise P1 coefficient integration.
use super::{surface, surface_centered, surface_gradient};

pub(super) const MODEL_VERSION: &str = "regional-surface-flat-incident-mobility-candidate-1";

pub(super) fn mobility_mean(
    h: f64,
    slopes: [f64; 2],
    weights: [f64; 2],
    roughness: f64,
    cap: f64,
) -> Result<f64, String> {
    if !h.is_finite()
        || h < 0.
        || !roughness.is_finite()
        || roughness <= 0.
        || !cap.is_finite()
        || cap < 0.
        || slopes.iter().any(|g| !g.is_finite() || *g < 0.)
        || weights.iter().any(|w| !w.is_finite() || *w <= 0.)
    {
        return Err("Invalid local incident-mobility coefficients.".into());
    }
    let sum = weights[0] + weights[1];
    if !sum.is_finite() {
        return Err("Unrepresentable local weight sum.".into());
    }
    if h == 0. || cap == 0. {
        return Ok(0.);
    }
    let beta = slopes.map(|g| (h.powf(5. / 3.) / (roughness * g.sqrt())).min(cap));
    // Preserve exact fully capped coefficients, not a rounded convex sum.
    if beta[0] == beta[1] {
        return Ok(beta[0]);
    }
    let value = beta[0] + (weights[1] / sum) * (beta[1] - beta[0]);
    if !value.is_finite() || value < 0. || value > cap {
        return Err("Unrepresentable incident-mobility mean.".into());
    }
    Ok(value)
}

pub(super) struct Prepared<'a> {
    gradient: &'a surface_gradient::Prepared,
    flat: surface_centered::Prepared<'a>,
}

impl<'a> Prepared<'a> {
    pub fn new(gradient: &'a surface_gradient::Prepared) -> Result<Self, String> {
        let flat = surface_centered::Prepared::new(gradient)?;
        if !gradient.has_positive_local_weights() {
            return Err("Incident mobility requires positive individual triangle weights.".into());
        }
        Ok(Self { gradient, flat })
    }

    pub fn advance(&self, stocks: surface::Stocks<'_>, seconds: f64) -> Result<usize, String> {
        self.flat.validate(&stocks, seconds)?;
        surface::advance_flat_incident(self.gradient, stocks, seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Mass, ResolutionBudget, surface_flow, surface_verification};
    use super::*;
    use crate::Surface;

    fn geometry(beds: Vec<f64>, land: Vec<bool>, cap: f64) -> surface_gradient::Prepared {
        let mesh = Surface::build(1, 1000.);
        let layout = surface_flow::Layout::from_fields(
            &mesh,
            1000.,
            beds,
            land,
            0.,
            surface_flow::Settings {
                roughness: 0.04,
                maximum_diffusivity_square_meters_per_second: cap,
            },
        )
        .unwrap();
        surface_gradient::Prepared::new(&mesh, layout, 1000.).unwrap()
    }

    fn flat(cap: f64) -> surface_gradient::Prepared {
        geometry(vec![0.; 42], vec![true; 42], cap)
    }

    fn advance(
        g: &surface_gradient::Prepared,
        initial: &[Mass],
        incident: bool,
    ) -> (Vec<Mass>, Vec<Mass>, Option<ResolutionBudget>) {
        let mut liquid = initial.to_vec();
        let mut transfers = vec![Mass::default(); 2 * g.layout().faces.len()];
        let mut resolution = Some(ResolutionBudget::default());
        let stocks = surface::Stocks {
            by_region: &vec![None; liquid.len()],
            liquid: &mut liquid,
            bodies: &mut [],
            transfers: &mut transfers,
            resolution: &mut resolution,
        };
        let count = if incident {
            Prepared::new(g).unwrap().advance(stocks, 0.01).unwrap()
        } else {
            surface_centered::Prepared::new(g)
                .unwrap()
                .advance(stocks, 0.01)
                .unwrap()
        };
        assert_eq!(count, 1);
        surface_verification::audit(g.layout(), initial, &liquid, &transfers).unwrap();
        (liquid, transfers, resolution)
    }

    #[test]
    fn separate_nonlinear_coefficients_match_an_independent_two_triangle_calculation() {
        // Unit triangles above/below an edge. Affine gradients (1,1), (1,-2)
        // have norms sqrt(2), sqrt(5); opposite half-cotangents are both 1/2.
        let h = 0.5_f64;
        let factor = h.powf(5. / 3.) / 0.04;
        let expected = factor * 0.5 * (2_f64.powf(-0.25) + 5_f64.powf(-0.25));
        let value =
            mobility_mean(h, [2_f64.sqrt(), 5_f64.sqrt()], [0.5, 0.5], 0.04, 1000.).unwrap();
        assert!((value / expected - 1.).abs() < 1e-14);
        let averaged_slope = factor / (0.5 * (2_f64.sqrt() + 5_f64.sqrt())).sqrt();
        assert!(value > averaged_slope);
        let asymmetric =
            mobility_mean(h, [2_f64.sqrt(), 5_f64.sqrt()], [0.2, 0.8], 0.04, 1000.).unwrap();
        let expected = factor * (0.2 * 2_f64.powf(-0.25) + 0.8 * 5_f64.powf(-0.25));
        assert!((asymmetric / expected - 1.).abs() < 1e-14);
        assert_eq!(
            asymmetric,
            mobility_mean(h, [2_f64.sqrt(), 5_f64.sqrt()], [2., 8.], 0.04, 1000.).unwrap()
        );
    }

    #[test]
    fn cap_is_applied_to_each_term_and_zero_depth_has_no_film() {
        assert_eq!(mobility_mean(1., [0., 1.], [1., 3.], 1., 4.).unwrap(), 1.75);
        assert_eq!(mobility_mean(1., [0., 0.], [1., 3.], 1., 4.).unwrap(), 4.);
        assert_eq!(mobility_mean(0., [0., 0.], [1., 3.], 1., 4.).unwrap(), 0.);
        assert_eq!(mobility_mean(1., [0., 0.], [1., 3.], 1., 0.).unwrap(), 0.);
        for (h, g, w, n, cap) in [
            (-1., [1., 1.], [1., 1.], 1., 1.),
            (f64::NAN, [1., 1.], [1., 1.], 1., 1.),
            (1., [-1., 1.], [1., 1.], 1., 1.),
            (1., [f64::INFINITY, 1.], [1., 1.], 1., 1.),
            (1., [1., 1.], [-1., 2.], 1., 1.),
            (1., [1., 1.], [0., 2.], 1., 1.),
            (1., [1., 1.], [f64::MAX, f64::MAX], 1., 1.),
            (1., [1., 1.], [1., 1.], 0., 1.),
            (1., [1., 1.], [1., 1.], 1., -1.),
        ] {
            assert!(mobility_mean(h, g, w, n, cap).is_err());
        }
    }

    #[test]
    fn fully_capped_frozen_transfers_match_the_centered_branch_exactly() {
        let g = flat(1e-6);
        let mut initial = vec![Mass::default(); 42];
        initial[0].high = 1000. * g.layout().areas[0];
        assert_eq!(advance(&g, &initial, true), advance(&g, &initial, false));
    }

    #[test]
    fn flat_rest_common_datum_zero_cap_and_frozen_dry_arrivals_remain_explicit() {
        let g = flat(1000.);
        let elevated = geometry(vec![1e15; 42], vec![true; 42], 1000.);
        let initial: Vec<_> = g
            .layout()
            .areas
            .iter()
            .map(|a| Mass {
                high: 1000. * a,
                low: 0.,
            })
            .collect();
        let (liquid, history, _) = advance(&g, &initial, true);
        assert_eq!(liquid, initial);
        assert!(history.iter().all(|m| *m == Mass::default()));
        let mut pool = vec![Mass::default(); 42];
        pool[0] = initial[0];
        let a = advance(&g, &pool, true);
        assert_eq!(a, advance(&elevated, &pool, true));
        assert_eq!(a, advance(&g, &pool, true));
        let neighbors: Vec<_> = g
            .layout()
            .faces
            .iter()
            .filter(|f| f.regions.contains(&0))
            .map(|f| {
                if f.regions[0] == 0 {
                    f.regions[1]
                } else {
                    f.regions[0]
                }
            })
            .collect();
        assert!(neighbors.iter().all(|&i| a.0[i].high > 0.));
        assert!(
            (1..42)
                .filter(|i| !neighbors.contains(i))
                .all(|i| a.0[i] == Mass::default())
        );
        let off = flat(0.);
        let b = advance(&off, &pool, true);
        assert_eq!(b.0, pool);
        assert!(b.1.iter().all(|m| *m == Mass::default()));
    }

    #[test]
    fn varying_beds_and_reference_owners_cannot_select_this_ablation() {
        let mut beds = vec![0.; 42];
        beds[1] = 1.;
        assert!(Prepared::new(&geometry(beds, vec![true; 42], 1000.)).is_err());
        let mut land = vec![true; 42];
        land[1] = false;
        assert!(Prepared::new(&geometry(vec![0.; 42], land, 1000.)).is_err());
    }

    #[test]
    fn malformed_or_excessive_calls_refuse_before_mutation() {
        let g = flat(1000.);
        let prepared = Prepared::new(&g).unwrap();
        for case in 0..10 {
            let mut liquid = vec![Mass::default(); 42];
            liquid[0].high = 1000. * g.layout().areas[0];
            let mut history = vec![Mass::default(); 2 * g.layout().faces.len()];
            let mut bodies = vec![];
            let mut by_region = vec![None; 42];
            let mut resolution = Some(ResolutionBudget::default());
            let mut seconds = 0.01;
            match case {
                0 => seconds = -1.,
                1 => seconds = f64::INFINITY,
                2 => seconds = g.layout().stable_seconds * 16385.,
                3 => {
                    by_region.pop();
                }
                4 => by_region[0] = Some(0),
                5 => bodies.push(Mass::default()),
                6 => {
                    liquid.pop();
                }
                7 => {
                    history.pop();
                }
                8 => history[0].high = -1.,
                _ => liquid[0] = Mass { high: 0., low: 1. },
            }
            let before = (liquid.clone(), history.clone(), bodies.clone(), resolution);
            assert!(
                prepared
                    .advance(
                        surface::Stocks {
                            by_region: &by_region,
                            liquid: &mut liquid,
                            bodies: &mut bodies,
                            transfers: &mut history,
                            resolution: &mut resolution
                        },
                        seconds
                    )
                    .is_err()
            );
            assert_eq!((liquid, history, bodies, resolution), before);
        }
    }
}
