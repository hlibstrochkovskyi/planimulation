//! Reproducible geometry survey for the opt-in continuous plate-root model.
use planimulation_core::{Surface, plate_roots, tectonics::Tectonics};
use serde_json::{Value, json};
use std::io::Write;

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.
    } else {
        values[mid]
    }
}

fn summary(values: &mut [f64]) -> Value {
    let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    json!({ "minimum": minimum, "median": median(values), "maximum": maximum })
}

fn plate_area_extremes(surface: &Surface, tectonics: &Tectonics, count: usize) -> (f64, f64) {
    let mut areas = vec![0.; count];
    for (region, &plate) in tectonics.owners.iter().enumerate() {
        areas[plate as usize] += surface.areas[region];
    }
    let total: f64 = surface.areas.iter().sum();
    (
        areas.iter().copied().fold(f64::INFINITY, f64::min) / total,
        areas.iter().copied().fold(0., f64::max) / total,
    )
}

fn root_projection(surface: &Surface, seed: &str, count: u32, projected: &[u32]) -> (f64, usize) {
    let directions = plate_roots::directions(seed, count).unwrap();
    let mut maximum_error = 0.;
    let mut displaced = 0;
    for (target, &region) in directions.iter().zip(projected) {
        let chosen: f64 = target
            .iter()
            .zip(surface.centers[region as usize])
            .map(|(a, b)| a * b)
            .sum();
        maximum_error = f64::max(maximum_error, chosen.clamp(-1., 1.).acos().to_degrees());
        let nearest = surface
            .centers
            .iter()
            .enumerate()
            .max_by(|(a_id, a), (b_id, b)| {
                let a_score: f64 = a.iter().zip(target).map(|(x, y)| x * y).sum();
                let b_score: f64 = b.iter().zip(target).map(|(x, y)| x * y).sum();
                a_score.total_cmp(&b_score).then_with(|| b_id.cmp(a_id))
            })
            .unwrap()
            .0;
        if nearest != region as usize {
            displaced += 1;
        }
    }
    (maximum_error, displaced)
}

fn run(seed_count: usize) -> Result<Value, String> {
    if !(1..=100).contains(&seed_count) {
        return Err("Plate-root survey requires 1–100 seeds.".into());
    }
    let mut rows = Vec::new();
    for level in [0, 1, 2, 4] {
        let surface = Surface::build(level, 6_371_000.);
        let counts: &[u32] = if level == 0 {
            &[2, 7, 12]
        } else {
            &[2, 12, 32]
        };
        for &count in counts {
            let mut legacy_min = Vec::new();
            let mut legacy_max = Vec::new();
            let mut candidate_min = Vec::new();
            let mut candidate_max = Vec::new();
            let mut legacy_segments = Vec::new();
            let mut candidate_segments = Vec::new();
            let mut projection_error = Vec::new();
            let mut displaced_roots = 0;
            for seed_id in 0..seed_count {
                let seed = format!("plate-survey-{seed_id:03}");
                let legacy = Tectonics::build(&surface, &seed, count, 8., 6_371_000.);
                let candidate =
                    Tectonics::build_with_continuous_roots(&surface, &seed, count, 8., 6_371_000.)?;
                if legacy.angular_velocities != candidate.angular_velocities {
                    return Err("Plate-root selector changed the independent motion stream.".into());
                }
                let (low, high) = plate_area_extremes(&surface, &legacy, count as usize);
                legacy_min.push(low);
                legacy_max.push(high);
                let (low, high) = plate_area_extremes(&surface, &candidate, count as usize);
                candidate_min.push(low);
                candidate_max.push(high);
                legacy_segments.push(legacy.boundary_types.len() as f64);
                candidate_segments.push(candidate.boundary_types.len() as f64);
                let (error, displaced) = root_projection(&surface, &seed, count, &candidate.seeds);
                projection_error.push(error);
                displaced_roots += displaced;
            }
            rows.push(json!({
                "subdivision": level, "plateCount": count, "seedCount": seed_count,
                "legacySmallestPlateAreaFraction": summary(&mut legacy_min),
                "candidateSmallestPlateAreaFraction": summary(&mut candidate_min),
                "legacyLargestPlateAreaFraction": summary(&mut legacy_max),
                "candidateLargestPlateAreaFraction": summary(&mut candidate_max),
                "legacyBoundarySegments": summary(&mut legacy_segments),
                "candidateBoundarySegments": summary(&mut candidate_segments),
                "candidateMaximumProjectionErrorDegrees": summary(&mut projection_error),
                "candidateRootsDisplacedByUniqueProjection": displaced_roots,
            }));
        }
    }
    Ok(json!({
        "reportVersion": 1,
        "modelVersion": "continuous-plates-1",
        "scope": "Static plate geometry only; no terrain, water, geological realism, or runtime claims",
        "seeds": "plate-survey-000 through plate-survey-099, truncated by seedCount",
        "rows": rows,
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let report = match args.as_slice() {
        [] => run(100)?,
        [mode] if mode == "--quick" => run(3)?,
        _ => return Err("Usage: plate_roots_survey [--quick]".into()),
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
    fn report_is_deterministic_and_retains_nonempty_plates() {
        let a = run(1).unwrap();
        assert_eq!(a, run(1).unwrap());
        assert_eq!(a["rows"].as_array().unwrap().len(), 12);
        for row in a["rows"].as_array().unwrap() {
            assert!(
                row["candidateSmallestPlateAreaFraction"]["minimum"]
                    .as_f64()
                    .unwrap()
                    > 0.
            );
            assert!(
                row["candidateMaximumProjectionErrorDegrees"]["maximum"]
                    .as_f64()
                    .unwrap()
                    <= 180.
            );
        }
        assert!(run(0).is_err());
        assert!(run(101).is_err());
    }
}
