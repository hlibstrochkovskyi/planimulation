//! Bounded sampling control: no renderer, dense recipe, World or checkpoint.
#[path = "support/initial_landscape_fields.rs"]
mod initial_landscape_fields;

use planimulation_core::{
    Recipe, Surface, continental_uplands, depression_analysis, geography_metrics,
    landscape_evolution::{self, Settings},
    water::{Water, WaterSettings},
};
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf, time::Instant};

const SETTINGS: Settings = Settings {
    erodibility_per_year: 1e-6,
    step_years: 25_000.,
    steps: 32,
};

fn reference_parameters(seed: String, continuous: bool) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: if continuous {
            "continuous-plates-1"
        } else {
            continental_uplands::MODEL_VERSION
        }
        .into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed,
        subdivision: 6,
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

fn parse(args: &[String]) -> Result<(PathBuf, u32, Recipe), String> {
    if !(3..=4).contains(&args.len())
        || args[0].trim().is_empty()
        || args.len() == 4 && args[3] != "--continuous"
    {
        return Err(
            "Usage: uplift_scale NEW_REPORT.json SEED SUBDIVISION(6..8) [--continuous]".into(),
        );
    }
    let output = PathBuf::from(&args[0]);
    if std::fs::symlink_metadata(&output).is_ok() {
        return Err("Uplift scale report must use a new output path.".into());
    }
    let parent = output.parent().filter(|p| !p.as_os_str().is_empty());
    if parent.is_some_and(|p| !p.is_dir()) {
        return Err("Uplift scale report parent must be an existing directory.".into());
    }
    let level = args[2]
        .parse::<u32>()
        .map_err(|_| "Invalid uplift scale subdivision.")?;
    if !(6..=8).contains(&level) {
        return Err("Uplift scale supports sampling subdivisions 6–8 only.".into());
    }
    let reference = reference_parameters(args[1].clone(), args.len() == 4);
    reference.validate()?;
    Ok((output, level, reference))
}

fn peak_rss_bytes() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|line| line.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()?
        .checked_mul(1024)
}

#[derive(Default)]
struct Sum {
    value: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, increment: f64) -> Result<(), String> {
        let adjusted = increment - self.correction;
        let next = self.value + adjusted;
        self.correction = (next - self.value) - adjusted;
        self.value = next;
        if !increment.is_finite() || !self.value.is_finite() || !self.correction.is_finite() {
            return Err("Independent scale ledger overflowed.".into());
        }
        Ok(())
    }
}

fn field_checksum(field: &[f64]) -> String {
    // Streaming FNV-1a over little-endian float bytes, not a save/checkpoint ID.
    let mut fingerprint = 0x811c9dc5_u32;
    for value in field {
        for byte in value.to_le_bytes() {
            fingerprint ^= u32::from(byte);
            fingerprint = fingerprint.wrapping_mul(0x01000193);
        }
    }
    format!("{fingerprint:08x}")
}

fn geography_summary(s: &Surface, bed: &[f64], water: &Water) -> Result<Value, String> {
    let metrics = geography_metrics::analyze(s, bed, water)?;
    let landmass_count = metrics.landmasses.len();
    let mut summary = serde_json::to_value(metrics).map_err(|e| e.to_string())?;
    let landmasses = summary["landmasses"]
        .as_array_mut()
        .ok_or("Missing geographic landmass summary.")?;
    landmasses.truncate(10);
    let top = summary
        .as_object_mut()
        .ok_or("Invalid geographic summary.")?;
    let largest = top.remove("landmasses").ok_or("Missing landmasses.")?;
    top.insert("largestTenLandmasses".into(), largest);
    top.insert("landmassCount".into(), json!(landmass_count));
    Ok(summary)
}

fn escape_summary(value: Option<&depression_analysis::EscapeAnalysis>) -> Value {
    value.map_or(Value::Null, |e| {
        json!({"outletRegionCount":e.outlet_region_count,
            "dryAreaSquareMeters":e.dry_area_square_meters,
            "positiveDepthDryAreaSquareMeters":e.positive_depth_dry_area_square_meters,
            "positiveDepthFractionOfDryLand":e.positive_depth_fraction_of_dry_land,
            "additionalGeometricVolumeCubicMeters":e.additional_geometric_volume_cubic_meters,
            "maximumAdditionalDepthMeters":e.maximum_additional_depth_meters,
            "positiveDepthComponentCount":e.components.len(),"depthBands":e.depth_bands})
    })
}

