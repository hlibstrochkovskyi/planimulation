//! Retained matched controls and long-run qualification, including refusals.
use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        MAX_ELAPSED_SECONDS, Model, Settings, SoilNumerics, State, SurfaceNumerics,
        TerminalNumerics,
    },
};
use serde_json::{Value, json};

struct Run {
    state: State,
    summary: Value,
    passed: bool,
}
fn run(world: &World, days: u32, settings: Settings) -> Result<Run, String> {
    let model = Model::from_world(world, settings, Default::default(), Default::default())?;
    let mut state = model.initial_state();
    let mut global: f64 = 0.;
    let mut local: f64 = 0.;
    let mut failure = None;
    let mut atomic = None;
    let caller = 3600;
    for _ in 0..days * 24 {
        let before = state.clone();
        match model.advance(&mut state, caller) {
            Ok(step) => {
                global = global.max(
                    step.budget.residual_kilograms.abs()
                        / step.budget.initial_mobile_water_kilograms.max(1.),
                );
                local = local.max(step.budget.maximum_relative_local_surface_ledger_residual);
            }
            Err(error) => {
                atomic = Some(before == state);
                failure = Some(error);
                break;
            }
        }
    }
    let text = serde_json::to_string(&state.checkpoint()).map_err(|e| e.to_string())?;
    let (restored_model, mut restored) =
        Model::restore(serde_json::from_str(&text).map_err(|e| e.to_string())?)?;
    let roundtrip = restored == state
        && serde_json::to_string(&restored.checkpoint()).map_err(|e| e.to_string())? == text;
    let mut continued = state.clone();
    let continuation = if state.elapsed_seconds() + u64::from(caller) <= MAX_ELAPSED_SECONDS {
        let a = model.advance(&mut continued, caller);
        let b = restored_model.advance(&mut restored, caller);
        Some(
            a.as_ref().map(|_| ()).map_err(String::as_str)
                == b.as_ref().map(|_| ()).map_err(String::as_str)
                && continued == restored,
        )
    } else {
        None
    };
    let budget = model.budget(&state)?;
    let soil = state.soil_low_kilograms().unwrap_or(&[]);
    let snow = state.snow_low_kilograms().unwrap_or(&[]);
    let liquid = state.surface_low_kilograms().unwrap_or(&[]);
    let terminal = state.terminal_low_kilograms().unwrap_or(&[]);
    let passed = failure.is_none() && roundtrip && continuation != Some(false);
    let summary = json!({"requestedDays":days,"callerIntervalSeconds":caller,"elapsedSeconds":state.elapsed_seconds(),
        "settings":settings,"temperatureSettings":state.checkpoint().temperature_settings,"windSettings":state.checkpoint().wind_settings,
        "modelVersion":model.model_version(),"surfaceModelVersion":model.surface_model_version(),"schemaVersion":model.checkpoint_schema_version(),"terminalStockModelVersion":state.checkpoint().terminal_stock_model_version,
        "actualMaximumCoupledStepSeconds":model.maximum_coupled_step_seconds()?,"budget":budget,
        "maximumRelativeMassResidual":global,"maximumRelativeLocalLedgerResidual":local,
        "nonzeroLowComponents":{"soil":soil.iter().filter(|&&v|v!=0.).count(),"snow":snow.iter().filter(|&&v|v!=0.).count(),"liquid":liquid.iter().filter(|&&v|v!=0.).count(),"terminal":terminal.iter().filter(|&&v|v!=0.).count()},
        "maximumAbsoluteLowComponentKilograms":soil.iter().chain(snow).chain(liquid).chain(terminal).map(|v|v.abs()).fold(0.,f64::max),
        "checkpointBytes":text.len(),"checkpointRoundTripExact":roundtrip,"checkpointContinuationExact":continuation,
        "continuationScope":"One more hour from the retained last accepted state; at the ten-year limit no further interval is permitted. A repeated failure is compared exactly, not counted as successful advance.",
        "failure":failure,"failedIntervalAtomic":atomic,"passed":passed});
    Ok(Run {
        state,
        summary,
        passed,
    })
}
fn settings(surface: bool, limit: u32) -> Settings {
    Settings {
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: surface.then_some(SurfaceNumerics::Compensated),
        terminal_numerics: surface.then_some(TerminalNumerics::Compensated),
        max_coupled_step_seconds: limit,
        ..Default::default()
    }
}
fn difference(a: &State, b: &State, scale: f64) -> f64 {
    let high = a
        .owned_stocks()
        .zip(b.owned_stocks())
        .map(|(a, b)| (a - b).abs())
        .sum::<f64>();
    let mut low = 0.;
    for (a, b) in [
        (a.soil_low_kilograms(), b.soil_low_kilograms()),
        (a.snow_low_kilograms(), b.snow_low_kilograms()),
        (a.surface_low_kilograms(), b.surface_low_kilograms()),
        (a.terminal_low_kilograms(), b.terminal_low_kilograms()),
    ] {
        low += match (a, b) {
            (Some(a), Some(b)) => a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum(),
            (Some(a), None) | (None, Some(a)) => a.iter().map(|v| v.abs()).sum(),
            (None, None) => 0.,
        };
    }
    (high + low) / scale.max(1.)
}
fn compare(world: &World) -> Result<Value, String> {
    let legacy = run(world, 40, settings(false, 3600))?;
    let short = run(world, 40, settings(true, 3600))?;
    let coarse = run(world, 365, settings(true, 3600))?;
    let fine = run(world, 365, settings(true, 60))?;
    let scale = Model::from_world(
        world,
        settings(true, 60),
        Default::default(),
        Default::default(),
    )?
    .budget(&fine.state)?
    .initial_mobile_water_kilograms;
    Ok(
        json!({"passed":legacy.passed && short.passed && coarse.passed && fine.passed,
        "legacyFortyDays":legacy.summary,"candidateFortyDays":short.summary,"candidateAnnual":coarse.summary,"refinedAnnual":fine.summary,
        "relativeComponentL1BoundMatchedLegacy":(legacy.passed && short.passed).then(||difference(&legacy.state,&short.state,scale)),
        "relativeComponentL1BoundAnnualTimeRefinement":(coarse.passed && fine.passed).then(||difference(&coarse.state,&fine.state,scale))}),
    )
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (long_refined, surface_only) = match args.as_slice() {
        [] => (false, false),
        [arg] if arg == "--long-refined" => (true, false),
        [arg, old] if arg == "--long-refined" && old == "--surface-only" => (true, true),
        _ => {
            return Err("Usage: surface_precision_report [--long-refined [--surface-only]]".into());
        }
    };
    let base: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    let mut cases = Vec::new();
    for seed in if long_refined {
        Vec::new()
    } else {
        vec!["seasonal-reference", "moisture-coast", "moisture-interior"]
    } {
        for radius in [1_000_000., 6_371_000.] {
            let mut recipe = base.clone();
            recipe.subdivision = 2;
            recipe.seed = seed.into();
            recipe.radius_meters = radius;
            eprintln!("Annual matched/refined case: {seed}, radius {radius} m");
            let result = World::generate(recipe.clone()).and_then(|w| compare(&w));
            cases.push(match result {
                Ok(report) => json!({"recipe":recipe,"stage":"matched 40-day controls and annual refinement","report":report}),
                Err(error) => json!({"recipe":recipe,"stage":"matched 40-day controls and annual refinement","failure":error}),
            });
        }
    }
    let mut extended = Vec::new();
    let extended_inputs = if long_refined {
        vec![("seasonal-reference", 2, 1_000_000., 3650, 60)]
    } else {
        vec![
            ("seasonal-reference", 3, 1_000_000., 365, 900),
            ("moisture-coast", 3, 6_371_000., 365, 900),
            ("moisture-interior", 4, 6_371_000., 365, 3600),
            ("seasonal-reference", 2, 6_371_000., 3650, 3600),
        ]
    };
    for (seed, level, radius, days, limit) in extended_inputs {
        let mut recipe = base.clone();
        recipe.seed = seed.into();
        recipe.subdivision = level;
        recipe.radius_meters = radius;
        eprintln!("Extended case: {seed}, level {level}, {days} days, limit {limit} s");
        let mut resolved = settings(true, limit);
        if surface_only {
            resolved.terminal_numerics = None;
        }
        let result = World::generate(recipe.clone()).and_then(|w| run(&w, days, resolved));
        extended.push(match result {
            Ok(run) => json!({"recipe":recipe,"stage":"extended scope","report":run.summary}),
            Err(error) => json!({"recipe":recipe,"stage":"extended scope","failure":error}),
        });
    }
    let failures = cases
        .iter()
        .chain(&extended)
        .filter(|c| c.get("failure").is_some() || c["report"]["passed"] != true)
        .count();
    println!("{}",serde_json::to_string_pretty(&json!({"reportVersion":1,"suite":if long_refined {"ten-year refined"} else {"matched annual and extended"},"qualificationPassed":failures==0,"failureCount":failures,"cases":cases,"extendedCases":extended,
        "surfaceOnlyControl":surface_only,"scope":"Compensated liquid/snow/soil and optional terminal representation; transit and vapor remain ordinary binary64. Resolved settings/pins distinguish versions 5/6/7. No tolerance widening, stock repair, physical calibration, spatial convergence claim, or default promotion."})).map_err(|e|e.to_string())?);
    if failures > 0 {
        return Err(
            "Surface precision qualification contains retained failures; inspect the JSON report."
                .into(),
        );
    }
    Ok(())
}
