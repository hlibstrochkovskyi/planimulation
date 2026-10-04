use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics,
        SurfaceNumerics, TerminalNumerics, closed_lake::Layout,
    },
};

fn recipe(coverage: f64) -> Recipe {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.subdivision = 2;
    recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
    recipe
}
fn settings() -> Settings {
    Settings {
        initial_active_surface_depth_meters: 10.,
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(ClosedLakeExchange::FrozenLeafExposureWithSpill),
        max_coupled_step_seconds: 900,
        ..Default::default()
    }
}
fn build() -> (World, Model, f64, f64) {
    let world = World::generate(recipe(0.71)).unwrap();
    let model = Model::from_world(
        &world,
        Settings {
            evaporation_enabled: false,
            precipitation_enabled: false,
            routing_enabled: false,
            ..settings()
        },
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let layout = Layout::from_world(&world).unwrap();
    let capacity = |r| {
        layout
            .lakes()
            .iter()
            .find(|l| l.terminal_region() == r)
            .unwrap()
            .capacity_cubic_meters()
            .unwrap()
            * 1000.
    };
    let (a, b) = (capacity(91), capacity(73));
    (world, model, a, b)
}
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    (s, (a - (s - v)) + (b - v))
}
// Fund a directed, ledger-balanced input queue from mobile reference water.
// This is not a claim that the ordinary climate generated this history.
fn funded(model: &Model, world: &World, inputs: &[(usize, f64)]) -> Checkpoint {
    let mut cp = model.initial_state().checkpoint();
    cp.elapsed_seconds = 1;
    let contact = world.water.body_ids.iter().position(|&id| id > 0).unwrap();
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let body = ids
        .iter()
        .position(|&id| id == world.water.body_ids[contact])
        .unwrap();
    let supply = total_mass(&inputs.iter().map(|&(_, mass)| mass).collect::<Vec<_>>());
    let high = cp.reference_body_high_kilograms.as_mut().unwrap();
    let low = cp.reference_body_low_kilograms.as_mut().unwrap();
    let (s, e) = two_sum(high[body], -supply);
    (high[body], low[body]) = two_sum(s, low[body] + e);
    assert!(high[body] > 0.);
    cp.cumulative_surface_transfers[contact].liquid_evaporation = supply;
    cp.cumulative_evaporation_kilograms[contact] = supply;
    for &(r, mass) in inputs {
        cp.cumulative_surface_transfers[r].rain = mass;
        cp.cumulative_precipitation_kilograms[r] = mass;
        cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] = mass;
        cp.leaf_spill_state
            .as_mut()
            .unwrap()
            .pending_input
            .high_kilograms[r] = mass;
    }
    cp
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
// These directed quiet fixtures use integral binary64 kilograms within i128.
// Unlike a rounded global budget, this independently sums every component.
fn exact_integral_components(values: impl IntoIterator<Item = f64>) -> i128 {
    values
        .into_iter()
        .map(|v| {
            assert!(v.abs() < 2_f64.powi(126));
            let integer = v as i128;
            assert_eq!(integer as f64, v);
            integer
        })
        .sum()
}
fn owned(model: &Model, state: &planimulation_core::seasonal_moisture::State) {
    let budget = model.budget(state).unwrap();
    close(
        total_mass(&state.owned_stock_components().copied().collect::<Vec<_>>()),
        budget.initial_mobile_water_kilograms,
    );
}

#[test]
fn spill_fills_first_unfilled_lake_instead_of_skipping_it() {
    let (world, model, cap91, cap73) = build();
    let input = cap91 + cap73 * 0.25;
    let cp = funded(&model, &world, &[(91, input)]);
    let (model, mut state) = Model::restore(cp).unwrap();
    owned(&model, &state); // The unapplied queue is already owned inventory.
    let body_before = state.reference_body_high_kilograms().unwrap().to_vec();
    let step = model.advance(&mut state, 1).unwrap();
    let after = state.checkpoint();
    assert_eq!(after.schema_version, 10);
    assert_eq!(after.terminal_water_kilograms[91], cap91);
    assert_eq!(after.terminal_low_kilograms.as_ref().unwrap()[91], 0.);
    close(after.terminal_water_kilograms[73], input - cap91);
    assert_eq!(
        after.reference_body_high_kilograms.as_ref().unwrap(),
        &body_before
    );
    let ledger = after.leaf_spill_state.as_ref().unwrap();
    assert!(
        ledger
            .pending_input
            .high_kilograms
            .iter()
            .chain(&ledger.pending_input.low_kilograms)
            .all(|&v| v == 0.)
    );
    assert_eq!(ledger.cumulative_outgoing.high_kilograms[73], 0.);
    let events = step.leaf_spill_events.unwrap();
    assert!(!events.is_empty());
    assert!(events.iter().all(|e| e.source_terminal_region == 91 && e.route.destination == planimulation_core::seasonal_moisture::closed_lake::spill::Destination::ClosedTerminal { terminal_region: 73 }));
    owned(&model, &state);
    assert!(
        Layout::from_world(&world)
            .unwrap()
            .capture(&model, &state)
            .unwrap()
            .iter()
            .all(|o| o.surface_failure.is_none())
    );
}

