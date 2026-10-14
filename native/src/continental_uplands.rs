//! Variable initial continental columns, not tectonic thickening over time.
//! Reuses frozen version-1 geographic fields so comparisons do not reroll structure.
use crate::{Surface, crust::Crust, geography};

pub const MODEL_VERSION: &str = "multiscale-geography-2";
pub const MAX_INTERIOR_THICKENING_METERS: f64 = 15_000.;
pub const INTERIOR_RESPONSE_SCALE: f64 = 0.25;

/// Potential excess above the fixed continental threshold is a procedural
/// interior proxy, not distance to coast, crust age or measured tectonic province.
pub fn thickness(continentality: f64, potential: f64, threshold: f64) -> Result<f64, String> {
    if !continentality.is_finite()
        || !(0. ..=1.).contains(&continentality)
        || !potential.is_finite()
        || !(-1. ..=1.).contains(&potential)
        || !threshold.is_finite()
        || !(-1.12..=1.12).contains(&threshold)
    {
        return Err("Invalid continental-uplands material inputs.".into());
    }
    let interior = (potential - threshold).max(0.);
    let response = interior / (interior + INTERIOR_RESPONSE_SCALE);
    Ok(
        7000.
            + 28000. * continentality
            + MAX_INTERIOR_THICKENING_METERS * continentality * response,
    )
}

pub fn build_crust(
    surface: &Surface,
    seed: &str,
    fraction: f64,
    scale: f64,
) -> Result<Crust, String> {
    let mut crust = geography::build_crust(surface, seed, fraction, scale)?;
    for (i, t) in crust.thickness_meters.iter_mut().enumerate() {
        *t = thickness(crust.continentality[i], crust.potential[i], crust.threshold)?;
    }
    Ok(crust)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_material_endpoints_and_response_have_physical_units() {
        assert_eq!(thickness(0., 1., 0.).unwrap(), 7000.);
        assert_eq!(thickness(1., 0., 0.).unwrap(), 35000.);
        assert_eq!(thickness(1., 0.25, 0.).unwrap(), 42500.);
        assert_eq!(thickness(0.5, 0.25, 0.).unwrap(), 24750.);
        assert_eq!(thickness(1., -0.25, 0.).unwrap(), 35000.);
        let h = crate::terrain::crust_baseline(42500., 2800.);
        assert!((h - 1303.030303030303).abs() < 1e-9);
    }
    #[test]
    fn material_response_is_monotone_bounded_and_rejects_invalid_inputs() {
        for c in [0., 0.1, 0.5, 1.] {
            let mut prior = 7000. + 28000. * c;
            for p in [-1., -0.5, 0., 0.25, 0.5, 1.] {
                let t = thickness(c, p, 0.).unwrap();
                assert!(t >= prior && (7000. ..=50000.).contains(&t));
                prior = t;
            }
        }
        for (c, p, t) in [
            (f64::NAN, 0., 0.),
            (1., f64::INFINITY, 0.),
            (1., 0., f64::NAN),
            (-0.1, 0., 0.),
            (1.1, 0., 0.),
            (1., 1.1, 0.),
            (1., 0., 1.13),
        ] {
            assert!(thickness(c, p, t).is_err());
        }
    }
    #[test]
    fn fixed_structure_and_shared_material_columns_do_not_depend_on_sampling() {
        let a = Surface::build(2, 6371000.);
        let b = Surface::build(3, 12742000.);
        let base = geography::build_crust(&a, "uplands", 0.38, 1.).unwrap();
        let coarse = build_crust(&a, "uplands", 0.38, 1.).unwrap();
        let fine = build_crust(&b, "uplands", 0.38, 1.).unwrap();
        assert_eq!(base.potential, coarse.potential);
        assert_eq!(base.continentality, coarse.continentality);
        assert_eq!(
            base.density_kg_per_cubic_meter,
            coarse.density_kg_per_cubic_meter
        );
        assert_eq!(coarse.threshold, fine.threshold);
        assert_eq!(
            coarse.thickness_meters,
            fine.thickness_meters[..a.centers.len()]
        );
        assert!(
            coarse
                .thickness_meters
                .iter()
                .zip(base.thickness_meters)
                .all(|(&a, b)| a >= b)
        );
    }
}
