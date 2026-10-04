use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics,
        SurfaceNumerics, TerminalNumerics, closed_lake::Layout,
    },
};

fn recipe(coverage: f64) -> Recipe {
    let mut r: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    r.subdivision = 2;
    r.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
    r
}
fn settings() -> Settings {
    Settings {
        initial_active_surface_depth_meters: 10.,
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(ClosedLakeExchange::FrozenLeafExposure),
        max_coupled_step_seconds: 900,
        ..Default::default()
    }
}
fn build(settings: Settings) -> (World, Model) {
    let w = World::generate(recipe(0.71)).unwrap();
    let m = Model::from_world(&w, settings, Default::default(), Default::default()).unwrap();
    (w, m)
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    (s, (a - (s - v)) + (b - v))
}
// A ledger-balanced directed input, not a claimed generated trajectory.
// Fund deposited rain/vapor from an actual reference body, never deep water.
fn funded(model: &Model, world: &World, terminal: usize, mass: f64, vapor: f64) -> Checkpoint {
    let mut cp = model.initial_state().checkpoint();
    cp.elapsed_seconds = 1;
    let contact = world.water.body_ids.iter().position(|&id| id > 0).unwrap();
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let b = ids
        .iter()
        .position(|&id| id == world.water.body_ids[contact])
        .unwrap();
    let supply = mass + vapor;
    let high = cp.reference_body_high_kilograms.as_mut().unwrap();
    let low = cp.reference_body_low_kilograms.as_mut().unwrap();
    let (s, e) = two_sum(high[b], -supply);
    (high[b], low[b]) = two_sum(s, low[b] + e);
    assert!(high[b] > 0.);
    cp.cumulative_surface_transfers[contact].liquid_evaporation = supply;
    cp.cumulative_evaporation_kilograms[contact] = supply;
    cp.cumulative_surface_transfers[terminal].rain = mass;
    cp.cumulative_precipitation_kilograms[terminal] = mass;
    cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[terminal] = mass;
    cp.terminal_water_kilograms[terminal] = mass;
    cp.vapor_kilograms[terminal] = vapor;
    cp
}

#[test]
fn generated_exchange_has_independent_lake_identities_and_exact_replay() {
    let (world, model) = build(settings());
    let layout = Layout::from_world(&world).unwrap();
    let mut state = model.initial_state();
    let mut hourly = state.clone();
    for _ in 0..2 {
        model.advance(&mut state, 86400).unwrap();
        for _ in 0..24 {
            model.advance(&mut hourly, 3600).unwrap();
        }
    }
    assert_eq!(state, hourly);
    for _ in 2..40 {
        model.advance(&mut state, 86400).unwrap();
    }
    let cp = state.checkpoint();
    assert_eq!(cp.schema_version, 9);
    assert_eq!(cp.model_version, "seasonal-moisture-9");
    assert!(total_mass(cp.cumulative_lake_capture_kilograms.as_ref().unwrap()) > 0.);
    for lake in layout.lakes() {
        let r = lake.terminal_region();
        let capture = total_mass(
            &lake
                .regions()
                .iter()
                .flat_map(|&i| {
                    [
                        cp.cumulative_lake_capture_kilograms.as_ref().unwrap()[i],
                        cp.cumulative_lake_capture_low_kilograms.as_ref().unwrap()[i],
                    ]
                })
                .collect::<Vec<_>>(),
        );
        let e = lake
            .regions()
            .iter()
            .map(|&i| cp.cumulative_runoff_transfers[i].terminal_evaporation)
            .sum::<f64>();
        close(
            cp.terminal_water_kilograms[r] + cp.terminal_low_kilograms.as_ref().unwrap()[r] + e,
            cp.cumulative_runoff_transfers[r].terminal_delivery + capture,
        );
    }
    let budget = model.budget(&state).unwrap();
    close(
        total_mass(&state.owned_stock_components().copied().collect::<Vec<_>>()),
        budget.initial_mobile_water_kilograms,
    );
    let text = serde_json::to_string(&cp).unwrap();
    let (resumed_model, mut resumed) =
        Model::restore(serde_json::from_str(&text).unwrap()).unwrap();
    assert_eq!(text, serde_json::to_string(&resumed.checkpoint()).unwrap());
    model.advance(&mut state, 3600).unwrap();
    resumed_model.advance(&mut resumed, 3600).unwrap();
    assert_eq!(state, resumed);
    assert!(
        layout
            .capture(&model, &state)
            .unwrap()
            .iter()
            .all(|o| o.surface_failure.is_none())
    );
}

