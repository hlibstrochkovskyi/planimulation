//! Reproducible sensitivity measurements for the versioned dry preparation heuristic.
use planimulation_core::{
    Recipe, Surface,
    crust::Crust,
    tectonics::Tectonics,
    terrain::Terrain,
    terrain_preparation::{
        MAX_CHANGE_PER_PASS_METERS, MAX_PASSES, RELAXATION_FRACTION, SLOPE_THRESHOLD, prepare,
    },
    water::{Water, WaterSettings},
};
use serde_json::{Value, json};
use std::io::Write;

fn recipe(seed: &str, subdivision: u32) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: "terrain-prep-1".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: seed.into(),
        subdivision,
        radius_meters: 6_371_000.,
        plate_count: 12,
        max_plate_speed_cm_per_year: 8.,
        continental_fraction: 0.38,
        continental_scale: 1.,
        relief_scale: 1.,
        boundary_width_km: 300.,
        detail_amplitude_meters: 300.,
        terrain_preparation_passes: Some(0),
        water: WaterSettings::Coverage { fraction: 0.71 },
    }
}

fn slope_diagnostics(surface: &Surface, heights: &[f64]) -> (usize, f64, f64) {
    let mut count = 0;
    let mut weighted_square = 0.;
    let mut edge_weight = 0.;
    let mut maximum: f64 = 0.;
    for region in 0..heights.len() {
        for edge in surface.offsets[region] as usize..surface.offsets[region + 1] as usize {
            let neighbor = surface.neighbors[edge] as usize;
            if neighbor > region {
                let slope = (heights[region] - heights[neighbor]).abs() / surface.distances[edge];
                let weight = (surface.areas[region] + surface.areas[neighbor]) / 2.;
                weighted_square += weight * slope * slope;
                edge_weight += weight;
                maximum = maximum.max(slope);
                if slope > SLOPE_THRESHOLD {
                    count += 1;
                }
            }
        }
    }
    (count, (weighted_square / edge_weight).sqrt(), maximum)
}

fn run(seed_count: usize, levels: &[u32], passes: &[u32]) -> Result<Value, String> {
    if seed_count == 0
        || seed_count > 12
        || levels.is_empty()
        || passes.is_empty()
        || levels.iter().any(|&level| !(2..=6).contains(&level))
        || passes.iter().any(|&count| count > MAX_PASSES)
    {
        return Err("Invalid bounded terrain-preparation study parameters.".into());
    }
    let mut samples = Vec::new();
    for seed_id in 0..seed_count {
        let seed = format!("terrain-study-{seed_id:02}");
        for &level in levels {
            let settings = recipe(&seed, level);
            settings.validate()?;
            let surface = Surface::build(level, settings.radius_meters);
            let crust = Crust::build(
                &surface,
                &seed,
                settings.continental_fraction,
                settings.continental_scale,
            );
            let plates = Tectonics::build(
                &surface,
                &seed,
                settings.plate_count,
                settings.max_plate_speed_cm_per_year,
                settings.radius_meters,
            );
            let raw = Terrain::build(&surface, &crust, &plates, &settings).elevation;
            let raw_water = Water::generate(&surface, &raw, &settings.water)?;
            let area: f64 = surface.areas.iter().sum();
            let material_before: f64 = surface.areas.iter().zip(&raw).map(|(a, h)| a * h).sum();
            let material_scale: f64 = surface
                .areas
                .iter()
                .zip(&raw)
                .map(|(a, h)| (a * h).abs())
                .sum();
            let (steep_before, rms_slope_before, max_slope_before) =
                slope_diagnostics(&surface, &raw);
            for &count in passes {
                let result = prepare(&surface, &raw, count)?;
                let after = &result.elevation_meters;
                let prepared_water = Water::generate(&surface, after, &settings.water)?;
                let mut changed_area = 0.;
                let mut weighted_abs = 0.;
                let mut weighted_squared = 0.;
                let mut maximum: f64 = 0.;
                for region in 0..raw.len() {
                    let difference = (after[region] - raw[region]).abs();
                    if difference > 1e-9 {
                        changed_area += surface.areas[region];
                    }
                    weighted_abs += surface.areas[region] * difference;
                    weighted_squared += surface.areas[region] * difference * difference;
                    maximum = maximum.max(difference);
                }
                let material_after: f64 = surface.areas.iter().zip(after).map(|(a, h)| a * h).sum();
                let (steep_after, rms_slope_after, max_slope_after) =
                    slope_diagnostics(&surface, after);
                samples.push(json!({
                    "seed": seed, "subdivision": level, "regions": raw.len(),
                    "requestedPasses": count, "appliedPasses": result.applied_passes,
                    "changedAreaFraction": changed_area / area,
                    "meanAbsoluteChangeMeters": weighted_abs / area,
                    "rmsChangeMeters": (weighted_squared / area).sqrt(),
                    "maximumChangeMeters": maximum,
                    "grossTransportEquivalentMeters": result.transported_cubic_meters / area,
                    "steepEdgesBefore": steep_before,
                    "steepEdgesAfter": steep_after,
                    "rmsSlopeBefore": rms_slope_before,
                    "rmsSlopeAfter": rms_slope_after,
                    "maximumSlopeBefore": max_slope_before,
                    "maximumSlopeAfter": max_slope_after,
                    "relativeMaterialLedgerError": (material_after - material_before).abs() / material_scale.max(1.),
                    "initialWaterVolumeChangeFraction":
                        (prepared_water.resolved_volume_cubic_meters - raw_water.resolved_volume_cubic_meters)
                            / raw_water.resolved_volume_cubic_meters.max(1.),
                }));
            }
        }
    }
    Ok(json!({
        "reportVersion": 1,
        "scope": "Dry initial-world preparation sensitivity, not elapsed-time erosion or resolution convergence proof",
        "seedCount": seed_count,
        "levels": levels,
        "passes": passes,
        "kernel": {"slopeThreshold": SLOPE_THRESHOLD, "relaxationFraction": RELAXATION_FRACTION,
            "maximumChangePerPassMeters": MAX_CHANGE_PER_PASS_METERS},
        "recipeTemplate": recipe("terrain-study-00", 2),
        "samples": samples,
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let report = match args.as_slice() {
        [] => run(12, &[2, 3, 4, 5, 6], &[0, 4, 8, 16])?,
        [mode] if mode == "--quick" => run(2, &[2, 3], &[0, 4, 8])?,
        _ => return Err("Usage: terrain_preparation_study [--quick]".into()),
    };
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, &report)?;
    writeln!(out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn study_replays_and_reports_a_zero_pass_control() {
        let a = run(1, &[4], &[0, 4]).unwrap();
        assert_eq!(a, run(1, &[4], &[0, 4]).unwrap());
        assert_eq!(a["kernel"]["slopeThreshold"], SLOPE_THRESHOLD);
        let samples = a["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0]["changedAreaFraction"], 0.);
        assert_eq!(samples[0]["grossTransportEquivalentMeters"], 0.);
        assert_eq!(samples[0]["initialWaterVolumeChangeFraction"], 0.);
        assert!(samples[1]["relativeMaterialLedgerError"].as_f64().unwrap() < 1e-12);
        assert!(
            samples[1]["rmsSlopeAfter"].as_f64().unwrap()
                < samples[1]["rmsSlopeBefore"].as_f64().unwrap()
        );
        assert!(run(1, &[7], &[4]).is_err());
        assert!(run(1, &[4], &[17]).is_err());
    }
}
