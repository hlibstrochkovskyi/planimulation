//! Bounded geography review, not a calibrated realism score or dense desktop run.
#[path = "support/depression_atlas.rs"]
mod depression_atlas;
#[path = "support/geography_atlas.rs"]
mod geography_atlas;

use planimulation_core::{
    Recipe, Surface, World, continental_uplands, depression_analysis, drainage::Drainage,
    geography, geography_metrics, hash, water::WaterSettings, wire,
};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

fn recipe(seed: &str, subdivision: u32, model: &str) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: model.into(),
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
        terrain_preparation_passes: Some(4),
        water: WaterSettings::Coverage { fraction: 0.71 },
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
    file.write_all(bytes)
        .map_err(|e| format!("Cannot write {}: {e}", path.display()))
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    write_new(path, &bytes)
}

fn checksum(world: &World) -> Result<String, String> {
    // Exactly the native snapshot fingerprint construction; not cryptographic.
    let mut bytes = serde_json::to_vec(&world.recipe).map_err(|e| e.to_string())?;
    bytes.extend_from_slice(&wire::arrays(world));
    Ok(format!("{:08x}", hash(&bytes)))
}

fn record(world: &World, generation_seconds: f64) -> Result<Value, String> {
    let started = Instant::now();
    let metrics =
        geography_metrics::analyze(&world.surface, &world.terrain.elevation, &world.water)?;
    let metrics_seconds = started.elapsed().as_secs_f64();
    let fingerprint = checksum(world)?;
    let mut dry_area = 0.;
    let mut saturated_area = 0.;
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for i in 0..world.surface.areas.len() {
        if world.water.body_ids[i] == 0 {
            dry_area += world.surface.areas[i];
            if world.crust.continentality[i] == 1. {
                saturated_area += world.surface.areas[i];
                minimum = minimum.min(world.terrain.baseline[i]);
                maximum = maximum.max(world.terrain.baseline[i]);
            }
        }
    }
    Ok(json!({
        "recipe":world.recipe, "nativeCompatibleInitialChecksum":fingerprint,
        "checksumConstruction":"32-bit FNV-1a of native serde_json(recipe) followed by wire::arrays(world); not cryptographic",
        "metrics":metrics,
        "initialWater":{"levelMeters":world.water.level_meters,"resolvedVolumeCubicMeters":world.water.resolved_volume_cubic_meters},
        "continentalInterior":{"fullyContinentalFractionOfDryArea":if dry_area>0. {saturated_area/dry_area} else {0.},
            "baselineRangeMeters":if saturated_area>0. {Some([minimum,maximum])} else {None}},
        "timingSeconds":{"generation":generation_seconds,"metrics":metrics_seconds},
        "timingScope":"Observations only; timings are excluded from the native-compatible initial checksum"
    }))
}

fn shared_crust(candidate: &World) -> Result<Value, String> {
    let surface = Surface::build(4, candidate.recipe.radius_meters);
    if !candidate.surface.centers.starts_with(&surface.centers) {
        return Err("Selected surface does not preserve the level-4 shared centers.".into());
    }
    let build = if candidate.recipe.model_version == continental_uplands::MODEL_VERSION {
        continental_uplands::build_crust
    } else {
        geography::build_crust
    };
    let crust = build(
        &surface,
        &candidate.recipe.seed,
        candidate.recipe.continental_fraction,
        candidate.recipe.continental_scale,
    )?;
    let n = surface.centers.len();
    let equal = crust.threshold.to_bits() == candidate.crust.threshold.to_bits()
        && crust.potential == candidate.crust.potential[..n]
        && crust.continentality == candidate.crust.continentality[..n]
        && crust.thickness_meters == candidate.crust.thickness_meters[..n]
        && crust.density_kg_per_cubic_meter == candidate.crust.density_kg_per_cubic_meter[..n];
    if !equal {
        return Err("Mesh-independent crust sampling failed at shared centers.".into());
    }
    Ok(
        json!({"seed":candidate.recipe.seed,"referenceSubdivision":4,
        "selectedSubdivision":candidate.recipe.subdivision,"sharedCenterCount":n,
        "exactlyEqual":true,"fixedReferenceThreshold":candidate.crust.threshold,
        "scope":"Crust sampling/material properties only. Boundary propagation, dry smoothing, terrain and hydrology are not claimed mesh independent. Selecting level 4 compares the same grid."}),
    )
}

