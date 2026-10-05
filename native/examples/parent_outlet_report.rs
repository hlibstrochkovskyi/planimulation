//! Retained bounded model-14 evidence; directed funding is not natural history.
use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics, State,
        SurfaceNumerics, TerminalNumerics, merged_lake::Candidate,
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
    let mut used = vec![false; world.surface.areas.len()];
    let mut funding = Vec::new();
    for &(r, amount) in inputs {
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
        funding.push(
            json!({"referenceBodyId":ids[body],"evaporationContact":contact,
            "rainTerminal":r,"kilograms":amount}),
        );
    }
    Ok((cp, json!(funding)))
}
fn inputs(group: &Candidate) -> Vec<(usize, f64)> {
    let mut inputs: Vec<_> = group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
        .map(|(&r, &c)| (r, c))
        .collect();
    inputs[0].1 += group.surplus_capacity_kilograms.unwrap() * (1. + 1e-6);
    inputs
}
fn replay(model: &Model, state: &State) -> Result<Value, String> {
    let (m, mut resumed) = Model::restore(
        serde_json::from_str(&serde_json::to_string(&state.checkpoint()).unwrap()).unwrap(),
    )?;
    let exact = *state == resumed;
    let mut continued = state.clone();
    model.advance(&mut continued, 3600)?;
    m.advance(&mut resumed, 3600)?;
    Ok(json!({"fullCheckpointRoundTripExact":exact,"nextHourContinuationExact":continued==resumed}))
}
fn directed() -> Result<Value, String> {
    let world = World::generate(recipe(0.3, 2))?;
    let configuration = settings(true, ClosedLakeExchange::FrozenCommonSillOutlets, 900);
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
        .find(|g| g.basin_node == 37)
        .ok_or("Retained parent changed.")?;
    let outlet = model
        .parent_spill_connections()
        .unwrap()
        .into_iter()
        .find(|c| c.basin_node == group.basin_node)
        .ok_or("Retained outlet changed.")?;
    let input = inputs(&group);
    let (cp, funding) = funded(&model, &world, &input)?;
    let before_bodies = json!({"highKilograms":cp.reference_body_high_kilograms,
        "lowKilograms":cp.reference_body_low_kilograms});
    let (m, mut state) = Model::restore(cp)?;
    let step = m.advance(&mut state, 1)?;
    let received = state.checkpoint();
    let frontier = received
        .merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap();
    let accepted = json!({"events":step.parent_spill_events,"leafEvents":step.leaf_spill_events,
        "parents":received.merged_lake_state.as_ref().unwrap().parents,"outgoingHistory":frontier.outgoing_spill,
        "bodiesBefore":before_bodies,"bodiesAfter":{"highKilograms":received.reference_body_high_kilograms,
            "lowKilograms":received.reference_body_low_kilograms},
        "surfaces":m.merged_lake_surfaces(&state)?,"budget":step.budget,"replay":replay(&m,&state)?});
    let mut cp = received;
    cp.settings.evaporation_enabled = true;
    let (m, mut state) = Model::restore(cp)?;
    let step = m.advance(&mut state, 3600)?;
    let cp = state.checkpoint();
    let contraction = json!({"parents":cp.merged_lake_state.as_ref().unwrap().parents,
        "outgoingHistory":cp.merged_lake_state.as_ref().unwrap().frontier.as_ref().unwrap().outgoing_spill,
        "budget":step.budget,"replay":replay(&m,&state)?});
    let old = Model::from_world(
        &world,
        settings(true, ClosedLakeExchange::FrozenCommonSillReceiving, 900),
        Default::default(),
        Default::default(),
    )?;
    let (cp, _) = funded(&old, &world, &input)?;
    let (old, mut previous) = Model::restore(cp)?;
    let before = previous.clone();
    let legacy_refusal = old
        .advance(&mut previous, 1)
        .err()
        .ok_or("Model 13 unexpectedly accepted parent output.")?;
    let other =
        planimulation_core::seasonal_moisture::closed_lake::spill::first_connections(&world)?
            .into_iter()
            .find(|c| {
                !group.child_terminals.contains(&c.terminal_region)
                    && c.capacity_cubic_meters.is_some_and(|v| v * 1000. < 1e17)
            })
            .ok_or("Missing independent closed leaf.")?;
    let mut concurrent = input.clone();
    concurrent.push((
        other.terminal_region,
        other.capacity_cubic_meters.unwrap() * 1000. * (1. + 1e-6),
    ));
    let (cp, concurrent_funding) = funded(&model, &world, &concurrent)?;
    let (m, mut state) = Model::restore(cp)?;
    let before_concurrent = state.clone();
    let refusal = m
        .advance(&mut state, 1)
        .err()
        .ok_or("Concurrent owners unexpectedly accepted.")?;
    Ok(
        json!({"recipe":world.recipe,"settings":configuration,"group":group,"outlet":outlet,"inputs":input,
        "fundingScope":"Synthetic rain histories debited from actual finite mobile reference stocks; not spontaneous weather or stationary hydrology.",
        "funding":funding,"accepted":accepted,"contraction":contraction,
        "legacyRefusal":{"message":legacy_refusal,"completeStateUnchanged":before==previous},
        "concurrentRefusal":{"inputs":concurrent,"funding":concurrent_funding,"message":refusal,
            "completeStateUnchanged":before_concurrent==state}}),
    )
}
fn normalize_to_thirteen(mut cp: Checkpoint) -> Checkpoint {
    cp.schema_version = 13;
    cp.model_version = "seasonal-moisture-13".into();
    cp.settings.closed_lake_exchange = Some(ClosedLakeExchange::FrozenCommonSillReceiving);
    cp.closed_lake_model_version = Some("closed-leaf-exchange-5".into());
    let merged = cp.merged_lake_state.as_mut().unwrap();
    merged.model_version = "common-sill-receiver-3".into();
    let frontier = merged.frontier.as_mut().unwrap();
    frontier.model_version = "common-sill-lifecycle-2".into();
    frontier.outgoing_spill = None;
    cp
}
fn ordinary(coverage: f64, limit: u32, subdivision: u32) -> Result<Value, String> {
    let world = World::generate(recipe(coverage, subdivision))?;
    let configuration = settings(false, ClosedLakeExchange::FrozenCommonSillOutlets, limit);
    let m = Model::from_world(
        &world,
        configuration,
        Default::default(),
        Default::default(),
    )?;
    let old = Model::from_world(
        &world,
        settings(false, ClosedLakeExchange::FrozenCommonSillReceiving, limit),
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
    let mut leaf_events = 0;
    let mut parent_events = 0;
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
                    .outgoing_spill
                    .as_ref()
                    .unwrap();
                let empty = ledger
                    .cumulative_outgoing
                    .high_kilograms
                    .iter()
                    .chain(&ledger.cumulative_outgoing.low_kilograms)
                    .chain(&ledger.cumulative_incoming.high_kilograms)
                    .chain(&ledger.cumulative_incoming.low_kilograms)
                    .all(|&v| v == 0.);
                exact &= empty && normalize_to_thirteen(cp) == b.checkpoint();
                global = global.max(
                    step.budget.residual_kilograms.abs()
                        / step.budget.initial_mobile_water_kilograms.max(1.),
                );
                local = local.max(step.budget.maximum_relative_local_surface_ledger_residual);
                leaf_events += step.leaf_spill_events.unwrap().len();
                parent_events += step.parent_spill_events.unwrap().len();
                days += 1;
            }
            Err(error) => {
                failure = Some(json!({"message":error,"completeStateUnchanged":before==a}));
                break;
            }
        }
    }
    Ok(
        json!({"recipe":world.recipe,"settings":configuration,"regions":world.surface.areas.len(),
        "acceptedDays":days,"failure":failure,"completePhysicalCheckpointsMatchModel13EachAcceptedDay":exact,
        "leafSpillEvents":leaf_events,"parentSpillEvents":parent_events,"maximumRelativeGlobalResidual":global,
        "maximumRelativeLocalResidual":local,"activeParents":a.checkpoint().merged_lake_state.unwrap().parents.len(),
        "replay":replay(&m,&a)?}),
    )
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || args[0] != "--output" {
        return Err("Usage: parent_outlet_report --output NEW_PATH".into());
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
    let report = json!({"reportVersion":"bounded-parent-outlet-validation-1","modelVersion":"seasonal-moisture-14",
        "scope":"One full one-level dry parent sends owned excess through one unique geographic outlet to a receiver that accepts the complete arrival. No nested activation, receiver overflow chain, concurrent sources, hydraulic law or desktop/default promotion.",
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
