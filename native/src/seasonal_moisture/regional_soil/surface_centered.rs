//! Flat-only centered-depth ablation; not a variable-bed or seasonal operator.
use super::{surface, surface_gradient};

pub(super) const MODEL_VERSION: &str = "regional-surface-flat-centered-depth-candidate-1";

/// Borrow immutable full-gradient geometry after admitting only flat closed land.
pub(super) struct Prepared<'a>(&'a surface_gradient::Prepared);

impl<'a> Prepared<'a> {
    pub fn new(gradient: &'a surface_gradient::Prepared) -> Result<Self, String> {
        let layout = gradient.layout();
        if layout.land.iter().any(|land| !land) || layout.beds.iter().any(|b| *b != layout.beds[0])
        {
            return Err(
                "Centered-depth verification requires flat closed terrestrial geometry.".into(),
            );
        }
        Ok(Self(gradient))
    }

    pub fn advance(&self, stocks: surface::Stocks<'_>, seconds: f64) -> Result<usize, String> {
        self.validate(&stocks, seconds)?;
        surface::advance_flat_centered(self.0, stocks, seconds)
    }

    pub(super) fn validate(
        &self,
        stocks: &surface::Stocks<'_>,
        seconds: f64,
    ) -> Result<(), String> {
        let layout = self.0.layout();
        if !seconds.is_finite()
            || seconds < 0.
            || !stocks.bodies.is_empty()
            || stocks.by_region.len() != layout.areas.len()
            || stocks.by_region.iter().any(Option::is_some)
            || stocks.liquid.len() != layout.areas.len()
            || stocks.transfers.len() != 2 * layout.faces.len()
        {
            return Err("Invalid flat centered-depth verification stocks or interval.".into());
        }
        // Refuse malformed owners before any mutation; no new floor or dry film.
        for owner in stocks.liquid.iter().chain(stocks.transfers.iter()) {
            owner.stock(f64::MAX)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Mass, ResolutionBudget, surface_flow, surface_verification};
    use super::*;
    use crate::{Surface, moisture_transport::total_mass};

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
        centered: bool,
    ) -> (Vec<Mass>, Vec<Mass>) {
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
        let steps = if centered {
            Prepared::new(g).unwrap().advance(stocks, 0.01).unwrap()
        } else {
            surface::advance_full_gradient(g, stocks, 0.01).unwrap()
        };
        assert_eq!(steps, 1);
        let residual =
            surface_verification::audit(g.layout(), initial, &liquid, &transfers).unwrap();
        assert!(residual.into_iter().all(|x| x < 1e-12));
        (liquid, transfers)
    }

    #[test]
    fn wet_dry_face_factor_is_exactly_the_depth_ablation_without_a_film() {
        let g = flat(1000.);
        let mut initial = vec![Mass::default(); 42];
        initial[0].high = 1000. * g.layout().areas[0];
        let source = advance(&g, &initial, false);
        let centered = advance(&g, &initial, true);
        let mut arrivals = 0;
        for (a, b) in source.1.iter().zip(&centered.1) {
            let old = total_mass(&[a.high, a.low]);
            let new = total_mass(&[b.high, b.low]);
            if old > 0. {
                arrivals += 1;
                assert!((new / old - 2_f64.powf(-5. / 3.)).abs() < 1e-14);
            } else {
                assert_eq!(new, 0.);
            }
        }
        assert!(arrivals > 0);
        assert!(centered.0.iter().skip(1).any(|m| m.high > 0.));
        assert!(centered.0.iter().skip(1).any(|m| *m == Mass::default()));
    }

    #[test]
    fn fully_capped_frozen_arrivals_match_source_depth_bytes() {
        let g = flat(1e-6);
        let mut initial = vec![Mass::default(); 42];
        initial[0].high = 1000. * g.layout().areas[0];
        assert_eq!(advance(&g, &initial, true), advance(&g, &initial, false));
    }

    #[test]
    fn flat_rest_is_exact_and_common_large_datum_does_not_change_arrivals() {
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
        let (liquid, transfers) = advance(&g, &initial, true);
        assert_eq!(liquid, initial);
        assert!(transfers.iter().all(|m| *m == Mass::default()));
        let mut pool = vec![Mass::default(); 42];
        pool[0] = initial[0];
        assert_eq!(advance(&g, &pool, true), advance(&elevated, &pool, true));
    }

    #[test]
    fn zero_cap_does_not_create_dry_water() {
        let g = flat(0.);
        let mut initial = vec![Mass::default(); 42];
        initial[0].high = 1000. * g.layout().areas[0];
        let (liquid, transfers) = advance(&g, &initial, true);
        assert_eq!(liquid, initial);
        assert!(transfers.iter().all(|m| *m == Mass::default()));
    }

    #[test]
    fn variable_beds_and_reference_bodies_cannot_select_flat_experiment() {
        let mut beds = vec![0.; 42];
        beds[1] = 1.;
        assert!(Prepared::new(&geometry(beds, vec![true; 42], 1000.)).is_err());
        let mut land = vec![true; 42];
        land[1] = false;
        assert!(Prepared::new(&geometry(vec![0.; 42], land, 1000.)).is_err());
    }

    #[test]
    fn malformed_owners_intervals_and_excessive_work_refuse_before_mutation() {
        let g = flat(1000.);
        let prepared = Prepared::new(&g).unwrap();
        for case in 0..9 {
            let mut liquid = vec![Mass::default(); 42];
            liquid[0].high = 1000. * g.layout().areas[0];
            let mut transfers = vec![Mass::default(); 2 * g.layout().faces.len()];
            let mut by_region = vec![None; 42];
            let mut bodies = Vec::new();
            let mut resolution = Some(ResolutionBudget::default());
            let mut seconds = 0.01;
            match case {
                0 => seconds = -1.,
                1 => seconds = f64::NAN,
                2 => seconds = g.layout().stable_seconds * 16385.,
                3 => {
                    by_region.pop();
                }
                4 => by_region[1] = Some(0),
                5 => bodies.push(Mass::default()),
                6 => {
                    liquid.pop();
                }
                7 => {
                    transfers.pop();
                }
                _ => transfers[0].high = -1.,
            }
            let before = (
                liquid.clone(),
                transfers.clone(),
                bodies.clone(),
                resolution,
            );
            assert!(
                prepared
                    .advance(
                        surface::Stocks {
                            by_region: &by_region,
                            liquid: &mut liquid,
                            bodies: &mut bodies,
                            transfers: &mut transfers,
                            resolution: &mut resolution
                        },
                        seconds
                    )
                    .is_err()
            );
            assert_eq!((liquid, transfers, bodies, resolution), before);
        }
    }
}
