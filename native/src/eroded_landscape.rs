//! Version-5 initial-condition synthesis: `multiscale-geography-5`. Reuses
//! version 4 and adds two explicit elevation contributions: the ocean floor
//! (`bathymetry::seafloor`) and landscape evolution (`erosion`). Both are
//! placeholders returning zero until their models land.
use crate::Surface;

pub const MODEL_VERSION: &str = "multiscale-geography-5";
/// Finest subdivision supported by version 5 (163,842 regions).
pub const MAX_SUBDIVISION: u32 = 7;

/// Per-region erosion (negative) and deposition (positive) in meters, applied
/// to the prepared bed after the initial water fit.
pub fn erosion(
    surface: &Surface,
    elevation: &[f64],
    wet: &[u32],
    _level: f64,
) -> Result<Vec<f64>, String> {
    if elevation.len() != surface.centers.len() || wet.len() != elevation.len() {
        return Err("Invalid erosion input.".into());
    }
    Ok(vec![0.; elevation.len()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Recipe, World, coastal_lowlands};

    fn recipe(model: &str, subdivision: u32) -> Recipe {
        serde_json::from_value(serde_json::json!({"schemaVersion":1,"modelVersion":model,
            "randomVersion":"fnv1a-utf8-mulberry32-1","seed":"first-light","subdivision":subdivision,
            "radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,
            "continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,"detailAmplitudeMeters":300.,
            "terrainPreparationPasses":4,"water":{"mode":"coverage","fraction":0.71}})).unwrap()
    }

    #[test]
    fn contributions_sum_to_elevation() {
        let w = World::generate(recipe(MODEL_VERSION, 4)).unwrap();
        let t = &w.terrain;
        for i in 0..t.elevation.len() {
            let sum = t.baseline[i]
                + t.convergence[i]
                + t.divergence[i]
                + t.detail[i]
                + t.preparation[i]
                + t.lowland[i]
                + t.seafloor[i]
                + t.erosion[i];
            assert!((t.elevation[i] - sum).abs() < 1e-8);
        }
        let v4 = World::generate(recipe(coastal_lowlands::MODEL_VERSION, 4)).unwrap();
        assert!(v4.terrain.seafloor.iter().all(|&v| v == 0.));
        assert!(v4.terrain.erosion.iter().all(|&v| v == 0.));
    }

    #[test]
    fn only_version_5_accepts_subdivision_7() {
        assert!(recipe(MODEL_VERSION, 7).validate().is_ok());
        assert!(recipe(MODEL_VERSION, 8).validate().is_err());
        assert!(
            recipe(coastal_lowlands::MODEL_VERSION, 7)
                .validate()
                .is_err()
        );
    }
}
