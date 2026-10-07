use planimulation_core::{
    Recipe, World,
    moisture_transport::Geometry,
    seasonal_moisture::regional_soil::{Checkpoint, Model, NumericalPolicy, Settings},
    seasonal_temperature,
    surface_water::ponded_soil::Mass,
};

fn fixture(settings: Settings, temperature: seasonal_temperature::Settings) -> (World, Model) {
    let mut recipe: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: 0.3 };
    let world = World::generate(recipe).unwrap();
    let model = Model::from_world(&world, settings, temperature, Default::default()).unwrap();
    (world, model)
}
fn add(m: &mut Mass, amount: f64) {
    let sum = m.high + amount;
    let v = sum - m.high;
    let tail = m.low + ((m.high - (sum - v)) + (amount - v));
    let high = sum + tail;
    let v = high - sum;
    m.high = high;
    m.low = (sum - (high - v)) + (tail - v);
}
fn warm() -> seasonal_temperature::Settings {
    seasonal_temperature::Settings {
        reference_temperature_celsius: 30.,
        sensitivity_celsius_per_watt_per_square_meter: 0.,
        lapse_rate_celsius_per_meter: 0.,
        ..Default::default()
    }
}
fn quiet() -> Settings {
    let mut settings = Settings {
        evaporation_enabled: false,
        precipitation_enabled: false,
        routing_enabled: false,
        ..Default::default()
    };
    settings
        .surface_flow
        .maximum_diffusivity_square_meters_per_second = 0.;
    settings
}
// Supplied history, not a naturally reached climate: finite body evaporation,
// one real neighboring atmospheric crossing, rain and explicitly owned infiltration.
fn fund(world: &World, cp: &mut Checkpoint, amount_per_area: f64) -> usize {
    let geometry = Geometry::from_surface(&world.surface, world.recipe.radius_meters).unwrap();
    let (face, source, target) = geometry
        .boundaries()
        .iter()
        .enumerate()
        .find_map(|(face, b)| {
            let [a, c] = b.regions;
            if world.water.body_ids[a] > 0 && world.water.body_ids[c] == 0 {
                Some((face, a, c))
            } else if world.water.body_ids[c] > 0 && world.water.body_ids[a] == 0 {
                Some((face, c, a))
            } else {
                None
            }
        })
        .unwrap();
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id != 0);
    let body = ids.binary_search(&world.water.body_ids[source]).unwrap();
    let amount = amount_per_area * world.surface.areas[target];
    add(&mut cp.reference_bodies[body], -amount);
    add(&mut cp.local_transfers[source].liquid_evaporation, amount);
    add(
        &mut cp.atmospheric_transfers[2 * face + usize::from(source > target)],
        amount,
    );
    add(&mut cp.local_transfers[target].rain, amount);
    add(&mut cp.liquid[target], amount);
    cp.elapsed_seconds = 900;
    target
}

#[test]
fn terrestrial_soil_remains_active_for_every_positive_film_in_the_actual_cycle() {
    // Funding requires enabled rain/evaporation histories, not forged disabled flags.
    let mut settings = quiet();
    settings.precipitation_enabled = true;
    settings.evaporation_enabled = true;
    let (world, model) = fixture(settings, warm());
    let mut cp = model.initial_state().checkpoint();
    let r = fund(&world, &mut cp, 130.);
    let soil = 120. * world.surface.areas[r];
    add(&mut cp.liquid[r], -soil);
    cp.soil[r] = Mass {
        high: soil,
        low: 0.,
    };
    cp.local_transfers[r].infiltration = cp.soil[r];
    for film in [0., 1e-12, 1e-6, 1.] {
        let mut variant = cp.clone();
        let target = fund(&world, &mut variant, film / world.surface.areas[r]);
        assert_eq!(target, r);
        let (model, mut state) = Model::restore(variant).unwrap();
        let before = state.checkpoint();
        let step = model.advance(&mut state, 900).unwrap();
        let after = state.checkpoint();
        assert!(
            after.local_transfers[r].infiltration.high
                > before.local_transfers[r].infiltration.high
        );
        assert!(
            after.local_transfers[r].soil_drainage.high
                > before.local_transfers[r].soil_drainage.high
        );
        assert!(step.budget.maximum_relative_local_residual < 128. * f64::EPSILON);
        assert!(after.drainage[r].high > 0.);
    }
}

