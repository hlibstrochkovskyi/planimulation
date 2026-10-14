//! Controlled solid-material retention preview, not a desktop generator/save.
#[path = "support/atlas_field.rs"]
mod atlas_field;
#[path = "support/drainage_overlay.rs"]
mod drainage_overlay;

use planimulation_core::{
    Recipe, World, continental_uplands, depression_analysis, geography_metrics, hash,
    landscape_evolution, sedimentary_landscape,
    water::{Water, WaterSettings},
    wire,
};
use serde_json::{Value, json};
use std::{
    fmt::Write as _,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

const CHARACTERISTIC_HEIGHT_METERS: f64 = 2000.;
const ERODIBILITY_PER_YEAR: f64 = 1e-6;
const PREPARATION_YEARS: f64 = 800_000.;
const MAXIMUM_AGGRADATION_RATE_METERS_PER_YEAR: f64 = 0.001;

fn recipe(seed: &str, level: u32) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: continental_uplands::MODEL_VERSION.into(),
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

fn parse(args: &[String]) -> Result<(PathBuf, u32, Option<String>, usize), String> {
    if !(2..=4).contains(&args.len()) || args[0].trim().is_empty() {
        return Err("Usage: sedimentary_landscape NEW_DIRECTORY SUBDIVISION(4..6) [SEED] [--steps=16|32|64]".into());
    }
    let directory = PathBuf::from(&args[0]);
    if std::fs::symlink_metadata(&directory).is_ok() {
        return Err("Sedimentary comparison requires a new directory.".into());
    }
    if directory
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .is_some_and(|p| !p.is_dir())
    {
        return Err("Sedimentary comparison parent must already exist.".into());
    }
    let level = args[1]
        .parse::<u32>()
        .map_err(|_| "Invalid sedimentary subdivision.")?;
    if !(4..=6).contains(&level) {
        return Err("Sedimentary comparison supports subdivisions 4–6.".into());
    }
    let mut seed = None;
    let mut steps = 32;
    let mut saw_steps = false;
    for value in &args[2..] {
        if let Some(control) = value.strip_prefix("--steps=") {
            let parsed = control
                .parse::<usize>()
                .map_err(|_| "Invalid sedimentary step control.")?;
            if saw_steps || ![16, 32, 64].contains(&parsed) {
                return Err("Sedimentary steps must be exactly one of 16, 32 or 64.".into());
            }
            steps = parsed;
            saw_steps = true;
        } else if value.starts_with("--") || seed.is_some() {
            return Err("Unknown or duplicate sedimentary argument.".into());
        } else {
            seed = Some(value.clone());
        }
    }
    recipe(seed.as_deref().unwrap_or("first-light"), level).validate()?;
    Ok((directory, level, seed, steps))
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

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    write_new(path, &bytes)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())
}

fn initialized_water(world: &World, bed: &[f64]) -> Result<Water, String> {
    Water::generate(
        &world.surface,
        bed,
        &WaterSettings::Volume {
            volume_cubic_meters: world.water.resolved_volume_cubic_meters,
        },
    )
}
fn diagnostics(world: &World, bed: &[f64], water: &Water) -> Result<Value, String> {
    let mut represented = 0.;
    let mut correction = 0.;
    for (&depth, &area) in water.depth_meters.iter().zip(&world.surface.areas) {
        let adjusted = depth * area - correction;
        let next = represented + adjusted;
        correction = (next - represented) - adjusted;
        represented = next;
    }
    let requested = world.water.resolved_volume_cubic_meters;
    let residual = (represented - requested).abs() / requested.max(1.);
    if !residual.is_finite() || residual > 1e-10 {
        return Err(
            "Final diagnostic initialization did not preserve the original water stock.".into(),
        );
    }
    Ok(
        json!({"metrics":geography_metrics::analyze(&world.surface,bed,water)?,
        "depressionGeometry":depression_analysis::analyze(&world.surface,bed,water)?,
        "initializationWater":{"levelMeters":water.level_meters,
            "originalRequestedVolumeCubicMeters":requested,
            "resolvedVolumeCubicMeters":water.resolved_volume_cubic_meters,
            "independentRepresentedVolumeCubicMeters":represented,
            "relativeRepresentedVolumeResidual":residual}}),
    )
}

