//! Version-4 initial-condition synthesis: coastal lowlands. Reuses version 3 and
//! compresses the crust-baseline part of dry height above sea level by a factor
//! that rises with graph distance from water. Belts and multiscale detail keep
//! their full relief. A procedural proxy for passive-margin subsidence
//! and coastal-plain sedimentation, not modeled flexure, transport or erosion.
use crate::{Surface, mountain_belts::Key};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const MODEL_VERSION: &str = "multiscale-geography-4";
/// Fraction of baseline height above sea level kept at the shoreline.
pub const COAST_FACTOR: f64 = 0.25;
/// Fraction kept in continental interiors, beyond the ramp.
pub const INTERIOR_FACTOR: f64 = 0.85;
/// Distance from water over which the kept fraction rises.
pub const RAMP_METERS: f64 = 1_200_000.;

/// Kept fraction of baseline height above sea level at distance `d` meters from water.
pub fn kept_fraction(d: f64) -> f64 {
    kept_fraction_between(d, [COAST_FACTOR, INTERIOR_FACTOR])
}

/// Kept fraction rising from `factors[0]` at the shore to `factors[1]` inland.
pub fn kept_fraction_between(d: f64, [coast, interior]: [f64; 2]) -> f64 {
    let t = (d / RAMP_METERS).clamp(0., 1.);
    coast + (interior - coast) * t * t * (3. - 2. * t)
}

/// Per-region lowering in meters (zero or negative). The compressible part of a
/// dry region is its crust baseline above `level`, capped by its height above
/// `level` without belt uplift, so every dry region stays above `level` and the
/// set of wet regions does not change.
pub fn lowering(
    surface: &Surface,
    elevation: &[f64],
    baseline: &[f64],
    convergence: &[f64],
    wet: &[u32],
    level: f64,
    factors: [f64; 2],
) -> Result<Vec<f64>, String> {
    let n = elevation.len();
    if wet.len() != n
        || baseline.len() != n
        || convergence.len() != n
        || surface.centers.len() != n
        || !level.is_finite()
    {
        return Err("Invalid coastal-lowland inputs.".into());
    }
    let mut distance = vec![f64::INFINITY; n];
    let mut queue = BinaryHeap::new();
    for i in (0..n).filter(|&i| wet[i] != 0) {
        distance[i] = 0.;
        queue.push(Reverse(Key(0., i)));
    }
    while let Some(Reverse(Key(d, cell))) = queue.pop() {
        if d > distance[cell] {
            continue;
        }
        for k in surface.offsets[cell] as usize..surface.offsets[cell + 1] as usize {
            let (j, next) = (surface.neighbors[k] as usize, d + surface.distances[k]);
            if next < distance[j] {
                distance[j] = next;
                queue.push(Reverse(Key(next, j)));
            }
        }
    }
    Ok((0..n)
        .map(|i| {
            let above = elevation[i] - level - convergence[i].max(0.);
            let compressible = (baseline[i] - level).min(above);
            if wet[i] != 0 || compressible <= 0. || !distance[i].is_finite() {
                0.
            } else {
                -(1. - kept_fraction_between(distance[i], factors)) * compressible
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Recipe, World, mountain_belts};

    fn recipe(model: &str, seed: &str) -> Recipe {
        serde_json::from_value(serde_json::json!({"schemaVersion":1,"modelVersion":model,
            "randomVersion":"fnv1a-utf8-mulberry32-1","seed":seed,"subdivision":4,"radiusMeters":6371000.,
            "plateCount":12,"maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,"continentalScale":1.,
            "reliefScale":1.,"boundaryWidthKm":300.,"detailAmplitudeMeters":300.,"terrainPreparationPasses":4,
            "water":{"mode":"coverage","fraction":0.71}})).unwrap()
    }

    #[test]
    fn kept_fraction_rises_smoothly_between_coast_and_interior() {
        assert_eq!(kept_fraction(0.), COAST_FACTOR);
        assert_eq!(kept_fraction(RAMP_METERS), INTERIOR_FACTOR);
        assert_eq!(kept_fraction(3. * RAMP_METERS), INTERIOR_FACTOR);
        let mut prior = 0.;
        for k in 0..=20 {
            let f = kept_fraction(RAMP_METERS * k as f64 / 20.);
            assert!(f >= prior && f <= 1.);
            prior = f;
        }
    }

    #[test]
    fn lowlands_keep_version_3_shorelines_belts_and_lower_only_dry_land() {
        for seed in ["first-light", "lowlands"] {
            let belts = World::generate(recipe(mountain_belts::MODEL_VERSION, seed)).unwrap();
            let low = World::generate(recipe(MODEL_VERSION, seed)).unwrap();
            // The refit level is a midpoint between the highest wet and lowest dry
            // bed, so it may move inside that gap; the wet set must not change.
            assert!(low.water.level_meters <= belts.water.level_meters);
            assert_eq!(belts.water.body_ids, low.water.body_ids);
            assert_eq!(belts.terrain.convergence, low.terrain.convergence);
            assert!(belts.terrain.lowland.iter().all(|&v| v == 0.));
            let t = &low.terrain;
            assert!(t.lowland.iter().any(|&v| v < -100.));
            for i in 0..t.elevation.len() {
                assert!(t.lowland[i] <= 0.);
                if low.water.body_ids[i] != 0 {
                    assert_eq!(t.lowland[i], 0.);
                } else {
                    assert!(t.elevation[i] > low.water.level_meters);
                }
                let sum = t.baseline[i]
                    + t.convergence[i]
                    + t.divergence[i]
                    + t.detail[i]
                    + t.preparation[i]
                    + t.lowland[i];
                assert!((t.elevation[i] - sum).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        let surface = Surface::build(1, 1000.);
        let n = surface.centers.len();
        assert!(
            lowering(
                &surface,
                &vec![0.; n],
                &vec![0.; n],
                &vec![0.; n],
                &vec![0; n - 1],
                0.,
                [COAST_FACTOR, INTERIOR_FACTOR]
            )
            .is_err()
        );
        assert!(
            lowering(
                &surface,
                &vec![0.; n],
                &vec![0.; n],
                &vec![0.; n],
                &vec![0; n],
                f64::NAN,
                [COAST_FACTOR, INTERIOR_FACTOR]
            )
            .is_err()
        );
    }
}
