use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics, State,
        SurfaceNumerics, TerminalNumerics,
        closed_lake::spill::Destination,
        merged_lake::{Candidate, outgoing::Connection},
    },
};

fn settings(mode: ClosedLakeExchange) -> Settings {
    Settings {
        initial_active_surface_depth_meters: 10.,
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(mode),
        evaporation_enabled: false,
        precipitation_enabled: false,
        routing_enabled: false,
        max_coupled_step_seconds: 900,
        ..Default::default()
    }
}
fn fixture(mode: ClosedLakeExchange) -> (World, Model, Candidate) {
    let mut r: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    r.water = planimulation_core::water::WaterSettings::Coverage { fraction: 0.3 };
    let world = World::generate(r).unwrap();
    let model = Model::from_world(
        &world,
        settings(mode),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let group = model
        .merge_candidates()
        .unwrap()
        .into_iter()
        .find(|g| g.basin_node == 37)
        .unwrap();
    assert_eq!(group.child_terminals, [8, 156]);
    (world, model, group)
}
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    (s, (a - (s - v)) + (b - v))
}
fn exact_units(values: impl IntoIterator<Item = f64>) -> i128 {
    values
        .into_iter()
        .map(|v| {
            let scaled = v * 1_048_576.;
            assert!(scaled.abs() < 2_f64.powi(126));
            let units = scaled as i128;
            assert_eq!(
                units as f64, scaled,
                "Outside the independent dyadic audit domain."
            );
            units
        })
        .sum()
}
fn credit(h: &mut f64, l: &mut f64, v: f64) {
    let (s, e) = two_sum(*h, v);
    (*h, *l) = two_sum(s, *l + e);
}
// Directed rain history, paid by actual finite reference water. Never a new source.
fn fund(mut cp: Checkpoint, world: &World, inputs: &[(usize, f64)]) -> Checkpoint {
    cp.elapsed_seconds = cp.elapsed_seconds.max(1);
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let mut used = vec![false; world.surface.areas.len()];
    for &(r, amount) in inputs {
        let body = cp
            .reference_body_high_kilograms
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        let contact = world
            .water
            .body_ids
            .iter()
            .enumerate()
            .find_map(|(i, &id)| (id == ids[body] && !used[i]).then_some(i))
            .unwrap();
        used[contact] = true;
        credit(
            &mut cp.reference_body_high_kilograms.as_mut().unwrap()[body],
            &mut cp.reference_body_low_kilograms.as_mut().unwrap()[body],
            -amount,
        );
        assert!(cp.reference_body_high_kilograms.as_ref().unwrap()[body] > 0.);
        cp.cumulative_surface_transfers[contact].liquid_evaporation += amount;
        cp.cumulative_evaporation_kilograms[contact] += amount;
        cp.cumulative_surface_transfers[r].rain += amount;
        cp.cumulative_precipitation_kilograms[r] += amount;
        credit(
            &mut cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r],
            &mut cp.cumulative_lake_capture_low_kilograms.as_mut().unwrap()[r],
            amount,
        );
        let node = world.basins.region_nodes()[r];
        let active = cp
            .merged_lake_state
            .as_ref()
            .unwrap()
            .parents
            .iter()
            .find(|p| {
                node == p.basin_node || world.basins.nodes()[node].parent == Some(p.basin_node)
            })
            .map(|p| p.basin_node);
        let queue_region = if let Some(parent) = active {
            let capture = &mut cp
                .merged_lake_state
                .as_mut()
                .unwrap()
                .frontier
                .as_mut()
                .unwrap()
                .capture_by_parent;
            credit(
                &mut capture.high_kilograms[r],
                &mut capture.low_kilograms[r],
                amount,
            );
            // Mirror active-parent capture's existing canonical owner queue.
            (0..world.surface.areas.len())
                .find(|&i| {
                    world.drainage.receivers[i] as usize == i
                        && world.basins.nodes()[world.basins.region_nodes()[i]].parent
                            == Some(parent)
                })
                .unwrap()
        } else {
            r
        };
        let q = &mut cp.leaf_spill_state.as_mut().unwrap().pending_input;
        credit(
            &mut q.high_kilograms[queue_region],
            &mut q.low_kilograms[queue_region],
            amount,
        );
    }
    cp
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
fn outgoing_index(model: &Model, group: &Candidate) -> usize {
    model
        .merge_candidates()
        .unwrap()
        .iter()
        .position(|g| g.basin_node == group.basin_node)
        .unwrap()
}
fn outlet(model: &Model, group: &Candidate) -> Connection {
    model
        .parent_spill_connections()
        .unwrap()
        .into_iter()
        .find(|c| c.basin_node == group.basin_node)
        .unwrap()
}
fn replay(model: &Model, state: &State, seconds: u32) {
    let cp = serde_json::from_str(&serde_json::to_string(&state.checkpoint()).unwrap()).unwrap();
    let (m, mut resumed) = Model::restore(cp).unwrap();
    assert_eq!(*state, resumed);
    let mut continued = state.clone();
    model.advance(&mut continued, seconds).unwrap();
    m.advance(&mut resumed, seconds).unwrap();
    assert_eq!(continued, resumed);
}
fn accepted() -> (World, Model, State, Candidate) {
    let (world, m, group) = fixture(ClosedLakeExchange::FrozenCommonSillOutlets);
    let (m, mut s) = Model::restore(fund(
        m.initial_state().checkpoint(),
        &world,
        &inputs(&group),
    ))
    .unwrap();
    m.advance(&mut s, 1).unwrap();
    (world, m, s, group)
}

