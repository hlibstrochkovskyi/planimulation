//! Bounded accepted trajectories and cold-storage diagnosis; never a clock reset.
use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Model, Settings, SoilNumerics, SurfaceNumerics, TerminalNumerics,
        preparation::{Criteria, DIAGNOSTIC_VERSION, Monitor},
        water_return::Audit,
    },
};
use serde_json::{Value, json};

fn run(
    recipe: Recipe,
    depth: f64,
    years: u32,
    limit: u32,
    return_audit: bool,
) -> Result<Value, String> {
    let world = World::generate(recipe.clone())?;
    let settings = Settings {
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        max_coupled_step_seconds: limit,
        initial_active_surface_depth_meters: depth,
        ..Default::default()
    };
    let model = Model::from_world(&world, settings, Default::default(), Default::default())?;
    let mut state = model.initial_state();
    let criteria = Criteria::default();
    let mut monitor = Monitor::new(&model, &state, criteria)?;
    let mut annual = Vec::new();
    let mut return_ownership = Vec::new();
    let mut failure = None;
    let mut atomic = None;
    let mut max_global: f64 = 0.;
    let mut max_local: f64 = 0.;
    'years: for _ in 0..years {
        for _ in 0..365 {
            let before = state.clone();
            match model.advance(&mut state, 86400) {
                Ok(step) => {
                    max_global = max_global.max(
                        step.budget.residual_kilograms.abs()
                            / step.budget.initial_mobile_water_kilograms.max(1.),
                    );
                    max_local =
                        max_local.max(step.budget.maximum_relative_local_surface_ledger_residual);
                }
                Err(error) => {
                    atomic = Some(state == before);
                    failure = Some(error);
                    break 'years;
                }
            }
        }
        let before = state.clone();
        annual.push(monitor.observe_year(&state)?);
        if return_audit {
            return_ownership.push(Audit::capture(&model, &state, 300)?);
        }
        if before != state {
            return Err("Preparation observation modified native state.".into());
        }
    }
    let cp = state.checkpoint();
    let text = serde_json::to_string(&cp).map_err(|e| e.to_string())?;
    let (resumed_model, mut resumed) =
        Model::restore(serde_json::from_str(&text).map_err(|e| e.to_string())?)?;
    let roundtrip = state == resumed
        && text == serde_json::to_string(&resumed.checkpoint()).map_err(|e| e.to_string())?;
    let continuation = if state.elapsed_seconds() + 3600
        <= planimulation_core::seasonal_moisture::MAX_ELAPSED_SECONDS
    {
        let mut a = state.clone();
        let x = model.advance(&mut a, 3600);
        let y = resumed_model.advance(&mut resumed, 3600);
        Some(
            x.as_ref().map(|_| ()).map_err(String::as_str)
                == y.as_ref().map(|_| ()).map_err(String::as_str)
                && a == resumed,
        )
    } else {
        None
    };
    let passed = failure.is_none() && roundtrip && continuation != Some(false);
    let candidate = annual.last().is_some_and(|a| a.stationarity_candidate);
    let mut report = json!({"recipe":recipe,"modelVersion":model.model_version(),"schemaVersion":model.checkpoint_schema_version(),
        "surfaceModelVersion":model.surface_model_version(),"terminalStockModelVersion":cp.terminal_stock_model_version,
        "transportModelVersion":cp.transport_model_version,"temperatureModelVersion":cp.temperature_model_version,
        "windModelVersion":cp.wind_model_version,"runoffModelVersion":cp.runoff_model_version,"orographicModelVersion":cp.orographic_model_version,
        "settings":settings,"temperatureSettings":model.temperature_settings(),"windSettings":model.wind_settings(),
        "criteria":criteria,"requestedYears":years,"callerIntervalSeconds":86400,"actualMaximumCoupledStepSeconds":model.maximum_coupled_step_seconds()?,
        "elapsedSeconds":state.elapsed_seconds(),"lastObservedYearBoundarySeconds":monitor.last_observed_seconds(),
        "thermalRegimes":monitor.thermal_regimes(),"annualAssessments":annual,"budget":model.budget(&state)?,
        "passed":passed,"maximumRelativeMassResidual":max_global,"maximumRelativeLocalLedgerResidual":max_local,
        "checkpointRoundTripExact":roundtrip,"checkpointContinuationExact":continuation,
        "failure":failure,"failedIntervalAtomic":atomic,
        "assessmentStatus":if candidate {"stationarityCandidateRequiresPhysicalReview"} else {"noStationarityCandidateWithinRecordedLimit"},
        "preparationOrClockResetPerformed":false});
    if return_audit {
        report["returnOwnershipAudits"] = json!(return_ownership);
    }
    Ok(report)
}
fn main() -> Result<(), String> {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let return_audit = args.iter().any(|arg| arg == "--return-audit");
    if return_audit {
        let index = args.iter().position(|arg| arg == "--return-audit").unwrap();
        args.remove(index);
    }
    let (smoke, refined) = match args.as_slice() {
        [] => (false, false),
        [arg] if arg == "--smoke" => (true, false),
        [arg] if arg == "--refined" => (false, true),
        _ => {
            return Err(
                "Usage: seasonal_preparation_report [--smoke | --refined] [--return-audit]".into(),
            );
        }
    };
    let base: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    let mut cases = Vec::new();
    let mut inputs = Vec::new();
    for seed in if smoke || refined {
        vec!["seasonal-reference"]
    } else {
        vec!["seasonal-reference", "moisture-coast", "moisture-interior"]
    } {
        for radius in if smoke || refined {
            vec![1_000_000.]
        } else {
            vec![1_000_000., 6_371_000.]
        } {
            inputs.push((seed, radius, 1.));
            if seed == "seasonal-reference" {
                inputs.push((seed, radius, 10.));
            }
        }
    }
    for (seed, radius, depth) in inputs {
        let mut recipe = base.clone();
        recipe.seed = seed.into();
        recipe.subdivision = if smoke { 1 } else { 2 };
        recipe.radius_meters = radius;
        let years = if smoke { 2 } else { 10 };
        let limit = if refined { 450 } else { 900 };
        eprintln!(
            "Preparation diagnosis: {seed}, radius {radius}, active depth {depth}, {years} years, ceiling {limit} s"
        );
        cases.push(match run(recipe.clone(),depth,years,limit,return_audit) {
            Ok(report)=>report,
            Err(error)=>json!({"recipe":recipe,"initialActiveSurfaceDepthMeters":depth,"requestedYears":years,"stage":"generation or observation validation","failure":error,"passed":false}),
        });
    }
    let failures = cases.iter().filter(|c| c["passed"] != true).count();
    let mut report = json!({"reportVersion":1,"diagnosticVersion":DIAGNOSTIC_VERSION,
        "numericRunQualificationPassed":failures==0,"failureCount":failures,"cases":cases,
        "stockColumns":["liquid","snow","soil","transit","terminal","vapor"],
        "flowColumns":["precipitation","evaporation","generatedRunoff","snowfall","terminalDelivery"],
        "scope":"Read-only whole-year comparisons under explicit uncalibrated criteria; physical stocks include signed low components. Annual flows difference native cumulative binary64 totals, not cumulative correction mass. Cold-locked inventory is a structural lower-bound set under operational monthly temperature and existing laws, not all inaccessible water or realistic glacier physics. A stationarity candidate is not climate readiness and can be quiet/dry. No default, law, tolerance, source repair, clock reset, checkpoint migration, or prepared-state export."});
    if return_audit {
        report["reportVersion"] = json!(2);
        report["waterReturnDiagnosticVersion"] =
            json!(planimulation_core::seasonal_moisture::water_return::DIAGNOSTIC_VERSION);
        report["waterReturnTransferColumns"] = json!([
            "terminalDelivery",
            "terminalEvaporation",
            "liquidEvaporation",
            "soilEvaporation"
        ]);
        report["waterReturnProbeScope"] = json!(
            "Twelve independent frozen-state 300-second liquid-only probes at each year boundary. Hold vapor fixed, change only the specified monthly temperature/capacity; omit precipitation, snowmelt, soil, transport, and feedback. Local grants use signed-low donor floors. Perfect-sharing caps use only liquid plus terminal mass within each existing connected reference body, including its cold liquid, and are approximate binary64 bounds, not conservative allocated grants or realized rainfall changes. No stock is moved; dry terminals are never pooled into a reference body. Terminal-bearing area is exact positive-stock support, not a modeled lake surface or useful evaporation footprint."
        );
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    if failures > 0 {
        return Err("Preparation diagnostic retained numerical failures; inspect JSON.".into());
    }
    Ok(())
}
