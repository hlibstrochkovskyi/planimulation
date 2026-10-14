//! Bounded generation-only sampling control, not a dense World or checkpoint.
#[path = "support/initial_landscape_fields.rs"]
mod initial_landscape_fields;

use planimulation_core::{
    Recipe, Surface, continental_uplands, depression_analysis, geography_metrics,
    landscape_evolution, sedimentary_landscape,
    terrain::Terrain,
    water::{Water, WaterSettings},
};
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf, time::Instant};

const SETTINGS: sedimentary_landscape::Settings = sedimentary_landscape::Settings {
    erodibility_per_year: 1e-6,
    step_years: 25_000.,
    steps: 32,
    maximum_aggradation_rate_meters_per_year: 0.001,
};
const CHARACTERISTIC_HEIGHT_METERS: f64 = 2000.;

fn reference_parameters(seed: String) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: continental_uplands::MODEL_VERSION.into(),
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
    if args.len() != 3 || args[0].trim().is_empty() {
        return Err("Usage: sedimentary_scale NEW_REPORT.json SEED SUBDIVISION(6..8)".into());
    }
    let output = PathBuf::from(&args[0]);
    if std::fs::symlink_metadata(&output).is_ok() {
        return Err(
            "Sedimentary scale requires a new report path, including no dangling symlink.".into(),
        );
    }
    if output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .is_some_and(|p| !p.is_dir())
    {
        return Err("Sedimentary scale report parent must already exist.".into());
    }
    let level = args[2]
        .parse::<u32>()
        .map_err(|_| "Invalid sedimentary scale subdivision.")?;
    if !(6..=8).contains(&level) {
        return Err("Sedimentary scale supports sampling subdivisions 6–8.".into());
    }
    let parameters = reference_parameters(args[1].clone());
    parameters.validate()?;
    Ok((output, level, parameters))
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

fn sum(values: impl Iterator<Item = f64>) -> Result<f64, String> {
    let mut total = 0.;
    let mut correction = 0.;
    for value in values {
        let adjusted = value - correction;
        let next = total + adjusted;
        correction = (next - total) - adjusted;
        total = next;
        if !value.is_finite() || !total.is_finite() || !correction.is_finite() {
            return Err("Sedimentary scale diagnostic sum overflowed.".into());
        }
    }
    Ok(total)
}

fn checksum(field: &[f64]) -> String {
    let mut hash = 0x811c9dc5_u32;
    for value in field {
        for byte in value.to_le_bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x01000193);
        }
    }
    format!("{hash:08x}")
}
fn relative(error: f64, scale: f64) -> f64 {
    error.abs() / if scale == 0. { 1. } else { scale }
}

fn diagnostics(s: &Surface, bed: &[f64], water: &Water, level: u32) -> Result<Value, String> {
    let metrics = geography_metrics::analyze(s, bed, water)?;
    let count = metrics.landmasses.len();
    let mut geography = serde_json::to_value(metrics).map_err(|e| e.to_string())?;
    let landmasses = geography["landmasses"]
        .as_array_mut()
        .ok_or("Missing landmass catalog.")?;
    landmasses.truncate(10);
    let object = geography
        .as_object_mut()
        .ok_or("Invalid geography summary.")?;
    let top = object
        .remove("landmasses")
        .ok_or("Missing landmass catalog.")?;
    object.insert("largestTenLandmasses".into(), top);
    object.insert("landmassCount".into(), json!(count));
    let escape_summary = |e: Option<&depression_analysis::EscapeAnalysis>| {
        e.map_or(Value::Null, |e| {
            json!({
        "outletRegionCount":e.outlet_region_count,
        "positiveDepthDryAreaSquareMeters":e.positive_depth_dry_area_square_meters,
        "positiveDepthFractionOfDryLand":e.positive_depth_fraction_of_dry_land,
        "additionalGeometricVolumeCubicMeters":e.additional_geometric_volume_cubic_meters,
        "maximumAdditionalDepthMeters":e.maximum_additional_depth_meters,
        "positiveDepthComponentCount":e.components.len(),"depthBands":e.depth_bands})
        })
    };
    let depressions = if level == 6 {
        let a = depression_analysis::analyze(s, bed, water)?;
        json!({"status":"measured","analysisVersion":a.analysis_version,
            "dryMinimumLeafCount":a.dry_minimum_leaves.len(),
            "toInitialWater":escape_summary(a.to_initial_water.as_ref()),
            "toMainOcean":escape_summary(a.to_main_ocean.as_ref())})
    } else {
        json!({"status":"omitted","reason":"Additional depression hierarchy and minimax diagnostics omitted above sampling subdivision 6 to bound cost. The preparation kernel still constructs its own authoritative basin hierarchy; omitted diagnostics are not substituted estimates."})
    };
    let represented = sum(water
        .depth_meters
        .iter()
        .zip(&s.areas)
        .map(|(&d, &a)| d * a))?;
    Ok(json!({"geography":geography,"depressions":depressions,
        "initializationWater":{"levelMeters":water.level_meters,"resolvedVolumeCubicMeters":water.resolved_volume_cubic_meters,
            "independentRepresentedVolumeCubicMeters":represented,"mainOceanId":water.main_ocean_id},
        "heightFieldFnv1aLittleEndian32":checksum(bed)}))
}

