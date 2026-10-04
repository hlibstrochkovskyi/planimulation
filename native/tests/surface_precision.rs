use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{Checkpoint, Model, Settings, SoilNumerics, SurfaceNumerics},
    surface_water::{self, Stocks},
    wire,
};

fn model(seed: &str, snow: bool, limit: u32) -> Model {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.subdivision = 2;
    recipe.radius_meters = 1_000_000.;
    recipe.seed = seed.into();
    Model::from_world(
        &World::generate(recipe).unwrap(),
        Settings {
            orography: Some(Default::default()),
            soil_numerics: Some(SoilNumerics::Compensated),
            surface_numerics: snow.then_some(SurfaceNumerics::Compensated),
            max_coupled_step_seconds: limit,
            ..Default::default()
        },
        Default::default(),
        Default::default(),
    )
    .unwrap()
}

#[test]
fn small_snowfall_is_owned_and_melt_cannot_erase_signed_tails() {
    let settings = surface_water::Settings {
        melt_kilograms_per_square_meter_degree_day: 1.,
        ..Default::default()
    };
    let mut stock = Stocks {
        snow: 2_f64.powi(53),
        ..Default::default()
    };
    let mut low = 0.;
    for _ in 0..1000 {
        let step = surface_water::advance_precise_surface(
            stock, 0., low, 0., 1., false, 0., 1., 0., 60., settings,
        )
        .unwrap();
        assert_eq!(step.step.transfers.snowfall, 1.);
        stock = step.step.stocks;
        low = step.snow_low_kilograms;
    }
    assert_eq!(stock.snow, 2_f64.powi(53) + 1000.);
    assert_eq!(low, 0.);
    for low in [-2_f64.powi(-54), 2_f64.powi(-54)] {
        let before = Stocks {
            snow: 1.,
            ..Default::default()
        };
        let step = surface_water::advance_precise_surface(
            before, 0., low, 0., 1., false, 1., 0., 0., 86400., settings,
        )
        .unwrap();
        assert_eq!(
            step.step.transfers.melt,
            if low < 0. { 1_f64.next_down() } else { 1. }
        );
        assert_eq!(step.step.stocks.snow, 2_f64.powi(-54));
        assert_eq!(step.snow_low_kilograms, 0.);
        let next = surface_water::advance_precise_surface(
            step.step.stocks,
            0.,
            0.,
            step.liquid_low_kilograms,
            1.,
            false,
            1.,
            0.,
            0.,
            86400.,
            settings,
        )
        .unwrap();
        assert_eq!(next.step.transfers.melt, 2_f64.powi(-54));
        assert_eq!(next.step.stocks.snow, 0.);
    }
    // Disabled melt preserves both components; snowfall may exist over water.
    let before = Stocks {
        snow: 1.,
        ..Default::default()
    };
    let low = 2_f64.powi(-54);
    let disabled = surface_water::advance_precise_surface(
        before,
        0.,
        low,
        0.,
        1.,
        false,
        20.,
        0.,
        100.,
        60.,
        surface_water::Settings {
            melt_kilograms_per_square_meter_degree_day: 0.,
            ..settings
        },
    )
    .unwrap();
    assert_eq!(disabled.step.stocks.snow, before.snow);
    assert_eq!(disabled.snow_low_kilograms, low);
}