fn dense_crust() -> Result<Value, String> {
    let started = Instant::now();
    let surface = Surface::build_generation_probe(8, 6_371_000.)?;
    let geometry_seconds = started.elapsed().as_secs_f64();
    let total_area: f64 = surface.areas.iter().sum();
    let mut rows = Vec::new();
    for scale in [0.5, 1., 2.] {
        let started = Instant::now();
        let crust = geography::build_crust(&surface, "first-light", 0.38, scale)?;
        let seconds = started.elapsed().as_secs_f64();
        let represented: f64 = surface
            .areas
            .iter()
            .zip(&crust.potential)
            .filter_map(|(&a, &p)| (p > crust.threshold).then_some(a))
            .sum::<f64>()
            / total_area;
        let mean_continentality: f64 = surface
            .areas
            .iter()
            .zip(&crust.continentality)
            .map(|(&a, &c)| (a / total_area) * c)
            .sum();
        if !represented.is_finite() || !mean_continentality.is_finite() {
            return Err("Dense crust diagnostic produced nonfinite represented areas.".into());
        }
        rows.push(json!({"seed":"first-light","continentalScale":scale,"targetContinentalFraction":0.38,
            "fixedReferenceThreshold":crust.threshold,"representedStrictSuperlevelAreaFraction":represented,
            "representedMinusTargetFraction":represented-0.38,"areaWeightedMeanContinentality":mean_continentality,
            "samplingSeconds":seconds}));
    }
    Ok(
        json!({"reportVersion":"dense-crust-diagnostic-1","modelVersion":geography::MODEL_VERSION,
        "probeSubdivision":8,"regionCount":surface.centers.len(),
        "thresholdReferenceSubdivision":geography::REFERENCE_SUBDIVISION,"geometrySeconds":geometry_seconds,"rows":rows,
        "scope":"Generation-probe-only crust fields. Fixed reference threshold is not refitted to the dense grid. No full World, climate, IPC, checkpoint, rendering or supported desktop resolution is instantiated. Strict potential classification and smooth mean continentality are distinct quantities."}),
    )
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Original,
    DenseCrust,
    Uplands,
}

fn fixed_outlet_drainage(world: &World, mask: &[u32]) -> Value {
    let graph = Drainage::build(&world.surface, &world.terrain.elevation, mask);
    let mut area = 0.;
    let mut closed = 0.;
    let mut sinks = 0;
    for (i, &body) in mask.iter().enumerate() {
        if body != 0 {
            continue;
        }
        area += world.surface.areas[i];
        if mask[graph.outlets[i] as usize] == 0 {
            closed += world.surface.areas[i];
        }
        if graph.receivers[i] as usize == i {
            sinks += 1;
        }
    }
    json!({"closedDrySinkCount":sinks,"closedDryAreaFraction":if area>0. {closed/area} else {0.},
        "scope":"Static receiver ablation with version-1 terminal mask fixed. This exogenous mask is not candidate water ownership or a simulated water state."})
}

