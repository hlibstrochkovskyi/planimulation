//! Retained bounded model-13 evidence; directed funding is not natural history.
use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics, State,
        SurfaceNumerics, TerminalNumerics, closed_lake::spill, merged_lake::Candidate,
    },
};
use serde_json::{Value, json};
use std::io::Write;

fn settings(quiet: bool, mode: ClosedLakeExchange, limit: u32) -> Settings {
    Settings {
        initial_active_surface_depth_meters: 10.,
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(mode),
        evaporation_enabled: !quiet,
        precipitation_enabled: !quiet,
        routing_enabled: !quiet,
        max_coupled_step_seconds: limit,
        ..Default::default()
    }
}
fn recipe(coverage: f64, subdivision: u32) -> Recipe {
    let mut r: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    r.subdivision = subdivision;
    r.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
    r
}
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    (s, (a - (s - v)) + (b - v))
}
fn funded(
    model: &Model,
    world: &World,
    inputs: &[(usize, f64)],
) -> Result<(Checkpoint, Value), String> {
    let mut cp = model.initial_state().checkpoint();
    cp.elapsed_seconds = 1;
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let mut ordered = inputs.to_vec();
    ordered.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut used = vec![false; world.surface.areas.len()];
    let mut funding = Vec::new();
    for (r, amount) in ordered {
        let body = cp
            .reference_body_high_kilograms
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .ok_or("No finite reference donor.")?
            .0;
        let contact = world
            .water
            .body_ids
            .iter()
            .enumerate()
            .find_map(|(i, &id)| (id == ids[body] && !used[i]).then_some(i))
            .ok_or("Insufficient funding contacts.")?;
        used[contact] = true;
        let h = cp.reference_body_high_kilograms.as_mut().unwrap();
        let l = cp.reference_body_low_kilograms.as_mut().unwrap();
        let (s, e) = two_sum(h[body], -amount);
        (h[body], l[body]) = two_sum(s, l[body] + e);
        if h[body] <= 0. {
            return Err("Directed history exceeds its finite donor.".into());
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
        funding.push(json!({"referenceBodyId":ids[body],"evaporationContact":contact,"rainTerminal":r,"kilograms":amount}));
    }
    Ok((cp, json!(funding)))
}
fn inputs(group: &Candidate, source: &spill::Connection, fraction: f64) -> Vec<(usize, f64)> {
    let mut inputs: Vec<_> = group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
        .map(|(&r, &c)| (r, c))
        .collect();
    let surplus = group.surplus_capacity_kilograms.unwrap() * fraction;
    inputs[0].1 += surplus;
    inputs.push((
        source.terminal_region,
        source.capacity_cubic_meters.unwrap() * 1000. + surplus,
    ));
    inputs
}
fn replay(model: &Model, state: &State) -> Result<Value, String> {
    let (m, mut resumed) = Model::restore(
        serde_json::from_str(&serde_json::to_string(&state.checkpoint()).unwrap()).unwrap(),
    )?;
    let round_trip = *state == resumed;
    let mut continued = state.clone();
    model.advance(&mut continued, 3600)?;
    m.advance(&mut resumed, 3600)?;
    Ok(
        json!({"fullCheckpointRoundTripExact":round_trip,"nextHourContinuationExact":continued==resumed}),
    )
}
fn directed() -> Result<Value, String> {
    let mut r = recipe(0.1, 2);
    r.seed = "receiver-2".into();
    r.relief_scale = 0.01;
    r.detail_amplitude_meters = 3.;
    let world = World::generate(r)?;
    let configuration = settings(true, ClosedLakeExchange::FrozenCommonSillReceiving, 900);
    let model = Model::from_world(
        &world,
        configuration,
        Default::default(),
        Default::default(),
    )?;
    let group = model
        .merge_candidates()
        .unwrap()
        .into_iter()
        .find(|g| g.basin_node == 15)
        .ok_or("Retained parent changed.")?;
    let source = spill::first_connections(&world)?
        .into_iter()
        .find(|s| s.terminal_region == 15 && s.has_unique_route)
        .ok_or("Retained source changed.")?;
    let input = inputs(&group, &source, 1e-6);
    let (cp, funding) = funded(&model, &world, &input)?;
    let (m, mut state) = Model::restore(cp)?;
    let step = m.advance(&mut state, 1)?;
    let received = state.checkpoint();
    let accepted = json!({"events":step.leaf_spill_events,"parents":received.merged_lake_state.as_ref().unwrap().parents,
        "receivingTerminalHighKilograms":received.merged_lake_state.as_ref().unwrap().frontier.as_ref().unwrap().spill_to_parent.as_ref().unwrap().high_kilograms[22],
        "receivingTerminalLowKilograms":received.merged_lake_state.as_ref().unwrap().frontier.as_ref().unwrap().spill_to_parent.as_ref().unwrap().low_kilograms[22],
        "surfaces":m.merged_lake_surfaces(&state)?,"budget":step.budget,"replay":replay(&m,&state)?});
    let mut cp = received;
    cp.settings.evaporation_enabled = true;
    let (m, mut state) = Model::restore(cp)?;
    let before_split_replay = replay(&m, &state)?;
    let step = m.advance(&mut state, 3600)?;
    let cp = state.checkpoint();
    let history = cp
        .merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap();
    let split = json!({"parents":cp.merged_lake_state.as_ref().unwrap().parents,"mergeCounts":history.merge_counts,"splitCounts":history.split_counts,
        "receivingTerminalHighKilograms":history.spill_to_parent.as_ref().unwrap().high_kilograms[22],
        "childLiquidHighKilograms":group.child_terminals.iter().map(|&r|cp.terminal_water_kilograms[r]).collect::<Vec<_>>(),
        "childLiquidLowKilograms":group.child_terminals.iter().map(|&r|cp.terminal_low_kilograms.as_ref().unwrap()[r]).collect::<Vec<_>>(),
        "budget":step.budget,"beforeSplitReplay":before_split_replay,"afterSplitReplay":replay(&m,&state)?});
    let old = Model::from_world(
        &world,
        settings(true, ClosedLakeExchange::FrozenCommonSillFrontier, 900),
        Default::default(),
        Default::default(),
    )?;
    let (cp, _) = funded(&old, &world, &input)?;
    let (old, mut previous) = Model::restore(cp)?;
    let before = previous.clone();
    let old_refusal = old
        .advance(&mut previous, 1)
        .err()
        .ok_or("Model 12 unexpectedly accepted receiving.")?;
    let mut excessive = inputs(&group, &source, 0.4);
    excessive[2].1 += group.surplus_capacity_kilograms.unwrap() * 0.4;
    let (cp, excessive_funding) = funded(&model, &world, &excessive)?;
    let (m, mut state) = Model::restore(cp)?;
    let before_excess = state.clone();
    let refusal = m
        .advance(&mut state, 1)
        .err()
        .ok_or("Over-capacity arrival unexpectedly accepted.")?;
    Ok(
        json!({"recipe":world.recipe,"settings":configuration,"group":group,"source":source,"inputs":input,
        "fundingScope":"Synthetic rain histories debited from actual finite mobile reference stocks; not spontaneous weather or stationary hydrology.",
        "funding":funding,"accepted":accepted,"split":split,"legacyRefusal":{"message":old_refusal,"completeStateUnchanged":before==previous},
        "capacityRefusal":{"inputs":excessive,"funding":excessive_funding,"message":refusal,"completeStateUnchanged":before_excess==state}}),
    )
}
fn normalize_to_twelve(mut cp: Checkpoint) -> Checkpoint {
    cp.schema_version = 12;
    cp.model_version = "seasonal-moisture-12".into();
    cp.settings.closed_lake_exchange = Some(ClosedLakeExchange::FrozenCommonSillFrontier);
    cp.closed_lake_model_version = Some("closed-leaf-exchange-4".into());
    let merged = cp.merged_lake_state.as_mut().unwrap();
    merged.model_version = "common-sill-frontier-2".into();
    let frontier = merged.frontier.as_mut().unwrap();
    frontier.model_version = "common-sill-lifecycle-1".into();
    frontier.spill_to_parent = None;
    cp
}
fn ordinary(coverage: f64, limit: u32, subdivision: u32) -> Result<Value, String> {
    let world = World::generate(recipe(coverage, subdivision))?;
    let configuration = settings(false, ClosedLakeExchange::FrozenCommonSillReceiving, limit);
    let m = Model::from_world(
        &world,
        configuration,
        Default::default(),
        Default::default(),
    )?;
    let old = Model::from_world(
        &world,
        settings(false, ClosedLakeExchange::FrozenCommonSillFrontier, limit),
        Default::default(),
        Default::default(),
    )?;
    let mut a = m.initial_state();
    let mut b = old.initial_state();
    let mut days = 0;
    let mut failure = None;
    let mut exact = true;
    let mut global: f64 = 0.;
    let mut local: f64 = 0.;
    let mut events = 0;
    for _ in 0..40 {
        let before = a.clone();
        match m.advance(&mut a, 86400) {
            Ok(step) => {
                old.advance(&mut b, 86400)?;
                let cp = a.checkpoint();
                let ledger = cp
                    .merged_lake_state
                    .as_ref()
                    .unwrap()
                    .frontier
                    .as_ref()
                    .unwrap()
                    .spill_to_parent
                    .as_ref()
                    .unwrap();
                let empty = ledger
                    .high_kilograms
                    .iter()
                    .chain(&ledger.low_kilograms)
                    .all(|&v| v == 0.);
                exact &= empty && normalize_to_twelve(cp) == b.checkpoint();
                global = global.max(
                    step.budget.residual_kilograms.abs()
                        / step.budget.initial_mobile_water_kilograms.max(1.),
                );
                local = local.max(step.budget.maximum_relative_local_surface_ledger_residual);
                events += step.leaf_spill_events.unwrap().len();
                days += 1;
            }
            Err(error) => {
                failure = Some(json!({"message":error,"completeStateUnchanged":before==a}));
                break;
            }
        }
    }
    Ok(
        json!({"recipe":world.recipe,"settings":configuration,"regions":world.surface.areas.len(),"acceptedDays":days,"failure":failure,
        "completePhysicalCheckpointsMatchModel12EachAcceptedDay":exact,"spillEvents":events,"maximumRelativeGlobalResidual":global,"maximumRelativeLocalResidual":local,
        "activeParents":a.checkpoint().merged_lake_state.unwrap().parents.len(),"replay":replay(&m,&a)?}),
    )
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "--output" {
        return Err("Usage: receiving_frontier_report --output NEW_PATH".into());
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
    let report = json!({"reportVersion":"bounded-common-sill-receiving-validation-1","modelVersion":"seasonal-moisture-13",
        "scope":"Unique leaf spill into already active one-level dry parents; head/next-sill ceilings. No parent outlet, nested activation, concurrent overflow, hydraulic law or desktop/default promotion.",
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