#[test]
fn submerged_nonterminal_keeps_soil_and_routes_owned_transit_without_double_evaporation() {
    let (world, model) = build(settings());
    let layout = Layout::from_world(&world).unwrap();
    let lake = layout
        .lakes()
        .iter()
        .find(|l| l.regions().len() > 1)
        .unwrap();
    let r = lake.terminal_region();
    let capacity = lake.capacity_cubic_meters().unwrap() * 1000.;
    let i = *lake.regions().iter().find(|&&i| i != r).unwrap();
    let mut cp = funded(&model, &world, r, capacity * 0.9, 0.);
    let soil = world.surface.areas[r] * 10.;
    let liquid = world.surface.areas[r] * 0.01;
    let transit = world.surface.areas[i] * 0.01;
    // Repartition deposited rain: preserve a submerged soil donor and liquid.
    cp.terminal_water_kilograms[r] -= soil + liquid + transit;
    cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] -= soil + liquid + transit;
    cp.soil_kilograms[r] = soil;
    cp.surface_kilograms[r] = liquid;
    cp.cumulative_surface_transfers[r].infiltration = soil;
    cp.cumulative_surface_transfers[r].rain -= transit;
    cp.cumulative_precipitation_kilograms[r] -= transit;
    cp.cumulative_surface_transfers[i].rain = transit;
    cp.cumulative_surface_transfers[i].liquid_runoff = transit;
    cp.cumulative_precipitation_kilograms[i] = transit;
    cp.pending_runoff_kilograms[i] = transit;
    let soil_tail = 2_f64.powi(-20);
    let liquid_tail = -2_f64.powi(-22);
    cp.soil_low_kilograms.as_mut().unwrap()[r] = soil_tail;
    cp.surface_low_kilograms.as_mut().unwrap()[r] = liquid_tail;
    cp.terminal_low_kilograms.as_mut().unwrap()[r] = -soil_tail - liquid_tail;
    cp.cumulative_lake_capture_low_kilograms.as_mut().unwrap()[r] = -soil_tail - liquid_tail;
    let (model, mut state) = Model::restore(cp).unwrap();
    let before = state.checkpoint();
    assert!(
        layout
            .capture(&model, &state)
            .unwrap()
            .iter()
            .find(|o| o.terminal_region == r)
            .unwrap()
            .surface
            .as_ref()
            .unwrap()
            .exposed_regions
            .contains(&i)
    );
    let step = model.advance(&mut state, 900).unwrap();
    let after = state.checkpoint();
    assert_eq!(after.soil_kilograms[r], before.soil_kilograms[r]);
    assert_eq!(
        after.soil_low_kilograms.as_ref().unwrap()[r],
        before.soil_low_kilograms.as_ref().unwrap()[r]
    );
    assert_eq!(step.surface_transfers[r].soil_evaporation, 0.);
    assert_eq!(step.surface_transfers[r].infiltration, 0.);
    assert_eq!(step.surface_transfers[r].liquid_runoff, 0.);
    assert_eq!(after.surface_kilograms[r], 0.);
    assert_eq!(after.surface_low_kilograms.as_ref().unwrap()[r], 0.);
    assert!(step.runoff_transfers[i].terminal_evaporation > 0.);
    assert_eq!(after.terminal_water_kilograms[i], 0.);
    assert_eq!(step.surface_transfers[i].liquid_evaporation, 0.);
    assert!(step.runoff_transfers[i].sent > 0.);
    assert!(after.pending_runoff_kilograms[i] < transit);
    close(
        step.evaporation_kilograms[i],
        step.runoff_transfers[i].terminal_evaporation,
    );
    assert!(
        after.cumulative_lake_capture_kilograms.as_ref().unwrap()[r]
            > before.cumulative_lake_capture_kilograms.as_ref().unwrap()[r]
    );
}