#[test]
fn full_lower_leaf_passes_actual_water_to_reference_contact_with_exact_replay() {
    let (world, model, cap91, cap73) = build();
    let input = cap91 + cap73 + cap73 * 0.125;
    let cp = funded(&model, &world, &[(91, input)]);
    let (model, mut state) = Model::restore(cp).unwrap();
    let before = state.checkpoint();
    let exact_before = exact_integral_components(state.owned_stock_components().copied());
    let step = model.advance(&mut state, 1).unwrap();
    let cp = state.checkpoint();
    assert_eq!(cp.terminal_water_kilograms[91], cap91);
    assert_eq!(cp.terminal_water_kilograms[73], cap73);
    let ledger = cp.leaf_spill_state.as_ref().unwrap();
    assert_eq!(
        exact_integral_components(state.owned_stock_components().copied()),
        exact_before
    );
    assert_eq!(
        exact_integral_components([
            ledger.cumulative_incoming.high_kilograms[73],
            ledger.cumulative_incoming.low_kilograms[73],
            -ledger.cumulative_outgoing.high_kilograms[73],
            -ledger.cumulative_outgoing.low_kilograms[73]
        ]),
        cap73 as i128
    );
    assert_eq!(
        exact_integral_components([
            ledger.cumulative_incoming.high_kilograms[66],
            ledger.cumulative_incoming.low_kilograms[66],
            cap91,
            cap73
        ]),
        input as i128
    );
    close(ledger.cumulative_outgoing.high_kilograms[91], input - cap91);
    close(ledger.cumulative_incoming.high_kilograms[73], input - cap91);
    close(
        ledger.cumulative_outgoing.high_kilograms[73],
        input - cap91 - cap73,
    );
    close(
        ledger.cumulative_incoming.high_kilograms[66],
        input - cap91 - cap73,
    );
    assert!(
        cp.reference_body_high_kilograms.as_ref().unwrap()[0]
            > before.reference_body_high_kilograms.as_ref().unwrap()[0]
    );
    let events = step.leaf_spill_events.unwrap();
    assert!(events.iter().any(|e| e.source_terminal_region == 73 && e.route.downhill_regions.last() == Some(&66)));
    owned(&model, &state);
    let text = serde_json::to_string(&cp).unwrap();
    let (resumed_model, mut resumed) =
        Model::restore(serde_json::from_str(&text).unwrap()).unwrap();
    assert_eq!(text, serde_json::to_string(&resumed.checkpoint()).unwrap());
    model.advance(&mut state, 3600).unwrap();
    resumed_model.advance(&mut resumed, 3600).unwrap();
    assert_eq!(state, resumed);
}

#[test]
fn below_threshold_and_negative_low_tail_do_not_authorize_spill() {
    let (world, model, cap91, _) = build();
    let mut cp = funded(&model, &world, &[(91, cap91)]);
    let tail = -1.;
    cp.leaf_spill_state
        .as_mut()
        .unwrap()
        .pending_input
        .low_kilograms[91] = tail;
    cp.cumulative_lake_capture_low_kilograms.as_mut().unwrap()[91] = tail;
    // A tiny physical stock adjustment remains within the independent ledgers'
    // declared tolerance. It is not snapped away by threshold selection.
    let (model, mut state) = Model::restore(cp).unwrap();
    let json = serde_json::to_string(&state.checkpoint()).unwrap();
    let (resumed_model, mut resumed) =
        Model::restore(serde_json::from_str(&json).unwrap()).unwrap();
    assert_eq!(state, resumed); // The negative tail is still in the owned queue.
    let step = model.advance(&mut state, 1).unwrap();
    assert!(step.leaf_spill_events.unwrap().is_empty());
    resumed_model.advance(&mut resumed, 1).unwrap();
    assert_eq!(state, resumed);
    assert_eq!(state.terminal_water_kilograms()[91], cap91);
    assert_eq!(state.terminal_low_kilograms().unwrap()[91], tail);
    assert_eq!(state.terminal_water_kilograms()[73], 0.);
    owned(&model, &state);
}