fn initial_native_fields(world: &World) -> Value {
    json!({"fieldRecordVersion":"sedimentary-initial-fields-1",
        "surface":{"centers":world.surface.centers,"faces":world.surface.faces,
            "neighborOffsets":world.surface.offsets,"neighbors":world.surface.neighbors,
            "neighborDistancesMeters":world.surface.distances,"areasSquareMeters":world.surface.areas,
            "boundaryOffsets":world.surface.boundary_offsets,"boundaryDirections":world.surface.boundaries},
        "tectonics":{"owners":world.tectonics.owners,"seeds":world.tectonics.seeds,
            "angularVelocities":world.tectonics.angular_velocities,"boundaryCells":world.tectonics.boundary_cells,
            "boundaryDirections":world.tectonics.boundary_directions,"boundaryMotion":world.tectonics.boundary_motion,
            "boundaryTypes":world.tectonics.boundary_types},
        "crust":{"potential":world.crust.potential,"threshold":world.crust.threshold,
            "continentality":world.crust.continentality,"thicknessMeters":world.crust.thickness_meters,
            "densityKgPerCubicMeter":world.crust.density_kg_per_cubic_meter},
        "terrain":{"baselineMeters":world.terrain.baseline,"convergenceMeters":world.terrain.convergence,
            "divergenceMeters":world.terrain.divergence,"detailMeters":world.terrain.detail,
            "preparationMeters":world.terrain.preparation,"elevationMeters":world.terrain.elevation,
            "oldTransportedCubicMeters":world.terrain.transported_cubic_meters,"oldAppliedPasses":world.terrain.applied_passes},
        "initialWater":{"levelMeters":world.water.level_meters,"resolvedVolumeCubicMeters":world.water.resolved_volume_cubic_meters,
            "depthMeters":world.water.depth_meters,"bodyIds":world.water.body_ids,"mainOceanId":world.water.main_ocean_id},
        "initialDrainage":{"receivers":world.drainage.receivers,"outlets":world.drainage.outlets,
            "flatSteps":world.drainage.flat_steps,"contributingAreaSquareMeters":world.drainage.contributing_area},
        "initialBasins":world.basins,"diagnosticField":world.field,
        "scope":"Complete initial native field provenance for this diagnostic. Not a simulation checkpoint or a prepared World; no restore protocol is supplied."})
}

fn independent_sum(values: impl Iterator<Item = f64>) -> Result<f64, String> {
    let mut total = 0.;
    let mut correction = 0.;
    for value in values {
        let adjusted = value - correction;
        let next = total + adjusted;
        correction = (next - total) - adjusted;
        total = next;
        if !value.is_finite() || !total.is_finite() || !correction.is_finite() {
            return Err("Independent sedimentary diagnostic sum overflowed.".into());
        }
    }
    Ok(total)
}

