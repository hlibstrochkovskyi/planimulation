use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Checkpoint, Model, ReferenceWaterPool, Settings, SoilNumerics, SurfaceNumerics,
        TerminalNumerics,
    },
    wire,
};

fn recipe() -> Recipe {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.subdivision = 1;
    recipe.radius_meters = 1_000_000.;
    recipe
}
fn settings(pooled: bool) -> Settings {
    Settings {
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: pooled.then_some(ReferenceWaterPool::FastConnectedBody),
        max_coupled_step_seconds: 900,
        ..Default::default()
    }
}
fn build(recipe: Recipe, settings: Settings) -> (World, Model) {
    let world = World::generate(recipe).unwrap();
    let model =
        Model::from_world(&world, settings, Default::default(), Default::default()).unwrap();
    (world, model)
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}
fn assert_body_identities(world: &World, initial: &Checkpoint, cp: &Checkpoint) {
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|id| *id > 0);
    assert_eq!(
        cp.reference_body_high_kilograms.as_ref().unwrap().len(),
        ids.len()
    );
    for (b, id) in ids.iter().enumerate() {
        let mut input = 0.;
        let mut output = 0.;
        for (i, &actual) in world.water.body_ids.iter().enumerate() {
            if actual == *id {
                assert_eq!(cp.surface_kilograms[i], 0.);
                assert_eq!(cp.surface_low_kilograms.as_ref().unwrap()[i], 0.);
                assert_eq!(cp.terminal_water_kilograms[i], 0.);
                assert_eq!(cp.terminal_low_kilograms.as_ref().unwrap()[i], 0.);
                let f = cp.cumulative_surface_transfers[i];
                input += f.rain + f.melt + cp.cumulative_runoff_transfers[i].terminal_delivery;
                output += f.liquid_evaporation;
                assert_eq!(cp.cumulative_runoff_transfers[i].terminal_evaporation, 0.);
            }
        }
        let start = initial.reference_body_high_kilograms.as_ref().unwrap()[b]
            + initial.reference_body_low_kilograms.as_ref().unwrap()[b];
        let end = cp.reference_body_high_kilograms.as_ref().unwrap()[b]
            + cp.reference_body_low_kilograms.as_ref().unwrap()[b];
        close(end, start + input - output);
    }
}

#[test]
fn finite_body_inventory_replaces_initial_regional_owners_and_retains_local_provenance() {
    let (world, model) = build(recipe(), settings(true));
    let initial = model.initial_state().checkpoint();
    assert_eq!(initial.schema_version, 8);
    assert_eq!(initial.model_version, "seasonal-moisture-8");
    let (_, legacy) = build(recipe(), settings(false));
    close(
        model
            .budget(&model.initial_state())
            .unwrap()
            .initial_mobile_water_kilograms,
        legacy
            .budget(&legacy.initial_state())
            .unwrap()
            .initial_mobile_water_kilograms,
    );
    let mut state = model.initial_state();
    for _ in 0..32 {
        let step = model.advance(&mut state, 86400).unwrap();
        let budget = &step.budget;
        close(
            budget.reference_body_water_kilograms.unwrap()
                + budget.surface_kilograms
                + budget.snow_kilograms
                + budget.soil_kilograms
                + budget.pending_runoff_kilograms
                + budget.terminal_water_kilograms
                + budget.vapor_kilograms,
            budget.initial_mobile_water_kilograms,
        );
        assert_body_identities(&world, &initial, &state.checkpoint());
    }
    let cp = state.checkpoint();
    assert!(
        cp.cumulative_runoff_transfers
            .iter()
            .any(|r| r.terminal_delivery > 0.)
    );
    assert!(cp.cumulative_surface_transfers.iter().any(|f| f.rain > 0.));
    assert!(
        cp.reference_body_low_kilograms
            .as_ref()
            .unwrap()
            .iter()
            .any(|v| *v != 0.)
    );
}

#[test]
fn complete_replay_and_aligned_caller_batching_include_body_components() {
    let (_, model) = build(recipe(), settings(true));
    let mut daily = model.initial_state();
    let mut hourly = daily.clone();
    for _ in 0..4 {
        model.advance(&mut daily, 86400).unwrap();
        for _ in 0..24 {
            model.advance(&mut hourly, 3600).unwrap();
        }
    }
    assert_eq!(daily, hourly);
    let text = serde_json::to_string(&daily.checkpoint()).unwrap();
    let (resumed_model, mut resumed) =
        Model::restore(serde_json::from_str(&text).unwrap()).unwrap();
    assert_eq!(serde_json::to_string(&resumed.checkpoint()).unwrap(), text);
    model.advance(&mut daily, 3600).unwrap();
    resumed_model.advance(&mut resumed, 3600).unwrap();
    assert_eq!(daily, resumed);
    let before = daily.clone();
    assert!(model.advance(&mut daily, 0).is_err());
    assert_eq!(daily, before);
}

