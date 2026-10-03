//! Reproducible typed water exchange with checkpointed seasonal forcing.
use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::{Model, SECONDS_PER_DAY, Settings, State},
    seasonal_temperature, seasonal_wind,
};
use serde_json::{Value, json};
use std::io::Write;

fn run(
    model: &Model,
    world: &World,
    days: usize,
    step_seconds: u32,
) -> Result<(State, Value), String> {
    let mut state = model.initial_state();
    let mut substeps = 0;
    let mut coupled_substeps = 0;
    let mut max_exchange_residual: f64 = 0.;
    let mut max_routing_residual: f64 = 0.;
    let mut max_relative_budget_residual: f64 = 0.;
    let mut max_vapor_column: f64 = 0.;
    let mut precipitation_on_dry = 0.;
    let mut monthly_precipitation = [0.; 12];
    let total_area = total_mass(&world.surface.areas);
    for _ in 0..days * SECONDS_PER_DAY as usize / step_seconds as usize {
        let day = state.elapsed_seconds() / SECONDS_PER_DAY;
        let month = (day as usize % 365) * 12 / 365;
        let step = model.advance(&mut state, step_seconds)?;
        substeps += step.transport_substeps;
        coupled_substeps += step.coupled_substeps;
        max_vapor_column =
            max_vapor_column.max(step.maximum_observed_vapor_column_kilograms_per_square_meter);
        max_exchange_residual =
            max_exchange_residual.max(step.maximum_absolute_local_exchange_residual_kilograms);
        max_routing_residual =
            max_routing_residual.max(step.maximum_absolute_routing_residual_kilograms);
        max_relative_budget_residual = max_relative_budget_residual.max(
            step.budget.residual_kilograms.abs()
                / step.budget.initial_mobile_water_kilograms.max(1.),
        );
        monthly_precipitation[month] += total_mass(&step.precipitation_kilograms) / total_area;
        for ((&rain, &initial_depth), (&vapor, &area)) in step
            .precipitation_kilograms
            .iter()
            .zip(&world.water.depth_meters)
            .zip(state.vapor_kilograms().iter().zip(&world.surface.areas))
        {
            if initial_depth == 0. {
                precipitation_on_dry += rain;
            }
            max_vapor_column = max_vapor_column.max(vapor / area);
        }
    }
    let budget = model.budget(&state)?;
    let summary = json!({
        "stepSeconds": step_seconds,
        "transportSubsteps": substeps,
        "coupledSubsteps": coupled_substeps,
        "maximumCoupledStepSeconds": model.initial_state().checkpoint().settings.max_coupled_step_seconds,
        "budget": budget,
        "maximumRelativeMobileWaterBudgetResidual": max_relative_budget_residual,
        "maximumAbsoluteLocalExchangeResidualKilograms": max_exchange_residual,
        "maximumAbsoluteRoutingResidualKilograms": max_routing_residual,
        "maximumObservedVaporColumnKilogramsPerSquareMeter": max_vapor_column,
        "precipitationOnInitiallyDryRegionsKilograms": precipitation_on_dry,
        "globalAreaWeightedPrecipitationTotalMillimetersWaterEquivalent": budget.cumulative_precipitation_kilograms / total_area,
        "globalAreaWeightedEvaporationTotalMillimetersWaterEquivalent": budget.cumulative_evaporation_kilograms / total_area,
        "globalAreaWeightedPrecipitationByNumberedMonthMillimetersWaterEquivalent": monthly_precipitation,
        "monthlyTotalsAggregateAllReportedYears": true,
    });
    Ok((state, summary))
}

fn report(recipe: Recipe, days: usize) -> Result<Value, String> {
    if !(1..=3650).contains(&days) {
        return Err("Report duration must be 1–3650 days.".into());
    }
    let world = World::generate(recipe)?;
    let model = Model::from_world(
        &world,
        Settings::default(),
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )?;
    let (state, baseline) = run(&model, &world, days, 86400)?;
    let (refined, refinement) = run(&model, &world, days, 900)?;
    let control_settings = Settings {
        routing_enabled: false,
        ..Default::default()
    };
    let control_model = Model::from_world(
        &world,
        control_settings,
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )?;
    let (_, without_routing) = run(&control_model, &world, days, 86400)?;
    let scale = model.budget(&state)?.initial_mobile_water_kilograms.max(1.);
    let difference = state
        .owned_stocks()
        .zip(refined.owned_stocks())
        .map(|(a, b)| (a - b).abs())
        .sum::<f64>()
        / scale;
    // Check the full saved clock/stocks/ledgers, then continue both states exactly.
    let checkpoint = state.checkpoint();
    let encoded = serde_json::to_vec(&checkpoint).map_err(|e| e.to_string())?;
    let decoded = serde_json::from_slice(&encoded).map_err(|e| e.to_string())?;
    let (restored_model, mut restored) = Model::restore(decoded)?;
    if restored != state {
        return Err("Moisture checkpoint round-trip changed state.".into());
    }
    if days < 3650 {
        let mut original = state.clone();
        model.advance(&mut original, 86400)?;
        restored_model.advance(&mut restored, 86400)?;
        if restored != original {
            return Err("Moisture checkpoint continuation diverged.".into());
        }
    }
    Ok(json!({
        "reportVersion": 3,
        "days": days,
        "regionCount": world.surface.areas.len(),
        "baseline": baseline,
        "refinement": refinement,
        "withoutRoutingControl": without_routing,
        "withoutRoutingControlSettings": control_settings,
        "relativeCombinedStockL1DifferenceAfterTimeRefinement": difference,
        "checkpointRoundTripExact": true,
        "checkpointContinuationChecked": days < 3650,
        "finalCheckpoint": checkpoint,
        "scope": "Closed six-stock partition on fixed initial geography. Runoff moves through neighboring single receivers with empirical linear storage response, remains in transit until arrival, and enters an evaporating terminal store at its physical first-contact location. Closed sinks retain their pool. Original wet/dry forcing is unchanged. No lake spill levels, groundwater, channel evaporation, energy feedback, shoreline update, or desktop playback. Inactive deep water is excluded. No-routing daily control uses the same recipe and surface/atmospheric laws."
    }))
}