fn depression_summary(
    s: &Surface,
    bed: &[f64],
    water: &Water,
    sampling_level: u32,
) -> Result<Value, String> {
    if sampling_level > 6 {
        return Ok(json!({"status":"omitted",
            "reason":"Basin hierarchy and two minimax sweeps omitted above subdivision 6 to bound diagnostic memory/time; no inferred depression depth or storage is substituted."}));
    }
    let analysis = depression_analysis::analyze(s, bed, water)?;
    Ok(
        json!({"status":"measured","analysisVersion":analysis.analysis_version,
        "dryMinimumLeafCount":analysis.dry_minimum_leaves.len(),
        "toInitialWater":escape_summary(analysis.to_initial_water.as_ref()),
        "toMainOcean":escape_summary(analysis.to_main_ocean.as_ref())}),
    )
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (output, sampling_level, parameters) = parse(&args)?;
    let total_started = Instant::now();
    let mut stages = Vec::new();
    let started = Instant::now();
    let surface = Surface::build_generation_probe(sampling_level, parameters.radius_meters)?;
    stages.push(
        json!({"name":"surface","seconds":started.elapsed().as_secs_f64(),
        "cumulativePeakRssBytes":peak_rss_bytes()}),
    );
    let n = surface.centers.len();
    let mut area = Sum::default();
    for &a in &surface.areas {
        area.add(a)?;
    }
    let sphere_area = 4. * std::f64::consts::PI * parameters.radius_meters.powi(2);
    let relative_area_error = (area.value - sphere_area).abs() / sphere_area;
    if n != 10 * 4_usize.pow(sampling_level) + 2
        || surface.neighbors.len() != 6 * n - 12
        || surface.boundaries.len() != 12 * n - 24
        || relative_area_error > 1e-12
    {
        return Err("Uplift scale geometry count or area invariant failed.".into());
    }
    let started = Instant::now();
    let (crust, tectonics, terrain) = initial_landscape_fields::build(&surface, &parameters)?;
    drop(crust);
    drop(tectonics);
    stages.push(
        json!({"name":"initialStructuralFields","seconds":started.elapsed().as_secs_f64(),
        "cumulativePeakRssBytes":peak_rss_bytes()}),
    );
    let started = Instant::now();
    let water = Water::generate(&surface, &terrain.elevation, &parameters.water)?;
    stages.push(
        json!({"name":"initialWater","seconds":started.elapsed().as_secs_f64(),
        "cumulativePeakRssBytes":peak_rss_bytes()}),
    );
    let started = Instant::now();
    let initial_summary = json!({"geography":geography_summary(&surface,&terrain.elevation,&water)?,
        "depressions":depression_summary(&surface,&terrain.elevation,&water,sampling_level)?,
        "waterLevelMeters":water.level_meters,"waterVolumeCubicMeters":water.resolved_volume_cubic_meters,
        "heightFieldFnv1aLittleEndian32":field_checksum(&terrain.elevation)});
    stages.push(
        json!({"name":"initialDiagnostics","seconds":started.elapsed().as_secs_f64(),
        "cumulativePeakRssBytes":peak_rss_bytes()}),
    );
    let mut variants = Vec::new();
    for characteristic in [0., 2000.] {
        let started = Instant::now();
        let rates = landscape_evolution::structural_uplift_rates(
            &terrain.convergence,
            &water,
            SETTINGS.erodibility_per_year,
            characteristic,
        )?;
        let preparation =
            landscape_evolution::prepare(&surface, &terrain.elevation, &water, &rates, SETTINGS)?;
        let preparation_seconds = started.elapsed().as_secs_f64();
        let preparation_peak = peak_rss_bytes();
        let mut uplift = Sum::default();
        let mut erosion = Sum::default();
        let mut net = Sum::default();
        let mut maximum_region_residual: f64 = 0.;
        for (i, &old) in terrain.elevation.iter().enumerate() {
            let source = preparation.accepted_uplift_displacement_meters[i];
            let removed = preparation.removed_erosion_thickness_meters[i];
            let change = preparation.elevation_meters[i] - old;
            uplift.add(source * surface.areas[i])?;
            erosion.add(removed * surface.areas[i])?;
            net.add(change * surface.areas[i])?;
            maximum_region_residual = maximum_region_residual
                .max(((source - removed) - change).abs() / source.max(removed).max(1.));
            if water.body_ids[i] != 0 && preparation.elevation_meters[i] != old {
                return Err("Uplift scale changed an original fixed wet bed.".into());
            }
        }
        let gross = uplift.value.max(erosion.value).max(1.);
        let residual = ((uplift.value - erosion.value) - net.value).abs() / gross;
        if residual > 1e-11
            || maximum_region_residual > 1e-11
            || (uplift.value - preparation.uplift_displacement_prism_cubic_meters).abs() / gross
                > 1e-11
            || (erosion.value - preparation.exported_erosion_material_cubic_meters).abs() / gross
                > 1e-11
        {
            return Err("Independent uplift scale prism reconstruction failed.".into());
        }
        let started = Instant::now();
        let final_water = Water::generate(
            &surface,
            &preparation.elevation_meters,
            &WaterSettings::Volume {
                volume_cubic_meters: water.resolved_volume_cubic_meters,
            },
        )?;
        let finalization_seconds = started.elapsed().as_secs_f64();
        let started = Instant::now();
        let geography = geography_summary(&surface, &preparation.elevation_meters, &final_water)?;
        let depressions = depression_summary(
            &surface,
            &preparation.elevation_meters,
            &final_water,
            sampling_level,
        )?;
        let diagnostic_seconds = started.elapsed().as_secs_f64();
        let mut represented_volume = Sum::default();
        for (&depth, &a) in final_water.depth_meters.iter().zip(&surface.areas) {
            represented_volume.add(depth * a)?;
        }
        let water_residual = (represented_volume.value - water.resolved_volume_cubic_meters).abs()
            / water.resolved_volume_cubic_meters.max(1.);
        if water_residual > 1e-10 {
            return Err(
                "Uplift scale final initialization changed the prescribed water stock.".into(),
            );
        }
        variants.push(json!({"characteristicHeightMeters":characteristic,"preparationSettings":SETTINGS,
            "maximumPrescribedUpliftMetersPerYear":rates.iter().copied().fold(0.,f64::max),
            "heightFieldFnv1aLittleEndian32":field_checksum(&preparation.elevation_meters),
            "geography":geography,"depressions":depressions,
            "upliftDisplacementPrismCubicMeters":preparation.uplift_displacement_prism_cubic_meters,
            "exportedErosionMaterialCubicMeters":preparation.exported_erosion_material_cubic_meters,
            "independentNetBedPrismChangeCubicMeters":net.value,
            "relativeIndependentPrismLedgerResidual":residual,
            "maximumRegionRelativePrismResidual":maximum_region_residual,
            "kernelRelativePrismLedgerResidual":preparation.relative_prism_ledger_residual,
            "finalInitializationWater":{"levelMeters":final_water.level_meters,
                "prescribedVolumeCubicMeters":water.resolved_volume_cubic_meters,
                "resolvedVolumeCubicMeters":final_water.resolved_volume_cubic_meters,
                "independentRepresentedVolumeCubicMeters":represented_volume.value,
                "relativeRepresentedVolumeResidual":water_residual},
            "timingSeconds":{"preparation":preparation_seconds,"finalWaterInitialization":finalization_seconds,
                "finalDiagnostics":diagnostic_seconds},"preparationCumulativePeakRssBytes":preparation_peak,
            "diagnosticsCumulativePeakRssBytes":peak_rss_bytes()}));
        println!(
            "Sampling {sampling_level}, characteristic height {characteristic:.0} m: {preparation_seconds:.3} s preparation, cumulative RSS {:?}.",
            peak_rss_bytes()
        );
    }
    let report = json!({"reportVersion":"uplift-scale-review-1","referenceParameters":parameters,
        "samplingSubdivision":sampling_level,"regions":n,"referenceAreaSquareMeters":area.value,
        "equivalentRegionWidthMeters":(area.value/n as f64).sqrt(),"relativeSphereAreaError":relative_area_error,
        "preparationModelVersion":landscape_evolution::MODEL_VERSION,
        "sourceLaw":{"equation":"U = K * characteristic_height * initial_convergence / convergence_reference",
            "convergenceReferenceMeters":landscape_evolution::CONVERGENCE_REFERENCE_METERS,
            "wetTerminalUpliftMetersPerYear":0.,"syntheticNotCalibrated":true},
        "initial":initial_summary,"variants":variants,"stageMeasurements":stages,
        "totalSecondsBeforeReportSerialization":total_started.elapsed().as_secs_f64(),
        "cumulativePeakRssBytes":peak_rss_bytes(),
        "scope":"Generation-only structural composition and two bounded dry preparation controls. No forced sink outlets, sediment deposition, climate, dynamic water, rendering, dense World, higher-resolution Recipe, wire or checkpoint support.",
        "accountingScope":"Uplift is kinematic reference-height displacement-prism bookkeeping, not newly created rock mass. Eroded reference-area-equivalent material is exported to a separately accounted reservoir.",
        "diagnosticScope":"Summary-only; landmass catalogs truncated to largest ten; depression geometry measured only at subdivision 6. Initial static drainage is not active rivers or a permanent endorheic fraction. Linux VmHWM is cumulative. Timings and RSS are observations, not physical identity."});
    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use planimulation_core::World;

    #[test]
    fn supported_composition_matches_world_and_does_not_upgrade_recipe_limits() {
        for continuous in [false, true] {
            let mut r = reference_parameters("first-light".into(), continuous);
            r.subdivision = 3;
            let world = World::generate(r.clone()).unwrap();
            let surface = Surface::build_generation_probe(3, r.radius_meters).unwrap();
            let (crust, tectonics, terrain) =
                initial_landscape_fields::build(&surface, &r).unwrap();
            assert_eq!(crust.potential, world.crust.potential);
            assert_eq!(crust.continentality, world.crust.continentality);
            assert_eq!(crust.thickness_meters, world.crust.thickness_meters);
            assert_eq!(
                crust.density_kg_per_cubic_meter,
                world.crust.density_kg_per_cubic_meter
            );
            assert_eq!(crust.threshold, world.crust.threshold);
            assert_eq!(tectonics, world.tectonics);
            assert_eq!(terrain, world.terrain);
            assert_eq!(SETTINGS.erodibility_per_year, 1e-6);
            assert_eq!(SETTINGS.step_years * SETTINGS.steps as f64, 800_000.);
            let u = landscape_evolution::structural_uplift_rates(
                &terrain.convergence,
                &world.water,
                SETTINGS.erodibility_per_year,
                2000.,
            )
            .unwrap();
            for (i, &rate) in u.iter().enumerate() {
                let expected = if world.water.body_ids[i] == 0 {
                    0.002 * (terrain.convergence[i] / 6000.)
                } else {
                    0.
                };
                assert_eq!(rate, expected);
            }
            r.terrain_preparation_passes = Some(1);
            assert!(initial_landscape_fields::build(&surface, &r).is_err());
            r.terrain_preparation_passes = Some(0);
            r.model_version = "terrain-prep-1".into();
            assert!(initial_landscape_fields::build(&surface, &r).is_err());
            r.subdivision = 8;
            assert!(r.validate().is_err());
        }
    }

    #[test]
    fn cli_and_streaming_fingerprint_have_explicit_bounds() {
        for args in [
            vec![],
            vec!["out"],
            vec!["out", "seed", "5"],
            vec!["out", "seed", "9"],
            vec!["out", "", "6"],
            vec!["out", "seed", "6", "--unknown"],
            vec!["", "seed", "6"],
        ] {
            assert!(parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        let values = [0., 1., -2.];
        let bytes: Vec<_> = values.iter().flat_map(|v: &f64| v.to_le_bytes()).collect();
        assert_eq!(
            field_checksum(&values),
            format!("{:08x}", planimulation_core::hash(&bytes))
        );
        assert_eq!(reference_parameters("seed".into(), false).subdivision, 6);
    }
}