#[test]
fn generated_parent_spills_to_actual_finite_body_contact_and_preserves_full_source() {
    let (world, m, group) = fixture(ClosedLakeExchange::FrozenCommonSillOutlets);
    let connection = outlet(&m, &group);
    assert!(connection.has_unique_route);
    assert_eq!(
        connection.routes[0].destination,
        Destination::ReferenceBody {
            body_id: 2,
            contact_region: 41
        }
    );
    let inputs = inputs(&group);
    // All fixture capacities/inputs are integral binary64 values. This oracle
    // sums their exact represented integers, independent of the filling code.
    assert!(inputs.iter().all(|(_, v)| v.fract() == 0.));
    assert!(
        group
            .child_capacities_kilograms
            .iter()
            .all(|v| v.fract() == 0.)
    );
    let expected = inputs.iter().map(|(_, v)| *v as i128).sum::<i128>()
        - group
            .child_capacities_kilograms
            .iter()
            .map(|v| *v as i128)
            .sum::<i128>()
        - group.surplus_capacity_kilograms.unwrap() as i128;
    assert!(expected > 0);
    let (m, mut s) = Model::restore(fund(m.initial_state().checkpoint(), &world, &inputs)).unwrap();
    replay(&m, &s, 1);
    let exact_before = exact_units(s.owned_stock_components().copied());
    let step = m.advance(&mut s, 1).unwrap();
    assert!(step.leaf_spill_events.unwrap().is_empty());
    let events = step.parent_spill_events.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].source_basin_node, 37);
    assert_eq!(events[0].input_terminal_region, 8);
    assert_eq!(events[0].route, connection.routes[0]);
    assert_eq!(events[0].kilograms as i128, expected);
    let cp = s.checkpoint();
    assert_eq!(
        exact_before,
        exact_units(s.owned_stock_components().copied())
    );
    assert_eq!(cp.schema_version, 14);
    let parent = &cp.merged_lake_state.as_ref().unwrap().parents[0];
    assert_eq!(parent.birth_low_kilograms, -16.);
    assert_eq!(
        (parent.surplus_high_kilograms, parent.surplus_low_kilograms),
        (group.surplus_capacity_kilograms.unwrap(), 0.)
    );
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
    assert_eq!(
        ledger.cumulative_outgoing.high_kilograms[outgoing_index(&m, &group)] as i128,
        expected
    );
    assert_eq!(
        ledger.cumulative_incoming.high_kilograms[41] as i128,
        expected
    );
    assert!(
        cp.leaf_spill_state
            .as_ref()
            .unwrap()
            .cumulative_outgoing
            .high_kilograms
            .iter()
            .all(|&v| v == 0.)
    );
    assert!(
        cp.leaf_spill_state
            .as_ref()
            .unwrap()
            .pending_input
            .high_kilograms
            .iter()
            .all(|&v| v == 0.)
    );
    let surface = m.merged_lake_surfaces(&s).unwrap();
    assert_eq!(
        surface[0].absolute_level_meters,
        connection.first_connection_level_meters
    );
    assert!(
        m.budget(&s)
            .unwrap()
            .maximum_relative_local_surface_ledger_residual
            < 1e-12
    );
    replay(&m, &s, 3600);
    // Current-owner observation stays read-only; old desktop/observer contracts
    // cannot silently encode this new parent source as a leaf source.
    let before = s.clone();
    let owners = m.closed_lake_frontier(&s).unwrap();
    assert_eq!(
        owners
            .owners
            .iter()
            .filter(|o| matches!(
                o,
                planimulation_core::seasonal_moisture::lake_frontier::Owner::Parent { .. }
            ))
            .count(),
        1
    );
    assert!(
        m.advance_observed(&mut s, 1, |_| panic!("Unsupported observer invoked."))
            .is_err()
    );
    assert!(
        m.advance_terminal_observed(&mut s, 1, |_| panic!("Unsupported observer invoked."))
            .is_err()
    );
    let mut bytes = Vec::new();
    assert!(planimulation_core::wire::seasonal_checkpoint(&mut bytes, &m, &s).is_err());
    assert!(planimulation_core::wire::seasonal_moisture(&mut bytes, &m, &s, None, 0).is_err());
    assert!(bytes.is_empty());
    assert_eq!(s, before);
}

