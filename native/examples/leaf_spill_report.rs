//! Reproducible native model-10 runs, with ordinary and explicitly funded inputs.
use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics,
        SurfaceNumerics, TerminalNumerics, closed_lake::Layout,
    },
};
use serde_json::{Value, json};
use std::io::Write;

fn settings(limit: u32) -> Settings {
    Settings {
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(ClosedLakeExchange::FrozenLeafExposureWithSpill),
        max_coupled_step_seconds: limit,
        ..Default::default()
    }
}
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    (s, (a - (s - v)) + (b - v))
}
fn funded(model: &Model, world: &World, inputs: &[(usize, f64)]) -> Result<Checkpoint, String> {
    let mut cp = model.initial_state().checkpoint();
    cp.elapsed_seconds = 1;
    let contact = world
        .water
        .body_ids
        .iter()
        .position(|&id| id > 0)
        .ok_or("No reference donor.")?;
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let b = ids
        .iter()
        .position(|&id| id == world.water.body_ids[contact])
        .unwrap();
    let supply = total_mass(&inputs.iter().map(|&(_, m)| m).collect::<Vec<_>>());
    let high = cp.reference_body_high_kilograms.as_mut().unwrap();
    let low = cp.reference_body_low_kilograms.as_mut().unwrap();
    let (s, e) = two_sum(high[b], -supply);
    (high[b], low[b]) = two_sum(s, low[b] + e);
    if high[b] <= 0. {
        return Err("Directed input exceeds the finite reference donor.".into());
    }
    cp.cumulative_surface_transfers[contact].liquid_evaporation = supply;
    cp.cumulative_evaporation_kilograms[contact] = supply;
    for &(r, m) in inputs {
        cp.cumulative_surface_transfers[r].rain = m;
        cp.cumulative_precipitation_kilograms[r] = m;
        cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] = m;
        cp.leaf_spill_state
            .as_mut()
            .unwrap()
            .pending_input
            .high_kilograms[r] = m;
    }
    Ok(cp)
}
fn replay(
    model: &Model,
    state: &planimulation_core::seasonal_moisture::State,
) -> Result<Value, String> {
    let text = serde_json::to_string(&state.checkpoint()).map_err(|e| e.to_string())?;
    let (m, mut resumed) = Model::restore(serde_json::from_str(&text).map_err(|e| e.to_string())?)?;
    let round_trip = resumed == *state;
    let mut continued = state.clone();
    model.advance(&mut continued, 3600)?;
    m.advance(&mut resumed, 3600)?;
    Ok(
        json!({"fullCheckpointRoundTripExact":round_trip,"nextHourContinuationExact":continued==resumed}),
    )
}
fn run(limit: u32) -> Result<Value, String> {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    recipe.subdivision = 2;
    let world = World::generate(recipe)?;
    let layout = Layout::from_world(&world)?;
    let model = Model::from_world(
        &world,
        settings(limit),
        Default::default(),
        Default::default(),
    )?;
    let mut state = model.initial_state();
    let mut maximum_mass: f64 = 0.;
    let mut maximum_local: f64 = 0.;
    let mut events = Vec::new();
    let mut failure = None;
    let mut atomic = None;
    for _ in 0..40 {
        let before = state.clone();
        match model.advance(&mut state, 86400) {
            Ok(step) => {
                maximum_mass = maximum_mass.max(
                    step.budget.residual_kilograms.abs()
                        / step.budget.initial_mobile_water_kilograms.max(1.),
                );
                maximum_local =
                    maximum_local.max(step.budget.maximum_relative_local_surface_ledger_residual);
                events.extend(step.leaf_spill_events.unwrap());
            }
            Err(e) => {
                atomic = Some(state == before);
                failure = Some(e);
                break;
            }
        }
    }
    let replay = if failure.is_none() {
        Some(replay(&model, &state)?)
    } else {
        None
    };
    let ordinary = json!({"requestedDays":40,"elapsedSeconds":state.elapsed_seconds(),"budget":model.budget(&state)?,"maximumRelativeMassResidual":maximum_mass,"maximumRelativeLocalLedgerResidual":maximum_local,"spillEvents":events,"lakeSurfaces":layout.capture(&model,&state)?,"replay":replay,"failure":failure,"failedIntervalAtomic":atomic});

    let quiet = Model::from_world(
        &world,
        Settings {
            initial_active_surface_depth_meters: 10.,
            evaporation_enabled: false,
            precipitation_enabled: false,
            routing_enabled: false,
            ..settings(limit)
        },
        Default::default(),
        Default::default(),
    )?;
    let capacity = |r| -> Result<f64, String> {
        layout
            .lakes()
            .iter()
            .find(|l| l.terminal_region() == r)
            .and_then(|l| l.capacity_cubic_meters())
            .map(|c| c * 1000.)
            .ok_or("Reference fixture changed its leaf capacities.".into())
    };
    let (cap91, cap73) = (capacity(91)?, capacity(73)?);
    let mut directed = Vec::new();
    for (name, input) in [
        ("firstRecipientStillUnfilled", cap91 + cap73 * 0.25),
        ("bothLeavesFullThenReferenceReturn", cap91 + cap73 * 1.125),
    ] {
        let cp = funded(&quiet, &world, &[(91, input)])?;
        let (m, mut s) = Model::restore(cp)?;
        let before = m.budget(&s)?;
        let step = m.advance(&mut s, 1)?;
        directed.push(json!({"name":name,"inputTerminal":91,"inputKilograms":input,"initialBudgetIncludingOwnedQueue":before,"finalBudget":step.budget,"spillEvents":step.leaf_spill_events,"lakeSurfaces":layout.capture(&m,&s)?,"finalSpillCheckpoint":s.checkpoint().leaf_spill_state,"replay":self::replay(&m,&s)?}));
    }
    let cp = funded(&quiet, &world, &[(91, cap91 * 1.1), (73, cap73 * 1.1)])?;
    let (m, mut s) = Model::restore(cp)?;
    let before = s.clone();
    let error = m.advance(&mut s, 3600).err();
    let rollback = json!({"name":"concurrentOverflowRefusal","error":error,"completeCheckpointUnchanged":s==before});
    Ok(
        json!({"reportVersion":1,"modelVersion":model.model_version(),"spillAccountingVersion":planimulation_core::seasonal_moisture::leaf_spill::MODEL_VERSION,"scope":"Opt-in headless bounded fast fill/spill on fixed generated minimum leaves. Ordinary climate and separately ledger-funded real-recipe controls; directed histories are not naturally generated climate. Full lower-sill receivers pass remaining input, same-sill merging and concurrent overflow sources remain unsupported. No hydraulic discharge law, ocean level evolution, general merge, stationarity or desktop promotion.","recipe":world.recipe,"ordinarySettings":model.settings(),"directedSettings":quiet.settings(),"temperatureSettings":model.temperature_settings(),"windSettings":model.wind_settings(),"actualMaximumCoupledStepSeconds":model.maximum_coupled_step_seconds()?,"firstSpillCertificates":planimulation_core::seasonal_moisture::closed_lake::spill::first_connections(&world)?,"ordinary":ordinary,"directed":directed,"retainedRefusal":rollback}),
    )
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
    if args.iter().any(|a| a != "--refined") || args.len() > 1 {
        return Err("Usage: leaf_spill_report [--refined] [--output NEW_FILE]".into());
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
    let report = run(if args.is_empty() { 900 } else { 450 })?;
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    if let Some(mut f) = file {
        f.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    } else {
        println!("{text}");
    }
    if !report["ordinary"]["failure"].is_null()
        || report["ordinary"]["replay"]["fullCheckpointRoundTripExact"] != true
        || report["ordinary"]["replay"]["nextHourContinuationExact"] != true
        || report["directed"].as_array().unwrap().iter().any(|d| {
            d["replay"]["fullCheckpointRoundTripExact"] != true
                || d["replay"]["nextHourContinuationExact"] != true
        })
        || report["retainedRefusal"]["error"].is_null()
        || report["retainedRefusal"]["completeCheckpointUnchanged"] != true
    {
        return Err("Leaf spill report retained a failed run or failed refusal control.".into());
    }
    Ok(())
}
