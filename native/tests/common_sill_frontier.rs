use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics, State,
        SurfaceNumerics, TerminalNumerics, merged_lake::Candidate,
    },
};

fn settings() -> Settings {
    Settings {
        initial_active_surface_depth_meters: 10.,
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(ClosedLakeExchange::FrozenCommonSillFrontier),
        evaporation_enabled: false,
        precipitation_enabled: false,
        routing_enabled: false,
        max_coupled_step_seconds: 900,
        ..Default::default()
    }
}
fn fixture() -> (World, Model, Candidate) {
    let mut recipe: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: 0.3 };
    let world = World::generate(recipe).unwrap();
    let model =
        Model::from_world(&world, settings(), Default::default(), Default::default()).unwrap();
    let group = model
        .merge_candidates()
        .unwrap()
        .into_iter()
        .find(|g| g.child_terminals.len() == 2)
        .unwrap();
    (world, model, group)
}
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    (s, (a - (s - v)) + (b - v))
}
fn credit(high: &mut f64, low: &mut f64, amount: f64) {
    let (s, e) = two_sum(*high, amount);
    (*high, *low) = two_sum(s, *low + e);
}
// Directed rain history, explicitly debited from finite reference-body water.
fn funded(model: &Model, world: &World, inputs: &[(usize, f64)]) -> Checkpoint {
    let mut cp = model.initial_state().checkpoint();
    cp.elapsed_seconds = 1;
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
    ids.retain(|&i| i > 0);
    let contacts: Vec<_> = world
        .water
        .body_ids
        .iter()
        .enumerate()
        .filter_map(|(r, &id)| (id == ids[body]).then_some(r))
        .take(inputs.len())
        .collect();
    assert_eq!(contacts.len(), inputs.len());
    for (&contact, &(r, amount)) in contacts.iter().zip(inputs) {
        credit(
            &mut cp.reference_body_high_kilograms.as_mut().unwrap()[body],
            &mut cp.reference_body_low_kilograms.as_mut().unwrap()[body],
            -amount,
        );
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
    assert!(cp.reference_body_high_kilograms.as_ref().unwrap()[body] > 0.);
    cp
}
fn activated(fraction: f64) -> (World, Model, State, Candidate) {
    let (world, model, group) = fixture();
    let mut inputs: Vec<_> = group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
        .map(|(&r, &cap)| (r, cap))
        .collect();
    inputs[0].1 += group.surplus_capacity_kilograms.unwrap() * fraction;
    let (model, mut state) = Model::restore(funded(&model, &world, &inputs)).unwrap();
    model.advance(&mut state, 1).unwrap();
    (world, model, state, group)
}
fn replay(model: &Model, state: &State, seconds: u32) {
    let json = serde_json::to_string(&state.checkpoint()).unwrap();
    let (restored, mut resumed) = Model::restore(serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(*state, resumed);
    let mut continued = state.clone();
    model.advance(&mut continued, seconds).unwrap();
    restored.advance(&mut resumed, seconds).unwrap();
    assert_eq!(continued, resumed);
}

#[test]
fn exact_common_sill_is_separate_and_underfilled_siblings_never_activate() {
    let (world, model, group) = fixture();
    for underfilled in [false, true] {
        let mut inputs: Vec<_> = group
            .child_terminals
            .iter()
            .zip(&group.child_capacities_kilograms)
            .map(|(&r, &cap)| (r, cap))
            .collect();
        if underfilled {
            inputs[0].1 = inputs[0].1.next_down();
        }
        let cp = funded(&model, &world, &inputs);
        let (m, mut s) = Model::restore(cp).unwrap();
        m.advance(&mut s, 1).unwrap();
        assert!(s.checkpoint().merged_lake_state.unwrap().parents.is_empty());
        replay(&m, &s, 60);
    }
}

#[test]
fn generated_parent_splits_during_seasonal_evaporation_and_replays() {
    let (world, _, state, group) = activated(1e-6);
    let mut cp = state.checkpoint();
    let parent = &cp.merged_lake_state.as_ref().unwrap().parents[0];
    assert_eq!(parent.birth_low_kilograms, -16.);
    assert!(parent.surplus_high_kilograms > 0.);
    cp.settings.evaporation_enabled = true;
    let (model, mut state) = Model::restore(cp).unwrap();
    replay(&model, &state, 3600);
    let step = model.advance(&mut state, 3600).unwrap();
    let cp = state.checkpoint();
    assert!(cp.merged_lake_state.as_ref().unwrap().parents.is_empty());
    let history = cp
        .merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap();
    let index = model
        .merge_candidates()
        .unwrap()
        .iter()
        .position(|g| g.basin_node == group.basin_node)
        .unwrap();
    assert_eq!(
        (history.merge_counts[index], history.split_counts[index]),
        (1, 1)
    );
    let sill = group
        .regions
        .iter()
        .copied()
        .find(|&r| world.terrain.elevation[r] == group.birth_level_meters)
        .unwrap();
    assert!(history.evaporation_from_parent.high_kilograms[sill] > 0.);
    for (&r, &cap) in group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
    {
        assert!(cp.terminal_water_kilograms[r] > 0. && cp.terminal_water_kilograms[r] < cap);
    }
    assert!(
        group
            .regions
            .iter()
            .any(|&r| step.runoff_transfers[r].terminal_evaporation > 0.)
    );
    assert!(
        step.budget.residual_kilograms.abs() / step.budget.initial_mobile_water_kilograms < 1e-12
    );
    assert!(step.budget.maximum_relative_local_surface_ledger_residual < 1e-12);
    replay(&model, &state, 3600);
    assert!(model.merged_lake_surfaces(&state).unwrap().is_empty());
}

#[test]
fn lifecycle_checkpoint_pins_shapes_and_ownership_reject_corruption() {
    let (_, model, state, group) = activated(0.25);
    let cp = state.checkpoint();
    let index = model
        .merge_candidates()
        .unwrap()
        .iter()
        .position(|g| g.basin_node == group.basin_node)
        .unwrap();
    let mut variants = Vec::new();
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .split_counts[index] = 1;
    variants.push(bad);
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .merge_counts
        .clear();
    variants.push(bad);
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .capture_by_parent
        .low_kilograms
        .pop();
    variants.push(bad);
    let mut bad = cp.clone();
    bad.merged_lake_state.as_mut().unwrap().frontier = None;
    variants.push(bad);
    let mut bad = cp.clone();
    bad.merged_lake_state.as_mut().unwrap().model_version = "common-sill-parent-1".into();
    variants.push(bad);
    let mut bad = cp.clone();
    bad.terminal_water_kilograms[group.child_terminals[0]] = 1.;
    variants.push(bad);
    let mut bad = cp.clone();
    bad.merged_lake_state.as_mut().unwrap().parents[0].birth_low_kilograms = 0.;
    variants.push(bad);
    let mut bad = cp.clone();
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .pending_to_parent
        .high_kilograms[0] = 1.;
    variants.push(bad);
    for bad in variants {
        assert!(Model::restore(bad).is_err());
    }
    let mut json = serde_json::to_value(&cp).unwrap();
    json["mergedLakeState"]["frontier"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(json).is_err());
    replay(&model, &state, 3600);
}

#[test]
fn next_parent_spill_remains_an_atomic_refusal() {
    let (world, model, group) = fixture();
    let mut inputs: Vec<_> = group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
        .map(|(&r, &cap)| (r, cap))
        .collect();
    inputs[0].1 += 2. * group.surplus_capacity_kilograms.unwrap();
    let (model, mut state) = Model::restore(funded(&model, &world, &inputs)).unwrap();
    let before = state.clone();
    assert!(
        model
            .advance(&mut state, 60)
            .unwrap_err()
            .contains("next-parent spill")
    );
    assert_eq!(state, before);
}

#[test]
fn rematch_full_children_after_a_real_split_reactivates_the_same_owner() {
    let (world, _, state, group) = activated(1e-6);
    let mut cp = state.checkpoint();
    cp.settings.evaporation_enabled = true;
    let (m, mut s) = Model::restore(cp).unwrap();
    m.advance(&mut s, 3600).unwrap();
    let mut cp = s.checkpoint();
    cp.settings.evaporation_enabled = false;
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
    for (index, (&r, &cap)) in group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
        .enumerate()
    {
        let amount = (cap - cp.terminal_water_kilograms[r])
            - cp.terminal_low_kilograms.as_ref().unwrap()[r]
            + if index == 0 {
                group.surplus_capacity_kilograms.unwrap() * 1e-6
            } else {
                0.
            };
        credit(
            &mut cp.reference_body_high_kilograms.as_mut().unwrap()[body],
            &mut cp.reference_body_low_kilograms.as_mut().unwrap()[body],
            -amount,
        );
        cp.cumulative_surface_transfers[contact].liquid_evaporation += amount;
        cp.cumulative_evaporation_kilograms[contact] += amount;
        cp.cumulative_surface_transfers[r].rain += amount;
        cp.cumulative_precipitation_kilograms[r] += amount;
        credit(
            &mut cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r],
            &mut cp.cumulative_lake_capture_low_kilograms.as_mut().unwrap()[r],
            amount,
        );
        cp.leaf_spill_state
            .as_mut()
            .unwrap()
            .pending_input
            .high_kilograms[r] = amount;
    }
    let (model, mut state) = Model::restore(cp).unwrap();
    model.advance(&mut state, 1).unwrap();
    let cp = state.checkpoint();
    let parent = &cp.merged_lake_state.as_ref().unwrap().parents[0];
    assert_eq!(parent.basin_node, group.basin_node);
    assert_eq!(parent.birth_low_kilograms, -16.);
    let history = cp
        .merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap();
    let index = model
        .merge_candidates()
        .unwrap()
        .iter()
        .position(|g| g.basin_node == group.basin_node)
        .unwrap();
    assert_eq!(
        (history.merge_counts[index], history.split_counts[index]),
        (2, 1)
    );
    replay(&model, &state, 3600);
}

#[test]
fn ordinary_states_match_model_eleven_before_any_merge() {
    for (coverage, limit, subdivision) in [
        (0.71, 900, 2),
        (0.71, 450, 2),
        (0.3, 900, 2),
        (0.71, 900, 3),
    ] {
        let mut recipe: Recipe =
            serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json"))
                .unwrap();
        recipe.subdivision = subdivision;
        recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
        let world = World::generate(recipe).unwrap();
        let configuration = Settings {
            evaporation_enabled: true,
            precipitation_enabled: true,
            routing_enabled: true,
            max_coupled_step_seconds: limit,
            ..settings()
        };
        let newer = Model::from_world(
            &world,
            configuration,
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let older = Model::from_world(
            &world,
            Settings {
                closed_lake_exchange: Some(ClosedLakeExchange::FrozenLeafExposureWithSpillAndMerge),
                ..configuration
            },
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let mut a = newer.initial_state();
        let mut b = older.initial_state();
        for _ in 0..40 {
            newer.advance(&mut a, 86400).unwrap();
            older.advance(&mut b, 86400).unwrap();
            let mut cp = a.checkpoint();
            assert!(cp.merged_lake_state.as_ref().unwrap().parents.is_empty());
            assert!(
                cp.merged_lake_state
                    .as_ref()
                    .unwrap()
                    .frontier
                    .as_ref()
                    .unwrap()
                    .merge_counts
                    .iter()
                    .all(|&c| c == 0)
            );
            cp.schema_version = 11;
            cp.model_version =
                planimulation_core::seasonal_moisture::MERGED_LAKE_MODEL_VERSION.into();
            cp.settings.closed_lake_exchange =
                Some(ClosedLakeExchange::FrozenLeafExposureWithSpillAndMerge);
            cp.closed_lake_model_version = Some("closed-leaf-exchange-3".into());
            cp.merged_lake_state.as_mut().unwrap().model_version = "common-sill-parent-1".into();
            cp.merged_lake_state.as_mut().unwrap().frontier = None;
            assert_eq!(cp, b.checkpoint());
        }
        replay(&newer, &a, 3600);
    }
}

#[test]
fn unsupported_observers_and_desktop_writers_refuse_without_mutation() {
    let (world, model, mut state, _) = activated(0.25);
    let before = state.clone();
    assert!(
        planimulation_core::seasonal_moisture::closed_lake::Layout::from_world(&world)
            .unwrap()
            .capture(&model, &state)
            .is_err()
    );
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
    assert_eq!(state, before);
    let mut bytes = Vec::new();
    assert!(planimulation_core::wire::seasonal_checkpoint(&mut bytes, &model, &state).is_err());
    assert!(
        planimulation_core::wire::seasonal_moisture(&mut bytes, &model, &state, None, 0).is_err()
    );
    assert!(bytes.is_empty());
}

#[test]
fn parent_only_rain_and_evaporation_remain_attributable_after_splitting() {
    let (world, _, state, group) = activated(1e-6);
    let sill = group
        .regions
        .iter()
        .copied()
        .find(|&r| world.terrain.elevation[r] == group.birth_level_meters)
        .unwrap();
    let mut cp = state.checkpoint();
    cp.settings.precipitation_enabled = true;
    let normals = planimulation_core::seasonal_temperature::Normals::from_world(
        &world,
        cp.temperature_settings,
    )
    .unwrap();
    let temperature = normals.monthly_temperature_celsius[0][sill];
    assert!(temperature > 0.);
    let cloud = 2.
        * planimulation_core::seasonal_moisture::saturation_column_kilograms_per_square_meter(
            temperature,
            cp.settings.effective_vapor_depth_meters,
        )
        .unwrap()
        * world.surface.areas[sill];
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
        -cloud,
    );
    cp.vapor_kilograms[sill] = cloud;
    cp.cumulative_surface_transfers[contact].liquid_evaporation += cloud;
    cp.cumulative_evaporation_kilograms[contact] += cloud;
    let (model, mut state) = Model::restore(cp).unwrap();
    let rain = model.advance(&mut state, 60).unwrap();
    assert!(rain.precipitation_kilograms[sill] > 0.);
    let cp = state.checkpoint();
    let history = cp
        .merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap();
    assert!(history.capture_by_parent.high_kilograms[sill] > 0.);
    assert_eq!(
        history.capture_by_parent.high_kilograms[sill],
        cp.cumulative_lake_capture_kilograms.as_ref().unwrap()[sill]
    );
    replay(&model, &state, 60);
    let mut cp = state.checkpoint();
    cp.settings.precipitation_enabled = false;
    cp.settings.evaporation_enabled = true;
    let (model, mut state) = Model::restore(cp).unwrap();
    model.advance(&mut state, 86400).unwrap();
    let cp = state.checkpoint();
    assert!(cp.merged_lake_state.as_ref().unwrap().parents.is_empty());
    let history = cp
        .merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap();
    assert!(history.capture_by_parent.high_kilograms[sill] > 0.);
    assert!(
        group
            .regions
            .iter()
            .any(|&r| history.evaporation_from_parent.high_kilograms[r] > 0.)
    );
    // Supersaturated cloud suppresses evaporation at this column until its parent dries.
    assert_eq!(history.evaporation_from_parent.high_kilograms[sill], 0.);
    replay(&model, &state, 3600);
    let mut bad = cp;
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .capture_by_parent
        .high_kilograms[sill] = 0.;
    bad.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
        .capture_by_parent
        .low_kilograms[sill] = 0.;
    assert!(Model::restore(bad).is_err());
}

#[test]
fn aligned_hourly_and_daily_callers_cross_the_same_split_exactly() {
    let (_, _, state, _) = activated(1e-6);
    let mut cp = state.checkpoint();
    cp.settings.evaporation_enabled = true;
    // Explicitly align the directed history: partial caller endpoints otherwise
    // introduce different within-grid intervals, not identical integrations.
    cp.elapsed_seconds = 900;
    let (model, mut daily) = Model::restore(cp).unwrap();
    let mut hourly = daily.clone();
    for _ in 0..2 {
        model.advance(&mut daily, 86400).unwrap();
        for _ in 0..24 {
            model.advance(&mut hourly, 3600).unwrap();
        }
        assert_eq!(daily, hourly);
    }
    assert!(
        daily
            .checkpoint()
            .merged_lake_state
            .unwrap()
            .parents
            .is_empty()
    );
}