fn parse_args(args: &[String]) -> Result<(PathBuf, u32, Mode), String> {
    if !(2..=3).contains(&args.len())
        || (args.len() == 3 && args[2] != "--dense-crust" && args[2] != "--uplands")
    {
        return Err(
            "Usage: geography_cohort NEW_DIRECTORY SUBDIVISION(4..6) [--dense-crust | --uplands]"
                .into(),
        );
    }
    let level: u32 = args[1]
        .parse()
        .map_err(|_| "Invalid geography subdivision.")?;
    if !(4..=6).contains(&level) {
        return Err("Geography cohort supports subdivisions 4–6 only.".into());
    }
    if args[0].trim().is_empty() {
        return Err("A new output directory is required.".into());
    }
    let mode = match args.get(2).map(String::as_str) {
        Some("--dense-crust") => Mode::DenseCrust,
        Some("--uplands") => Mode::Uplands,
        _ => Mode::Original,
    };
    Ok((PathBuf::from(&args[0]), level, mode))
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (directory, subdivision, mode) = parse_args(&args)?;
    // Atomic refusal of existing files/directories/symlinks; never overwrite.
    std::fs::create_dir(&directory)
        .map_err(|e| format!("Cannot create new directory {}: {e}", directory.display()))?;
    let mut cases = vec![
        ("first-light".into(), "ordinary-00".into(), 1., 1., 300.),
        ("4836187e".into(), "ordinary-01".into(), 1., 1., 300.),
    ];
    for i in 0..18 {
        cases.push((
            format!("geography-{i:02}"),
            format!("ordinary-{:02}", i + 2),
            1.,
            1.,
            300.,
        ));
    }
    cases.extend([
        ("first-light".into(), "fragmented".into(), 0.5, 1., 300.),
        ("first-light".into(), "broad".into(), 2., 1., 300.),
        ("first-light".into(), "low-relief".into(), 1., 0., 0.),
    ]);
    let mut rows = Vec::new();
    let mut shared = Value::Null;
    for (seed, name, scale, relief, detail) in cases {
        let mut old_recipe = recipe(
            &seed,
            subdivision,
            if mode == Mode::Uplands {
                geography::MODEL_VERSION
            } else {
                "continuous-plates-1"
            },
        );
        old_recipe.continental_scale = scale;
        old_recipe.relief_scale = relief;
        old_recipe.detail_amplitude_meters = detail;
        let mut new_recipe = old_recipe.clone();
        new_recipe.model_version = if mode == Mode::Uplands {
            continental_uplands::MODEL_VERSION
        } else {
            geography::MODEL_VERSION
        }
        .into();
        let started = Instant::now();
        let old = World::generate(old_recipe)?;
        let old_seconds = started.elapsed().as_secs_f64();
        let started = Instant::now();
        let new = World::generate(new_recipe)?;
        let new_seconds = started.elapsed().as_secs_f64();
        if name == "ordinary-00" {
            shared = shared_crust(&new)?;
        }
        let svg_filename = format!("{name}.svg");
        let depression_filename = format!("{name}-depressions.svg");
        let json_filename = format!("{name}.json");
        let old_depressions =
            depression_analysis::analyze(&old.surface, &old.terrain.elevation, &old.water)?;
        let new_depressions =
            depression_analysis::analyze(&new.surface, &new.terrain.elevation, &new.water)?;
        let fixed_volume_control = if mode == Mode::Uplands {
            let mut r = new.recipe.clone();
            r.water = WaterSettings::Volume {
                volume_cubic_meters: old.water.resolved_volume_cubic_meters,
            };
            let started = Instant::now();
            let w = World::generate(r)?;
            Some(record(&w, started.elapsed().as_secs_f64())?)
        } else {
            None
        };
        let row = json!({"case":name,"seed":seed,
            "category":if name.starts_with("ordinary-") { "ordinary" } else { "directed" },
            "atlasFile":svg_filename,"reportFile":json_filename,
            "depressionAtlasFile":depression_filename,
            "depressionGeometry":{"baseline":old_depressions,"candidate":new_depressions},
            "baseline":record(&old,old_seconds)?,"candidate":record(&new,new_seconds)?,
            "candidateWithBaselineWaterVolume":fixed_volume_control,
            "fixedVersion1TerminalMaskAblation":if mode==Mode::Uplands {
                Some(json!({"baseline":fixed_outlet_drainage(&old,&old.water.body_ids),
                    "candidate":fixed_outlet_drainage(&new,&old.water.body_ids)}))
            } else {None},
            "scope":"Opt-in initial geography synthesis with identical requested parameters and four existing dry smoothing passes. No new stream-power incision, geological history, biomes or calibrated realism claim."});
        write_new(
            &directory.join(&svg_filename),
            geography_atlas::render(&old, &new).as_bytes(),
        )?;
        write_new(
            &directory.join(&depression_filename),
            depression_atlas::render(&old, &new, &old_depressions, &new_depressions).as_bytes(),
        )?;
        write_json(&directory.join(&json_filename), &row)?;
        println!(
            "{name}: {} regions; baseline {old_seconds:.3} s, candidate {new_seconds:.3} s.",
            new.surface.centers.len()
        );
        rows.push(row);
    }
    let dense = if mode == Mode::DenseCrust {
        let report = dense_crust()?;
        write_json(&directory.join("dense-crust.json"), &report)?;
        report
    } else {
        Value::Null
    };
    // Written last: absence of manifest identifies a failed/partial diagnostic run.
    let manifest = json!({"reportVersion":"geography-cohort-3","subdivision":subdivision,"comparison":format!("{mode:?}"),
        "ordinaryPairCount":20,"directedPairCount":3,"rows":rows,
        "meshIndependentCrustCheck":shared,"denseCrustDiagnostic":dense,
        "controlScope":"Fragmented and broad name scale settings 0.5 and 2, not guaranteed archipelago or supercontinent outcomes. Low-relief disables boundary relief/detail, not crust buoyancy or existing dry smoothing.",
        "comparisonScope":"Common fixed height/depth palette, independently fitted requested coverage. Coast crossing counts and neighbor-scale roughness depend on sampling. No universal realism score or cherry-picked seed rejection.",
        "outputScope":"Diagnostic files, not transactional or resumable simulation saves. Existing directories/files are never overwritten; failed runs can leave partial outputs without a manifest."});
    write_json(&directory.join("manifest.json"), &manifest)?;
    println!(
        "Completed 20 ordinary and 3 directed paired comparisons in {}.",
        directory.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn argument_contract_refuses_unbounded_or_ambiguous_runs() {
        for args in [
            vec![],
            vec!["out"],
            vec!["out", "3"],
            vec!["out", "7"],
            vec!["out", "bad"],
            vec!["", "4"],
            vec!["out", "4", "--unknown"],
            vec!["out", "4", "--dense-crust", "extra"],
        ] {
            assert!(parse_args(&args.into_iter().map(str::to_string).collect::<Vec<_>>()).is_err());
        }
        let (_, level, mode) =
            parse_args(&["out".into(), "6".into(), "--dense-crust".into()]).unwrap();
        assert_eq!(level, 6);
        assert_eq!(mode, Mode::DenseCrust);
        assert_eq!(
            parse_args(&["out".into(), "6".into(), "--uplands".into()])
                .unwrap()
                .2,
            Mode::Uplands
        );
    }
    #[test]
    fn fixed_recipe_pairs_change_only_explicit_model_and_controls() {
        let old = recipe("first-light", 4, "continuous-plates-1");
        let mut new = recipe("first-light", 4, geography::MODEL_VERSION);
        old.validate().unwrap();
        new.validate().unwrap();
        new.model_version = old.model_version.clone();
        assert_eq!(old, new);
        assert_eq!(old.terrain_preparation_passes, Some(4));
    }
}