#[test]
fn concurrent_overflows_refuse_without_changing_the_complete_checkpoint() {
    let (world, model, cap91, cap73) = build();
    let cp = funded(&model, &world, &[(91, cap91 * 1.1), (73, cap73 * 1.1)]);
    let (model, mut state) = Model::restore(cp).unwrap();
    let before = serde_json::to_string(&state.checkpoint()).unwrap();
    let error = model.advance(&mut state, 3600).unwrap_err();
    assert!(error.contains("Concurrent leaf spill"), "{error}");
    assert_eq!(serde_json::to_string(&state.checkpoint()).unwrap(), before);
}

#[test]
fn strict_mode_shapes_owners_and_edge_provenance_reject_forgery() {
    let (world, model, cap91, cap73) = build();
    let cp = funded(&model, &world, &[(91, cap91 + cap73 * 1.2)]);
    let mut relocated = cp.clone();
    relocated
        .leaf_spill_state
        .as_mut()
        .unwrap()
        .pending_input
        .high_kilograms
        .swap(91, 73);
    assert!(Model::restore(relocated).is_err()); // Globally balanced, locally stolen input.
    let (model, mut state) = Model::restore(cp).unwrap();
    model.advance(&mut state, 1).unwrap();
    let cp = state.checkpoint();
    let mut bad = cp.clone();
    bad.leaf_spill_state.as_mut().unwrap().model_version = "wrong".into();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.leaf_spill_state = None;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.leaf_spill_state
        .as_mut()
        .unwrap()
        .pending_input
        .low_kilograms
        .pop();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.leaf_spill_state
        .as_mut()
        .unwrap()
        .pending_input
        .high_kilograms[0] = 1.;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.cumulative_runoff_transfers.clear();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    let incoming = &mut bad.leaf_spill_state.as_mut().unwrap().cumulative_incoming;
    incoming.high_kilograms.swap(66, 73);
    incoming.low_kilograms.swap(66, 73);
    assert!(Model::restore(bad).is_err()); // Same gross total, wrong actual contacts.
    let mut json = serde_json::to_value(&cp).unwrap();
    json["leafSpillState"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(json).is_err());
    let mut bad = cp;
    bad.schema_version = 9;
    bad.model_version = "seasonal-moisture-9".into();
    assert!(Model::restore(bad).is_err());
}

#[test]
fn generated_exchange_has_owned_queues_budgets_and_hourly_daily_replay() {
    let world = World::generate(recipe(0.71)).unwrap();
    let model =
        Model::from_world(&world, settings(), Default::default(), Default::default()).unwrap();
    let mut daily = model.initial_state();
    let mut hourly = daily.clone();
    for _ in 0..2 {
        model.advance(&mut daily, 86400).unwrap();
        for _ in 0..24 {
            model.advance(&mut hourly, 3600).unwrap();
        }
    }
    assert_eq!(daily, hourly);
    for _ in 2..40 {
        model.advance(&mut daily, 86400).unwrap();
    }
    owned(&model, &daily);
    let cp = daily.checkpoint();
    assert!(
        cp.cumulative_lake_capture_kilograms
            .as_ref()
            .unwrap()
            .iter()
            .any(|&x| x > 0.)
    );
    assert_eq!(
        model.budget(&daily).unwrap().pending_lake_input_kilograms,
        Some(0.)
    );
    let (resumed_model, mut resumed) =
        Model::restore(serde_json::from_str(&serde_json::to_string(&cp).unwrap()).unwrap())
            .unwrap();
    model.advance(&mut daily, 3600).unwrap();
    resumed_model.advance(&mut resumed, 3600).unwrap();
    assert_eq!(daily, resumed);
}

