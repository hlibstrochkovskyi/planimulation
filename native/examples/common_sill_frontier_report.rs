//! Bounded reversible one-level model-12 evidence. Directed funding is not natural history.
use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics, State,
        SurfaceNumerics, TerminalNumerics,
    },
};
use serde_json::{Value, json};
use std::io::Write;

fn recipe(coverage: f64, subdivision: u32) -> Recipe {
    let mut r: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    r.subdivision = subdivision;
    r.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
    r
}
fn settings(limit: u32, quiet: bool) -> Settings {
    Settings {
        initial_active_surface_depth_meters: 10.,
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(ClosedLakeExchange::FrozenCommonSillFrontier),
        max_coupled_step_seconds: limit,
        evaporation_enabled: !quiet,
        precipitation_enabled: !quiet,
        routing_enabled: !quiet,
        ..Default::default()
    }
}
fn replay(model: &Model, state: &State) -> Result<Value, String> {
    let cp = state.checkpoint();
    let (restored, mut resumed) =
        Model::restore(serde_json::from_str(&serde_json::to_string(&cp).unwrap()).unwrap())?;
    let round_trip = resumed == *state;
    let mut continued = state.clone();
    model.advance(&mut continued, 3600)?;
    restored.advance(&mut resumed, 3600)?;
    Ok(
        json!({"fullCheckpointRoundTripExact":round_trip,"nextHourContinuationExact":continued==resumed}),
    )
}
fn ordinary(coverage: f64, limit: u32, subdivision: u32) -> Result<Value, String> {
    let world = World::generate(recipe(coverage, subdivision))?;
    let model = Model::from_world(
        &world,
        settings(limit, false),
        Default::default(),
        Default::default(),
    )?;
    let mut state = model.initial_state();
    let mut accepted_days = 0;
    let mut max_global: f64 = 0.;
    let mut max_local: f64 = 0.;
    let mut failure = None;
    let mut spill_events = 0;
    for _ in 0..40 {
        let before = state.clone();
        match model.advance(&mut state, 86400) {
            Ok(step) => {
                max_global = max_global.max(
                    step.budget.residual_kilograms.abs()
                        / step.budget.initial_mobile_water_kilograms.max(1.),
                );
                max_local =
                    max_local.max(step.budget.maximum_relative_local_surface_ledger_residual);
                spill_events += step.leaf_spill_events.unwrap().len();
                accepted_days += 1;
            }
            Err(error) => {
                failure = Some(json!({"message":error,"completeStateUnchanged":state==before}));
                break;
            }
        }
    }
    Ok(
        json!({"recipe":world.recipe,"settings":model.settings(),"regions":world.surface.areas.len(),
        "eligibleParents":model.merge_candidates().unwrap(),"acceptedDays":accepted_days,"failure":failure,
        "maximumRelativeGlobalResidual":max_global,"maximumRelativeLocalResidual":max_local,
        "spillEvents":spill_events,"lifecycle":lifecycle(&state),
        "finalBudget":model.budget(&state)?,"replay":replay(&model,&state).unwrap_or_else(|error| json!({"refusal":error}))}),
    )
}
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    (s, (a - (s - v)) + (b - v))
}
fn funded(model: &Model, world: &World, inputs: &[(usize, f64)]) -> Result<Checkpoint, String> {
    let mut cp = model.initial_state().checkpoint();
    cp.elapsed_seconds = 1;
    let b = cp
        .reference_body_high_kilograms
        .as_ref()
        .unwrap()
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .ok_or("No finite donor.")?
        .0;
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let contacts: Vec<_> = world
        .water
        .body_ids
        .iter()
        .enumerate()
        .filter_map(|(r, &id)| (id == ids[b]).then_some(r))
        .take(inputs.len())
        .collect();
    if contacts.len() != inputs.len() {
        return Err("Insufficient donor contacts for directed history.".into());
    }
    for (&contact, &(r, amount)) in contacts.iter().zip(inputs) {
        let high = cp.reference_body_high_kilograms.as_mut().unwrap();
        let low = cp.reference_body_low_kilograms.as_mut().unwrap();
        let (s, e) = two_sum(high[b], -amount);
        (high[b], low[b]) = two_sum(s, low[b] + e);
        if high[b] <= 0. {
            return Err("Directed history exceeds finite mobile water.".into());
        }
        cp.cumulative_surface_transfers[contact].liquid_evaporation = amount;
        cp.cumulative_evaporation_kilograms[contact] = amount;
        cp.cumulative_surface_transfers[r].rain = amount;
        cp.cumulative_precipitation_kilograms[r] = amount;
        cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] = amount;
        cp.leaf_spill_state
            .as_mut()
            .unwrap()
            .pending_input
            .high_kilograms[r] = amount;
    }
    Ok(cp)
}