#[test]
fn configurations_shapes_pins_and_duplicate_owners_reject() {
    assert!(
        Settings {
            reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    let (world, model) = build(recipe(), settings(true));
    let mut state = model.initial_state();
    model.advance(&mut state, 86400).unwrap();
    let cp = state.checkpoint();
    let wet = world.water.body_ids.iter().position(|id| *id != 0).unwrap();
    for edit in 0..12 {
        let mut bad = cp.clone();
        match edit {
            0 => bad.reference_body_high_kilograms = None,
            1 => bad.reference_body_low_kilograms = None,
            2 => {
                bad.reference_body_high_kilograms.as_mut().unwrap().push(0.);
            }
            3 => bad.reference_body_low_kilograms.as_mut().unwrap()[0] = f64::NAN,
            4 => bad.reference_body_model_version = None,
            5 => bad.reference_body_model_version = Some("reference-water-pool-0".into()),
            6 => bad.settings.reference_water_pool = None,
            7 => bad.schema_version = 7,
            8 => bad.model_version = "seasonal-moisture-7".into(),
            9 => bad.surface_kilograms[wet] = 1.,
            10 => bad.terminal_water_kilograms[wet] = 1.,
            _ => bad.reference_body_high_kilograms.as_mut().unwrap()[0] *= 1.01,
        }
        assert!(Model::restore(bad).is_err(), "edit {edit}");
    }
    for key in [
        "referenceBodyHighKilograms",
        "referenceBodyLowKilograms",
        "referenceBodyModelVersion",
    ] {
        let mut value = serde_json::to_value(&cp).unwrap();
        value[key] = serde_json::Value::Null;
        assert!(serde_json::from_value::<Checkpoint>(value).is_err());
    }
    let mut value = serde_json::to_value(&cp).unwrap();
    value["settings"]["referenceWaterPool"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(value).is_err());
    let (_, legacy) = build(recipe(), settings(false));
    let before = state.clone();
    assert!(legacy.advance(&mut state, 3600).is_err());
    assert_eq!(state, before);
    let mut legacy_cp = legacy.initial_state().checkpoint();
    legacy_cp.reference_body_high_kilograms = Some(vec![0.]);
    assert!(Model::restore(legacy_cp).is_err());
}

#[test]
fn independent_bodies_do_not_fund_one_another_or_accept_balanced_mass_forgery() {
    let world = [0.025, 0.05, 0.1, 0.15, 0.2, 0.25]
        .into_iter()
        .find_map(|fraction| {
            let mut recipe = recipe();
            recipe.subdivision = 2;
            recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction };
            let world = World::generate(recipe).unwrap();
            let mut ids = world.water.body_ids.clone();
            ids.sort_unstable();
            ids.dedup();
            (ids.iter().filter(|id| **id > 0).count() >= 2).then_some(world)
        })
        .expect("Bounded generated fixture must contain independent water bodies");
    let model = Model::from_world(
        &world,
        settings(true),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let initial = model.initial_state().checkpoint();
    let mut state = model.initial_state();
    for _ in 0..16 {
        model.advance(&mut state, 86400).unwrap();
    }
    assert_body_identities(&world, &initial, &state.checkpoint());
    let mut bad = state.checkpoint();
    let high = bad.reference_body_high_kilograms.as_mut().unwrap();
    let transfer = high[0].min(high[1]) * 0.01;
    assert!(transfer > 0.);
    high[0] -= transfer;
    high[1] += transfer;
    // Remove low tails so rejection must not depend on incidental normalization.
    let low = bad.reference_body_low_kilograms.as_mut().unwrap();
    low[0] = 0.;
    low[1] = 0.;
    let error = Model::restore(bad).err().unwrap();
    assert!(error.contains("ledger is inconsistent"), "{error}");
}

#[test]
fn dry_empty_disabled_and_cold_worlds_do_not_invent_water_or_evaporation() {
    for coverage in [0., 1.] {
        for empty in [false, true] {
            for disabled in [false, true] {
                let mut recipe = recipe();
                recipe.water =
                    planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
                let (_, model) = build(
                    recipe,
                    Settings {
                        initial_active_surface_depth_meters: if empty { 0. } else { 1. },
                        evaporation_enabled: !disabled,
                        ..settings(true)
                    },
                );
                let mut state = model.initial_state();
                model.advance(&mut state, 86400).unwrap();
                let budget = model.budget(&state).unwrap();
                if empty || disabled || coverage == 0. {
                    assert_eq!(budget.cumulative_evaporation_kilograms, 0.);
                }
            }
        }
    }
    let world = World::generate(recipe()).unwrap();
    let temperature = planimulation_core::seasonal_temperature::Settings {
        reference_temperature_celsius: -20.,
        sensitivity_celsius_per_watt_per_square_meter: 0.,
        lapse_rate_celsius_per_meter: 0.,
        ..Default::default()
    };
    let model = Model::from_world(&world, settings(true), temperature, Default::default()).unwrap();
    let mut state = model.initial_state();
    model.advance(&mut state, 86400).unwrap();
    assert_eq!(
        model
            .budget(&state)
            .unwrap()
            .cumulative_evaporation_kilograms,
        0.
    );
}

#[test]
fn old_observers_analysis_and_desktop_packets_refuse_new_owner_semantics_atomically() {
    let (_, model) = build(recipe(), settings(true));
    let mut state = model.initial_state();
    let before = state.clone();
    let mut observed = false;
    assert!(
        model
            .advance_observed(&mut state, 3600, |_| observed = true)
            .is_err()
    );
    assert!(
        model
            .advance_terminal_observed(&mut state, 3600, |_| observed = true)
            .is_err()
    );
    assert!(!observed);
    assert_eq!(state, before);
    assert!(
        planimulation_core::seasonal_moisture::water_return::Audit::capture(&model, &state, 300)
            .is_err()
    );
    assert!(
        planimulation_core::seasonal_moisture::preparation::Monitor::new(
            &model,
            &state,
            Default::default()
        )
        .is_err()
    );
    let mut bytes = Vec::new();
    assert!(wire::seasonal_checkpoint(&mut bytes, &model, &state).is_err());
    assert!(wire::seasonal_moisture(&mut bytes, &model, &state, None, 0).is_err());
    assert!(bytes.is_empty());
    let (_, old) = build(recipe(), settings(false));
    let old_cp = serde_json::to_value(old.initial_state().checkpoint()).unwrap();
    for key in [
        "referenceBodyHighKilograms",
        "referenceBodyLowKilograms",
        "referenceBodyModelVersion",
    ] {
        assert!(old_cp.get(key).is_none());
    }
    assert!(old_cp["settings"].get("referenceWaterPool").is_none());
    assert!(
        serde_json::to_value(old.budget(&old.initial_state()).unwrap())
            .unwrap()
            .get("referenceBodyWaterKilograms")
            .is_none()
    );
}

#[test]
fn closed_dry_terminals_keep_their_local_identity_and_process_switches_remain_effective() {
    let mut recipe = recipe();
    recipe.subdivision = 2;
    recipe.radius_meters = 6_371_000.;
    let (world, model) = build(recipe.clone(), settings(true));
    let dry_terminals: Vec<_> = world
        .drainage
        .receivers
        .iter()
        .enumerate()
        .filter_map(|(i, &r)| (r as usize == i && world.water.body_ids[i] == 0).then_some(i))
        .collect();
    assert!(!dry_terminals.is_empty());
    let mut state = model.initial_state();
    for _ in 0..32 {
        model.advance(&mut state, 86400).unwrap();
    }
    let cp = state.checkpoint();
    assert!(
        dry_terminals
            .iter()
            .any(|&i| cp.terminal_water_kilograms[i] > 0.)
    );
    for &i in &dry_terminals {
        let route = cp.cumulative_runoff_transfers[i];
        close(
            cp.terminal_water_kilograms[i] + cp.terminal_low_kilograms.as_ref().unwrap()[i],
            route.terminal_delivery - route.terminal_evaporation,
        );
    }
    let (_, no_routing) = build(
        recipe.clone(),
        Settings {
            routing_enabled: false,
            ..settings(true)
        },
    );
    let mut state = no_routing.initial_state();
    for _ in 0..4 {
        no_routing.advance(&mut state, 86400).unwrap();
    }
    assert!(state.terminal_water_kilograms().iter().all(|v| *v == 0.));
    assert!(
        state
            .checkpoint()
            .cumulative_runoff_transfers
            .iter()
            .all(|r| r.values().iter().all(|v| *v == 0.))
    );
    let (_, no_precipitation) = build(
        recipe,
        Settings {
            precipitation_enabled: false,
            ..settings(true)
        },
    );
    let mut state = no_precipitation.initial_state();
    for _ in 0..4 {
        no_precipitation.advance(&mut state, 86400).unwrap();
    }
    let budget = no_precipitation.budget(&state).unwrap();
    assert_eq!(budget.cumulative_precipitation_kilograms, 0.);
    assert!(budget.cumulative_evaporation_kilograms > 0.);
}
