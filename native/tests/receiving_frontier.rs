use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics, State,
        SurfaceNumerics, TerminalNumerics, closed_lake::spill, merged_lake::Candidate,
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
fn fixture(mode: ClosedLakeExchange) -> (World, Model, Candidate, spill::Connection) {
    fixture_coverage(mode, 0.1)
}
fn fixture_coverage(
    mode: ClosedLakeExchange,
    coverage: f64,
) -> (World, Model, Candidate, spill::Connection) {
    let mut recipe: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    recipe.seed = "receiver-2".into();
    recipe.relief_scale = 0.01;
    recipe.detail_amplitude_meters = 3.;
    recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
    let world = World::generate(recipe).unwrap();
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
        .find(|g| g.basin_node == 15)
        .unwrap();
    assert_eq!(group.child_terminals, [22, 33]);
    let source = spill::first_connections(&world)
        .unwrap()
        .into_iter()
        .find(|s| s.terminal_region == 15)
        .unwrap();
    assert!(source.has_unique_route);
    assert_eq!(
        source.routes[0].destination,
        spill::Destination::ClosedTerminal {
            terminal_region: 22
        }
    );
    assert!(source.first_connection_level_meters.unwrap() > group.birth_level_meters);
    (world, model, group, source)
}
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    (s, (a - (s - v)) + (b - v))
}
fn funded(model: &Model, world: &World, inputs: &[(usize, f64)]) -> Checkpoint {
    fund_checkpoint(model.initial_state().checkpoint(), world, inputs)
}
fn fund_checkpoint(mut cp: Checkpoint, world: &World, inputs: &[(usize, f64)]) -> Checkpoint {
    cp.elapsed_seconds = cp.elapsed_seconds.max(1);
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&i| i > 0);
    let mut ordered = inputs.to_vec();
    ordered.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut used_contacts = vec![false; world.surface.areas.len()];
    for &(r, amount) in &ordered {
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
            .find_map(|(i, &id)| (id == ids[body] && !used_contacts[i]).then_some(i))
            .unwrap();
        used_contacts[contact] = true;
        let high = cp.reference_body_high_kilograms.as_mut().unwrap();
        let low = cp.reference_body_low_kilograms.as_mut().unwrap();
        let (s, e) = two_sum(high[body], -amount);
        (high[body], low[body]) = two_sum(s, low[body] + e);
        assert!(high[body] > 0.);
        cp.cumulative_surface_transfers[contact].liquid_evaporation += amount;
        cp.cumulative_evaporation_kilograms[contact] += amount;
        cp.cumulative_surface_transfers[r].rain += amount;
        cp.cumulative_precipitation_kilograms[r] += amount;
        let h = cp.cumulative_lake_capture_kilograms.as_mut().unwrap();
        let l = cp.cumulative_lake_capture_low_kilograms.as_mut().unwrap();
        let (s, e) = two_sum(h[r], amount);
        (h[r], l[r]) = two_sum(s, l[r] + e);
        let pending = &mut cp.leaf_spill_state.as_mut().unwrap().pending_input;
        let (s, e) = two_sum(pending.high_kilograms[r], amount);
        (pending.high_kilograms[r], pending.low_kilograms[r]) =
            two_sum(s, pending.low_kilograms[r] + e);
    }
    cp
}
fn input(group: &Candidate, source: &spill::Connection, fraction: f64) -> Vec<(usize, f64)> {
    let mut inputs: Vec<_> = group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
        .map(|(&r, &c)| (r, c))
        .collect();
    let amount = group.surplus_capacity_kilograms.unwrap() * fraction;
    inputs[0].1 += amount;
    inputs.push((
        source.terminal_region,
        source.capacity_cubic_meters.unwrap() * 1000. + amount,
    ));
    inputs
}
fn accepted() -> (Model, State, Candidate, spill::Connection) {
    let (world, model, group, source) = fixture(ClosedLakeExchange::FrozenCommonSillReceiving);
    let cp = funded(&model, &world, &input(&group, &source, 1e-6));
    let (model, mut state) = Model::restore(cp).unwrap();
    model.advance(&mut state, 1).unwrap();
    (model, state, group, source)
}
fn replay(model: &Model, state: &State, seconds: u32) {
    let cp: Checkpoint =
        serde_json::from_str(&serde_json::to_string(&state.checkpoint()).unwrap()).unwrap();
    let (resumed_model, mut resumed) = Model::restore(cp).unwrap();
    assert_eq!(*state, resumed);
    let mut continued = state.clone();
    model.advance(&mut continued, seconds).unwrap();
    resumed_model.advance(&mut resumed, seconds).unwrap();
    assert_eq!(continued, resumed);
}

