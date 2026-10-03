//! A bounded, source-free column-water run using the existing seasonal winds.
use planimulation_core::{
    Recipe, World,
    moisture_transport::{Flow, Geometry, MODEL_VERSION, Settings, total_mass},
    seasonal_temperature::{DAYS_PER_YEAR, MONTHS_PER_YEAR},
};
use serde_json::{Value, json};
use std::io::Write;

const SECONDS_PER_DAY: f64 = 86400.;
const WET_COLUMN_KG_PER_SQUARE_METER: f64 = 25.;
const DRY_COLUMN_KG_PER_SQUARE_METER: f64 = 5.;

fn run(
    flows: &[Flow],
    initial: &[f64],
    days: usize,
    settings: Settings,
) -> Result<(Vec<f64>, Value), String> {
    let mut stock = initial.to_vec();
    let mut substeps = 0;
    let mut max_outgoing_fraction: f64 = 0.;
    let mut max_relative_daily_residual: f64 = 0.;
    let mut transported = 0.;
    for day in 0..days {
        let month = (day % DAYS_PER_YEAR) * MONTHS_PER_YEAR / DAYS_PER_YEAR;
        let step = flows[month].advance(&stock, SECONDS_PER_DAY, settings)?;
        substeps += step.budget.substeps;
        max_outgoing_fraction = max_outgoing_fraction.max(step.budget.max_outgoing_fraction);
        max_relative_daily_residual = max_relative_daily_residual
            .max(step.budget.residual_kilograms.abs() / step.budget.initial_kilograms.max(1.));
        transported += step.budget.transported_kilograms;
        stock = step.stock_kilograms;
    }
    let initial_total = total_mass(initial);
    let final_total = total_mass(&stock);
    let residual = final_total - initial_total;
    if residual.abs() > 1e-12 * initial_total.max(1.) {
        return Err("Accumulated moisture run exceeds its declared mass tolerance.".into());
    }
    Ok((
        stock,
        json!({
            "initialKilograms": initial_total,
            "finalKilograms": final_total,
            "residualKilograms": residual,
            "relativeResidual": residual / initial_total.max(1.),
            "maximumRelativeDailyResidual": max_relative_daily_residual,
            "transportedKilograms": transported,
            "substeps": substeps,
            "maximumOutgoingFraction": max_outgoing_fraction,
        }),
    ))
}

fn report(recipe: Recipe, days: usize) -> Result<Value, String> {
    if !(1..=3650).contains(&days) {
        return Err("Report duration must be 1–3650 days.".into());
    }
    let world = World::generate(recipe)?;
    let geometry = Geometry::from_surface(&world.surface, world.recipe.radius_meters)?;
    let tilt = planimulation_core::seasonal_temperature::Settings::default().axial_tilt_degrees;
    let wind_settings = planimulation_core::seasonal_wind::Settings::default();
    let mut flows = Vec::with_capacity(MONTHS_PER_YEAR);
    for month in 0..MONTHS_PER_YEAR {
        flows.push(Flow::seasonal_month(&geometry, month, wind_settings, tilt)?);
    }
    // Declared artificial initial condition, never deducted from generated surface water.
    let initial: Vec<_> = world
        .surface
        .areas
        .iter()
        .zip(&world.water.depth_meters)
        .map(|(area, depth)| {
            area * if *depth > 0. {
                WET_COLUMN_KG_PER_SQUARE_METER
            } else {
                DRY_COLUMN_KG_PER_SQUARE_METER
            }
        })
        .collect();
    let settings = Settings::default();
    let (stock, budget) = run(&flows, &initial, days, settings)?;
    let refined_settings = Settings {
        max_outgoing_fraction: 0.4,
        ..settings
    };
    let (refined, refined_budget) = run(&flows, &initial, days, refined_settings)?;
    let initial_total = total_mass(&initial);
    let columns: Vec<_> = stock
        .iter()
        .zip(&world.surface.areas)
        .map(|(mass, area)| mass / area)
        .collect();
    let column_min = columns.iter().copied().fold(f64::INFINITY, f64::min);
    let column_max = columns.iter().copied().fold(0., f64::max);
    let dry_final_mass: f64 = stock
        .iter()
        .zip(&world.water.depth_meters)
        .filter(|(_, depth)| **depth == 0.)
        .map(|(mass, _)| mass)
        .sum();
    let dry_initial_mass: f64 = initial
        .iter()
        .zip(&world.water.depth_meters)
        .filter(|(_, depth)| **depth == 0.)
        .map(|(mass, _)| mass)
        .sum();
    Ok(json!({
        "reportVersion": 1,
        "transportModelVersion": MODEL_VERSION,
        "windModelVersion": planimulation_core::seasonal_wind::MODEL_VERSION,
        "seasonalGeometryModelVersion": planimulation_core::seasonal_temperature::MODEL_VERSION,
        "recipe": world.recipe,
        "regionCount": stock.len(),
        "sharedBoundaryCount": geometry.boundaries().len(),
        "settings": settings,
        "windSettings": wind_settings,
        "axialTiltDegrees": tilt,
        "initialCondition": {
            "kind": "prescribed-column-water-from-initial-wet-dry-mask",
            "wetKilogramsPerSquareMeter": WET_COLUMN_KG_PER_SQUARE_METER,
            "dryKilogramsPerSquareMeter": DRY_COLUMN_KG_PER_SQUARE_METER,
            "deductedFromSurfaceWater": false
        },
        "calendar": { "startDay": 0, "endDay": days, "daysPerYear": DAYS_PER_YEAR, "secondsPerDay": SECONDS_PER_DAY, "windSampling": "piecewise-constant-monthly-mean-at-boundary-segment-midpoints" },
        "budget": budget,
        "refinedSettings": refined_settings,
        "refinedBudget": refined_budget,
        "relativeStockL1DifferenceAfterTimeRefinement": stock.iter().zip(&refined).map(|(a,b)| (a-b).abs()).sum::<f64>() / initial_total,
        "finalColumnMinimumKilogramsPerSquareMeter": column_min,
        "finalColumnMaximumKilogramsPerSquareMeter": column_max,
        "netTransferToInitiallyDryRegionsKilograms": dry_final_mass - dry_initial_mass,
        "finalStockKilograms": stock,
        "scope": "Source-free passive column-water transport; prescribed initial stock, no evaporation, saturation, precipitation, air-mass dynamics, surface exchange, or desktop playback. Concentration may grow in convergent winds. This report is not a full climate checkpoint."
    }))
}

