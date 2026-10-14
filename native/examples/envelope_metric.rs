//! Read-only point-source control for graph-envelope directional distance bias.
use planimulation_core::{Surface, terrain};
use serde_json::json;
use std::{io::Write, path::PathBuf, time::Instant};
const RADIUS: f64 = 6_371_000.;
const WIDTH: f64 = 300_000.;
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let sine = cross.iter().map(|v| v * v).sum::<f64>().sqrt();
    let cosine = a.into_iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    sine.atan2(cosine) * RADIUS
}
fn parse(args: &[String]) -> Result<(PathBuf, u32, usize), String> {
    if !(2..=3).contains(&args.len()) || args[0].trim().is_empty() {
        return Err(
            "Usage: envelope_metric NEW_REPORT.json SUBDIVISION(4..8) [SOURCE_REGION]".into(),
        );
    }
    let output = PathBuf::from(&args[0]);
    if std::fs::symlink_metadata(&output).is_ok() {
        return Err("Envelope metric report requires a new output.".into());
    }
    if output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .is_some_and(|p| !p.is_dir())
    {
        return Err("Envelope metric parent must exist.".into());
    }
    let level = args[1]
        .parse::<u32>()
        .map_err(|_| "Invalid envelope metric sampling level.")?;
    if !(4..=8).contains(&level) {
        return Err("Envelope metric supports levels4–8.".into());
    }
    let source = args.get(2).map_or(Ok(0), |v| {
        v.parse::<usize>()
            .map_err(|_| "Invalid envelope source region.")
    })?;
    if source >= 10 * 4_usize.pow(level) + 2 {
        return Err("Envelope source region exceeds sampling grid.".into());
    }
    Ok((output, level, source))
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (output, level, source) = parse(&args)?;
    let started = Instant::now();
    let s = Surface::build_generation_probe(level, RADIUS)?;
    let mut sources = vec![0.; s.centers.len()];
    sources[source] = 1.;
    let profile = terrain::spread(&s, &sources, WIDTH);
    let mut errors = Vec::new();
    let mut area = 0.;
    let mut weighted = 0.;
    let mut maximum = 0_f64;
    let (mut near_area, mut near_weighted, mut near_maximum) = (0., 0., 0_f64);
    for (i, &value) in profile.iter().enumerate() {
        if !value.is_finite() || value <= 0. || value > 1. {
            return Err("Invalid observed graph envelope.".into());
        }
        let exact = distance(s.centers[source], s.centers[i]);
        if exact < 500_000. {
            continue;
        }
        let graph = -WIDTH * value.ln();
        if !graph.is_finite() || graph + 1e-6 < exact {
            return Err("Observed graph distance violates spherical lower bound.".into());
        }
        let relative = (graph - exact) / exact;
        errors.push((relative, s.areas[i]));
        area += s.areas[i];
        weighted += s.areas[i] * relative;
        maximum = maximum.max(graph - exact);
        if exact <= 1_500_000. {
            near_area += s.areas[i];
            near_weighted += s.areas[i] * relative;
            near_maximum = near_maximum.max(relative);
        }
    }
    errors.sort_by(|a, b| a.0.total_cmp(&b.0));
    let quantile = |q: f64| {
        let mut cumulative = 0.;
        for &(e, a) in &errors {
            cumulative += a;
            if cumulative >= q * area {
                return e;
            }
        }
        errors.last().unwrap().0
    };
    let report = json!({"reportVersion":"envelope-distance-control-2","samplingSubdivision":level,
        "regions":s.centers.len(),"sourceRegion":source,"sourceDirection":s.centers[source],
        "sourceDegree":s.offsets[source+1]-s.offsets[source],
        "radiusMeters":RADIUS,"widthMeters":WIDTH,"comparisonMinimumDistanceMeters":500000.,
        "areaWeightedMeanRelativeDistanceError":weighted/area,"areaWeightedMedianRelativeDistanceError":quantile(0.5),
        "areaWeightedP95RelativeDistanceError":quantile(0.95),"maximumRelativeDistanceError":errors.last().unwrap().0,
        "maximumAbsoluteDistanceErrorMeters":maximum,"comparedReferenceAreaSquareMeters":area,
        "nearField":{"minimumDistanceMeters":500000.,"maximumDistanceMeters":1500000.,
            "comparedReferenceAreaSquareMeters":near_area,"areaWeightedMeanRelativeDistanceError":near_weighted/near_area,
            "maximumRelativeDistanceError":near_maximum},
        "elapsedSeconds":started.elapsed().as_secs_f64(),
        "scope":"Invert the unchanged single-source exponential graph envelope; compare against analytic great-circle distance at actual canonical nodes. No bed/flow/water/plate field is mutated. Shared source and fixed physical exclusion across grids. This point-source control measures graph-metric error, not every tectonic shape artifact or a replacement source model."});
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|e| e.to_string())?;
    f.write_all(&serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analytic_distance_and_bounded_output_controls_are_explicit() {
        assert_eq!(distance([1., 0., 0.], [1., 0., 0.]), 0.);
        assert_eq!(
            distance([1., 0., 0.], [0., 1., 0.]),
            std::f64::consts::FRAC_PI_2 * RADIUS
        );
        assert_eq!(
            distance([1., 0., 0.], [-1., 0., 0.]),
            std::f64::consts::PI * RADIUS
        );
        assert!(parse(&[]).is_err());
        assert!(parse(&[".".into(), "8".into()]).is_err());
        assert!(parse(&["envelope-test-uncreated.json".into(), "9".into()]).is_err());
        assert!(
            parse(&[
                "envelope-test-uncreated.json".into(),
                "4".into(),
                "999999".into()
            ])
            .is_err()
        );
    }
}