fn independent_ledger(
    world: &World,
    p: &sedimentary_landscape::Preparation,
) -> Result<Value, String> {
    if !p.independent_net_bed_prism_change_cubic_meters.is_finite()
        || [
            p.remobilized_sediment_volume_cubic_meters,
            p.main_ocean_export_volume_cubic_meters,
            p.other_initial_wet_export_volume_cubic_meters,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.)
    {
        return Err("Invalid sedimentary net prism or external transfer.".into());
    }
    let n = world.surface.areas.len();
    for field in [
        &p.elevation_meters,
        &p.sediment_thickness_meters,
        &p.pending_volume_cubic_meters,
        &p.accepted_uplift_displacement_meters,
        &p.removed_erosion_thickness_meters,
        &p.deposited_thickness_meters,
    ] {
        if field.len() != n || field.iter().any(|v| !v.is_finite()) {
            return Err("Malformed sedimentary preparation diagnostic fields.".into());
        }
    }
    let prism = |thickness: &[f64]| {
        independent_sum(
            thickness
                .iter()
                .zip(&world.surface.areas)
                .map(|(&h, &a)| h * a),
        )
    };
    let uplift = prism(&p.accepted_uplift_displacement_meters)?;
    let erosion = prism(&p.removed_erosion_thickness_meters)?;
    let deposition = prism(&p.deposited_thickness_meters)?;
    let active = prism(&p.sediment_thickness_meters)?;
    let pending = independent_sum(p.pending_volume_cubic_meters.iter().copied())?;
    let net =
        independent_sum((0..n).map(|i| {
            (p.elevation_meters[i] - world.terrain.elevation[i]) * world.surface.areas[i]
        }))?;
    let export = independent_sum(
        [
            p.main_ocean_export_volume_cubic_meters,
            p.other_initial_wet_export_volume_cubic_meters,
        ]
        .into_iter(),
    )?;
    let relative = |error: f64, scale: f64| error.abs() / if scale == 0. { 1. } else { scale };
    let bed_scale = uplift.max(erosion).max(deposition);
    let material_scale = erosion.max(deposition).max(export).max(pending);
    let active_scale = deposition
        .max(p.remobilized_sediment_volume_cubic_meters)
        .max(active);
    let bed_residual = relative((uplift - erosion + deposition) - net, bed_scale);
    let transfer_residual = relative(erosion - deposition - export - pending, material_scale);
    let surviving_residual = relative(
        deposition - p.remobilized_sediment_volume_cubic_meters - active,
        active_scale,
    );
    let mut region_residual: f64 = 0.;
    for i in 0..n {
        let source = p.accepted_uplift_displacement_meters[i];
        let removed = p.removed_erosion_thickness_meters[i];
        let added = p.deposited_thickness_meters[i];
        if source < 0.
            || removed < 0.
            || added < 0.
            || p.sediment_thickness_meters[i] < 0.
            || p.pending_volume_cubic_meters[i] < 0.
        {
            return Err("Negative sedimentary diagnostic stock or gross transfer.".into());
        }
        let change = p.elevation_meters[i] - world.terrain.elevation[i];
        region_residual = region_residual.max(relative(
            (source - removed + added) - change,
            source.max(removed).max(added),
        ));
        if world.water.body_ids[i] != 0
            && (p.elevation_meters[i] != world.terrain.elevation[i]
                || p.sediment_thickness_meters[i] != 0.
                || p.pending_volume_cubic_meters[i] != 0.)
        {
            return Err("Sedimentary preparation changed an original fixed wet collector.".into());
        }
    }
    let mut reported_residual: f64 = 0.;
    for (actual, reported) in [
        (uplift, p.uplift_displacement_prism_cubic_meters),
        (erosion, p.gross_erosion_volume_cubic_meters),
        (deposition, p.gross_deposition_volume_cubic_meters),
        (active, p.surviving_sediment_volume_cubic_meters),
        (pending, p.pending_volume_total_cubic_meters),
    ] {
        if !reported.is_finite() || reported < 0. {
            return Err("Invalid reported sedimentary stock or gross transfer.".into());
        }
        reported_residual =
            reported_residual.max(relative(actual - reported, actual.max(reported)));
    }
    reported_residual = reported_residual.max(relative(
        net - p.independent_net_bed_prism_change_cubic_meters,
        bed_scale,
    ));
    let mut expected_bodies = vec![false; n + 1];
    for &body in &world.water.body_ids {
        expected_bodies[body as usize] = true;
    }
    if p.wet_body_exports.len()
        != expected_bodies
            .iter()
            .skip(1)
            .filter(|&&observed| observed)
            .count()
    {
        return Err("Wet export catalog omits or adds initial collectors.".into());
    }
    let mut previous_body = 0;
    for collector in &p.wet_body_exports {
        if collector.body_id <= previous_body
            || collector.body_id as usize > n
            || !expected_bodies[collector.body_id as usize]
            || !collector.volume_cubic_meters.is_finite()
            || collector.volume_cubic_meters < 0.
        {
            return Err("Invalid wet export collector identity or inventory.".into());
        }
        previous_body = collector.body_id;
    }
    let catalog_ocean = independent_sum(
        p.wet_body_exports
            .iter()
            .filter(|b| b.body_id == world.water.main_ocean_id)
            .map(|b| b.volume_cubic_meters),
    )?;
    let catalog_other = independent_sum(
        p.wet_body_exports
            .iter()
            .filter(|b| b.body_id != world.water.main_ocean_id)
            .map(|b| b.volume_cubic_meters),
    )?;
    let collector_residual = relative(
        catalog_ocean - p.main_ocean_export_volume_cubic_meters,
        catalog_ocean.max(p.main_ocean_export_volume_cubic_meters),
    )
    .max(relative(
        catalog_other - p.other_initial_wet_export_volume_cubic_meters,
        catalog_other.max(p.other_initial_wet_export_volume_cubic_meters),
    ));
    if [
        bed_residual,
        transfer_residual,
        surviving_residual,
        region_residual,
        reported_residual,
        collector_residual,
    ]
    .iter()
    .any(|r| !r.is_finite() || *r > 1e-11)
    {
        return Err("Independent sedimentary stock/prism reconstruction failed.".into());
    }
    Ok(
        json!({"upliftPrismCubicMeters":uplift,"erosionPrismCubicMeters":erosion,
        "depositionPrismCubicMeters":deposition,"activeSedimentCubicMeters":active,
        "pendingSedimentCubicMeters":pending,"netBedPrismChangeCubicMeters":net,
        "relativeBedLedgerResidual":bed_residual,"relativeTransferLedgerResidual":transfer_residual,
        "relativeSurvivingSedimentResidual":surviving_residual,
        "relativeWetCollectorPartitionResidual":collector_residual,
        "independentMainOceanExportCubicMeters":catalog_ocean,
        "independentOtherInitialWetExportCubicMeters":catalog_other,
        "maximumRegionRelativeBedLedgerResidual":region_residual,
        "maximumReportedPrismRelativeResidual":reported_residual}),
    )
}