fn ensemble_report() -> Result<Value, String> {
    let base: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    let mut failures = 0;
    for seed in ["seasonal-reference", "moisture-coast", "moisture-interior"] {
        for subdivision in [2, 3, 4] {
            for radius in [1_000_000., 6_371_000.] {
                let mut recipe = base.clone();
                recipe.seed = seed.into();
                recipe.subdivision = subdivision;
                recipe.radius_meters = radius;
                match report(recipe.clone(), 365) {
                    Ok(value) => rows.push(json!({
                        "status": "accepted",
                        "recipe": recipe,
                        "budget": value["budget"],
                        "refinedBudget": value["refinedBudget"],
                        "relativeStockL1DifferenceAfterTimeRefinement": value["relativeStockL1DifferenceAfterTimeRefinement"],
                        "finalColumnMinimumKilogramsPerSquareMeter": value["finalColumnMinimumKilogramsPerSquareMeter"],
                        "finalColumnMaximumKilogramsPerSquareMeter": value["finalColumnMaximumKilogramsPerSquareMeter"],
                        "netTransferToInitiallyDryRegionsKilograms": value["netTransferToInitiallyDryRegionsKilograms"],
                    })),
                    Err(error) => {
                        failures += 1;
                        rows.push(json!({ "status": "rejected", "recipe": recipe, "error": error }));
                    }
                }
            }
        }
    }
    Ok(json!({
        "reportVersion": 1,
        "transportModelVersion": MODEL_VERSION,
        "windModelVersion": planimulation_core::seasonal_wind::MODEL_VERSION,
        "seasonalGeometryModelVersion": planimulation_core::seasonal_temperature::MODEL_VERSION,
        "settings": Settings::default(),
        "refinedSettings": Settings { max_outgoing_fraction: 0.4, ..Settings::default() },
        "windSettings": planimulation_core::seasonal_wind::Settings::default(),
        "axialTiltDegrees": planimulation_core::seasonal_temperature::Settings::default().axial_tilt_degrees,
        "days": 365,
        "secondsPerDay": SECONDS_PER_DAY,
        "initialWetColumnKilogramsPerSquareMeter": WET_COLUMN_KG_PER_SQUARE_METER,
        "initialDryColumnKilogramsPerSquareMeter": DRY_COLUMN_KG_PER_SQUARE_METER,
        "failureCount": failures,
        "samples": rows,
        "scope": "18 bounded source-free runs with artificial initial stocks; all failures retained. Generated wet masks differ across meshes, so this is not a spatial-convergence study or climate calibration."
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("Usage: moisture_transport_report RECIPE.json [DAYS] | --ensemble")?;
    if path == "--ensemble" {
        if args.next().is_some() {
            return Err("Unexpected ensemble argument.".into());
        }
        let output = ensemble_report()?;
        let mut out = std::io::stdout().lock();
        serde_json::to_writer_pretty(&mut out, &output)?;
        writeln!(out)?;
        return Ok(());
    }
    let days = args
        .next()
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(365);
    if args.next().is_some() {
        return Err("Unexpected report argument.".into());
    }
    let recipe: Recipe = serde_json::from_slice(&std::fs::read(path)?)?;
    let output = report(recipe, days)?;
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &output)?;
    writeln!(out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasonal_transport_report_replays_and_moves_water_onto_dry_regions() {
        let mut recipe: Recipe = serde_json::from_str(include_str!(
            "../../docs/scenarios/seasonal-temperature.json"
        ))
        .unwrap();
        recipe.subdivision = 2;
        let result = report(recipe.clone(), 365).unwrap();
        assert_eq!(result, report(recipe, 365).unwrap());
        assert_eq!(result["transportModelVersion"], "moisture-transport-1");
        assert!(result["budget"]["relativeResidual"].as_f64().unwrap().abs() < 1e-12);
        assert!(
            result["netTransferToInitiallyDryRegionsKilograms"]
                .as_f64()
                .unwrap()
                > 0.
        );
        assert!(
            result["finalStockKilograms"]
                .as_array()
                .unwrap()
                .iter()
                .all(|mass| mass.as_f64().unwrap() >= 0.)
        );
    }
}
