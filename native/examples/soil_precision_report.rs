//! Matched leading/low stock controls; retain every rejected case and its stage.
use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{Model, Settings, SoilNumerics, State},
    seasonal_temperature, seasonal_wind,
};
use serde_json::{Value, json};

fn run(world: &World, days: u32, compensated: bool, limit: u32) -> Result<(State, Value), String> {
    let model = Model::from_world(
        world,
        Settings {
            orography: Some(Default::default()),
            max_coupled_step_seconds: limit,
            soil_numerics: compensated.then_some(SoilNumerics::Compensated),
            ..Default::default()
        },
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )?;
    let mut state = model.initial_state();
    let mut max_global: f64 = 0.;
    let mut max_local: f64 = 0.;
    for _ in 0..days * 24 {
        let step = model.advance(&mut state, 3600)?;
        max_global = max_global.max(
            step.budget.residual_kilograms.abs()
                / step.budget.initial_mobile_water_kilograms.max(1.),
        );
        max_local = max_local.max(step.budget.maximum_relative_local_surface_ledger_residual);
    }
    let saved = state.clone();
    let json = serde_json::to_string(&state.checkpoint()).map_err(|e| e.to_string())?;
    let (restored_model, mut restored) =
        Model::restore(serde_json::from_str(&json).map_err(|e| e.to_string())?)?;
    let roundtrip = restored == saved;
    let mut continued = saved.clone();
    model.advance(&mut continued, 3600)?;
    restored_model.advance(&mut restored, 3600)?;
    let low = state.soil_low_kilograms().unwrap_or(&[]);
    let summary = json!({"days":days,"callerIntervalSeconds":3600,"settings":model.settings(),"modelVersion":model.model_version(),
        "surfaceModelVersion":model.surface_model_version(),"schemaVersion":model.checkpoint_schema_version(),
        "actualMaximumCoupledStepSeconds":model.maximum_coupled_step_seconds()?,"budget":model.budget(&state)?,
        "maximumRelativeMassResidual":max_global,"maximumRelativeLocalLedgerResidual":max_local,
        "maximumAbsoluteSoilLowComponentKilograms":low.iter().map(|v|v.abs()).fold(0.,f64::max),
        "nonzeroSoilLowComponents":low.iter().filter(|&&v|v!=0.).count(),
        "checkpointRoundTripExact":roundtrip,"checkpointContinuationExact":restored==continued});
    Ok((state, summary))
}
fn difference(a: &State, b: &State, scale: f64) -> f64 {
    let leading = a
        .owned_stocks()
        .zip(b.owned_stocks())
        .map(|(a, b)| (a - b).abs())
        .sum::<f64>();
    let low = match (a.soil_low_kilograms(), b.soil_low_kilograms()) {
        (Some(a), Some(b)) => a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum(),
        (Some(a), None) | (None, Some(a)) => a.iter().map(|v| v.abs()).sum(),
        (None, None) => 0.,
    };
    // A componentwise L1 upper bound, not rounding low components away in a+b.
    (leading + low) / scale
}
fn short(world: &World) -> Result<Value, String> {
    let (legacy, baseline) = run(world, 40, false, 3600)?;
    let (coarse, candidate) = run(world, 40, true, 3600)?;
    let (fine, refined) = run(world, 40, true, 60)?;
    let scale = baseline["budget"]["initialMobileWaterKilograms"]
        .as_f64()
        .unwrap()
        .max(1.);
    Ok(
        json!({"baselineVersion4":baseline,"candidate":candidate,"refined":refined,
        "relativeOwnedComponentL1BoundCandidateVersusLegacy":difference(&coarse,&legacy,scale),
        "relativeOwnedComponentL1BoundTimeRefinement":difference(&coarse,&fine,scale)}),
    )
}
fn main() -> Result<(), String> {
    let base: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    let mut cases = Vec::new();
    for seed in ["seasonal-reference", "moisture-coast", "moisture-interior"] {
        for radius in [1_000_000., 6_371_000.] {
            let mut recipe = base.clone();
            recipe.subdivision = 2;
            recipe.seed = seed.into();
            recipe.radius_meters = radius;
            let result = World::generate(recipe.clone()).and_then(|w| short(&w));
            cases.push(match result {
                Ok(report) => {
                    json!({"recipe":recipe,"stage":"40-day matched comparison","report":report})
                }
                Err(failure) => {
                    json!({"recipe":recipe,"stage":"40-day matched comparison","failure":failure})
                }
            });
        }
    }
    let mut extended = Vec::new();
    for seed in ["seasonal-reference", "moisture-coast"] {
        let mut recipe = base.clone();
        recipe.subdivision = 2;
        recipe.radius_meters = 1_000_000.;
        recipe.seed = seed.into();
        let result = World::generate(recipe.clone()).and_then(|w| run(&w, 365, true, 60));
        extended.push(match result {
            Ok((_, report)) => {
                json!({"recipe":recipe,"stage":"365-day 60-second candidate","report":report})
            }
            Err(failure) => {
                json!({"recipe":recipe,"stage":"365-day 60-second candidate","failure":failure})
            }
        });
    }
    let failures = cases
        .iter()
        .chain(&extended)
        .filter(|case| case.get("failure").is_some())
        .count();
    println!("{}",serde_json::to_string_pretty(&json!({"reportVersion":1,"qualificationPassed":failures==0,"failureCount":failures,"cases":cases,"extendedCases":extended,
        "scope":"Compensated soil candidate only; other stocks remain binary64. No tolerance widening, final stock repair, desktop promotion, or unrestricted stability claim."})).map_err(|e|e.to_string())?);
    if failures > 0 {
        return Err(
            "Candidate qualification contains retained rejected cases; inspect the JSON report."
                .into(),
        );
    }
    Ok(())
}