#[test]
fn crossing_first_connection_rolls_back_all_stocks_clock_and_ledgers() {
    let (world, model) = build(settings());
    let layout = Layout::from_world(&world).unwrap();
    let lake = &layout.lakes()[0];
    let r = lake.terminal_region();
    let capacity = lake.capacity_cubic_meters().unwrap() * 1000.;
    let normals = planimulation_core::seasonal_temperature::Normals::from_world(
        &world,
        model.temperature_settings(),
    )
    .unwrap();
    let vapor = planimulation_core::seasonal_moisture::saturation_column_kilograms_per_square_meter(
        normals.monthly_temperature_celsius[0][r],
        settings().effective_vapor_depth_meters,
    )
    .unwrap()
        * world.surface.areas[r] * 2.;
    let cp = funded(&model, &world, r, capacity.next_down(), vapor);
    let (model, mut state) = Model::restore(cp).unwrap();
    let before = serde_json::to_string(&state.checkpoint()).unwrap();
    let error = model.advance(&mut state, 3600).unwrap_err();
    assert!(error.contains("Coupled lake"), "{error}");
    assert_eq!(serde_json::to_string(&state.checkpoint()).unwrap(), before);
    let cp = funded(&model, &world, r, capacity, 0.);
    assert!(Model::restore(cp).is_err());
}

#[test]
fn cold_deposition_and_warm_snowmelt_keep_separate_owners_and_respect_disabled_evaporation() {
    for temperature in [-20., 20.] {
        let world = World::generate(recipe(0.71)).unwrap();
        let model = Model::from_world(
            &world,
            Settings {
                evaporation_enabled: false,
                ..settings()
            },
            planimulation_core::seasonal_temperature::Settings {
                reference_temperature_celsius: temperature,
                sensitivity_celsius_per_watt_per_square_meter: 0.,
                lapse_rate_celsius_per_meter: 0.,
                ..Default::default()
            },
            Default::default(),
        )
        .unwrap();
        let layout = Layout::from_world(&world).unwrap();
        let lake = &layout.lakes()[0];
        let r = lake.terminal_region();
        let mass = world.surface.areas[r] * 100.;
        let vapor =
            planimulation_core::seasonal_moisture::saturation_column_kilograms_per_square_meter(
                temperature,
                settings().effective_vapor_depth_meters,
            )
            .unwrap()
                * world.surface.areas[r]
                * 2.;
        let mut cp = funded(&model, &world, r, mass, vapor);
        cp.terminal_water_kilograms[r] = mass * 0.5;
        cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r] = mass * 0.5;
        cp.cumulative_surface_transfers[r].rain = mass * 0.5;
        cp.cumulative_surface_transfers[r].snowfall = mass * 0.5;
        cp.snow_kilograms[r] = mass * 0.5;
        let (model, mut state) = Model::restore(cp).unwrap();
        let before = state.checkpoint();
        let step = model.advance(&mut state, 900).unwrap();
        let after = state.checkpoint();
        assert_eq!(step.evaporation_kilograms.iter().sum::<f64>(), 0.);
        assert_eq!(step.runoff_transfers[r].terminal_evaporation, 0.);
        if temperature < 0. {
            assert!(step.surface_transfers[r].snowfall > 0.);
            assert_eq!(step.surface_transfers[r].melt, 0.);
            assert_eq!(
                after.terminal_water_kilograms[r],
                before.terminal_water_kilograms[r]
            );
            assert!(after.snow_kilograms[r] > before.snow_kilograms[r]);
        } else {
            assert!(step.surface_transfers[r].melt > 0.);
            assert!(after.terminal_water_kilograms[r] > before.terminal_water_kilograms[r]);
            assert_eq!(step.surface_transfers[r].infiltration, 0.);
            assert!(after.snow_kilograms[r] < before.snow_kilograms[r]);
        }
    }
}