#[test]
fn actual_delayed_terminal_delivery_drives_parent_outlet_and_owner_ledger() {
    let (world, _model, s, _group) = accepted();
    let mut cp = s.checkpoint();
    let amount = 1e10;
    let region = 141;
    assert_ne!(world.drainage.receivers[region] as usize, region);
    let body = cp
        .reference_body_high_kilograms
        .as_ref()
        .unwrap()
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let contact = world
        .water
        .body_ids
        .iter()
        .position(|&id| id == ids[body])
        .unwrap();
    credit(
        &mut cp.reference_body_high_kilograms.as_mut().unwrap()[body],
        &mut cp.reference_body_low_kilograms.as_mut().unwrap()[body],
        -amount,
    );
    // Historical land rain/runoff before the parent activation; still-owned
    // transit now arrives under the active parent. This is not new rain capture.
    cp.cumulative_surface_transfers[contact].liquid_evaporation += amount;
    cp.cumulative_evaporation_kilograms[contact] += amount;
    cp.cumulative_surface_transfers[region].rain += amount;
    cp.cumulative_surface_transfers[region].liquid_runoff += amount;
    cp.cumulative_precipitation_kilograms[region] += amount;
    cp.pending_runoff_kilograms[region] += amount;
    cp.settings.routing_enabled = true;
    let (m, mut s) = Model::restore(cp).unwrap();
    replay(&m, &s, 900);
    let step = m.advance(&mut s, 900).unwrap();
    let events = step.parent_spill_events.unwrap();
    assert!(!events.is_empty());
    assert!(events.iter().all(|e| e.source_basin_node == 37));
    let cp = s.checkpoint();
    let history = cp
        .merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap();
    assert!(
        history
            .delivery_to_parent
            .high_kilograms
            .iter()
            .sum::<f64>()
            > 0.
    );
    assert!(
        m.budget(&s)
            .unwrap()
            .maximum_relative_local_surface_ledger_residual
            < 1e-12
    );
}

#[test]
fn old_receiver_model_keeps_next_parent_refusal_and_complete_rollback() {
    let (world, m, group) = fixture(ClosedLakeExchange::FrozenCommonSillReceiving);
    let (m, mut s) = Model::restore(fund(
        m.initial_state().checkpoint(),
        &world,
        &inputs(&group),
    ))
    .unwrap();
    let before = s.clone();
    assert!(
        m.advance(&mut s, 1)
            .unwrap_err()
            .contains("next-parent spill")
    );
    assert_eq!(s, before);
    assert!(m.parent_spill_connections().is_none());
}