#[test]
fn actual_rain_and_delayed_runoff_crossings_are_applied_in_the_seasonal_operator() {
    for rain in [true, false] {
        let world = World::generate(recipe(0.71)).unwrap();
        let model = Model::from_world(
            &world,
            Settings {
                evaporation_enabled: false,
                precipitation_enabled: rain,
                ..settings()
            },
            planimulation_core::seasonal_temperature::Settings {
                reference_temperature_celsius: 20.,
                sensitivity_celsius_per_watt_per_square_meter: 0.,
                lapse_rate_celsius_per_meter: 0.,
                ..Default::default()
            },
            Default::default(),
        )
        .unwrap();
        let geometry = Layout::from_world(&world).unwrap();
        let cap = geometry
            .lakes()
            .iter()
            .find(|l| l.terminal_region() == 91)
            .unwrap()
            .capacity_cubic_meters()
            .unwrap()
            * 1000.;
        let liquid = cap.next_down();
        let input = if rain {
            planimulation_core::seasonal_moisture::saturation_column_kilograms_per_square_meter(
                20.,
                settings().effective_vapor_depth_meters,
            )
            .unwrap()
                * world.surface.areas[91]
                * 2.
        } else {
            world.surface.areas[91]
        };
        let mut cp = funded(&model, &world, &[(91, liquid + input)]);
        cp.terminal_water_kilograms[91] = liquid;
        cp.leaf_spill_state
            .as_mut()
            .unwrap()
            .pending_input
            .high_kilograms[91] = 0.;
        cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[91] = liquid;
        cp.cumulative_surface_transfers[91].rain = liquid;
        cp.cumulative_precipitation_kilograms[91] = liquid;
        if rain {
            cp.vapor_kilograms[91] = input;
        } else {
            let i = world
                .drainage
                .receivers
                .iter()
                .enumerate()
                .find(|&(i, &r)| i != 91 && r == 91)
                .unwrap()
                .0;
            assert_eq!(world.water.body_ids[i], 0);
            cp.cumulative_surface_transfers[i].rain = input;
            cp.cumulative_surface_transfers[i].liquid_runoff = input;
            cp.cumulative_precipitation_kilograms[i] = input;
            cp.pending_runoff_kilograms[i] = input;
        }
        let (model, mut state) = Model::restore(cp).unwrap();
        let step = model.advance(&mut state, 900).unwrap();
        assert!(!step.leaf_spill_events.unwrap().is_empty());
        assert_eq!(state.terminal_water_kilograms()[91], cap);
        assert!(state.terminal_water_kilograms()[73] > 0.);
        if rain {
            assert!(step.precipitation_kilograms[91] > 0.);
        } else {
            assert!(step.runoff_transfers[91].terminal_delivery > 0.);
        }
        owned(&model, &state);
    }
}

#[test]
fn dry_wet_limits_and_obsolete_desktop_contracts_do_not_invent_owners() {
    for coverage in [0., 1.] {
        let world = World::generate(recipe(coverage)).unwrap();
        let model =
            Model::from_world(&world, settings(), Default::default(), Default::default()).unwrap();
        let mut state = model.initial_state();
        model.advance(&mut state, 86400).unwrap();
        if coverage == 0. {
            assert!(state.owned_stock_components().all(|&v| v == 0.));
        } else {
            let old = Model::from_world(
                &world,
                Settings {
                    closed_lake_exchange: Some(ClosedLakeExchange::FrozenLeafExposure),
                    ..settings()
                },
                Default::default(),
                Default::default(),
            )
            .unwrap();
            let mut old_state = old.initial_state();
            old.advance(&mut old_state, 86400).unwrap();
            let mut cp = state.checkpoint();
            cp.schema_version = 9;
            cp.model_version = "seasonal-moisture-9".into();
            cp.closed_lake_model_version = Some("closed-leaf-exchange-1".into());
            cp.settings.closed_lake_exchange = Some(ClosedLakeExchange::FrozenLeafExposure);
            cp.leaf_spill_state = None;
            assert_eq!(cp, old_state.checkpoint());
        }
        let before = state.clone();
        assert!(
            model
                .advance_observed(&mut state, 3600, |_| panic!("obsolete local observer"))
                .is_err()
        );
        assert!(
            model
                .advance_terminal_observed(&mut state, 3600, |_| panic!(
                    "obsolete terminal observer"
                ))
                .is_err()
        );
        assert_eq!(state, before);
        let mut bytes = Vec::new();
        assert!(planimulation_core::wire::seasonal_checkpoint(&mut bytes, &model, &state).is_err());
        assert!(bytes.is_empty());
        assert!(
            planimulation_core::wire::seasonal_moisture(&mut bytes, &model, &state, None, 0)
                .is_err()
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
}
