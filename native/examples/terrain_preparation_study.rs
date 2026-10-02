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

struct PreviousLevel {
    subdivision: u32,
    centers: Vec<[f64; 3]>,
    areas: Vec<f64>,
    raw: Vec<f64>,
    deltas: Vec<Vec<f64>>,
}

fn paired_locations(
    previous: &PreviousLevel,
    surface: &Surface,
    raw: &[f64],
    delta: &[f64],
    pass_index: usize,
    passes: u32,
) -> Result<Value, String> {
    let n = previous.raw.len();
    if previous.centers != surface.centers[..n]
        || previous.areas.len() != n
        || previous.deltas[pass_index].len() != n
        || raw.len() < n
        || delta.len() < n
    {
        return Err("Refined mesh did not retain the coarse sample locations.".into());
    }
    let mut raw_difference = 0.;
    let mut preparation_difference = 0.;
    let mut activation_disagreement = 0.;
    let total_area: f64 = previous.areas.iter().sum();
    for region in 0..n {
        let area = previous.areas[region];
        let coarse_delta = previous.deltas[pass_index][region];
        let fine_delta = delta[region];
        raw_difference += area * (raw[region] - previous.raw[region]).abs();
        preparation_difference += area * (fine_delta - coarse_delta).abs();
        if (coarse_delta.abs() > 1e-9) != (fine_delta.abs() > 1e-9) {
            activation_disagreement += area;
        }
    }
    Ok(json!({
        "coarseSubdivision": previous.subdivision,
        "fineSubdivision": previous.subdivision + 1,
        "commonLocations": n,
        "requestedPasses": passes,
        "meanAbsoluteRawDifferenceMeters": raw_difference / total_area,
        "meanAbsolutePreparationDifferenceMeters": preparation_difference / total_area,
        "activationDisagreementAreaFraction": activation_disagreement / total_area,
    }))
}

fn analytic_height(center: [f64; 3]) -> f64 {
    let [x, y, z] = center;
    4000. * (11. * x + 3. * y).sin()
        + 3000. * (7. * y - 5. * z).cos()
        + 2000. * (13. * z - 4. * x).sin()
}

fn analytic_pairs(levels: &[u32], passes: &[u32]) -> Result<Vec<Value>, String> {
    let mut previous: Option<PreviousLevel> = None;
    let mut pairs = Vec::new();
    for &level in levels {
        let surface = Surface::build(level, 6_371_000.);
        let raw: Vec<f64> = surface
            .centers
            .iter()
            .copied()
            .map(analytic_height)
            .collect();
        let mut deltas = Vec::new();
        for (pass_index, &count) in passes.iter().enumerate() {
            let result = prepare(&surface, &raw, count)?;
            let delta: Vec<f64> = result
                .elevation_meters
                .iter()
                .zip(&raw)
                .map(|(a, b)| a - b)
                .collect();
            if let Some(coarse) = &previous {
                pairs.push(paired_locations(
                    coarse, &surface, &raw, &delta, pass_index, count,
                )?);
            }
            deltas.push(delta);
        }
        previous = Some(PreviousLevel {
            subdivision: level,
            centers: surface.centers,
            areas: surface.areas,
            raw,
            deltas,
        });
    }
    Ok(pairs)
}

fn run(seed_count: usize, levels: &[u32], passes: &[u32]) -> Result<Value, String> {
    if seed_count == 0
        || seed_count > 12
        || levels.is_empty()
        || passes.is_empty()
        || levels.iter().any(|&level| !(2..=6).contains(&level))
        || levels.windows(2).any(|pair| pair[1] != pair[0] + 1)
        || passes.iter().any(|&count| count > MAX_PASSES)
    {
        return Err("Invalid bounded terrain-preparation study parameters.".into());
    }
    let mut samples = Vec::new();
    let mut generated_pairs = Vec::new();
    for seed_id in 0..seed_count {
        let seed = format!("terrain-study-{seed_id:02}");
        let mut previous: Option<PreviousLevel> = None;
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
            let mut deltas = Vec::new();
            for (pass_index, &count) in passes.iter().enumerate() {
                let result = prepare(&surface, &raw, count)?;
                let after = &result.elevation_meters;
                let delta: Vec<f64> = after.iter().zip(&raw).map(|(a, b)| a - b).collect();
                if let Some(coarse) = &previous {
                    let mut pair =
                        paired_locations(coarse, &surface, &raw, &delta, pass_index, count)?;
                    pair["seed"] = json!(seed);
                    generated_pairs.push(pair);
                }
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
                deltas.push(delta);
            }
            previous = Some(PreviousLevel {
                subdivision: level,
                centers: surface.centers,
                areas: surface.areas,
                raw,
                deltas,
            });
        }
    }
    let analytic_pairs = analytic_pairs(levels, passes)?;
    Ok(json!({
        "reportVersion": 2,
        "scope": "Dry initial-world preparation sensitivity, not elapsed-time erosion or resolution convergence proof",
        "seedCount": seed_count,
        "levels": levels,
        "passes": passes,
        "kernel": {"slopeThreshold": SLOPE_THRESHOLD, "relaxationFraction": RELAXATION_FRACTION,
            "maximumChangePerPassMeters": MAX_CHANGE_PER_PASS_METERS},
        "recipeTemplate": recipe("terrain-study-00", 2),
        "samples": samples,
        "pairedGeneratedLocations": generated_pairs,
        "pairedAnalyticLocations": analytic_pairs,
        "analyticBed": "4000*sin(11*x+3*y)+3000*cos(7*y-5*z)+2000*sin(13*z-4*x), unit-sphere coordinates, meters",
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

    #[test]
    fn common_locations_separate_upstream_changes_from_kernel_resolution_effects() {
        let report = run(1, &[2, 3], &[0, 4]).unwrap();
        assert_eq!(report["reportVersion"], 2);
        let generated = report["pairedGeneratedLocations"].as_array().unwrap();
        let analytic = report["pairedAnalyticLocations"].as_array().unwrap();
        assert_eq!((generated.len(), analytic.len()), (2, 2));
        assert_eq!(generated[0]["commonLocations"], 162);
        assert_eq!(generated[0]["meanAbsolutePreparationDifferenceMeters"], 0.);
        assert!(
            generated[0]["meanAbsoluteRawDifferenceMeters"]
                .as_f64()
                .unwrap()
                > 0.
        );
        assert_eq!(analytic[0]["meanAbsoluteRawDifferenceMeters"], 0.);
        assert_eq!(analytic[1]["meanAbsoluteRawDifferenceMeters"], 0.);
        assert!(
            analytic[1]["meanAbsolutePreparationDifferenceMeters"]
                .as_f64()
                .unwrap()
                > 0.
        );
        assert!(run(1, &[2, 4], &[4]).is_err());
    }
}
