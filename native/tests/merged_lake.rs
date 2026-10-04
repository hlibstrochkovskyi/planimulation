use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics,
        SurfaceNumerics, TerminalNumerics, merged_lake::Candidate,
    },
};

fn settings(merge: bool) -> Settings {
    Settings {
        initial_active_surface_depth_meters: 10.,
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(if merge {
            ClosedLakeExchange::FrozenLeafExposureWithSpillAndMerge
        } else {
            ClosedLakeExchange::FrozenLeafExposureWithSpill
        }),
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
    // A resolved generated-world recipe, not hand-edited terrain or topology.
    recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: 0.3 };
    let world = World::generate(recipe).unwrap();
    let model = Model::from_world(
        &world,
        settings(true),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let candidates = model.merge_candidates().unwrap();
    let body = model
        .initial_state()
        .reference_body_high_kilograms()
        .unwrap()
        .iter()
        .copied()
        .fold(0., f64::max);
    let candidate = candidates
        .into_iter()
        .find(|g| {
            g.child_terminals.len() == 2 && total_mass(&g.child_capacities_kilograms) < body * 0.9
        })
        .expect("A mobile-water funded two-leaf parent is required.");
    (world, model, candidate)
}
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    (s, (a - (s - v)) + (b - v))
}
// Explicitly funded synthetic history on an unmodified generated world.
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
    let mut supply = (0., 0.);
    for &(_, amount) in inputs {
        let (s, e) = two_sum(supply.0, amount);
        supply = two_sum(s, supply.1 + e);
    }
    let high = cp.reference_body_high_kilograms.as_mut().unwrap();
    let low = cp.reference_body_low_kilograms.as_mut().unwrap();
    for amount in [supply.0, supply.1] {
        let (s, e) = two_sum(high[body], -amount);
        (high[body], low[body]) = two_sum(s, low[body] + e);
    }
    assert!(high[body] > 0.);
    for (&contact, &(r, amount)) in contacts.iter().zip(inputs) {
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
    cp
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
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

#[test]
fn filled_siblings_become_one_parent_and_replay_exactly() {
    let (world, model, group) = fixture();
    let inputs: Vec<_> = group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
        .map(|(&r, &c)| (r, c))
        .collect();
    let (model, mut state) = Model::restore(funded(&model, &world, &inputs)).unwrap();
    let before_total = total_mass(&state.owned_stock_components().copied().collect::<Vec<_>>());
    let exact_before = exact_units(state.owned_stock_components().copied());
    model.advance(&mut state, 1).unwrap();
    let after = state.checkpoint();
    let parent = &after.merged_lake_state.as_ref().unwrap().parents[0];
    assert_eq!(parent.basin_node, group.basin_node);
    close(
        parent.birth_high_kilograms + parent.birth_low_kilograms,
        total_mass(&group.child_capacities_kilograms),
    );
    assert_eq!(parent.surplus_high_kilograms, 0.);
    assert_ne!(
        parent.birth_low_kilograms, 0.,
        "This fixture must expose rounded child-sum loss."
    );
    assert_eq!(
        exact_units([parent.birth_high_kilograms, parent.birth_low_kilograms]),
        exact_units(group.child_capacities_kilograms.iter().copied())
    );
    assert_eq!(
        exact_before,
        exact_units(state.owned_stock_components().copied())
    );
    for &r in &group.child_terminals {
        assert_eq!(after.terminal_water_kilograms[r], 0.);
        assert_eq!(after.terminal_low_kilograms.as_ref().unwrap()[r], 0.);
    }
    close(
        before_total,
        total_mass(&state.owned_stock_components().copied().collect::<Vec<_>>()),
    );
    let surface = &model.merged_lake_surfaces(&state).unwrap()[0];
    assert_eq!(
        surface.absolute_level_meters,
        Some(group.birth_level_meters)
    );
    assert!(
        surface
            .exposed_regions
            .iter()
            .all(|&r| world.terrain.elevation[r] < group.birth_level_meters)
    );
    let json = serde_json::to_string(&after).unwrap();
    let (restored, mut resumed) = Model::restore(serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(resumed.checkpoint(), after);
    model.advance(&mut state, 3600).unwrap();
    restored.advance(&mut resumed, 3600).unwrap();
    assert_eq!(state.checkpoint(), resumed.checkpoint());
}

#[test]
fn surplus_wets_the_connecting_sill_and_single_source_spill_merges() {
    let (world, model, group) = fixture();
    let connections =
        planimulation_core::seasonal_moisture::closed_lake::spill::first_connections(&world)
            .unwrap();
    let source = group.child_terminals.iter().copied().find(|&r| connections.iter().any(|c|
        c.terminal_region == r && c.routes.len() == 1 && matches!(c.routes[0].destination,
            planimulation_core::seasonal_moisture::closed_lake::spill::Destination::ClosedTerminal { terminal_region }
            if group.child_terminals.contains(&terminal_region)))).unwrap();
    let input = total_mass(&group.child_capacities_kilograms)
        + group.surplus_capacity_kilograms.unwrap() * 0.25;
    let (model, mut state) = Model::restore(funded(&model, &world, &[(source, input)])).unwrap();
    let before = exact_units(state.owned_stock_components().copied());
    let step = model.advance(&mut state, 1).unwrap();
    assert!(!step.leaf_spill_events.unwrap().is_empty());
    let cp = state.checkpoint();
    let parent = &cp.merged_lake_state.as_ref().unwrap().parents[0];
    assert!(parent.surplus_high_kilograms > 0.);
    assert_eq!(exact_units(parent_components(parent)), exact_units([input]));
    assert_eq!(before, exact_units(state.owned_stock_components().copied()));
    let surface = &model.merged_lake_surfaces(&state).unwrap()[0];
    assert!(surface.relative_height_above_birth_meters > 0.);
    assert!(
        group
            .regions
            .iter()
            .any(|&r| world.terrain.elevation[r] == group.birth_level_meters
                && surface.exposed_regions.contains(&r))
    );
    let (restored, mut resumed) =
        Model::restore(serde_json::from_str(&serde_json::to_string(&cp).unwrap()).unwrap())
            .unwrap();
    model.advance(&mut state, 3600).unwrap();
    restored.advance(&mut resumed, 3600).unwrap();
    assert_eq!(state.checkpoint(), resumed.checkpoint());
}
fn parent_components(p: &planimulation_core::seasonal_moisture::merged_lake::Parent) -> [f64; 4] {
    [
        p.birth_high_kilograms,
        p.birth_low_kilograms,
        p.surplus_high_kilograms,
        p.surplus_low_kilograms,
    ]
}

fn activated(
    surplus_fraction: f64,
) -> (
    World,
    Model,
    planimulation_core::seasonal_moisture::State,
    Candidate,
) {
    let (world, model, group) = fixture();
    let mut inputs: Vec<_> = group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
        .map(|(&r, &c)| (r, c))
        .collect();
    inputs[0].1 += group.surplus_capacity_kilograms.unwrap() * surplus_fraction;
    let (model, mut state) = Model::restore(funded(&model, &world, &inputs)).unwrap();
    model.advance(&mut state, 1).unwrap();
    (world, model, state, group)
}

#[test]
fn seasonal_evaporation_uses_parent_surface_and_split_refusal_is_atomic() {
    let (world, _, state, group) = activated(0.25);
    let mut cp = state.checkpoint();
    cp.settings.evaporation_enabled = true;
    let (model, mut state) = Model::restore(cp).unwrap();
    let before = state.checkpoint();
    let surface = model.merged_lake_surfaces(&state).unwrap().remove(0);
    let step = model.advance(&mut state, 3600).unwrap();
    let after = state.checkpoint();
    assert!(
        group
            .regions
            .iter()
            .map(|&r| step.runoff_transfers[r].terminal_evaporation)
            .sum::<f64>()
            > 0.
    );
    assert!(
        after.merged_lake_state.as_ref().unwrap().parents[0].surplus_high_kilograms
            < before.merged_lake_state.as_ref().unwrap().parents[0].surplus_high_kilograms
    );
    for &r in &group.regions {
        if !surface.exposed_regions.contains(&r) {
            assert_eq!(step.runoff_transfers[r].terminal_evaporation, 0.);
        }
        assert_eq!(after.terminal_water_kilograms[r], 0.);
    }
    let sill = group
        .regions
        .iter()
        .copied()
        .find(|&r| world.terrain.elevation[r] == group.birth_level_meters)
        .unwrap();
    assert!(step.runoff_transfers[sill].terminal_evaporation > 0.);
    let (_, _, state, _) = activated(0.);
    let mut cp = state.checkpoint();
    cp.settings.evaporation_enabled = true;
    let (model, mut state) = Model::restore(cp).unwrap();
    let before = serde_json::to_string(&state.checkpoint()).unwrap();
    assert!(
        model
            .advance(&mut state, 1)
            .unwrap_err()
            .contains("drying/splitting")
    );
    assert_eq!(before, serde_json::to_string(&state.checkpoint()).unwrap());
}

#[test]
fn rain_on_newly_wet_parent_column_enters_parent_not_land_or_child() {
    let (world, _, state, group) = activated(0.25);
    let mut cp = state.checkpoint();
    cp.settings.precipitation_enabled = true;
    let sill = group
        .regions
        .iter()
        .copied()
        .find(|&r| world.terrain.elevation[r] == group.birth_level_meters)
        .unwrap();
    let normals = planimulation_core::seasonal_temperature::Normals::from_world(
        &world,
        cp.temperature_settings,
    )
    .unwrap();
    let t = normals.monthly_temperature_celsius[0][sill];
    assert!(t > 0.);
    let cloud = 2.
        * planimulation_core::seasonal_moisture::saturation_column_kilograms_per_square_meter(
            t,
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
    let h = cp.reference_body_high_kilograms.as_mut().unwrap();
    let l = cp.reference_body_low_kilograms.as_mut().unwrap();
    let (s, e) = two_sum(h[body], -cloud);
    (h[body], l[body]) = two_sum(s, l[body] + e);
    cp.vapor_kilograms[sill] = cloud;
    cp.cumulative_surface_transfers[contact].liquid_evaporation += cloud;
    cp.cumulative_evaporation_kilograms[contact] += cloud;
    let (model, mut state) = Model::restore(cp).unwrap();
    let before = state.checkpoint();
    let step = model.advance(&mut state, 60).unwrap();
    assert!(step.precipitation_kilograms[sill] > 0.);
    let after = state.checkpoint();
    assert!(after.cumulative_lake_capture_kilograms.as_ref().unwrap()[sill] > 0.);
    assert_eq!(after.surface_kilograms[sill], 0.);
    assert_eq!(after.soil_kilograms[sill], 0.);
    assert!(
        after.merged_lake_state.as_ref().unwrap().parents[0].surplus_high_kilograms
            > before.merged_lake_state.as_ref().unwrap().parents[0].surplus_high_kilograms
    );
    for &r in &group.child_terminals {
        assert_eq!(after.terminal_water_kilograms[r], 0.);
    }
    let (restored, mut resumed) =
        Model::restore(serde_json::from_str(&serde_json::to_string(&after).unwrap()).unwrap())
            .unwrap();
    model.advance(&mut state, 60).unwrap();
    restored.advance(&mut resumed, 60).unwrap();
    assert_eq!(state.checkpoint(), resumed.checkpoint());
}

#[test]
fn underfilled_sibling_does_not_merge_and_next_spill_refuses_atomically() {
    let (world, model, group) = fixture();
    let inputs = [
        (
            group.child_terminals[0],
            group.child_capacities_kilograms[0],
        ),
        (
            group.child_terminals[1],
            group.child_capacities_kilograms[1].next_down(),
        ),
    ];
    let (model, mut state) = Model::restore(funded(&model, &world, &inputs)).unwrap();
    model.advance(&mut state, 1).unwrap();
    assert!(
        state
            .checkpoint()
            .merged_lake_state
            .unwrap()
            .parents
            .is_empty()
    );
    let (world, model, group) = fixture();
    let input = total_mass(&group.child_capacities_kilograms)
        + 2. * group.surplus_capacity_kilograms.unwrap();
    let cp = funded(&model, &world, &[(group.child_terminals[0], input)]);
    let (model, mut state) = Model::restore(cp).unwrap();
    let before = serde_json::to_string(&state.checkpoint()).unwrap();
    let error = model.advance(&mut state, 1).unwrap_err();
    assert!(error.contains("next-parent spill"), "{error}");
    assert_eq!(before, serde_json::to_string(&state.checkpoint()).unwrap());
}

#[test]
fn parent_schema_and_exclusive_ownership_reject_corruption() {
    let (world, model, group) = fixture();
    let inputs: Vec<_> = group
        .child_terminals
        .iter()
        .zip(&group.child_capacities_kilograms)
        .map(|(&r, &c)| (r, c))
        .collect();
    let (model, mut state) = Model::restore(funded(&model, &world, &inputs)).unwrap();
    model.advance(&mut state, 1).unwrap();
    let cp = state.checkpoint();
    let mut bad = cp.clone();
    bad.merged_lake_state.as_mut().unwrap().model_version = "unknown".into();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    let parent = bad.merged_lake_state.as_ref().unwrap().parents[0].clone();
    bad.merged_lake_state.as_mut().unwrap().parents.push(parent);
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.merged_lake_state.as_mut().unwrap().parents[0].birth_low_kilograms = 0.;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.terminal_water_kilograms[group.child_terminals[0]] = 1.;
    bad.merged_lake_state.as_mut().unwrap().parents[0].surplus_high_kilograms = 1.;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.merged_lake_state.as_mut().unwrap().parents[0].basin_node = world.basins.root();
    assert!(Model::restore(bad).is_err());
    for (high, low) in [
        (1., 2.),
        (0., -1.),
        (f64::NAN, 0.),
        (group.surplus_capacity_kilograms.unwrap().next_up(), 0.),
    ] {
        let mut bad = cp.clone();
        let parent = &mut bad.merged_lake_state.as_mut().unwrap().parents[0];
        parent.surplus_high_kilograms = high;
        parent.surplus_low_kilograms = low;
        assert!(Model::restore(bad).is_err());
    }
    let mut json = serde_json::to_value(&cp).unwrap();
    json["mergedLakeState"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(json).is_err());
    let mut json = serde_json::to_value(&cp).unwrap();
    json.as_object_mut().unwrap().remove("mergedLakeState");
    assert!(Model::restore(serde_json::from_value(json).unwrap()).is_err());
    let mut json = serde_json::to_value(&cp).unwrap();
    json["mergedLakeState"]["unexpected"] = true.into();
    assert!(serde_json::from_value::<Checkpoint>(json).is_err());
    let mut json = serde_json::to_value(&cp).unwrap();
    json["mergedLakeState"]["parents"][0]
        .as_object_mut()
        .unwrap()
        .remove("birthLowKilograms");
    assert!(serde_json::from_value::<Checkpoint>(json).is_err());
    assert!(
        planimulation_core::seasonal_moisture::closed_lake::Layout::from_world(&world)
            .unwrap()
            .capture(&model, &state)
            .is_err()
    );
    let before = state.clone();
    assert!(
        model
            .advance_observed(&mut state, 1, |_| panic!(
                "Obsolete surface observer invoked."
            ))
            .is_err()
    );
    assert!(
        model
            .advance_terminal_observed(&mut state, 1, |_| panic!(
                "Obsolete terminal observer invoked."
            ))
            .is_err()
    );
    assert_eq!(state, before);
    let mut bytes = Vec::new();
    assert!(planimulation_core::wire::seasonal_checkpoint(&mut bytes, &model, &state).is_err());
    assert!(
        planimulation_core::wire::seasonal_moisture(&mut bytes, &model, &state, None, 0).is_err()
    );
    assert!(bytes.is_empty());
    assert!(
        planimulation_core::seasonal_moisture::body_preparation::Monitor::new(
            &model,
            &state,
            Default::default()
        )
        .is_err()
    );
}

fn as_model_ten(mut cp: Checkpoint) -> Checkpoint {
    cp.schema_version = 10;
    cp.model_version = planimulation_core::seasonal_moisture::LEAF_SPILL_MODEL_VERSION.into();
    cp.settings.closed_lake_exchange = Some(ClosedLakeExchange::FrozenLeafExposureWithSpill);
    cp.closed_lake_model_version = Some("closed-leaf-exchange-2".into());
    cp.merged_lake_state = None;
    cp
}

#[test]
fn ordinary_generated_runs_preserve_legacy_ten_before_any_merge() {
    for (coverage, limit) in [(0., 900), (1., 900), (0.71, 900), (0.71, 450), (0.3, 900)] {
        let mut recipe: Recipe =
            serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json"))
                .unwrap();
        recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
        let world = World::generate(recipe).unwrap();
        let configuration = |merge| Settings {
            max_coupled_step_seconds: limit,
            evaporation_enabled: true,
            precipitation_enabled: true,
            routing_enabled: true,
            ..settings(merge)
        };
        let ten = Model::from_world(
            &world,
            configuration(false),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let eleven = Model::from_world(
            &world,
            configuration(true),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let mut a = ten.initial_state();
        let mut b = eleven.initial_state();
        for _ in 0..40 {
            ten.advance(&mut a, 86400).unwrap();
            eleven.advance(&mut b, 86400).unwrap();
            assert!(
                b.checkpoint()
                    .merged_lake_state
                    .as_ref()
                    .unwrap()
                    .parents
                    .is_empty()
            );
            assert_eq!(a.checkpoint(), as_model_ten(b.checkpoint()));
        }
        let cp = b.checkpoint();
        let (restored, mut resumed) =
            Model::restore(serde_json::from_str(&serde_json::to_string(&cp).unwrap()).unwrap())
                .unwrap();
        eleven.advance(&mut b, 3600).unwrap();
        restored.advance(&mut resumed, 3600).unwrap();
        assert_eq!(b.checkpoint(), resumed.checkpoint());
    }
}

#[test]
fn aligned_hourly_and_daily_callers_match_for_a_wet_merged_parent() {
    let (_, _, state, _) = activated(0.25);
    let mut cp = state.checkpoint();
    cp.settings.evaporation_enabled = true;
    // Align the synthetic history to the shared coupling grid before comparing
    // partitions. Starting with a one-second partial interval would change it.
    cp.elapsed_seconds = 900;
    let (model, mut daily) = Model::restore(cp.clone()).unwrap();
    let (hourly_model, mut hourly) = Model::restore(cp).unwrap();
    for _ in 0..3 {
        model.advance(&mut daily, 86400).unwrap();
        for _ in 0..24 {
            hourly_model.advance(&mut hourly, 3600).unwrap();
        }
        assert_eq!(daily.checkpoint(), hourly.checkpoint());
    }
}