#[test]
fn generated_unique_spill_receives_in_parent_with_geographic_and_owner_provenance() {
    let (world, model, group, source) = fixture(ClosedLakeExchange::FrozenCommonSillReceiving);
    let inputs = input(&group, &source, 1e-6);
    let expected = inputs[2].1 - source.capacity_cubic_meters.unwrap() * 1000.;
    let (model, mut state) = Model::restore(funded(&model, &world, &inputs)).unwrap();
    let step = model.advance(&mut state, 1).unwrap();
    let events = step.leaf_spill_events.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kilograms, expected);
    assert_eq!(events[0].route, source.routes[0]);
    let cp = state.checkpoint();
    assert_eq!(cp.schema_version, 13);
    let history = cp
        .merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap();
    let receipt = history.spill_to_parent.as_ref().unwrap();
    assert_eq!(receipt.high_kilograms[22], expected);
    assert_eq!(receipt.low_kilograms[22], 0.);
    assert_eq!(
        cp.leaf_spill_state
            .as_ref()
            .unwrap()
            .cumulative_incoming
            .high_kilograms[22],
        expected
    );
    for &r in &group.child_terminals {
        assert_eq!(cp.terminal_water_kilograms[r], 0.);
    }
    let budget = model.budget(&state).unwrap();
    assert!(budget.residual_kilograms.abs() / budget.initial_mobile_water_kilograms < 1e-12);
    assert!(budget.maximum_relative_local_surface_ledger_residual < 1e-12);
    let observation = model.closed_lake_frontier(&state).unwrap();
    assert!(observation.owners.iter().any(|owner| matches!(owner,planimulation_core::seasonal_moisture::lake_frontier::Owner::Parent {contents,..} if contents.basin_node==group.basin_node)));
    replay(&model, &state, 3600);
}

#[test]
fn old_frontier_keeps_receiving_refusal_and_complete_caller_rollback() {
    let (world, model, group, source) = fixture(ClosedLakeExchange::FrozenCommonSillFrontier);
    let (model, mut state) =
        Model::restore(funded(&model, &world, &input(&group, &source, 1e-6))).unwrap();
    let before = state.clone();
    assert!(
        model
            .advance(&mut state, 1)
            .unwrap_err()
            .contains("receiving-frontier policy")
    );
    assert_eq!(state, before);
    assert!(
        state
            .checkpoint()
            .merged_lake_state
            .unwrap()
            .frontier
            .unwrap()
            .spill_to_parent
            .is_none()
    );
}

#[test]
fn parent_capacity_refusal_rolls_back_full_caller_interval() {
    let (world, model, group, source) = fixture(ClosedLakeExchange::FrozenCommonSillReceiving);
    // 0.4 in each source exceeds remaining parent room once a parent is activated.
    let mut inputs = input(&group, &source, 0.4);
    inputs[2].1 += group.surplus_capacity_kilograms.unwrap() * 0.4;
    // Distinct actual contacts fund this history from multiple finite bodies.
    let cp = funded(&model, &world, &inputs);
    let (model, mut state) = Model::restore(cp).unwrap();
    let before = state.clone();
    assert!(
        model
            .advance(&mut state, 1)
            .unwrap_err()
            .contains("next-sill capacity")
    );
    assert_eq!(state, before);
}