#[test]
fn pins_missing_null_shapes_and_globally_balanced_capture_forgery_refuse() {
    let (world, model) = build(settings());
    let layout = Layout::from_world(&world).unwrap();
    let lake = &layout.lakes()[0];
    let cp = funded(
        &model,
        &world,
        lake.terminal_region(),
        lake.capacity_cubic_meters().unwrap() * 100.,
        0.,
    );
    Model::restore(cp.clone()).unwrap();
    assert!(layout.lakes().len() > 1);
    let mut balanced = cp.clone();
    let r = lake.terminal_region();
    let other = layout.lakes()[1].terminal_region();
    let transfer = cp.terminal_water_kilograms[r] * 0.125;
    balanced.terminal_water_kilograms[r] -= transfer;
    balanced.terminal_water_kilograms[other] = transfer;
    let error = Model::restore(balanced).err().unwrap();
    assert!(error.contains("ledger 10"), "{error}");
    let mut bad = cp.clone();
    bad.closed_lake_model_version = None;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.cumulative_lake_capture_low_kilograms
        .as_mut()
        .unwrap()
        .pop();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.schema_version = 8;
    bad.model_version = "seasonal-moisture-8".into();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    let capture = bad.cumulative_lake_capture_kilograms.as_mut().unwrap();
    let i = capture.iter().position(|&c| c > 0.).unwrap();
    capture[i] *= 0.5;
    bad.cumulative_lake_capture_low_kilograms.as_mut().unwrap()[i] = 0.;
    assert!(Model::restore(bad).is_err()); // This changes no globally owned stock.
    for field in [
        "closedLakeModelVersion",
        "cumulativeLakeCaptureKilograms",
        "cumulativeLakeCaptureLowKilograms",
    ] {
        let mut v = serde_json::to_value(&cp).unwrap();
        v[field] = serde_json::Value::Null;
        assert!(serde_json::from_value::<Checkpoint>(v).is_err());
    }
    let mut v = serde_json::to_value(settings()).unwrap();
    v["closedLakeExchange"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Settings>(v).is_err());
    assert!(
        Settings {
            reference_water_pool: None,
            ..settings()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn no_lake_reference_preserves_complete_version_eight_physics() {
    let world = World::generate(recipe(1.)).unwrap();
    let legacy = Model::from_world(
        &world,
        Settings {
            closed_lake_exchange: None,
            ..settings()
        },
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let model =
        Model::from_world(&world, settings(), Default::default(), Default::default()).unwrap();
    let mut a = legacy.initial_state();
    let mut b = model.initial_state();
    for _ in 0..2 {
        legacy.advance(&mut a, 86400).unwrap();
        model.advance(&mut b, 86400).unwrap();
    }
    let mut cp = b.checkpoint();
    cp.schema_version = 8;
    cp.model_version = "seasonal-moisture-8".into();
    cp.settings.closed_lake_exchange = None;
    cp.closed_lake_model_version = None;
    cp.cumulative_lake_capture_kilograms = None;
    cp.cumulative_lake_capture_low_kilograms = None;
    assert_eq!(cp, a.checkpoint());
    let dry = World::generate(recipe(0.)).unwrap();
    let model =
        Model::from_world(&dry, settings(), Default::default(), Default::default()).unwrap();
    let mut state = model.initial_state();
    model.advance(&mut state, 86400).unwrap();
    assert!(state.owned_stock_components().all(|&v| v == 0.));
}

#[test]
fn unsupported_observers_diagnostics_and_desktop_writes_refuse_before_mutation() {
    let (_, model) = build(settings());
    let mut state = model.initial_state();
    let before = state.clone();
    assert!(
        model
            .advance_observed(&mut state, 3600, |_| panic!("obsolete observer"))
            .is_err()
    );
    assert!(
        model
            .advance_terminal_observed(&mut state, 3600, |_| panic!("obsolete observer"))
            .is_err()
    );
    assert_eq!(state, before);
    let mut bytes = Vec::new();
    assert!(planimulation_core::wire::seasonal_checkpoint(&mut bytes, &model, &state).is_err());
    assert!(bytes.is_empty());
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

#[test]
fn stale_hierarchy_or_drainage_cannot_be_used_as_coupled_geometry() {
    let (mut world, _) = build(settings());
    world.terrain.elevation[0] += 1.;
    assert!(Model::from_world(&world, settings(), Default::default(), Default::default()).is_err());
    let (mut world, _) = build(settings());
    world.drainage.contributing_area[0] += 1.;
    assert!(Model::from_world(&world, settings(), Default::default(), Default::default()).is_err());
}