#[test]
fn generated_exchange_transport_and_complete_replay_share_the_same_owned_water() {
    let (world, model) = fixture(Settings::default(), Default::default());
    let mut state = model.initial_state();
    for _ in 0..40 {
        model.advance(&mut state, 86400).unwrap();
    }
    let cp = state.checkpoint();
    assert_eq!(cp.model_version, "regional-seasonal-water-1");
    assert!(cp.local_transfers.iter().any(|f| f.infiltration.high > 0.));
    assert!(cp.atmospheric_transfers.iter().any(|f| f.high > 0.));
    assert!(cp.vapor.iter().any(|v| v.low != 0.));
    let (resumed, mut saved) =
        Model::restore(serde_json::from_str(&serde_json::to_string(&cp).unwrap()).unwrap())
            .unwrap();
    model.advance(&mut state, 3600).unwrap();
    resumed.advance(&mut saved, 3600).unwrap();
    assert_eq!(state, saved);
    let mut direct = model.initial_state();
    let mut split = direct.clone();
    model.advance(&mut direct, 86400).unwrap();
    for _ in 0..24 {
        model.advance(&mut split, 3600).unwrap();
    }
    assert_eq!(direct, split);
    assert_eq!(
        world.terrain.elevation,
        World::generate(world.recipe.clone())
            .unwrap()
            .terrain
            .elevation
    );
}

#[test]
fn zero_and_fully_wet_worlds_and_disabled_forcing_do_not_create_terrestrial_owners() {
    let (world, _) = fixture(Settings::default(), Default::default());
    for coverage in [0., 1.] {
        let mut recipe = world.recipe.clone();
        recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
        let world = World::generate(recipe).unwrap();
        let model =
            Model::from_world(&world, quiet(), Default::default(), Default::default()).unwrap();
        let mut state = model.initial_state();
        model.advance(&mut state, 86400).unwrap();
        let cp = state.checkpoint();
        assert!(
            cp.liquid
                .iter()
                .chain(&cp.soil)
                .chain(&cp.drainage)
                .chain(&cp.vapor)
                .all(|m| m.high == 0.)
        );
        assert_eq!(model.budget(&state).unwrap().relative_global_residual, 0.);
    }
}

#[test]
fn corrupt_pins_components_ownership_and_balanced_teleports_are_rejected() {
    let (_, model) = fixture(Settings::default(), Default::default());
    let mut state = model.initial_state();
    model.advance(&mut state, 86400).unwrap();
    let cp = state.checkpoint();
    for variant in 0..7 {
        let mut bad = cp.clone();
        match variant {
            0 => bad.soil_model_version = "wrong".into(),
            1 => {
                bad.vapor.pop();
            }
            2 => {
                bad.vapor[0] = Mass {
                    high: 0.,
                    low: 1e-30,
                }
            }
            3 => bad.atmospheric_transfers[0].high += 1e15,
            4 => {
                bad.elapsed_seconds = 0;
            }
            5 => {
                add(&mut bad.vapor[0], 1e12);
                add(&mut bad.vapor[1], -1e12);
            }
            _ => bad.drainage_sent[0].high += 1e15,
        }
        assert!(
            Model::restore(bad).is_err(),
            "accepted corrupted variant {variant}"
        );
    }
    let before = state.clone();
    assert!(model.advance(&mut state, 0).is_err());
    assert_eq!(state, before);
    let mut json = serde_json::to_value(cp).unwrap();
    assert!(
        serde_json::from_value::<planimulation_core::seasonal_moisture::Checkpoint>(json.clone())
            .is_err()
    );
    json["vapor"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(json).is_err());
}

#[test]
fn excessive_surface_work_rolls_back_the_complete_caller_after_vertical_preparation() {
    let (world, _) = fixture(Settings::default(), Default::default());
    let mut recipe = world.recipe.clone();
    recipe.radius_meters = 100_000.;
    recipe.subdivision = 5;
    let world = World::generate(recipe).unwrap();
    let mut settings = quiet();
    // Warm finite bodies fund positive vapor receipts before the later flow refusal.
    settings.evaporation_enabled = true;
    settings
        .surface_flow
        .maximum_diffusivity_square_meters_per_second = 1e8;
    let model = Model::from_world(&world, settings, warm(), Default::default()).unwrap();
    let mut state = model.initial_state();
    let before = state.clone();
    let message = model.advance(&mut state, 86400).unwrap_err();
    assert!(message.contains("surface flow exceeds"), "{message}");
    assert_eq!(state, before);
}

#[test]
fn dense_generated_precision_witness_is_retained_only_under_the_explicit_new_policy() {
    let mut recipe: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    recipe.seed = "first-light".into();
    recipe.subdivision = 3;
    recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: 0.3 };
    let world = World::generate(recipe).unwrap();
    let strict = Model::from_world(
        &world,
        Settings::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut rejected = strict.initial_state();
    let before = rejected.clone();
    assert!(
        strict
            .advance(&mut rejected, 86400)
            .unwrap_err()
            .contains("represented stock precision")
    );
    assert_eq!(rejected, before);
    let settings = Settings {
        numerical_policy: NumericalPolicy::RetainDonor,
        ..Default::default()
    };
    let model =
        Model::from_world(&world, settings, Default::default(), Default::default()).unwrap();
    let mut state = model.initial_state();
    for _ in 0..2 {
        model.advance(&mut state, 86400).unwrap();
    }
    let cp = state.checkpoint();
    assert!(cp.resolution.unwrap().deferred_requests > 0);
    let (resumed, mut replay) =
        Model::restore(serde_json::from_str(&serde_json::to_string(&cp).unwrap()).unwrap())
            .unwrap();
    model.advance(&mut state, 3600).unwrap();
    resumed.advance(&mut replay, 3600).unwrap();
    assert_eq!(replay, state);
    for variant in 0..5 {
        let mut bad = cp.clone();
        match variant {
            0 => bad.resolution = None,
            1 => {
                bad.resolution
                    .as_mut()
                    .unwrap()
                    .summed_deferred_request_kilograms = f64::NAN
            }
            2 => bad.settings.numerical_policy = NumericalPolicy::RejectUnrepresentable,
            3 => bad.soil_model_version = "ponded-soil-exchange-1".into(),
            _ => bad.transport_model_version = "moisture-transport-paired-1".into(),
        }
        assert!(
            Model::restore(bad).is_err(),
            "accepted numerical policy corruption {variant}"
        );
    }
    let mut json = serde_json::to_value(&cp).unwrap();
    json["resolution"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(json).is_err());
    assert!(
        !serde_json::to_value(before.checkpoint())
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("resolution")
    );
}

