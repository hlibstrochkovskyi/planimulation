use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{Model, Settings, SoilNumerics},
    seasonal_temperature, seasonal_wind, surface_water, wire,
};

fn world(seed: &str) -> World {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.subdivision = 2;
    recipe.seed = seed.into();
    recipe.radius_meters = 1_000_000.;
    World::generate(recipe).unwrap()
}
fn model(seed: &str, compensated: bool) -> Model {
    Model::from_world(
        &world(seed),
        Settings {
            orography: Some(Default::default()),
            max_coupled_step_seconds: 60,
            soil_numerics: compensated.then_some(SoilNumerics::Compensated),
            ..Default::default()
        },
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )
    .unwrap()
}

#[test]
fn both_retained_soil_witnesses_complete_and_replay_without_relaxed_budgets() {
    for seed in ["seasonal-reference", "moisture-coast"] {
        let model = model(seed, true);
        let mut state = model.initial_state();
        assert_eq!(model.model_version(), "seasonal-moisture-5");
        for day in 1..=40 {
            model.advance(&mut state, 86400).unwrap();
            let b = model.budget(&state).unwrap();
            assert!(b.residual_kilograms.abs() / b.initial_mobile_water_kilograms < 1e-12);
            assert!(b.maximum_relative_local_surface_ledger_residual < 1e-12);
            if day == 31 {
                assert!(
                    state
                        .soil_low_kilograms()
                        .unwrap()
                        .iter()
                        .any(|low| *low != 0.)
                );
                let text = serde_json::to_string(&state.checkpoint()).unwrap();
                let (restored_model, mut restored) =
                    Model::restore(serde_json::from_str(&text).unwrap()).unwrap();
                assert_eq!(restored, state);
                assert_eq!(serde_json::to_string(&restored.checkpoint()).unwrap(), text);
                let mut continued = state.clone();
                model.advance(&mut continued, 3600).unwrap();
                restored_model.advance(&mut restored, 3600).unwrap();
                assert_eq!(restored, continued);
            }
        }
    }
}

#[test]
fn observations_and_hourly_daily_batching_do_not_change_owned_components() {
    for compensated in [false, true] {
        let model = model("seasonal-reference", compensated);
        let mut daily = model.initial_state();
        let mut hourly = daily.clone();
        let mut operations = 0;
        model
            .advance_observed(&mut daily, 86400, |_| operations += 1)
            .unwrap();
        for _ in 0..24 {
            model.advance(&mut hourly, 3600).unwrap();
        }
        assert_eq!(daily, hourly);
        assert!(operations > 0);
    }
}

#[test]
fn checkpoint_cannot_drop_mislabel_or_forge_the_soil_low_component() {
    let model = model("seasonal-reference", true);
    let state = model.initial_state();
    let cp = state.checkpoint();
    let mut bad = cp.clone();
    bad.soil_low_kilograms = None;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.soil_low_kilograms.as_mut().unwrap().pop();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.soil_low_kilograms.as_mut().unwrap()[0] = f64::NAN;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.soil_low_kilograms.as_mut().unwrap()[0] = 1e-30;
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.schema_version = 4;
    bad.model_version = "seasonal-moisture-4".into();
    assert!(Model::restore(bad).is_err());
    let mut bad = cp.clone();
    bad.surface_model_version = surface_water::MODEL_VERSION.into();
    assert!(Model::restore(bad).is_err());
    let mut value = serde_json::to_value(cp).unwrap();
    value["soilLowKilograms"] = serde_json::Value::Null;
    assert!(
        serde_json::from_value::<planimulation_core::seasonal_moisture::Checkpoint>(value).is_err()
    );
    let legacy = model_for_legacy();
    let mut bad = legacy.initial_state().checkpoint();
    bad.soil_low_kilograms = Some(vec![0.; bad.soil_kilograms.len()]);
    assert!(Model::restore(bad).is_err());
}
fn model_for_legacy() -> Model {
    model("seasonal-reference", false)
}

#[test]
fn legacy_records_omit_new_fields_and_candidate_has_no_desktop_protocol() {
    let legacy = model_for_legacy();
    let encoded = serde_json::to_value(legacy.initial_state().checkpoint()).unwrap();
    assert!(encoded.get("soilLowKilograms").is_none());
    assert!(encoded["settings"].get("soilNumerics").is_none());
    let candidate = model("seasonal-reference", true);
    let state = candidate.initial_state();
    let mut output = Vec::new();
    assert!(
        wire::seasonal_moisture(&mut output, &candidate, &state, None, 0)
            .unwrap_err()
            .contains("headless-only")
    );
    assert!(output.is_empty());
    assert!(
        wire::seasonal_checkpoint(&mut output, &candidate, &state)
            .unwrap_err()
            .contains("headless-only")
    );
    assert!(output.is_empty());
}

#[test]
fn soil_low_parts_survive_cold_intervals_and_near_capacity_without_extra_water() {
    let settings = surface_water::Settings {
        soil_retained_fraction: 1.,
        ..Default::default()
    };
    let high = 150.;
    let low = -2_f64.powi(-48);
    let before = surface_water::Stocks {
        liquid: 1.,
        soil: high,
        ..Default::default()
    };
    let cold =
        surface_water::advance_compensated(before, low, 1., true, -1., 0., 1000., 60., settings)
            .unwrap();
    assert_eq!(cold.soil_low_kilograms, low);
    assert_eq!(cold.step.stocks.soil, high);
    let warm =
        surface_water::advance_compensated(before, low, 1., true, 20., 0., 0., 86400., settings)
            .unwrap();
    assert_eq!(warm.step.transfers.infiltration, -low);
    assert_eq!(warm.step.stocks.soil, high);
    assert_eq!(warm.soil_low_kilograms, 0.);
    assert_eq!(warm.step.transfers.soil_drainage, 0.);
    assert!(warm.step.residual_kilograms.abs() < 32. * f64::EPSILON * before.total());
    assert!(
        surface_water::advance_compensated(before, -low, 1., true, 20., 0., 0., 60., settings)
            .is_err()
    );
}