fn numerical_observations(p: &sedimentary_landscape::Preparation) -> Result<Value, String> {
    if p.history.iter().any(|s| {
        [
            s.unresolved_deposition_candidate_volume_cubic_meters,
            s.source_assembly_rounding_volume_cubic_meters,
            s.debited_deposition_volume_cubic_meters,
            s.relative_deposition_inventory_ledger_residual,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.)
    }) {
        return Err("Invalid sedimentary numerical observation.".into());
    }
    let count = |get: fn(&sedimentary_landscape::Step) -> usize| {
        p.history.iter().try_fold(0usize, |total, step| {
            total
                .checked_add(get(step))
                .ok_or("Numerical observation count overflowed.")
        })
    };
    let proposal_sum = independent_sum(
        p.history
            .iter()
            .map(|s| s.unresolved_deposition_candidate_volume_cubic_meters),
    )?;
    let rounding_sum = independent_sum(
        p.history
            .iter()
            .map(|s| s.source_assembly_rounding_volume_cubic_meters),
    )?;
    Ok(
        json!({"unresolvedPositiveUpliftRegionStepCount":count(|s|s.unresolved_positive_uplift_regions)?,
        "unresolvedPositiveIncisionRegionStepCount":count(|s|s.unresolved_positive_incision_regions)?,
        "unresolvedPositiveDepositionLeafStepCount":count(|s|s.unresolved_positive_deposition_leaves)?,
        "unresolvedSourceAdditionRegionStepCount":count(|s|s.unresolved_source_additions)?,
        "unresolvedDepositionCandidateStepVolumeSumCubicMeters":proposal_sum,
        "maximumUnresolvedDepositionCandidateStepVolumeCubicMeters":p.history.iter().map(|s|s.unresolved_deposition_candidate_volume_cubic_meters).fold(0.,f64::max),
        "sourceAssemblyAbsoluteRoundingStepVolumeSumCubicMeters":rounding_sum,
        "maximumAbsoluteSourceAssemblyStepRoundingVolumeCubicMeters":p.history.iter().map(|s|s.source_assembly_rounding_volume_cubic_meters).fold(0.,f64::max),
        "measuredDepositionDebitVolumeCubicMeters":independent_sum(p.history.iter().map(|s|s.debited_deposition_volume_cubic_meters))?,
        "maximumRelativeDepositionInventoryLedgerResidual":p.history.iter().map(|s|s.relative_deposition_inventory_ledger_residual).fold(0.,f64::max),
        "scope":"Counts sum region/leaf-step observations, not unique features. Unresolved candidate-volume sums can repeat the same proposal across steps and are NOT retained material, accepted deposition, missing stock or a correction. Source-assembly rounding is the sum of absolute per-cell TwoSum errors, observed but never booked as material."}),
    )
}