fn ensemble() -> Result<Value, String> {
    let base: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    let mut cases = Vec::new();
    for seed in ["seasonal-reference", "moisture-coast", "moisture-interior"] {
        for subdivision in [2, 3, 4] {
            for radius in [1_000_000., 6_371_000.] {
                let mut recipe = base.clone();
                recipe.seed = seed.into();
                recipe.subdivision = subdivision;
                recipe.radius_meters = radius;
                cases.push((recipe, 365));
            }
        }
    }
    cases.push((base, 3650));
    let mut samples = Vec::new();
    let mut failures = 0;
    for (recipe, days) in cases {
        eprintln!(
            "Seasonal moisture: seed={}, subdivision={}, radius={}, days={}",
            recipe.seed, recipe.subdivision, recipe.radius_meters, days
        );
        match report(recipe.clone(), days) {
            Ok(value) => samples.push(json!({
                "status": "accepted", "recipe": recipe, "days": days,
                "baseline": value["baseline"], "refinement": value["refinement"],
                "withoutRoutingControl": value["withoutRoutingControl"],
                "relativeCombinedStockL1DifferenceAfterTimeRefinement": value["relativeCombinedStockL1DifferenceAfterTimeRefinement"],
                "checkpointRoundTripExact": value["checkpointRoundTripExact"],
                "checkpointContinuationChecked": value["checkpointContinuationChecked"],
            })),
            Err(error) => { failures += 1; samples.push(json!({ "status": "rejected", "recipe": recipe, "days": days, "error": error })); }
        }
    }
    Ok(json!({
        "reportVersion": 3,
        "modelVersion": planimulation_core::seasonal_moisture::MODEL_VERSION,
        "transportModelVersion": planimulation_core::moisture_transport::MODEL_VERSION,
        "temperatureModelVersion": seasonal_temperature::MODEL_VERSION,
        "windModelVersion": seasonal_wind::MODEL_VERSION,
        "surfaceModelVersion": planimulation_core::surface_water::MODEL_VERSION,
        "runoffModelVersion": planimulation_core::runoff_transport::MODEL_VERSION,
        "withoutRoutingControlSettings": Settings { routing_enabled: false, ..Default::default() },
        "settings": Settings::default(), "temperatureSettings": seasonal_temperature::Settings::default(), "windSettings": seasonal_wind::Settings::default(),
        "failureCount": failures, "samples": samples,
        "scope": "18 generated one-year cases plus one ten-year case, each with a 15-minute repeat and a daily no-routing control; default internal coupled steps are at most one hour. All failures retained. Numerical checks, not climate calibration or generated-geography spatial convergence."
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("Usage: seasonal_moisture_report RECIPE.json [DAYS] | --ensemble")?;
    let output = if path == "--ensemble" {
        if args.next().is_some() {
            return Err("Unexpected ensemble argument.".into());
        }
        ensemble()?
    } else {
        let days = args
            .next()
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(365);
        if args.next().is_some() {
            return Err("Unexpected report argument.".into());
        }
        report(serde_json::from_slice(&std::fs::read(path)?)?, days)?
    };
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &output)?;
    writeln!(out)?;
    if output["failureCount"]
        .as_u64()
        .is_some_and(|count| count > 0)
    {
        return Err("Seasonal ensemble retained rejected cases; inspect its JSON report.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coupled_report_replays_with_precipitation_and_complete_checkpoint() {
        let mut recipe: Recipe = serde_json::from_str(include_str!(
            "../../docs/scenarios/seasonal-temperature.json"
        ))
        .unwrap();
        recipe.subdivision = 1;
        let value = report(recipe.clone(), 60).unwrap();
        assert_eq!(value, report(recipe, 60).unwrap());
        assert_eq!(value["checkpointRoundTripExact"], true);
        assert_eq!(value["checkpointContinuationChecked"], true);
        assert!(
            value["baseline"]["globalAreaWeightedPrecipitationTotalMillimetersWaterEquivalent"]
                .as_f64()
                .unwrap()
                > 0.
        );
        assert_eq!(
            value["finalCheckpoint"]["modelVersion"],
            "seasonal-moisture-3"
        );
    }
}