#[test]
fn several_queues_of_one_parent_are_one_owner_not_concurrent_leaf_sources() {
    let (world, m, group) = fixture(ClosedLakeExchange::FrozenCommonSillOutlets);
    let mut input = inputs(&group);
    input[1].1 += group.surplus_capacity_kilograms.unwrap() * 1e-6;
    let (m, mut s) = Model::restore(fund(m.initial_state().checkpoint(), &world, &input)).unwrap();
    let step = m.advance(&mut s, 1).unwrap();
    let events = step.parent_spill_events.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(
        (
            events[0].input_terminal_region,
            events[1].input_terminal_region
        ),
        (8, 156)
    );
    assert!(events.iter().all(|e| e.source_basin_node == 37));
    assert!(
        m.budget(&s)
            .unwrap()
            .maximum_relative_local_surface_ledger_residual
            < 1e-12
    );
    replay(&m, &s, 900);
}

#[test]
fn existing_full_parent_receives_new_local_input_spills_and_later_evaporates() {
    let (world, _model, mut s, group) = accepted();
    let before = s.checkpoint().merged_lake_state.unwrap().parents[0].clone();
    let cp = fund(s.checkpoint(), &world, &[(156, 1e10)]);
    let (m2, s2) = Model::restore(cp).unwrap();
    s = s2;
    let step = m2.advance(&mut s, 1).unwrap();
    assert_eq!(step.parent_spill_events.unwrap()[0].kilograms, 1e10);
    assert_eq!(
        s.checkpoint().merged_lake_state.as_ref().unwrap().parents[0],
        before
    );
    let mut cp = s.checkpoint();
    cp.settings.evaporation_enabled = true;
    let (m, mut s) = Model::restore(cp).unwrap();
    replay(&m, &s, 3600);
    m.advance(&mut s, 3600).unwrap();
    assert!(
        s.checkpoint().merged_lake_state.as_ref().unwrap().parents[0].surplus_high_kilograms
            < group.surplus_capacity_kilograms.unwrap()
    );
    assert!(
        m.budget(&s)
            .unwrap()
            .maximum_relative_local_surface_ledger_residual
            < 1e-12
    );
    // Observation stays read-only and does not count provenance as water.
    let before = s.clone();
    assert!(!m.closed_lake_frontier(&s).unwrap().owners.is_empty());
    assert_eq!(s, before);
}

#[test]
fn parent_and_unrelated_leaf_overflow_refuses_before_publishing_any_transfer() {
    let (world, m, group) = fixture(ClosedLakeExchange::FrozenCommonSillOutlets);
    let source =
        planimulation_core::seasonal_moisture::closed_lake::spill::first_connections(&world)
            .unwrap()
            .into_iter()
            .find(|c| {
                !group.child_terminals.contains(&c.terminal_region)
                    && c.capacity_cubic_meters.is_some_and(|v| v * 1000. < 1e17)
            })
            .unwrap();
    let mut input = inputs(&group);
    input.push((
        source.terminal_region,
        source.capacity_cubic_meters.unwrap() * 1000. * (1. + 1e-6),
    ));
    let (m, mut s) = Model::restore(fund(m.initial_state().checkpoint(), &world, &input)).unwrap();
    let before = s.clone();
    assert!(
        m.advance(&mut s, 1)
            .unwrap_err()
            .contains("Concurrent parent/leaf")
    );
    assert_eq!(s, before);
}

#[test]
fn parent_outgoing_checkpoints_reject_versions_shapes_null_and_balanced_teleports() {
    let (_, m, s, group) = accepted();
    let cp = s.checkpoint();
    let index = outgoing_index(&m, &group);
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .outgoing_spill = None;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .outgoing_spill
        .as_mut()
        .unwrap()
        .model_version = "unrecorded-outlet-policy".into();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .outgoing_spill
        .as_mut()
        .unwrap()
        .cumulative_outgoing
        .low_kilograms
        .pop();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    let ledger = bad
        .merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .outgoing_spill
        .as_mut()
        .unwrap();
    let amount = ledger.cumulative_incoming.high_kilograms[41];
    ledger.cumulative_incoming.high_kilograms[41] = 0.;
    ledger.cumulative_incoming.high_kilograms[8] = amount;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .outgoing_spill
        .as_mut()
        .unwrap()
        .cumulative_outgoing
        .high_kilograms[index] = 0.;
    assert!(Model::restore(bad).is_err());
    let mut value = serde_json::to_value(cp).unwrap();
    value["mergedLakeState"]["frontier"]["outgoingSpill"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(value).is_err());
}