#[test]
fn observations_keep_every_owned_component_and_cannot_change_continuation_or_geography() {
    let settings = Settings {
        numerical_policy: NumericalPolicy::RetainDonor,
        ..Default::default()
    };
    let (world, model) = fixture(settings, Default::default());
    let mut cp = model.initial_state().checkpoint();
    fund(&world, &mut cp, 50_000.);
    let (model, mut observed) = Model::restore(cp).unwrap();
    let mut unobserved = observed.clone();
    let before = observed.checkpoint();
    for _ in 0..3 {
        let view = model.observe(&observed).unwrap();
        assert_eq!(
            view.observation_model_version,
            "regional-soil-observation-1"
        );
        assert_eq!(view.stocks.liquid, before.liquid);
        assert_eq!(view.stocks.soil, before.soil);
        assert_eq!(view.stocks.snow, before.snow);
        assert_eq!(view.stocks.vapor, before.vapor);
        assert_eq!(view.stocks.drainage, before.drainage);
        assert_eq!(view.resolution, before.resolution);
        assert_eq!(view.cumulative_local_transfers, before.local_transfers);
        assert_eq!(
            view.reference_bodies
                .iter()
                .map(|b| b.liquid)
                .collect::<Vec<_>>(),
            before.reference_bodies
        );
        assert!(
            view.reference_bodies
                .windows(2)
                .all(|p| p[0].body_id < p[1].body_id)
        );
        for r in 0..world.surface.areas.len() {
            let liquid_depth = before.liquid[r].high / (1000. * world.surface.areas[r]);
            assert_eq!(view.regional_liquid_depth_meters[r], liquid_depth);
            let wet = world.water.body_ids[r] > 0;
            let expected = if wet {
                (world.water.level_meters - world.terrain.elevation[r]).max(0.)
            } else {
                liquid_depth
            };
            assert_eq!(view.visible_water_depth_meters[r], expected);
            if expected > 0. {
                assert_eq!(
                    view.visible_water_level_meters[r],
                    if wet {
                        world.water.level_meters
                    } else {
                        world.terrain.elevation[r] + liquid_depth
                    }
                );
            } else {
                assert_eq!(view.visible_water_level_meters[r], 0.);
            }
        }
        assert_eq!(observed.checkpoint(), before);
    }
    model.advance(&mut observed, 86400).unwrap();
    model.advance(&mut unobserved, 86400).unwrap();
    let view = model.observe(&observed).unwrap();
    assert_eq!(observed, unobserved);
    assert_eq!(view.settings.numerical_policy, NumericalPolicy::RetainDonor);
    assert_eq!(view.soil_model_version, "ponded-soil-exchange-2");
    assert_eq!(view.stocks.vapor, observed.checkpoint().vapor);
    assert!(view.stocks.vapor.iter().any(|m| m.low != 0.));
    assert_eq!(view.stocks.liquid, observed.checkpoint().liquid);
    assert_eq!(view.stocks.soil, observed.checkpoint().soil);
    assert!(
        view.cumulative_surface_incoming_kilograms
            .iter()
            .any(|&v| v > 0.)
    );
    let sum = |v: &[f64]| planimulation_core::moisture_transport::total_mass(v);
    assert!(
        (sum(&view.cumulative_surface_incoming_kilograms)
            - sum(&view.cumulative_surface_outgoing_kilograms))
        .abs()
            <= 32. * f64::EPSILON * sum(&view.cumulative_surface_incoming_kilograms)
    );
    model.advance(&mut observed, 3600).unwrap();
    model.advance(&mut unobserved, 3600).unwrap();
    assert_eq!(observed, unobserved);
    assert_eq!(
        world.terrain.elevation,
        World::generate(world.recipe.clone())
            .unwrap()
            .terrain
            .elevation
    );
    let (_, other) = fixture(Settings::default(), Default::default());
    assert!(other.observe(&observed).is_err());
}