fn final_water(s: &Surface, bed: &[f64], original: &Water) -> Result<(Water, f64), String> {
    let water = Water::generate(
        s,
        bed,
        &WaterSettings::Volume {
            volume_cubic_meters: original.resolved_volume_cubic_meters,
        },
    )?;
    let represented = sum(water
        .depth_meters
        .iter()
        .zip(&s.areas)
        .map(|(&d, &a)| d * a))?;
    let residual = relative(
        represented - original.resolved_volume_cubic_meters,
        original.resolved_volume_cubic_meters,
    );
    if !residual.is_finite() || residual > 1e-10 {
        return Err("Sedimentary scale final initialization changed original water stock.".into());
    }
    Ok((water, residual))
}

fn old_ledger(
    s: &Surface,
    old: &[f64],
    water: &Water,
    p: &landscape_evolution::Preparation,
) -> Result<Value, String> {
    let uplift = sum(p
        .accepted_uplift_displacement_meters
        .iter()
        .zip(&s.areas)
        .map(|(&d, &a)| d * a))?;
    let erosion = sum(p
        .removed_erosion_thickness_meters
        .iter()
        .zip(&s.areas)
        .map(|(&d, &a)| d * a))?;
    let net = sum((0..old.len()).map(|i| (p.elevation_meters[i] - old[i]) * s.areas[i]))?;
    let mut region_residual: f64 = 0.;
    for (i, &h) in old.iter().enumerate() {
        let source = p.accepted_uplift_displacement_meters[i];
        let removed = p.removed_erosion_thickness_meters[i];
        region_residual = region_residual.max(relative(
            source - removed - (p.elevation_meters[i] - h),
            source.max(removed),
        ));
        if water.body_ids[i] != 0 && p.elevation_meters[i] != h {
            return Err("Frozen scale control changed a wet bed.".into());
        }
    }
    let residual = relative(uplift - erosion - net, uplift.max(erosion));
    let source_residual = relative(
        uplift - p.uplift_displacement_prism_cubic_meters,
        uplift.max(p.uplift_displacement_prism_cubic_meters),
    );
    let erosion_residual = relative(
        erosion - p.exported_erosion_material_cubic_meters,
        erosion.max(p.exported_erosion_material_cubic_meters),
    );
    if [residual, source_residual, erosion_residual, region_residual]
        .iter()
        .any(|v| !v.is_finite() || *v > 1e-11)
    {
        return Err("Independent frozen scale prism ledger failed.".into());
    }
    Ok(
        json!({"upliftPrismCubicMeters":uplift,"exportedErosionPrismCubicMeters":erosion,
        "netBedPrismChangeCubicMeters":net,"relativeBedLedgerResidual":residual,
        "relativeSourceReconstructionResidual":source_residual,"relativeErosionReconstructionResidual":erosion_residual,
        "maximumRegionRelativeBedLedgerResidual":region_residual}),
    )
}

