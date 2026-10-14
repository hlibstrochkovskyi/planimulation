//! Actual-bed ablations of belt width and physical-length initial detail.
#[path = "support/initial_landscape_fields.rs"]
mod initial_landscape_fields;
#[path = "support/relief_display.rs"]
mod relief_display;
use planimulation_core::{
    Recipe, Surface, continental_uplands, geography_metrics, landscape_evolution, relief_spectrum,
    sedimentary_landscape,
    water::{Water, WaterSettings},
};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
const PREPARATION: sedimentary_landscape::Settings = sedimentary_landscape::Settings {
    erodibility_per_year: 1e-6,
    step_years: 25000.,
    steps: 32,
    maximum_aggradation_rate_meters_per_year: 0.001,
};
fn variant_controls() -> [(&'static str, f64, bool); 4] {
    [
        ("wide-300", 300., false),
        ("physical-detail-300", 300., true),
        ("narrow-100", 100., false),
        ("physical-detail-100", 100., true),
    ]
}
fn recipe(seed: &str) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: continental_uplands::MODEL_VERSION.into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: seed.into(),
        subdivision: 6,
        radius_meters: 6371000.,
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
fn parse(args: &[String]) -> Result<(PathBuf, u32, Option<String>), String> {
    if !(2..=3).contains(&args.len()) || args[0].trim().is_empty() {
        return Err("Usage: relief_landscape NEW_DIRECTORY SUBDIVISION(6..8) [SEED]".into());
    }
    let directory = PathBuf::from(&args[0]);
    if std::fs::symlink_metadata(&directory).is_ok() {
        return Err("Relief review requires a new directory.".into());
    }
    if directory
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .is_some_and(|p| !p.is_dir())
    {
        return Err("Relief review parent must exist.".into());
    }
    let level = args[1]
        .parse::<u32>()
        .map_err(|_| "Invalid relief review sampling level.")?;
    if !(6..=8).contains(&level) {
        return Err("Relief review sampling supports levels6–8.".into());
    }
    let seed = args.get(2).cloned();
    recipe(seed.as_deref().unwrap_or("first-light")).validate()?;
    Ok((directory, level, seed))
}
fn cases(seed: Option<String>) -> Vec<(String, String, f64, f64, f64)> {
    if let Some(seed) = seed {
        return vec![(seed, "selected".into(), 1., 1., 300.)];
    }
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
    cases
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())
}
fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    write_new(
        path,
        &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
}
fn initialization_water(
    s: &Surface,
    bed: &[f64],
    settings: &WaterSettings,
    baseline: Option<&(Vec<f64>, Water)>,
) -> Result<Water, String> {
    match baseline {
        // A null initial-bed intervention must not perturb an identical water
        // state merely through a second volume-fit's rounding tolerance.
        Some((h, water)) if h == bed => Ok(water.clone()),
        Some((_, water)) => Water::generate(
            s,
            bed,
            &WaterSettings::Volume {
                volume_cubic_meters: water.resolved_volume_cubic_meters,
            },
        ),
        None => Water::generate(s, bed, settings),
    }
}
/// Raw owned field record, not a checkpoint: eight f64-LE columns per region.
fn write_fields(
    path: &Path,
    initial: &[f64],
    source: &[f64],
    p: &sedimentary_landscape::Preparation,
) -> Result<(), String> {
    let mut file = std::io::BufWriter::new(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?,
    );
    for i in 0..initial.len() {
        for v in [
            initial[i],
            source[i],
            p.elevation_meters[i],
            p.sediment_thickness_meters[i],
            p.pending_volume_cubic_meters[i],
            p.removed_erosion_thickness_meters[i],
            p.deposited_thickness_meters[i],
            p.accepted_uplift_displacement_meters[i],
        ] {
            file.write_all(&v.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
    }
    file.flush().map_err(|e| e.to_string())
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (directory, level, seed) = parse(&args)?;
    let selected = seed.is_some();
    let s = Surface::build_generation_probe(level, 6371000.)?;
    // A single explicit owned-grid footprint proxy; not chosen by the camera.
    let footprint = (s.areas.iter().sum::<f64>() / s.areas.len() as f64).sqrt();
    std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for (seed, name, scale, relief, amplitude) in cases(seed) {
        let mut reference = recipe(&seed);
        reference.continental_scale = scale;
        reference.relief_scale = relief;
        reference.detail_amplitude_meters = amplitude;
        let mut water_volume = None;
        let mut baseline_initial: Option<(Vec<f64>, Water)> = None;
        let mut variants = Vec::new();
        let mut svg = String::from(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1600\" height=\"960\"><rect width=\"1600\" height=\"960\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style><text x=\"24\" y=\"28\" font-size=\"18\">Actual prepared beds · belt width x detail spectrum</text><defs><clipPath id=\"clip\"><rect width=\"760\" height=\"380\"/></clipPath></defs>",
        );
        for (j, (label, width, new_detail)) in variant_controls().into_iter().enumerate() {
            let started = Instant::now();
            let mut parameters = reference.clone();
            parameters.boundary_width_km = width;
            let (crust, tectonics, mut terrain) = initial_landscape_fields::build(&s, &parameters)?;
            drop(tectonics);
            let settings = relief_spectrum::Settings {
                radius_meters: parameters.radius_meters,
                sampling_length_meters: footprint,
                amplitude_meters: amplitude,
            };
            let field = relief_spectrum::Field::new(&seed, settings)?;
            if new_detail {
                for i in 0..s.areas.len() {
                    terrain.detail[i] = field
                        .sample(
                            s.centers[i],
                            crust.continentality[i],
                            terrain.convergence[i],
                        )?
                        .signed_detail_meters;
                    terrain.elevation[i] = terrain.baseline[i]
                        + terrain.convergence[i]
                        + terrain.divergence[i]
                        + terrain.detail[i];
                }
            }
            drop(crust);
            let reused = baseline_initial
                .as_ref()
                .is_some_and(|(h, _)| *h == terrain.elevation);
            let w = initialization_water(
                &s,
                &terrain.elevation,
                &parameters.water,
                baseline_initial.as_ref(),
            )?;
            water_volume.get_or_insert(w.resolved_volume_cubic_meters);
            if baseline_initial.is_none() {
                baseline_initial = Some((terrain.elevation.clone(), w.clone()));
            }
            let source = landscape_evolution::structural_uplift_rates(
                &terrain.convergence,
                &w,
                1e-6,
                2000.,
            )?;
            let initial_metrics = geography_metrics::analyze(&s, &terrain.elevation, &w)?;
            let process_started = Instant::now();
            let p =
                sedimentary_landscape::prepare(&s, &terrain.elevation, &w, &source, PREPARATION)?;
            let preparation_seconds = process_started.elapsed().as_secs_f64();
            let final_water = Water::generate(
                &s,
                &p.elevation_meters,
                &WaterSettings::Volume {
                    volume_cubic_meters: water_volume.unwrap(),
                },
            )?;
            let metrics = geography_metrics::analyze(&s, &p.elevation_meters, &final_water)?;
            let represented = final_water
                .depth_meters
                .iter()
                .zip(&s.areas)
                .map(|(&d, &a)| d * a)
                .sum::<f64>();
            let stock_residual =
                (represented - water_volume.unwrap()).abs() / water_volume.unwrap().max(1.);
            if !stock_residual.is_finite() || stock_residual > 1e-10 {
                return Err("Relief review water stock reconstruction failed.".into());
            }
            let file = format!("{name}-{label}.f64le");
            write_fields(&directory.join(&file), &terrain.elevation, &source, &p)?;
            let record = json!({"variant":label,"referenceRecipe":parameters,"samplingSubdivision":level,
                "detailModelVersion":if new_detail {relief_spectrum::MODEL_VERSION}else{"multiscale-geography-1.detail"},
                "physicalDetailSettings":if new_detail {json!(settings)}else{Value::Null},
                "physicalDetailBands":if new_detail {json!(field.bands())}else{Value::Null},
                "initialMetrics":initial_metrics,"preparedMetrics":metrics,"preparationModelVersion":p.model_version,
                "preparationSettings":PREPARATION,"upliftCharacteristicHeightMeters":2000.,
                "grossErosionVolumeCubicMeters":p.gross_erosion_volume_cubic_meters,"grossDepositionVolumeCubicMeters":p.gross_deposition_volume_cubic_meters,
                "remobilizedSedimentVolumeCubicMeters":p.remobilized_sediment_volume_cubic_meters,"activeSedimentVolumeCubicMeters":p.surviving_sediment_volume_cubic_meters,
                "pendingVolumeCubicMeters":p.pending_volume_total_cubic_meters,"wetBodyExports":p.wet_body_exports,
                "relativeLedgerResidual":p.relative_ledger_residual,"history":p.history,
                "upliftDisplacementPrismCubicMeters":p.uplift_displacement_prism_cubic_meters,
                "netBedPrismChangeCubicMeters":p.independent_net_bed_prism_change_cubic_meters,
                "initialWaterLevelMeters":w.level_meters,"finalWaterLevelMeters":final_water.level_meters,
                "identicalInitialBedReusedOriginalWater":reused,
                "originalWaterVolumeCubicMeters":water_volume,"relativeWaterStockResidual":stock_residual,
                "physicalFieldsFile":file,"preparationSeconds":preparation_seconds,"totalVariantSeconds":started.elapsed().as_secs_f64()});
            write_json(&directory.join(format!("{name}-{label}.json")), &record)?;
            if level == 8 {
                let single = relief_display::single(&s, &p.elevation_meters, &final_water)?;
                write_new(
                    &directory.join(format!("{name}-{label}.svg")),
                    single.as_bytes(),
                )?;
            } else {
                relief_display::panel(
                    &mut svg,
                    &s,
                    &p.elevation_meters,
                    &final_water,
                    24 + (j % 2) * 792,
                    60 + (j / 2) * 430,
                    label,
                )?;
            }
            println!(
                "{name} {label}: closed {:.2}%, sinks{}, p95{:?}m, preparation{preparation_seconds:.3}s.",
                metrics.static_drainage.closed_drainage_fraction_of_dry_land * 100.,
                metrics.static_drainage.closed_dry_sink_count,
                metrics
                    .dry_hypsometry
                    .as_ref()
                    .map(|h| h.quantile_height_above_water_meters[4])
            );
            variants.push(record);
        }
        svg.push_str("<text x=\"24\" y=\"916\" font-size=\"12\">Actual slope hillshade, display exaggeration20x; NW45°. Not rivers or biome colors.</text><text x=\"24\" y=\"940\" font-size=\"12\">Common palette. Blue static receivers; yellow dry terminals. Shading does not change beds.</text></svg>");
        if level < 8 {
            write_new(&directory.join(format!("{name}.svg")), svg.as_bytes())?;
        }
        rows.push(json!({"case":name,"seed":seed,"category":if selected{"selected"}else if name.starts_with("ordinary-"){"ordinary"}else{"directed"},"variants":variants}));
    }
    write_json(
        &directory.join("manifest.json"),
        &json!({"reportVersion":"physical-relief-review-2","samplingSubdivision":level,
        "regions":s.areas.len(),"physicalSamplingLengthMeters":footprint,"rows":rows,
        "display":{"hillshade":"actual-neighbor-tangent-least-squares-1","verticalExaggeration":20.,"sunAzimuthDegrees":315.,"sunElevationDegrees":45.,"physicalMutation":false},
        "atlasScope":if level==8 {"Separate per-variant SVGs bound each document below the converter's million-element limit. No combined dense SVG."}else{"Four-panel actual-field atlas."},
        "fieldRecord":{"format":"headerless-f64-le-region-major-1","columns":["initialBedMeters","upliftRateMetersPerYear","preparedBedMeters","activeSedimentThicknessMeters","pendingSedimentVolumeCubicMeters","cumulativeErosionThicknessMeters","cumulativeDepositionThicknessMeters","acceptedUpliftDisplacementMeters"]},
        "scope":"Initial-world ablations, not one world's dynamic intervention. Same seed/columns/plate roots; width changes structural amplitudes/source. Candidate detail replaces only initial detail. Original wide-case water stock held fixed. Physical spectrum is procedural initial-condition synthesis, not folding/erosion or calibrated Earth terrain. Probe metadata and new detail pin are not a supported Recipe/World/checkpoint. Native arrays remain authoritative; atlas approximate projection does not modify them. Manifest written last."}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_sampling_controls_and_refusals_precede_allocation() {
        for a in [
            vec![],
            vec!["out"],
            vec!["out", "5"],
            vec!["out", "9"],
            vec!["out", "6", ""],
            vec![".", "6"],
        ] {
            assert!(parse(&a.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        assert_eq!(cases(None).len(), 23);
        assert_eq!(variant_controls().len(), 4);
        assert_eq!(
            variant_controls().map(|(_, width, new)| (width, new)),
            [(300., false), (300., true), (100., false), (100., true)]
        );
        assert_eq!(cases(Some("first-light".into())).len(), 1);
        let (_, level, r) = parse(&[
            "relief-review-unit-uncreated".into(),
            "8".into(),
            "first-light".into(),
        ])
        .unwrap();
        assert_eq!(level, 8);
        assert_eq!(r.as_deref(), Some("first-light"));
        recipe("first-light").validate().unwrap();
    }
    #[test]
    fn null_bed_intervention_preserves_exact_initial_water_not_a_second_fit() {
        let s = Surface::build(2, 1000.);
        let bed: Vec<_> = (0..s.areas.len()).map(|i| (i % 11) as f64).collect();
        let settings = WaterSettings::Coverage { fraction: 0.71 };
        let water = initialization_water(&s, &bed, &settings, None).unwrap();
        let baseline = (bed.clone(), water.clone());
        let same = initialization_water(&s, &bed, &settings, Some(&baseline)).unwrap();
        assert_eq!(same.level_meters, water.level_meters);
        assert_eq!(same.depth_meters, water.depth_meters);
        assert_eq!(same.body_ids, water.body_ids);
        assert_eq!(
            same.resolved_volume_cubic_meters,
            water.resolved_volume_cubic_meters
        );
        let shifted: Vec<_> = bed.iter().map(|h| h + 10.).collect();
        let changed = initialization_water(&s, &shifted, &settings, Some(&baseline)).unwrap();
        assert!((changed.level_meters - water.level_meters - 10.).abs() < 1e-9);
        assert!(
            (changed.resolved_volume_cubic_meters - water.resolved_volume_cubic_meters).abs()
                / water.resolved_volume_cubic_meters
                < 1e-10
        );
        assert_eq!(baseline.0, bed);
    }
}