#[test]
fn parent_received_spill_survives_split_with_lifetime_identity_and_replay() {
    let (_, state, group, _) = accepted();
    let mut cp = state.checkpoint();
    cp.settings.evaporation_enabled = true;
    let (model, mut state) = Model::restore(cp).unwrap();
    replay(&model, &state, 3600);
    model.advance(&mut state, 3600).unwrap();
    let cp = state.checkpoint();
    let merged = cp.merged_lake_state.as_ref().unwrap();
    assert!(merged.parents.is_empty());
    assert!(
        merged
            .frontier
            .as_ref()
            .unwrap()
            .spill_to_parent
            .as_ref()
            .unwrap()
            .high_kilograms[22]
            > 0.
    );
    for &r in &group.child_terminals {
        assert!(cp.terminal_water_kilograms[r] > 0.);
    }
    assert!(
        model
            .budget(&state)
            .unwrap()
            .maximum_relative_local_surface_ledger_residual
            < 1e-12
    );
    replay(&model, &state, 3600);
}

#[test]
fn receiving_save_rejects_missing_null_wrong_shape_pin_and_owner_provenance() {
    let (_, state, _, source) = accepted();
    let cp = state.checkpoint();
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .spill_to_parent = None;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .model_version = "common-sill-lifecycle-1".into();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .spill_to_parent
        .as_mut()
        .unwrap()
        .low_kilograms
        .pop();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    let f = bad
        .merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .spill_to_parent
        .as_mut()
        .unwrap();
    let amount = f.high_kilograms[22];
    f.high_kilograms[22] = 0.;
    f.high_kilograms[source.terminal_region] = amount;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .spill_to_parent
        .as_mut()
        .unwrap()
        .high_kilograms[22] *= 2.;
    assert!(Model::restore(bad).is_err());
    let mut value = serde_json::to_value(cp).unwrap();
    value["mergedLakeState"]["frontier"]["spillToParent"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(value).is_err());
}

#[test]
fn preexisting_parent_accepts_later_external_spill_and_rejects_geographic_teleport() {
    let (world, model, group, source) = fixture(ClosedLakeExchange::FrozenCommonSillReceiving);
    let inputs = input(&group, &source, 1e-6);
    let (model, mut state) = Model::restore(funded(&model, &world, &inputs[..2])).unwrap();
    model.advance(&mut state, 1).unwrap();
    assert_eq!(
        state
            .checkpoint()
            .merged_lake_state
            .as_ref()
            .unwrap()
            .parents
            .len(),
        1
    );
    let before = state.checkpoint();
    let (model, mut state) = Model::restore(fund_checkpoint(before, &world, &inputs[2..])).unwrap();
    model.advance(&mut state, 1).unwrap();
    replay(&model, &state, 900);
    let cp = state.checkpoint();
    let mut bad = cp.clone();
    let receipt = bad
        .merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .spill_to_parent
        .as_mut()
        .unwrap();
    let amount = receipt.high_kilograms[22];
    receipt.high_kilograms[22] = 0.;
    receipt.high_kilograms[33] = amount;
    assert!(Model::restore(bad.clone()).is_err());
    // Global and parent totals stay unchanged, but no route enters child 33.
    let incoming = &mut bad.leaf_spill_state.as_mut().unwrap().cumulative_incoming;
    incoming.high_kilograms[22] = 0.;
    incoming.high_kilograms[33] = amount;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp;
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .spill_to_parent
        .as_mut()
        .unwrap()
        .high_kilograms[33] = 1e-13;
    assert!(Model::restore(bad).is_err());
}

#[test]
fn receiving_mode_preserves_old_observers_and_wire_refusals() {
    let (model, mut state, _, _) = accepted();
    let before = state.checkpoint();
    assert!(
        model
            .advance_observed(&mut state, 1, |_| panic!("Unsupported observer invoked."))
            .is_err()
    );
    assert!(
        model
            .advance_terminal_observed(&mut state, 1, |_| panic!("Unsupported observer invoked."))
            .is_err()
    );
    let mut bytes = Vec::new();
    assert!(planimulation_core::wire::seasonal_checkpoint(&mut bytes, &model, &state).is_err());
    assert!(
        planimulation_core::wire::seasonal_moisture(&mut bytes, &model, &state, None, 0).is_err()
    );
    assert!(bytes.is_empty());
    assert_eq!(state.checkpoint(), before);
}