fn candidate_ledger(
    s: &Surface,
    old: &[f64],
    water: &Water,
    p: &sedimentary_landscape::Preparation,
) -> Result<Value, String> {
    let n = old.len();
    let prism = |field: &[f64]| sum(field.iter().zip(&s.areas).map(|(&d, &a)| d * a));
    for field in [
        &p.elevation_meters,
        &p.sediment_thickness_meters,
        &p.pending_volume_cubic_meters,
        &p.accepted_uplift_displacement_meters,
        &p.removed_erosion_thickness_meters,
        &p.deposited_thickness_meters,
    ] {
        if field.len() != n || field.iter().any(|v| !v.is_finite()) {
            return Err("Malformed candidate scale fields.".into());
        }
    }
    let uplift = prism(&p.accepted_uplift_displacement_meters)?;
    let erosion = prism(&p.removed_erosion_thickness_meters)?;
    let deposition = prism(&p.deposited_thickness_meters)?;
    let active = prism(&p.sediment_thickness_meters)?;
    let pending = sum(p.pending_volume_cubic_meters.iter().copied())?;
    let net = sum((0..n).map(|i| (p.elevation_meters[i] - old[i]) * s.areas[i]))?;
    let exports = sum([
        p.main_ocean_export_volume_cubic_meters,
        p.other_initial_wet_export_volume_cubic_meters,
    ]
    .into_iter())?;
    let bed_residual = relative(
        uplift - erosion + deposition - net,
        uplift.max(erosion).max(deposition),
    );
    let material_residual = relative(
        erosion - deposition - exports - pending,
        erosion.max(deposition).max(exports).max(pending),
    );
    let active_residual = relative(
        deposition - p.remobilized_sediment_volume_cubic_meters - active,
        deposition
            .max(p.remobilized_sediment_volume_cubic_meters)
            .max(active),
    );
    let mut observed = vec![false; n + 1];
    for &body in &water.body_ids {
        observed[body as usize] = true;
    }
    if p.wet_body_exports.len() != observed.iter().skip(1).filter(|&&v| v).count() {
        return Err("Candidate scale wet collector catalog changed.".into());
    }
    let mut previous = 0;
    for body in &p.wet_body_exports {
        if body.body_id <= previous
            || body.body_id as usize > n
            || !observed[body.body_id as usize]
            || !body.volume_cubic_meters.is_finite()
            || body.volume_cubic_meters < 0.
        {
            return Err("Invalid candidate wet collector.".into());
        }
        previous = body.body_id;
    }
    let ocean = sum(p
        .wet_body_exports
        .iter()
        .filter(|b| b.body_id == water.main_ocean_id)
        .map(|b| b.volume_cubic_meters))?;
    let other = sum(p
        .wet_body_exports
        .iter()
        .filter(|b| b.body_id != water.main_ocean_id)
        .map(|b| b.volume_cubic_meters))?;
    let collector_residual = relative(
        ocean - p.main_ocean_export_volume_cubic_meters,
        ocean.max(p.main_ocean_export_volume_cubic_meters),
    )
    .max(relative(
        other - p.other_initial_wet_export_volume_cubic_meters,
        other.max(p.other_initial_wet_export_volume_cubic_meters),
    ));
    let mut region_residual: f64 = 0.;
    for (i, &h) in old.iter().enumerate() {
        let source = p.accepted_uplift_displacement_meters[i];
        let removed = p.removed_erosion_thickness_meters[i];
        let added = p.deposited_thickness_meters[i];
        if [
            source,
            removed,
            added,
            p.sediment_thickness_meters[i],
            p.pending_volume_cubic_meters[i],
        ]
        .iter()
        .any(|v| *v < 0.)
        {
            return Err("Negative candidate scale transfer or inventory.".into());
        }
        region_residual = region_residual.max(relative(
            source - removed + added - (p.elevation_meters[i] - h),
            source.max(removed).max(added),
        ));
        if water.body_ids[i] != 0
            && (p.elevation_meters[i] != h
                || p.sediment_thickness_meters[i] != 0.
                || p.pending_volume_cubic_meters[i] != 0.)
        {
            return Err("Candidate scale changed an original wet collector.".into());
        }
    }
    let mut reconstruction_residual: f64 = 0.;
    for (actual, reported) in [
        (uplift, p.uplift_displacement_prism_cubic_meters),
        (erosion, p.gross_erosion_volume_cubic_meters),
        (deposition, p.gross_deposition_volume_cubic_meters),
        (active, p.surviving_sediment_volume_cubic_meters),
        (pending, p.pending_volume_total_cubic_meters),
    ] {
        if !reported.is_finite() || reported < 0. {
            return Err("Invalid candidate scale reported stock.".into());
        }
        reconstruction_residual =
            reconstruction_residual.max(relative(actual - reported, actual.max(reported)));
    }
    if !p.independent_net_bed_prism_change_cubic_meters.is_finite() {
        return Err("Invalid candidate scale reported net bed prism.".into());
    }
    reconstruction_residual = reconstruction_residual.max(relative(
        net - p.independent_net_bed_prism_change_cubic_meters,
        uplift.max(erosion).max(deposition),
    ));
    if [
        bed_residual,
        material_residual,
        active_residual,
        collector_residual,
        region_residual,
        reconstruction_residual,
    ]
    .iter()
    .any(|v| !v.is_finite() || *v > 1e-11)
    {
        return Err("Independent candidate scale ledger failed.".into());
    }
    Ok(
        json!({"upliftPrismCubicMeters":uplift,"grossErosionPrismCubicMeters":erosion,
        "grossDepositionPrismCubicMeters":deposition,"activeSedimentVolumeCubicMeters":active,
        "pendingVolumeCubicMeters":pending,"remobilizedSedimentVolumeCubicMeters":p.remobilized_sediment_volume_cubic_meters,
        "mainOceanExportVolumeCubicMeters":ocean,"otherInitialWetExportVolumeCubicMeters":other,
        "wetCollectorCount":p.wet_body_exports.len(),"netBedPrismChangeCubicMeters":net,
        "relativeBedLedgerResidual":bed_residual,"relativeMaterialLedgerResidual":material_residual,
        "relativeActiveSedimentLedgerResidual":active_residual,"relativeCollectorPartitionResidual":collector_residual,
        "maximumRegionRelativeBedLedgerResidual":region_residual,"maximumReportedReconstructionResidual":reconstruction_residual}),
    )
}

