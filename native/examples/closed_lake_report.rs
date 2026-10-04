//! Legacy model-8 probes or separately selected bounded model-9 evolution.
use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::{
        ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics, SurfaceNumerics,
        TerminalNumerics,
        closed_lake::{self, Layout},
    },
};
use serde_json::{Value, json};
use std::io::Write;

fn run(limit: u32, years: u32, coupled: bool) -> Result<Value, String> {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    recipe.subdivision = 2;
    recipe.radius_meters = 6_371_000.;
    let world = World::generate(recipe)?;
    let layout = Layout::from_world(&world)?;
    let settings = Settings {
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: coupled.then_some(ClosedLakeExchange::FrozenLeafExposure),
        max_coupled_step_seconds: limit,
        ..Default::default()
    };
    let model = Model::from_world(&world, settings, Default::default(), Default::default())?;
    let normals = planimulation_core::seasonal_temperature::Normals::from_world(
        &world,
        model.temperature_settings(),
    )?;
    let mut state = model.initial_state();
    let mut previous_runoff = state.checkpoint().cumulative_runoff_transfers;
    let mut previous_capture = state.checkpoint().cumulative_lake_capture_kilograms;
    let mut previous_capture_low = state.checkpoint().cumulative_lake_capture_low_kilograms;
    let mut annual = Vec::new();
    let mut failure = None;
    let mut atomic = None;
    let mut maximum_mass: f64 = 0.;
    let mut maximum_local: f64 = 0.;
    let mut last_p = 0.;
    let mut last_e = 0.;
    let area = total_mass(&world.surface.areas);
    'years: for year in 1..=years {
        for _ in 0..365 {
            let before = state.clone();
            match model.advance(&mut state, 86400) {
                Ok(step) => {
                    maximum_mass = maximum_mass.max(
                        step.budget.residual_kilograms.abs()
                            / step.budget.initial_mobile_water_kilograms.max(1.),
                    );
                    maximum_local = maximum_local
                        .max(step.budget.maximum_relative_local_surface_ledger_residual);
                }
                Err(e) => {
                    atomic = Some(state == before);
                    failure = Some(e);
                    break 'years;
                }
            }
        }
        let before = serde_json::to_string(&state.checkpoint()).map_err(|e| e.to_string())?;
        let observations = layout.capture(&model, &state)?;
        let checkpoint = state.checkpoint();
        let mut lakes = Vec::new();
        for (i, o) in observations.into_iter().enumerate() {
            let mut probes = Vec::new();
            for month in 0..if coupled { 0 } else { 12 } {
                probes.push(match layout.probe(&model, &state, i, month, 300) {
                    Ok(p) => json!({"monthIndex":month,"result":p,"failure":null}),
                    Err(e) => json!({"monthIndex":month,"result":null,"failure":e}),
                });
            }
            let r = o.terminal_region;
            let delivered = checkpoint.cumulative_runoff_transfers[r].terminal_delivery
                - previous_runoff[r].terminal_delivery;
            let evaporated = checkpoint.cumulative_runoff_transfers[r].terminal_evaporation
                - previous_runoff[r].terminal_evaporation;
            lakes.push(json!({"observation":o,"terminalAreaSquareMeters":world.surface.areas[r],"upstreamDryCatchmentAreaSquareMeters":world.drainage.contributing_area[r],"terminalTemperatureCelsiusByMonth":normals.monthly_temperature_celsius.iter().map(|m|m[r]).collect::<Vec<_>>(),"annualActualTerminalDeliveryKilograms":delivered,"annualActualTerminalEvaporationKilograms":evaporated,"annualActualTerminalDeliveryMillimeters":delivered/world.surface.areas[r],"annualActualTerminalEvaporationMillimeters":evaporated/world.surface.areas[r],"frozenProbes":probes}));
            if coupled {
                let regions = layout.lakes()[i].regions();
                let annual_capture = total_mass(
                    &regions
                        .iter()
                        .flat_map(|&i| {
                            [
                                checkpoint
                                    .cumulative_lake_capture_kilograms
                                    .as_ref()
                                    .unwrap()[i]
                                    - previous_capture.as_ref().unwrap()[i],
                                checkpoint
                                    .cumulative_lake_capture_low_kilograms
                                    .as_ref()
                                    .unwrap()[i]
                                    - previous_capture_low.as_ref().unwrap()[i],
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
                let annual_evaporation = total_mass(
                    &regions
                        .iter()
                        .map(|&i| {
                            checkpoint.cumulative_runoff_transfers[i].terminal_evaporation
                                - previous_runoff[i].terminal_evaporation
                        })
                        .collect::<Vec<_>>(),
                );
                let soil = total_mass(
                    &regions
                        .iter()
                        .flat_map(|&i| {
                            [
                                checkpoint.soil_kilograms[i],
                                checkpoint.soil_low_kilograms.as_ref().unwrap()[i],
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
                let record = lakes.last_mut().unwrap();
                record["annualActualLakeCaptureKilograms"] = json!(annual_capture);
                record["annualActualLakeEvaporationKilograms"] = json!(annual_evaporation);
                record["leafSoilWaterKilograms"] = json!(soil);
                record["leafRegions"] = json!(regions);
            }
        }
        let budget = model.budget(&state)?;
        let p = budget.cumulative_precipitation_kilograms;
        let e = budget.cumulative_evaporation_kilograms;
        annual.push(json!({"year":year,"budget":budget,"precipitationMillimeters":(p-last_p)/area,"evaporationMillimeters":(e-last_e)/area,"lakes":lakes,"observationLeavesCheckpointExact":before==serde_json::to_string(&state.checkpoint()).map_err(|e|e.to_string())?}));
        last_p = p;
        last_e = e;
        previous_runoff = checkpoint.cumulative_runoff_transfers;
        previous_capture = checkpoint.cumulative_lake_capture_kilograms;
        previous_capture_low = checkpoint.cumulative_lake_capture_low_kilograms;
    }
    let text = serde_json::to_string(&state.checkpoint()).map_err(|e| e.to_string())?;
    let (restored_model, mut restored) =
        Model::restore(serde_json::from_str(&text).map_err(|e| e.to_string())?)?;
    let exact = restored == state;
    let continuation = if !(coupled && failure.is_some())
        && state.elapsed_seconds() + 3600
            <= planimulation_core::seasonal_moisture::MAX_ELAPSED_SECONDS
    {
        let mut a = state.clone();
        model.advance(&mut a, 3600)?;
        restored_model.advance(&mut restored, 3600)?;
        Some(a == restored)
    } else {
        None
    };
    let mut report = json!({"reportVersion":1,"componentVersion":closed_lake::COMPONENT_VERSION,"scope":"One unchanged finite model-8 trajectory. Derived exclusive minimum-leaf surfaces and twelve independent 300-second endpoint evaporation operators are not applied to physical state. Local monthly thermal fields remain fixed reference-land ones. Above-first-connection and arithmetic refusals are retained per lake/probe, not clipped. No coupled lake evaporation, precipitation interception, dynamic mask, spill, merge, climate readiness or desktop promotion.","recipe":world.recipe,"settings":settings,"modelVersion":model.model_version(),"temperatureSettings":model.temperature_settings(),"windSettings":model.wind_settings(),"actualMaximumCoupledStepSeconds":model.maximum_coupled_step_seconds()?,"callerIntervalSeconds":86400,"requestedYears":years,"elapsedSeconds":state.elapsed_seconds(),"annual":annual,"maximumRelativeMassResidual":maximum_mass,"maximumRelativeLocalLedgerResidual":maximum_local,"checkpointRoundTripExact":exact,"checkpointContinuationExact":continuation,"failure":failure,"failedIntervalAtomic":atomic,"physicalRunPassed":failure.is_none()&&exact&&continuation!=Some(false)});
    if coupled {
        report["reportVersion"] = json!(2);
        report["scope"] = json!(
            "Bounded coupled model-9 leaf lakes: pre-half-stage exposure, intercepted liquid/rain/melt, inactive separately owned submerged soil, delayed runoff, and finite regional lake evaporation. Fixed bed/thermal normals. First-connection crossings reject atomically; no spill/merge/backpressure, calibration, preparedness or desktop promotion. No endpoint probes are evaluated in this coupled report."
        );
        report["lakeExchangeModelVersion"] = json!(state.checkpoint().closed_lake_model_version);
        report["continuationNotAttemptedAfterRefusal"] = json!(failure.is_some());
        report["finalLakeCaptureKilograms"] = json!(state.cumulative_lake_capture_kilograms());
        report["finalLakeCaptureLowKilograms"] =
            json!(state.cumulative_lake_capture_low_kilograms());
    }
    Ok(report)
}

fn main() -> Result<(), String> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let output = if let Some(i) = args.iter().position(|a| a == "--output") {
        if i + 1 >= args.len() || args[i + 1].starts_with("--") {
            return Err("--output requires a new file path.".into());
        }
        let path = args.remove(i + 1);
        args.remove(i);
        Some(path)
    } else {
        None
    };
    if args
        .iter()
        .any(|a| a != "--decade" && a != "--refined" && a != "--coupled")
        || args.iter().filter(|a| *a == "--decade").count() > 1
        || args.iter().filter(|a| *a == "--refined").count() > 1
        || args.iter().filter(|a| *a == "--coupled").count() > 1
    {
        return Err(
            "Usage: closed_lake_report [--decade] [--refined] [--coupled] [--output NEW_FILE]"
                .into(),
        );
    }
    let file = output
        .map(|p| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(p)
                .map_err(|e| e.to_string())
        })
        .transpose()?;
    let years = if args.iter().any(|a| a == "--decade") {
        10
    } else {
        1
    };
    let limit = if args.iter().any(|a| a == "--refined") {
        450
    } else {
        900
    };
    eprintln!("Closed-lake controls: years {years}, coupling ceiling {limit} seconds");
    let r = run(limit, years, args.iter().any(|a| a == "--coupled"))?;
    if let Some(mut f) = file {
        serde_json::to_writer(&mut f, &r).map_err(|e| e.to_string())?;
        writeln!(f).map_err(|e| e.to_string())?;
        eprintln!("Saved complete closed-lake report.");
    } else {
        println!("{}", serde_json::to_string(&r).map_err(|e| e.to_string())?);
    }
    Ok(())
}