fn atlas(
    world: &World,
    old_bed: &[f64],
    old_water: &Water,
    new_bed: &[f64],
    new_water: &Water,
    steps: usize,
) -> String {
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1600\" height=\"1040\"><rect width=\"1600\" height=\"1040\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style><text x=\"24\" y=\"30\" font-size=\"20\">Sedimentary prehistory — actual beds + STATIC drainage, NOT active rivers</text><defs><clipPath id=\"clip\"><rect width=\"760\" height=\"380\"/></clipPath></defs>",
    );
    for (i, (title, bed, water)) in [
        (
            "Original geography II · no preparation",
            world.terrain.elevation.as_slice(),
            &world.water,
        ),
        (
            "H=2000 m · previous all-material export control",
            old_bed,
            old_water,
        ),
        (
            "H=2000 m · supplied dry-basin aggradation",
            new_bed,
            new_water,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let x = 24 + (i % 2) * 792;
        let y = 68 + (i / 2) * 434;
        write!(svg,"<text x=\"{x}\" y=\"{y}\" font-size=\"14\">{title}</text><g transform=\"translate({x},{})\" clip-path=\"url(#clip)\">",y+16).unwrap();
        atlas_field::append_field(&mut svg, &world.surface, |j| {
            atlas_field::terrain_color(bed[j] - water.level_meters, water.depth_meters[j])
        });
        drainage_overlay::append(
            &mut svg,
            &world.surface,
            bed,
            &water.body_ids,
            50_000_000_000.,
        );
        svg.push_str("</g>");
    }
    writeln!(svg,"<text x=\"816\" y=\"560\" font-size=\"16\">Blue: static bed receivers, not discharge.</text><text x=\"816\" y=\"590\" font-size=\"14\">Shown from 50,000 km² contributing dry area.</text><text x=\"816\" y=\"620\" font-size=\"14\">Yellow rings: genuine closed dry terminals.</text><text x=\"816\" y=\"650\" font-size=\"14\">Wet exports distinguish main ocean and other initial water.</text><text x=\"24\" y=\"958\" font-size=\"13\">Same original bed and water stock; K=1e-6/year, {steps} x {:.0} years, H=2000 m. Prescribed dry-basin aggradation cap: 0.001 m/year.</text><text x=\"24\" y=\"986\" font-size=\"13\">Common height/depth palette: 0–4500 / 0–5000 m; explicit saturation. Potential drainage and colors are not rivers or biomes.</text><text x=\"24\" y=\"1014\" font-size=\"13\">Deposits use supplied solid material only. Separate pending inventory and wet exports; no forced outlet or calibrated sediment model.</text></svg>",PREPARATION_YEARS/steps as f64).unwrap();
    svg
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (directory, level, seed, steps) = parse(&args)?;
    let selected = seed.is_some();
    let settings = sedimentary_landscape::Settings {
        erodibility_per_year: ERODIBILITY_PER_YEAR,
        step_years: PREPARATION_YEARS / steps as f64,
        steps,
        maximum_aggradation_rate_meters_per_year: MAXIMUM_AGGRADATION_RATE_METERS_PER_YEAR,
    };
    let old_settings = landscape_evolution::Settings {
        erodibility_per_year: settings.erodibility_per_year,
        step_years: settings.step_years,
        steps: settings.steps,
    };
    std::fs::create_dir(&directory)
        .map_err(|e| format!("Cannot create new sedimentary directory: {e}"))?;
    let mut rows = Vec::new();
    for (seed, name, scale, relief, detail) in cases(seed) {
        let mut r = recipe(&seed, level);
        r.continental_scale = scale;
        r.relief_scale = relief;
        r.detail_amplitude_meters = detail;
        let world = World::generate(r)?;
        let mut bytes = serde_json::to_vec(&world.recipe).map_err(|e| e.to_string())?;
        bytes.extend_from_slice(&wire::arrays(&world));
        let checksum = format!("{:08x}", hash(&bytes));
        drop(bytes);
        let initial = json!({"recipe":world.recipe,"nativeCompatibleInitialChecksum":checksum,
            "diagnostics":diagnostics(&world,&world.terrain.elevation,&world.water)?});
        let rates = landscape_evolution::structural_uplift_rates(
            &world.terrain.convergence,
            &world.water,
            ERODIBILITY_PER_YEAR,
            CHARACTERISTIC_HEIGHT_METERS,
        )?;
        let initial_file = format!("{name}-initial.json");
        write_json(
            &directory.join(&initial_file),
            &json!({"initial":initial,
            "initialNativeFields":initial_native_fields(&world),"upliftRatesMetersPerYear":rates}),
        )?;

        let started = Instant::now();
        let old = landscape_evolution::prepare(
            &world.surface,
            &world.terrain.elevation,
            &world.water,
            &rates,
            old_settings,
        )?;
        let old_seconds = started.elapsed().as_secs_f64();
        let old_water = initialized_water(&world, &old.elevation_meters)?;
        let old_summary = json!({"variant":"previous-export-control","modelVersion":landscape_evolution::MODEL_VERSION,
            "characteristicHeightMeters":CHARACTERISTIC_HEIGHT_METERS,"settings":old_settings,
            "diagnostics":diagnostics(&world,&old.elevation_meters,&old_water)?,
            "upliftDisplacementPrismCubicMeters":old.uplift_displacement_prism_cubic_meters,
            "exportedErosionMaterialCubicMeters":old.exported_erosion_material_cubic_meters,
            "relativePrismLedgerResidual":old.relative_prism_ledger_residual,"preparationSeconds":old_seconds});
        let old_file = format!("{name}-previous-export.json");
        write_json(
            &directory.join(&old_file),
            &json!({"initial":initial,"summary":old_summary,"preparation":old,
            "candidateInitialWater":{"levelMeters":old_water.level_meters,"resolvedVolumeCubicMeters":old_water.resolved_volume_cubic_meters,
                "depthMeters":old_water.depth_meters,"bodyIds":old_water.body_ids,"mainOceanId":old_water.main_ocean_id}}),
        )?;

        let started = Instant::now();
        let new = sedimentary_landscape::prepare(
            &world.surface,
            &world.terrain.elevation,
            &world.water,
            &rates,
            settings,
        )?;
        let new_seconds = started.elapsed().as_secs_f64();
        let independent = independent_ledger(&world, &new)?;
        let observations = numerical_observations(&new)?;
        let new_water = initialized_water(&world, &new.elevation_meters)?;
        let new_summary = json!({"variant":"sedimentary-terminal-retention","modelVersion":sedimentary_landscape::MODEL_VERSION,
            "characteristicHeightMeters":CHARACTERISTIC_HEIGHT_METERS,"settings":settings,
            "diagnostics":diagnostics(&world,&new.elevation_meters,&new_water)?,
            "upliftDisplacementPrismCubicMeters":new.uplift_displacement_prism_cubic_meters,
            "grossErosionVolumeCubicMeters":new.gross_erosion_volume_cubic_meters,
            "grossDepositionVolumeCubicMeters":new.gross_deposition_volume_cubic_meters,
            "remobilizedSedimentVolumeCubicMeters":new.remobilized_sediment_volume_cubic_meters,
            "mainOceanExportVolumeCubicMeters":new.main_ocean_export_volume_cubic_meters,
            "otherInitialWetExportVolumeCubicMeters":new.other_initial_wet_export_volume_cubic_meters,
            "pendingVolumeTotalCubicMeters":new.pending_volume_total_cubic_meters,
            "survivingSedimentVolumeCubicMeters":new.surviving_sediment_volume_cubic_meters,
            "netBedPrismChangeCubicMeters":new.independent_net_bed_prism_change_cubic_meters,
            "relativeLedgerResidual":new.relative_ledger_residual,
            "independentLedger":independent,"numericalObservations":observations,"preparationSeconds":new_seconds});
        let new_file = format!("{name}-sedimentary.json");
        write_json(
            &directory.join(&new_file),
            &json!({"initial":initial,"summary":new_summary,"preparation":new,
            "candidateInitialWater":{"levelMeters":new_water.level_meters,"resolvedVolumeCubicMeters":new_water.resolved_volume_cubic_meters,
                "depthMeters":new_water.depth_meters,"bodyIds":new_water.body_ids,"mainOceanId":new_water.main_ocean_id},
            "scope":"Raw diagnostic state only. Final water is an independent stock-preserving initialization diagnostic, not water advanced during deposition or a resumable prepared World."}),
        )?;
        let atlas_file = format!("{name}.svg");
        write_new(
            &directory.join(&atlas_file),
            atlas(
                &world,
                &old.elevation_meters,
                &old_water,
                &new.elevation_meters,
                &new_water,
                steps,
            )
            .as_bytes(),
        )?;
        println!(
            "{name}: export control {old_seconds:.3} s, sedimentary {new_seconds:.3} s; ledger {:.3e}, pending {:.3e} m³.",
            new.relative_ledger_residual, new.pending_volume_total_cubic_meters
        );
        rows.push(json!({"case":name,"seed":seed,"category":if selected{"selected"}else if name.starts_with("ordinary-"){"ordinary"}else{"directed"},
            "initial":initial,"previousExport":old_summary,"sedimentary":new_summary,
            "files":{"initial":initial_file,"previousExport":old_file,"sedimentary":new_file,"atlas":atlas_file}}));
    }
    write_json(
        &directory.join("manifest.json"),
        &json!({"reportVersion":"sedimentary-landscape-review-1","subdivision":level,
        "modelVersion":sedimentary_landscape::MODEL_VERSION,"characteristicHeightMeters":CHARACTERISTIC_HEIGHT_METERS,
        "settings":settings,"rows":rows,
        "scope":"Synthetic initial-condition prehistory: source, physical downhill incision, delivered-material dry-terminal retention and bounded aggradation. No forced ocean connection, hydraulic water advance, evolving plates, soil fertility, biomes, calibration or selected desktop model.",
        "accountingScope":"Reference-sphere solid-equivalent prisms, not density mass or porosity-expanded sediment. Gross erosion/deposition/remobilization, remaining deposit, pending inventory and separate main-ocean/other-initial-wet exports are distinct. Uplift is kinematic displacement bookkeeping.",
        "outputScope":"New-directory-only diagnostics; full initial native fields and raw preparations retained. Manifest written last; partial directories are not complete runs. No snapshot/checkpoint restore promise; timings are observations, not physical identity."}),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arguments_case_controls_and_source_units_are_explicit() {
        for args in [
            vec![],
            vec!["out"],
            vec!["out", "3"],
            vec!["out", "7"],
            vec!["out", "6", ""],
            vec!["out", "6", "seed", "extra"],
            vec!["out", "6", "--unknown"],
            vec!["out", "6", "--steps=17"],
            vec!["out", "6", "--steps=16", "--steps=64"],
            vec!["", "6"],
        ] {
            assert!(parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        assert_eq!(cases(None).len(), 23);
        let controls: Vec<_> = cases(None)
            .into_iter()
            .filter(|(_, name, _, _, _)| !name.starts_with("ordinary-"))
            .collect();
        assert_eq!(controls[0].2, 0.5);
        assert_eq!(controls[1].2, 2.);
        assert_eq!((controls[2].3, controls[2].4), (0., 0.));
        assert_eq!(cases(Some("first-light".into())).len(), 1);
        for steps in [16, 32, 64] {
            assert_eq!((PREPARATION_YEARS / steps as f64) * steps as f64, 800_000.);
            assert_eq!(
                MAXIMUM_AGGRADATION_RATE_METERS_PER_YEAR * (PREPARATION_YEARS / steps as f64),
                800. / steps as f64
            );
        }
        assert_eq!(ERODIBILITY_PER_YEAR * CHARACTERISTIC_HEIGHT_METERS, 0.002);
        let selected = parse(&[
            "sedimentary-cli-unit-uncreated-output".into(),
            "6".into(),
            "first-light".into(),
            "--steps=64".into(),
        ])
        .unwrap();
        assert_eq!(selected.1, 6);
        assert_eq!(selected.2.as_deref(), Some("first-light"));
        assert_eq!(selected.3, 64);
        let cohort = parse(&[
            "sedimentary-cli-unit-uncreated-output".into(),
            "4".into(),
            "--steps=16".into(),
        ])
        .unwrap();
        assert!(cohort.2.is_none());
        assert_eq!(cohort.3, 16);
        assert!(parse(&[".".into(), "6".into()]).is_err());
    }

    #[test]
    fn independent_zero_input_and_fully_wet_ledgers_keep_zero_scales_explicit() {
        for fully_wet in [false, true] {
            let mut r = recipe("first-light", 2);
            if fully_wet {
                r.water = WaterSettings::Coverage { fraction: 1. };
            }
            let world = World::generate(r).unwrap();
            let p = sedimentary_landscape::prepare(
                &world.surface,
                &world.terrain.elevation,
                &world.water,
                &vec![0.; world.surface.areas.len()],
                sedimentary_landscape::Settings {
                    erodibility_per_year: 0.,
                    step_years: 1.,
                    steps: 2,
                    maximum_aggradation_rate_meters_per_year: 0.001,
                },
            )
            .unwrap();
            let checked = independent_ledger(&world, &p).unwrap();
            for key in [
                "relativeBedLedgerResidual",
                "relativeTransferLedgerResidual",
                "relativeSurvivingSedimentResidual",
                "relativeWetCollectorPartitionResidual",
                "maximumRegionRelativeBedLedgerResidual",
                "maximumReportedPrismRelativeResidual",
            ] {
                assert_eq!(checked[key], 0.);
            }
            assert_eq!(p.elevation_meters, world.terrain.elevation);
            let observations = numerical_observations(&p).unwrap();
            assert_eq!(observations["unresolvedPositiveDepositionLeafStepCount"], 0);
            assert_eq!(observations["measuredDepositionDebitVolumeCubicMeters"], 0.);
            let mut malformed = p.clone();
            malformed.wet_body_exports.clear();
            assert!(independent_ledger(&world, &malformed).is_err());
            let mut malformed = p.clone();
            malformed.independent_net_bed_prism_change_cubic_meters = f64::NAN;
            assert!(independent_ledger(&world, &malformed).is_err());
            if !fully_wet {
                let dry = world
                    .water
                    .body_ids
                    .iter()
                    .position(|&body| body == 0)
                    .unwrap();
                let mut malformed = p.clone();
                malformed.elevation_meters[dry] += 1000.;
                let accepted = malformed.elevation_meters[dry] - world.terrain.elevation[dry];
                malformed.accepted_uplift_displacement_meters[dry] = accepted;
                malformed.uplift_displacement_prism_cubic_meters =
                    accepted * world.surface.areas[dry];
                malformed.independent_net_bed_prism_change_cubic_meters =
                    malformed.uplift_displacement_prism_cubic_meters;
                // Large valid displacement must not hide a tiny unsourced stock.
                malformed.pending_volume_cubic_meters[dry] = 1e-5;
                malformed.pending_volume_total_cubic_meters = 1e-5;
                assert!(independent_ledger(&world, &malformed).is_err());
            }
        }
    }

    #[test]
    fn numerical_observations_distinguish_repeated_proposals_from_material() {
        let mut r = recipe("first-light", 2);
        r.water = WaterSettings::Coverage { fraction: 1. };
        let world = World::generate(r).unwrap();
        let mut p = sedimentary_landscape::prepare(
            &world.surface,
            &world.terrain.elevation,
            &world.water,
            &vec![0.; world.surface.areas.len()],
            sedimentary_landscape::Settings {
                erodibility_per_year: 0.,
                step_years: 1.,
                steps: 2,
                maximum_aggradation_rate_meters_per_year: 0.001,
            },
        )
        .unwrap();
        for (i, step) in p.history.iter_mut().enumerate() {
            step.unresolved_positive_deposition_leaves = 1;
            step.unresolved_deposition_candidate_volume_cubic_meters = 4.;
            step.unresolved_positive_uplift_regions = i + 1;
            step.unresolved_positive_incision_regions = 2;
            step.unresolved_source_additions = 1;
            step.source_assembly_rounding_volume_cubic_meters = if i == 0 { 0.25 } else { 0.5 };
            step.relative_deposition_inventory_ledger_residual = (i + 1) as f64 * 1e-15;
        }
        let observations = numerical_observations(&p).unwrap();
        assert_eq!(observations["unresolvedPositiveUpliftRegionStepCount"], 3);
        assert_eq!(observations["unresolvedPositiveIncisionRegionStepCount"], 4);
        assert_eq!(observations["unresolvedPositiveDepositionLeafStepCount"], 2);
        assert_eq!(observations["unresolvedSourceAdditionRegionStepCount"], 2);
        assert_eq!(
            observations["unresolvedDepositionCandidateStepVolumeSumCubicMeters"],
            8.
        );
        assert_eq!(
            observations["maximumUnresolvedDepositionCandidateStepVolumeCubicMeters"],
            4.
        );
        assert_eq!(
            observations["sourceAssemblyAbsoluteRoundingStepVolumeSumCubicMeters"],
            0.75
        );
        assert_eq!(
            observations["maximumAbsoluteSourceAssemblyStepRoundingVolumeCubicMeters"],
            0.5
        );
        assert_eq!(
            observations["maximumRelativeDepositionInventoryLedgerResidual"],
            2e-15
        );
        assert_eq!(p.pending_volume_total_cubic_meters, 0.);
        assert_eq!(p.gross_deposition_volume_cubic_meters, 0.);
        independent_ledger(&world, &p).unwrap();
    }
}