fn numerical_observations(p: &sedimentary_landscape::Preparation) -> Result<Value, String> {
    let count = |get: fn(&sedimentary_landscape::Step) -> usize| {
        p.history.iter().try_fold(0usize, |total, s| {
            total
                .checked_add(get(s))
                .ok_or("Scale observation count overflowed.")
        })
    };
    let proposals = sum(p
        .history
        .iter()
        .map(|s| s.unresolved_deposition_candidate_volume_cubic_meters))?;
    let rounding = sum(p
        .history
        .iter()
        .map(|s| s.source_assembly_rounding_volume_cubic_meters))?;
    Ok(
        json!({"unresolvedPositiveUpliftRegionStepCount":count(|s|s.unresolved_positive_uplift_regions)?,
        "unresolvedPositiveIncisionRegionStepCount":count(|s|s.unresolved_positive_incision_regions)?,
        "unresolvedPositiveDepositionLeafStepCount":count(|s|s.unresolved_positive_deposition_leaves)?,
        "unresolvedSourceAdditionRegionStepCount":count(|s|s.unresolved_source_additions)?,
        "unresolvedDepositionCandidateStepVolumeSumCubicMeters":proposals,
        "maximumUnresolvedDepositionCandidateStepVolumeCubicMeters":p.history.iter().map(|s|s.unresolved_deposition_candidate_volume_cubic_meters).fold(0.,f64::max),
        "sourceAssemblyAbsoluteRoundingStepVolumeSumCubicMeters":rounding,
        "maximumAbsoluteSourceAssemblyStepRoundingVolumeCubicMeters":p.history.iter().map(|s|s.source_assembly_rounding_volume_cubic_meters).fold(0.,f64::max),
        "measuredDepositionDebitVolumeCubicMeters":sum(p.history.iter().map(|s|s.debited_deposition_volume_cubic_meters))?,
        "maximumRelativeDepositionInventoryLedgerResidual":p.history.iter().map(|s|s.relative_deposition_inventory_ledger_residual).fold(0.,f64::max),
        "scope":"Counts are region/leaf-step observations, not unique features. Unresolved candidate volumes can repeat the same proposals and are NOT material stock, accepted deposition or a repair. Source-assembly rounding sums absolute per-cell TwoSum errors and is never booked as material."}),
    )
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (output, level, parameters) = parse(&args)?;
    let total_started = Instant::now();
    let mut stages = Vec::new();
    let started = Instant::now();
    let surface = Surface::build_generation_probe(level, parameters.radius_meters)?;
    stages.push(json!({"name":"surface","seconds":started.elapsed().as_secs_f64(),"cumulativePeakRssBytes":peak_rss_bytes()}));
    let n = surface.areas.len();
    let area = sum(surface.areas.iter().copied())?;
    let sphere = 4. * std::f64::consts::PI * parameters.radius_meters.powi(2);
    let area_residual = relative(area - sphere, sphere);
    if n != 10 * 4_usize.pow(level) + 2
        || surface.neighbors.len() != 6 * n - 12
        || surface.boundaries.len() != 12 * n - 24
        || area_residual > 1e-12
    {
        return Err("Sedimentary scale geometry invariant failed.".into());
    }
    let started = Instant::now();
    let (crust, tectonics, terrain) = initial_landscape_fields::build(&surface, &parameters)?;
    drop(crust);
    drop(tectonics);
    let Terrain {
        elevation: initial_bed,
        convergence,
        ..
    } = terrain;
    let original_water = Water::generate(&surface, &initial_bed, &parameters.water)?;
    let rates = landscape_evolution::structural_uplift_rates(
        &convergence,
        &original_water,
        SETTINGS.erodibility_per_year,
        CHARACTERISTIC_HEIGHT_METERS,
    )?;
    stages.push(json!({"name":"structuralFieldsWaterAndSource","seconds":started.elapsed().as_secs_f64(),"cumulativePeakRssBytes":peak_rss_bytes()}));
    let started = Instant::now();
    let initial = diagnostics(&surface, &initial_bed, &original_water, level)?;
    stages.push(json!({"name":"initialDiagnostics","seconds":started.elapsed().as_secs_f64(),"cumulativePeakRssBytes":peak_rss_bytes()}));
    let old_settings = landscape_evolution::Settings {
        erodibility_per_year: SETTINGS.erodibility_per_year,
        step_years: SETTINGS.step_years,
        steps: SETTINGS.steps,
    };
    let started = Instant::now();
    let old = landscape_evolution::prepare(
        &surface,
        &initial_bed,
        &original_water,
        &rates,
        old_settings,
    )?;
    let old_seconds = started.elapsed().as_secs_f64();
    let old_peak = peak_rss_bytes();
    let started = Instant::now();
    let old_accounting = old_ledger(&surface, &initial_bed, &original_water, &old)?;
    let (old_water, old_water_residual) =
        final_water(&surface, &old.elevation_meters, &original_water)?;
    let old_diagnostics = diagnostics(&surface, &old.elevation_meters, &old_water, level)?;
    let old_reporting_seconds = started.elapsed().as_secs_f64();
    let previous = json!({"modelVersion":landscape_evolution::MODEL_VERSION,"settings":old_settings,
        "diagnostics":old_diagnostics,"independentLedger":old_accounting,
        "kernelRelativeLedgerResidual":old.relative_prism_ledger_residual,
        "relativeOriginalWaterVolumeResidual":old_water_residual,
        "numericalObservations":{"unresolvedPositiveUpliftRegionStepCount":old.history.iter().map(|s|s.unresolved_positive_uplift_nodes).sum::<usize>(),
            "unresolvedPositiveIncisionRegionStepCount":old.history.iter().map(|s|s.unresolved_positive_incision_nodes).sum::<usize>()},
        "timingSeconds":{"preparation":old_seconds,"finalWaterAndDiagnostics":old_reporting_seconds},
        "preparationCumulativePeakRssBytes":old_peak});
    drop(old);
    drop(old_water);
    let started = Instant::now();
    let new =
        sedimentary_landscape::prepare(&surface, &initial_bed, &original_water, &rates, SETTINGS)?;
    let new_seconds = started.elapsed().as_secs_f64();
    let new_peak = peak_rss_bytes();
    let started = Instant::now();
    let new_accounting = candidate_ledger(&surface, &initial_bed, &original_water, &new)?;
    let (new_water, new_water_residual) =
        final_water(&surface, &new.elevation_meters, &original_water)?;
    let new_diagnostics = diagnostics(&surface, &new.elevation_meters, &new_water, level)?;
    let observations = numerical_observations(&new)?;
    let history:Vec<_>=new.history.iter().map(|s|json!({"step":s.step,
        "grossErosionVolumeCubicMeters":s.gross_erosion_volume_cubic_meters,
        "grossDepositionVolumeCubicMeters":s.gross_deposition_volume_cubic_meters,
        "remobilizedSedimentVolumeCubicMeters":s.remobilized_sediment_volume_cubic_meters,
        "mainOceanExportVolumeCubicMeters":s.main_ocean_export_volume_cubic_meters,
        "otherInitialWetExportVolumeCubicMeters":s.other_initial_wet_export_volume_cubic_meters,
        "pendingVolumeTotalCubicMeters":s.pending_volume_total_cubic_meters,
        "changedDepositionRegions":s.changed_deposition_regions,
        "eligibleDepositionLeaves":s.eligible_deposition_leaves,
        "dryTerminalCountAfterDeposition":s.dry_terminal_count_after_deposition,
        "unresolvedPositiveDepositionLeaves":s.unresolved_positive_deposition_leaves,
        "unresolvedCandidateVolumeCubicMeters":s.unresolved_deposition_candidate_volume_cubic_meters})).collect();
    let candidate = json!({"modelVersion":new.model_version,"incisionModelVersion":new.incision_model_version,
        "deliveryModelVersion":new.delivery_model_version,"depositionModelVersion":new.deposition_model_version,
        "settings":SETTINGS,"diagnostics":new_diagnostics,"independentLedger":new_accounting,
        "kernelRelativeLedgerResidual":new.relative_ledger_residual,"relativeOriginalWaterVolumeResidual":new_water_residual,
        "numericalObservations":observations,"historySummary":history,
        "timingSeconds":{"preparation":new_seconds,"finalWaterAndDiagnostics":started.elapsed().as_secs_f64()},
        "preparationCumulativePeakRssBytes":new_peak,"diagnosticsCumulativePeakRssBytes":peak_rss_bytes()});
    let report = json!({"reportVersion":"sedimentary-scale-review-1","referenceParameters":parameters,
        "samplingSubdivision":level,"regions":n,"referenceAreaSquareMeters":area,
        "equivalentRegionWidthMeters":(area/n as f64).sqrt(),"relativeSphereAreaError":area_residual,
        "sourceLaw":{"equation":"U = K * characteristic_height * initial_convergence / convergence_reference",
            "characteristicHeightMeters":CHARACTERISTIC_HEIGHT_METERS,
            "convergenceReferenceMeters":landscape_evolution::CONVERGENCE_REFERENCE_METERS,
            "upliftRateFieldFnv1aLittleEndian32":checksum(&rates),
            "maximumActualPrescribedUpliftMetersPerYear":rates.iter().copied().fold(0.,f64::max),
            "originalWetTerminalUpliftMetersPerYear":0.,"syntheticNotCalibrated":true},
        "initial":initial,"previousExport":previous,"sedimentary":candidate,"stageMeasurements":stages,
        "totalSecondsBeforeReportSerialization":total_started.elapsed().as_secs_f64(),"cumulativePeakRssBytes":peak_rss_bytes(),
        "scope":"Summary-only generation probe, not a dense World, supported higher-resolution Recipe, wire, checkpoint or desktop setting. Same initial v2 fields, original wet collectors and stock; no physical sink correction or hydrodynamic water advance.",
        "accountingScope":"Kinematic uplift reference prisms, gross solid-equivalent erosion/deposition/remobilization, surviving sediment, pending inventory and split original-wet exports. No density mass, porosity, compaction or calibrated settling law.",
        "measurementScope":"Sequential control/candidate share a process and allocator history. Linux VmHWM is cumulative, NOT isolated variant memory. Timings/RSS are observations, not physical identity. Additional depression diagnostics omitted above subdivision 6; preparation's own hierarchy remains active. Static drainage is not discharge or a permanent endorheic fraction. Landmass catalogs truncated to ten."});
    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    println!(
        "Sedimentary scale {n} samples: export control {old_seconds:.3} s, candidate {new_seconds:.3} s, cumulative RSS {:?}, report {} bytes.",
        peak_rss_bytes(),
        bytes.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_and_reference_parameters_preserve_supported_limits() {
        for args in [
            vec![],
            vec!["out"],
            vec!["out", "seed", "5"],
            vec!["out", "seed", "9"],
            vec!["out", "", "6"],
            vec!["out", "seed", "6", "extra"],
            vec!["", "seed", "6"],
            vec![".", "seed", "6"],
        ] {
            assert!(parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        for level in [6, 7, 8] {
            let (_, actual, reference) = parse(&[
                "sedimentary-scale-unit-uncreated.json".into(),
                "first-light".into(),
                level.to_string(),
            ])
            .unwrap();
            assert_eq!(actual, level);
            assert_eq!(reference.subdivision, 6);
            assert_eq!(reference.model_version, continental_uplands::MODEL_VERSION);
            assert_eq!(reference.terrain_preparation_passes, Some(0));
            reference.validate().unwrap();
        }
        let mut unsupported = reference_parameters("first-light".into());
        unsupported.subdivision = 8;
        assert!(unsupported.validate().is_err());
        assert_eq!(SETTINGS.step_years * SETTINGS.steps as f64, 800_000.);
        assert_eq!(
            SETTINGS.maximum_aggradation_rate_meters_per_year * SETTINGS.step_years,
            25.
        );
        let values = [0., 1., -2.];
        let bytes: Vec<_> = values.iter().flat_map(|v: &f64| v.to_le_bytes()).collect();
        assert_eq!(
            checksum(&values),
            format!("{:08x}", planimulation_core::hash(&bytes))
        );
    }

    #[test]
    fn independent_ledgers_reconstruct_actual_sphere_and_refuse_corrupt_catalog() {
        let s = Surface::build(1, 1000.);
        let h: Vec<_> = (0..s.areas.len()).map(|i| (i % 11) as f64).collect();
        let w = Water::generate(&s, &h, &WaterSettings::Coverage { fraction: 0.71 }).unwrap();
        let rates: Vec<_> = w
            .body_ids
            .iter()
            .map(|&b| if b == 0 { 0.001 } else { 0. })
            .collect();
        let p = sedimentary_landscape::prepare(&s, &h, &w, &rates, SETTINGS).unwrap();
        candidate_ledger(&s, &h, &w, &p).unwrap();
        let mut broken = p.clone();
        broken.wet_body_exports.clear();
        assert!(candidate_ledger(&s, &h, &w, &broken).is_err());
        broken = p;
        broken.independent_net_bed_prism_change_cubic_meters = f64::NAN;
        assert!(candidate_ledger(&s, &h, &w, &broken).is_err());
        let old = landscape_evolution::prepare(
            &s,
            &h,
            &w,
            &rates,
            landscape_evolution::Settings {
                erodibility_per_year: SETTINGS.erodibility_per_year,
                step_years: SETTINGS.step_years,
                steps: SETTINGS.steps,
            },
        )
        .unwrap();
        old_ledger(&s, &h, &w, &old).unwrap();
    }

    #[test]
    fn omitted_extra_diagnostics_and_zero_wet_world_do_not_invent_measurements() {
        let s = Surface::build(0, 1000.);
        let h = vec![0.; s.areas.len()];
        let w = Water::generate(&s, &h, &WaterSettings::Coverage { fraction: 1. }).unwrap();
        let p = sedimentary_landscape::prepare(&s, &h, &w, &vec![0.; h.len()], SETTINGS).unwrap();
        let report = candidate_ledger(&s, &h, &w, &p).unwrap();
        assert_eq!(report["relativeActiveSedimentLedgerResidual"], 0.);
        assert_eq!(report["pendingVolumeCubicMeters"], 0.);
        let d = diagnostics(&s, &h, &w, 7).unwrap();
        assert_eq!(d["depressions"]["status"], "omitted");
        assert!(d["depressions"].get("toInitialWater").is_none());
        let observations = numerical_observations(&p).unwrap();
        assert_eq!(observations["unresolvedPositiveDepositionLeafStepCount"], 0);
    }
}