fn lifecycle(state: &State) -> Value {
    let cp = state.checkpoint();
    let parent = cp.merged_lake_state.as_ref().unwrap();
    let history = parent.frontier.as_ref().unwrap();
    json!({"parents":parent.parents,"mergeCounts":history.merge_counts,"splitCounts":history.split_counts,
        "pendingToParentKilograms":total_mass(&history.pending_to_parent.high_kilograms.iter().chain(&history.pending_to_parent.low_kilograms).copied().collect::<Vec<_>>()),
        "captureByParentKilograms":total_mass(&history.capture_by_parent.high_kilograms.iter().chain(&history.capture_by_parent.low_kilograms).copied().collect::<Vec<_>>()),
        "evaporationFromParentKilograms":total_mass(&history.evaporation_from_parent.high_kilograms.iter().chain(&history.evaporation_from_parent.low_kilograms).copied().collect::<Vec<_>>())})
}
fn directed() -> Result<Value, String> {
    let world = World::generate(recipe(0.3, 2))?;
    let model = Model::from_world(
        &world,
        settings(900, true),
        Default::default(),
        Default::default(),
    )?;
    let group = model
        .merge_candidates()
        .unwrap()
        .into_iter()
        .find(|g| g.child_terminals.len() == 2)
        .ok_or("Missing parent.")?;
    let mut inputs: Vec<_> = group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
        .map(|(&r, &c)| (r, c))
        .collect();
    inputs[0].1 += group.surplus_capacity_kilograms.unwrap() * 1e-6;
    let (m, mut state) = Model::restore(funded(&model, &world, &inputs)?)?;
    m.advance(&mut state, 1)?;
    let activated = json!({"lifecycle":lifecycle(&state),"surfaces":m.merged_lake_surfaces(&state)?,"budget":m.budget(&state)?,"replay":replay(&m,&state)?});
    let mut cp = state.checkpoint();
    cp.settings.evaporation_enabled = true;
    let (m, mut state) = Model::restore(cp)?;
    let before_split_replay = replay(&m, &state)?;
    let step = m.advance(&mut state, 3600)?;
    let split = json!({"lifecycle":lifecycle(&state),"surfaces":m.merged_lake_surfaces(&state)?,
        "childLiquidHighKilograms":group.child_terminals.iter().map(|&r|state.checkpoint().terminal_water_kilograms[r]).collect::<Vec<_>>(),
        "childLiquidLowKilograms":group.child_terminals.iter().map(|&r|state.checkpoint().terminal_low_kilograms.unwrap()[r]).collect::<Vec<_>>(),
        "parentRegionEvaporationKilograms":group.regions.iter().map(|&r|step.runoff_transfers[r].terminal_evaporation).collect::<Vec<_>>(),
        "budget":step.budget,"beforeSplitReplay":before_split_replay,"afterSplitReplay":replay(&m,&state)?});
    let mut excessive = inputs.clone();
    excessive[0].1 =
        group.child_capacities_kilograms[0] + 2. * group.surplus_capacity_kilograms.unwrap();
    let (m, mut state) = Model::restore(funded(&model, &world, &excessive)?)?;
    let before = state.clone();
    let error = m
        .advance(&mut state, 1)
        .err()
        .ok_or("Next-parent spill should refuse.")?;
    Ok(
        json!({"recipe":world.recipe,"settings":model.settings(),"group":group,"inputs":inputs,
        "funding":"Explicit synthetic rain queues debited from finite mobile reference water; not spontaneous climate history.",
        "activated":activated,"split":split,
        "nextSpillRefusal":{"message":error,"completeStateUnchanged":state==before}}),
    )
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "--output" {
        return Err("Usage: common_sill_frontier_report --output NEW_PATH".into());
    }
    let ordinary = [
        (0.71, 900, 2),
        (0.71, 450, 2),
        (0.3, 900, 2),
        (0.71, 900, 3),
    ]
    .into_iter()
    .map(|(c, l, s)| ordinary(c, l, s))
    .collect::<Result<Vec<_>, _>>()?;
    let report = json!({"reportVersion":"bounded-common-sill-frontier-validation-1","modelVersion":"seasonal-moisture-12",
        "scope":"One-level all-dry reversible parents; frozen half-stage demand and bounded endpoint rounding. No next-parent spill, nested/external receiving frontier, climate calibration or desktop promotion.",
        "ordinary":ordinary,"directed":directed()?});
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])
        .map_err(|e| e.to_string())?;
    file.write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes())
        .map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    Ok(())
}
