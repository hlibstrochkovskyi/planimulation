//! Compare legacy mesh-indexed roots with continuous-direction candidate roots.
use planimulation_core::{
    Recipe, Surface, crust::Crust, plate_roots, tectonics::Tectonics, terrain::Terrain,
    water::WaterSettings,
};
use serde_json::{Value, json};
use std::io::Write;

fn angle_degrees(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    dot.clamp(-1., 1.).acos().to_degrees()
}

struct PreviousLevel {
    centers: Vec<[f64; 3]>,
    areas: Vec<f64>,
    legacy_roots: Vec<[f64; 3]>,
    candidate_roots: Vec<[f64; 3]>,
    legacy_owners: Vec<u32>,
    candidate_owners: Vec<u32>,
    legacy_heights: Vec<f64>,
    candidate_heights: Vec<f64>,
}

fn terrain_recipe(seed: &str, level: u32) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: "terrain-prep-1".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: seed.into(),
        subdivision: level,
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

fn run(seed_count: usize) -> Result<Value, String> {
    if seed_count == 0 || seed_count > 12 {
        return Err("Invalid plate-root study seed count.".into());
    }
    let mut rows = Vec::new();
    for seed_id in 0..seed_count {
        let seed = format!("terrain-study-{seed_id:02}");
        let directions = plate_roots::directions(&seed, 12)?;
        let mut prior: Option<PreviousLevel> = None;
        for level in 2..=6 {
            let surface = Surface::build(level, 6_371_000.);
            let legacy = Tectonics::build(&surface, &seed, 12, 8., 6_371_000.);
            let candidate =
                Tectonics::build_with_continuous_roots(&surface, &seed, 12, 8., 6_371_000.)?;
            let projected = plate_roots::project(&surface, &directions)?;
            if candidate.seeds != projected
                || candidate.angular_velocities != legacy.angular_velocities
            {
                return Err("Candidate roots changed independent motion or projection.".into());
            }
            let crust = Crust::build(&surface, &seed, 0.38, 1.);
            let settings = terrain_recipe(&seed, level);
            let legacy_heights = Terrain::build(&surface, &crust, &legacy, &settings).elevation;
            let candidate_heights =
                Terrain::build(&surface, &crust, &candidate, &settings).elevation;
            let old_positions: Vec<_> = legacy
                .seeds
                .iter()
                .map(|&id| surface.centers[id as usize])
                .collect();
            let new_positions: Vec<_> = projected
                .iter()
                .map(|&id| surface.centers[id as usize])
                .collect();
            let errors: Vec<_> = directions
                .iter()
                .zip(&new_positions)
                .map(|(target, position)| angle_degrees(*target, *position))
                .collect();
            let mean_error = errors.iter().sum::<f64>() / errors.len() as f64;
            if let Some(previous) = &prior {
                let common = previous.centers.len();
                if previous.centers != surface.centers[..common] {
                    return Err("Refined mesh did not retain coarse plate-study locations.".into());
                }
                let legacy_shifts: Vec<_> = previous
                    .legacy_roots
                    .iter()
                    .zip(&old_positions)
                    .map(|(before, after)| angle_degrees(*before, *after))
                    .collect();
                let candidate_shifts: Vec<_> = previous
                    .candidate_roots
                    .iter()
                    .zip(&new_positions)
                    .map(|(before, after)| angle_degrees(*before, *after))
                    .collect();
                let total_area: f64 = previous.areas.iter().sum();
                let mut legacy_owner_disagreement = 0.;
                let mut candidate_owner_disagreement = 0.;
                let mut legacy_height_difference = 0.;
                let mut candidate_height_difference = 0.;
                for region in 0..common {
                    let area = previous.areas[region];
                    if previous.legacy_owners[region] != legacy.owners[region] {
                        legacy_owner_disagreement += area;
                    }
                    if previous.candidate_owners[region] != candidate.owners[region] {
                        candidate_owner_disagreement += area;
                    }
                    legacy_height_difference +=
                        area * (legacy_heights[region] - previous.legacy_heights[region]).abs();
                    candidate_height_difference += area
                        * (candidate_heights[region] - previous.candidate_heights[region]).abs();
                }
                rows.push(json!({
                    "seed": seed, "coarseSubdivision": level - 1, "fineSubdivision": level,
                    "commonLocations": common,
                    "legacyMeanRootShiftDegrees": legacy_shifts.iter().sum::<f64>() / legacy_shifts.len() as f64,
                    "legacyMaximumRootShiftDegrees": legacy_shifts.iter().copied().fold(0., f64::max),
                    "candidateMeanRootShiftDegrees": candidate_shifts.iter().sum::<f64>() / candidate_shifts.len() as f64,
                    "candidateMaximumRootShiftDegrees": candidate_shifts.iter().copied().fold(0., f64::max),
                    "candidateMeanProjectionErrorDegrees": mean_error,
                    "candidateMaximumProjectionErrorDegrees": errors.iter().copied().fold(0., f64::max),
                    "legacyOwnerDisagreementAreaFraction": legacy_owner_disagreement / total_area,
                    "candidateOwnerDisagreementAreaFraction": candidate_owner_disagreement / total_area,
                    "legacyMeanAbsoluteRawHeightDifferenceMeters": legacy_height_difference / total_area,
                    "candidateMeanAbsoluteRawHeightDifferenceMeters": candidate_height_difference / total_area,
                }));
            }
            prior = Some(PreviousLevel {
                centers: surface.centers,
                areas: surface.areas,
                legacy_roots: old_positions,
                candidate_roots: new_positions,
                legacy_owners: legacy.owners,
                candidate_owners: candidate.owners,
                legacy_heights,
                candidate_heights,
            });
        }
    }
    Ok(json!({
        "reportVersion": 1,
        "scope": "Standalone candidate partition and raw terrain comparison; no generated-world model change",
        "proposalVersion": plate_roots::PROPOSAL_VERSION,
        "seedCount": seed_count, "plateCount": 12,
        "seeds": "terrain-study-00 through terrain-study-11, truncated by seedCount",
        "rows": rows,
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let report = match args.as_slice() {
        [] => run(12)?,
        [mode] if mode == "--quick" => run(2)?,
        _ => return Err("Usage: plate_roots_study [--quick]".into()),
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
    fn report_replays_and_candidate_shifts_remain_bounded() {
        let a = run(1).unwrap();
        assert_eq!(a, run(1).unwrap());
        assert_eq!(a["rows"].as_array().unwrap().len(), 4);
        for row in a["rows"].as_array().unwrap() {
            assert!(row["candidateMeanProjectionErrorDegrees"].as_f64().unwrap() >= 0.);
            assert!(row["candidateMeanRootShiftDegrees"].as_f64().unwrap() < 20.);
        }
        assert!(run(0).is_err());
        assert!(run(13).is_err());
    }
}