#[test]
fn snow_schema_pins_components_and_legacy_shape_are_strict() {
    let model = model("seasonal-reference", true, 60);
    let cp = model.initial_state().checkpoint();
    assert_eq!(cp.schema_version, 6);
    assert_eq!(cp.model_version, "seasonal-moisture-6");
    assert_eq!(
        cp.surface_model_version,
        surface_water::PRECISE_SURFACE_MODEL_VERSION
    );
    for edit in 0..12 {
        let mut bad = cp.clone();
        match edit {
            0 => bad.snow_low_kilograms = None,
            1 => {
                bad.snow_low_kilograms.as_mut().unwrap().pop();
            }
            2 => bad.snow_low_kilograms.as_mut().unwrap()[0] = f64::NAN,
            3 => bad.snow_low_kilograms.as_mut().unwrap()[0] = f64::from_bits(1),
            4 => bad.settings.surface_numerics = None,
            5 => bad.surface_model_version = surface_water::COMPENSATED_MODEL_VERSION.into(),
            6 => bad.soil_low_kilograms = None,
            7 => {
                bad.schema_version = 5;
                bad.model_version = "seasonal-moisture-5".into();
            }
            8 => bad.surface_low_kilograms = None,
            9 => {
                bad.surface_low_kilograms.as_mut().unwrap().pop();
            }
            10 => bad.surface_low_kilograms.as_mut().unwrap()[0] = f64::INFINITY,
            _ => bad.surface_low_kilograms.as_mut().unwrap()[0] = f64::from_bits(1),
        }
        assert!(Model::restore(bad).is_err(), "edit {edit}");
    }
    let mut value = serde_json::to_value(&cp).unwrap();
    value["snowLowKilograms"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(value).is_err());
    let mut value = serde_json::to_value(&cp).unwrap();
    value["surfaceLowKilograms"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(value).is_err());
    let mut value = serde_json::to_value(&cp).unwrap();
    value["settings"]["surfaceNumerics"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(value).is_err());
    assert!(
        Settings {
            surface_numerics: Some(SurfaceNumerics::Compensated),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    let legacy = self::model("seasonal-reference", false, 60);
    let value = serde_json::to_value(legacy.initial_state().checkpoint()).unwrap();
    assert!(value.get("snowLowKilograms").is_none());
    assert!(value.get("surfaceLowKilograms").is_none());
    assert!(value["settings"].get("surfaceNumerics").is_none());
    let mut forged = legacy.initial_state().checkpoint();
    forged.snow_low_kilograms = Some(vec![0.; forged.snow_kilograms.len()]);
    assert!(Model::restore(forged).is_err());
    let mut bytes = Vec::new();
    let state = model.initial_state();
    assert!(wire::seasonal_checkpoint(&mut bytes, &model, &state).is_err());
    assert!(wire::seasonal_moisture(&mut bytes, &model, &state, None, 0).is_err());
    assert!(bytes.is_empty());
}

#[test]
fn snow_replay_observation_batching_and_rejection_are_transactional() {
    let model = model("seasonal-reference", true, 3600);
    let mut daily = model.initial_state();
    let mut hourly = daily.clone();
    let mut observed = 0;
    model
        .advance_observed(&mut daily, 86400, |_| observed += 1)
        .unwrap();
    for _ in 0..24 {
        model.advance(&mut hourly, 3600).unwrap();
    }
    assert_eq!(daily, hourly);
    assert!(observed > 0);
    for _ in 1..31 {
        model.advance(&mut daily, 86400).unwrap();
    }
    assert!(daily.snow_low_kilograms().unwrap().iter().any(|v| *v != 0.));
    let encoded = serde_json::to_string(&daily.checkpoint()).unwrap();
    let (restored_model, mut restored) =
        Model::restore(serde_json::from_str(&encoded).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_string(&restored.checkpoint()).unwrap(),
        encoded
    );
    model.advance(&mut daily, 3600).unwrap();
    restored_model.advance(&mut restored, 3600).unwrap();
    assert_eq!(daily, restored);
    let before_rejection = hourly.clone();
    assert!(model.advance(&mut hourly, 86401).is_err());
    assert_eq!(hourly, before_rejection);
}

// The release qualification is explicit so ordinary debug CI is not burdened
// with over a million coupled ticks. Fast oracle/representation tests run normally.
#[test]
#[ignore = "long qualification: run --release --test surface_precision -- --ignored"]
fn both_annual_snow_witnesses_complete_with_unchanged_ledgers() {
    for seed in ["seasonal-reference", "moisture-coast"] {
        let model = model(seed, true, 60);
        let mut state = model.initial_state();
        for _ in 0..365 {
            model.advance(&mut state, 86400).unwrap();
        }
        let budget = model.budget(&state).unwrap();
        assert!(budget.maximum_relative_local_surface_ledger_residual < 1e-12);
        let encoded = serde_json::to_string(&state.checkpoint()).unwrap();
        let (resumed_model, mut resumed) =
            Model::restore(serde_json::from_str(&encoded).unwrap()).unwrap();
        model.advance(&mut state, 3600).unwrap();
        resumed_model.advance(&mut resumed, 3600).unwrap();
        assert_eq!(state, resumed);
    }
}

#[test]
fn liquid_credits_evaporation_and_infiltration_respect_the_same_represented_mass() {
    let settings = surface_water::Settings::default();
    let mut before = Stocks {
        liquid: 2_f64.powi(53),
        ..Default::default()
    };
    let mut low = 0.;
    for _ in 0..1000 {
        let step = surface_water::advance_precise_surface(
            before, 0., 0., low, 1., false, 20., 1., 0., 60., settings,
        )
        .unwrap();
        assert_eq!(step.step.transfers.rain, 1.);
        before = step.step.stocks;
        low = step.liquid_low_kilograms;
    }
    assert_eq!(before.liquid, 2_f64.powi(53) + 1000.);
    for low in [-2_f64.powi(-54), 2_f64.powi(-54)] {
        let step = surface_water::advance_precise_surface(
            Stocks {
                liquid: 1.,
                ..Default::default()
            },
            0.,
            0.,
            low,
            1.,
            false,
            20.,
            0.,
            1.,
            60.,
            settings,
        )
        .unwrap();
        assert_eq!(
            step.step.transfers.liquid_evaporation,
            if low < 0. { 1_f64.next_down() } else { 1. }
        );
        assert_eq!(step.step.stocks.liquid, 2_f64.powi(-54));
        assert_eq!(step.liquid_low_kilograms, 0.);
    }
    // Near capacity, the infiltrated tail is bounded by both donor and receiver.
    let tail = 2_f64.powi(-48);
    let step = surface_water::advance_precise_surface(
        Stocks {
            liquid: 1.,
            soil: 150.,
            ..Default::default()
        },
        -tail,
        0.,
        -2_f64.powi(-54),
        1.,
        true,
        20.,
        0.,
        0.,
        86400.,
        surface_water::Settings {
            soil_retained_fraction: 1.,
            ..settings
        },
    )
    .unwrap();
    assert_eq!(step.step.transfers.infiltration, tail);
    assert_eq!(step.step.stocks.soil, 150.);
    assert_eq!(step.soil_low_kilograms, 0.);
    assert!(step.step.residual_kilograms.abs() < 32. * f64::EPSILON * 151.);
}
