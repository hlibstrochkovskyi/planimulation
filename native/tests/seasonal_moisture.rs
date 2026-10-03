use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::{
        Checkpoint, Model, Settings, exchange, saturation_column_kilograms_per_square_meter,
        saturation_pressure_pascals,
    },
    seasonal_temperature, seasonal_wind,
};

fn world() -> World {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.subdivision = 2;
    World::generate(recipe).unwrap()
}

fn model(settings: Settings) -> Model {
    Model::from_world(
        &world(),
        settings,
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )
    .unwrap()
}

#[test]
fn vapor_pressure_matches_independent_reference_values_and_increases_with_temperature() {
    // Pa: Murphy–Koop values near the triple point and tabulated room-temperature pressure.
    assert!((saturation_pressure_pascals(0.01).unwrap() - 611.657).abs() < 0.02);
    assert!((saturation_pressure_pascals(20.).unwrap() - 2339.).abs() < 1.);
    assert!((saturation_pressure_pascals(-20.).unwrap() - 103.25).abs() < 0.1);
    let mut previous = 0.;
    for t in -100..=50 {
        let value = saturation_pressure_pascals(t as f64).unwrap();
        assert!(value > previous && value.is_finite());
        previous = value;
    }
    let q = saturation_column_kilograms_per_square_meter(20., 2000.).unwrap();
    assert!(q > 34. && q < 35.);
    for t in [f64::NAN, -101., 51.] {
        assert!(saturation_pressure_pascals(t).is_err());
    }
}

#[test]
fn warm_box_evaporates_by_the_analytic_relaxation_and_cannot_overdraw_the_source() {
    let settings = Settings::default();
    let capacity = saturation_column_kilograms_per_square_meter(20., 2000.).unwrap();
    let first = exchange(1000., 0., 1., 20., 86400., settings).unwrap();
    let expected = capacity * (1. - (-0.2_f64).exp());
    assert!((first.evaporated_kilograms - expected).abs() < 1e-12);
    assert_eq!(first.precipitated_kilograms, 0.);
    let second = exchange(
        first.surface_kilograms,
        first.vapor_kilograms,
        1.,
        20.,
        86400.,
        settings,
    )
    .unwrap();
    assert!((second.vapor_kilograms - capacity * (1. - (-0.4_f64).exp())).abs() < 1e-12);
    let scarce = exchange(0.1, 0., 1., 20., 86400., settings).unwrap();
    assert_eq!(scarce.surface_kilograms, 0.);
    assert_eq!(scarce.evaporated_kilograms, 0.1);
    assert_eq!(scarce.vapor_kilograms, 0.1);
    let dry = exchange(0., 0., 1., 20., 86400., settings).unwrap();
    assert_eq!(dry.evaporated_kilograms, 0.);
}

#[test]
fn supersaturation_and_cooling_deposit_water_into_the_same_local_surface_stock() {
    let settings = Settings::default();
    let q = saturation_column_kilograms_per_square_meter(20., 2000.).unwrap();
    let excess = 10.;
    let result = exchange(0., q + excess, 1., 20., 86400., settings).unwrap();
    let expected = excess * (1. - (-4_f64).exp());
    assert!((result.precipitated_kilograms - expected).abs() < 1e-12);
    assert_eq!(result.evaporated_kilograms, 0.);
    assert_eq!(result.surface_kilograms, result.precipitated_kilograms);
    assert!((result.vapor_kilograms - q - excess * (-4_f64).exp()).abs() < 1e-12);
    let cold = exchange(1000., q, 1., -20., 86400., settings).unwrap();
    assert!(cold.precipitated_kilograms > 0.);
    assert_eq!(cold.evaporated_kilograms, 0.);
    assert!(cold.residual_kilograms.abs() < 1e-12);
}

#[test]
fn process_switches_stop_the_requested_flux_without_destroying_stocks() {
    let settings = Settings::default();
    assert_eq!(
        exchange(
            1000.,
            0.,
            1.,
            20.,
            86400.,
            Settings {
                evaporation_enabled: false,
                ..settings
            }
        )
        .unwrap()
        .evaporated_kilograms,
        0.
    );
    let no_rain = exchange(
        1000.,
        200.,
        1.,
        20.,
        86400.,
        Settings {
            precipitation_enabled: false,
            ..settings
        },
    )
    .unwrap();
    assert_eq!(no_rain.precipitated_kilograms, 0.);
    assert_eq!(no_rain.vapor_kilograms, 200.);
}

#[test]
fn initial_inventory_is_a_finite_partition_of_generated_water_and_zero_vapor() {
    let world = world();
    let original = world.water.depth_meters.clone();
    let model = Model::from_world(
        &world,
        Settings::default(),
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )
    .unwrap();
    let state = model.initial_state();
    for (i, &depth) in original.iter().enumerate() {
        assert_eq!(
            state.surface_kilograms()[i],
            world.surface.areas[i] * depth.min(1.) * 1000.
        );
        assert_eq!(state.vapor_kilograms()[i], 0.);
    }
    assert_eq!(world.water.depth_meters, original);
    assert!(model.budget(&state).unwrap().initial_mobile_water_kilograms > 0.);
    let empty = model_with_no_water(&world);
    let mut dry = empty.initial_state();
    empty.advance(&mut dry, 86400).unwrap();
    assert_eq!(total_mass(dry.vapor_kilograms()), 0.);
}

fn model_with_no_water(world: &World) -> Model {
    Model::from_world(
        world,
        Settings {
            initial_active_surface_depth_meters: 0.,
            ..Settings::default()
        },
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )
    .unwrap()
}

#[test]
fn a_generated_year_conserves_both_stocks_and_local_exchange_ledgers() {
    let world = world();
    let model = Model::from_world(
        &world,
        Settings::default(),
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )
    .unwrap();
    let mut state = model.initial_state();
    let mut dry_precipitation = 0.;
    for _ in 0..365 {
        let step = model.advance(&mut state, 86400).unwrap();
        for (&rain, &depth) in step
            .precipitation_kilograms
            .iter()
            .zip(&world.water.depth_meters)
        {
            if depth == 0. {
                dry_precipitation += rain;
            }
        }
        assert!(
            state
                .surface_kilograms()
                .iter()
                .chain(state.vapor_kilograms())
                .all(|v| v.is_finite() && *v >= 0.)
        );
        assert!(
            step.budget.residual_kilograms.abs() / step.budget.initial_mobile_water_kilograms
                < 1e-12
        );
        assert!(step.budget.maximum_relative_local_surface_ledger_residual < 1e-12);
    }
    assert!(dry_precipitation > 0.);
    let budget = model.budget(&state).unwrap();
    assert!(budget.cumulative_evaporation_kilograms > 0.);
    assert!(budget.cumulative_precipitation_kilograms > 0.);
    assert_eq!(state.elapsed_seconds(), 365 * 86400);
}

#[test]
fn complete_checkpoint_replays_across_month_and_year_boundaries() {
    let model = model(Settings::default());
    let mut uninterrupted = model.initial_state();
    for _ in 0..360 {
        model.advance(&mut uninterrupted, 86400).unwrap();
    }
    let bytes = serde_json::to_vec(&uninterrupted.checkpoint()).unwrap();
    let cp: Checkpoint = serde_json::from_slice(&bytes).unwrap();
    let (restored_model, mut restored) = Model::restore(cp).unwrap();
    for _ in 0..15 {
        model.advance(&mut uninterrupted, 86400).unwrap();
        restored_model.advance(&mut restored, 86400).unwrap();
    }
    assert_eq!(uninterrupted, restored);
    // A non-day-aligned step splits at the next day boundary identically.
    let mut split = restored.clone();
    restored_model.advance(&mut restored, 43200).unwrap();
    restored_model.advance(&mut split, 43200).unwrap();
    let mut joined = split.clone();
    restored_model.advance(&mut joined, 86400).unwrap();
    restored_model.advance(&mut split, 43200).unwrap();
    restored_model.advance(&mut split, 43200).unwrap();
    assert_eq!(joined, split);
}

#[test]
fn mismatched_versions_corrupt_stocks_local_ownership_and_failed_steps_reject() {
    let model = model(Settings::default());
    let mut state = model.initial_state();
    model.advance(&mut state, 86400).unwrap();
    let snapshot = state.clone();
    assert!(model.advance(&mut state, 0).is_err());
    assert!(model.advance(&mut state, 86401).is_err());
    assert_eq!(state, snapshot);
    let mut cases = Vec::new();
    let mut cp = state.checkpoint();
    cp.model_version = "future".into();
    cases.push(cp);
    let mut cp = state.checkpoint();
    cp.vapor_kilograms[0] = -1.;
    cases.push(cp);
    let mut cp = state.checkpoint();
    cp.surface_kilograms.pop();
    cases.push(cp);
    let mut cp = state.checkpoint();
    cp.elapsed_seconds = u64::MAX;
    cases.push(cp);
    let mut cp = state.checkpoint();
    cp.cumulative_evaporation_kilograms[0] += 1e12;
    cases.push(cp);
    // Global mass alone is insufficient: swapping surface reservoirs breaks local ownership.
    let mut cp = state.checkpoint();
    let wet = cp.surface_kilograms.iter().position(|v| *v > 0.).unwrap();
    let dry = cp.surface_kilograms.iter().position(|v| *v == 0.).unwrap();
    cp.surface_kilograms.swap(wet, dry);
    cases.push(cp);
    for cp in cases {
        assert!(Model::restore(cp).is_err());
    }
    assert_eq!(state, snapshot);
    let incompatible = model_with_no_water(&world());
    assert!(incompatible.advance(&mut state, 86400).is_err());
    assert_eq!(state, snapshot);
}

#[test]
fn invalid_settings_and_exchange_overflow_reject() {
    for settings in [
        Settings {
            evaporation_response_seconds: 0.,
            ..Settings::default()
        },
        Settings {
            effective_vapor_depth_meters: f64::NAN,
            ..Settings::default()
        },
        Settings {
            initial_active_surface_depth_meters: -1.,
            ..Settings::default()
        },
    ] {
        assert!(settings.validate().is_err());
    }
    assert!(
        exchange(
            f64::MAX,
            f64::MAX,
            f64::MAX,
            20.,
            86400.,
            Settings::default()
        )
        .is_err()
    );
    assert!(exchange(-1., 1., 1., 20., 86400., Settings::default()).is_err());
}

#[test]
fn smaller_coupled_steps_converge_without_hiding_a_mobile_water_deficit() {
    let model = model(Settings::default());
    let run = |seconds: u32| {
        let mut state = model.initial_state();
        for _ in 0..60 * 86400 / seconds {
            model.advance(&mut state, seconds).unwrap();
        }
        state
    };
    let reference = run(450);
    let mut errors = Vec::new();
    for seconds in [86400, 1800, 900] {
        let state = run(seconds);
        errors.push(
            state
                .surface_kilograms()
                .iter()
                .chain(state.vapor_kilograms())
                .zip(
                    reference
                        .surface_kilograms()
                        .iter()
                        .chain(reference.vapor_kilograms()),
                )
                .map(|(a, b)| (a - b).abs())
                .sum::<f64>(),
        );
        let budget = model.budget(&state).unwrap();
        assert!(budget.residual_kilograms.abs() / budget.initial_mobile_water_kilograms < 1e-12);
    }
    for pair in errors.windows(2) {
        assert!(pair[1] < pair[0], "Coupled refinement errors: {errors:?}");
    }
}

#[test]
fn rejected_transport_rolls_back_the_preceding_evaporation_and_clock() {
    let mut recipe = world().recipe;
    recipe.radius_meters = 100_000.;
    let world = World::generate(recipe).unwrap();
    let settings = Settings {
        transport: planimulation_core::moisture_transport::Settings {
            max_substeps: 1,
            ..Default::default()
        },
        ..Settings::default()
    };
    let model = Model::from_world(
        &world,
        settings,
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )
    .unwrap();
    let mut state = model.initial_state();
    let snapshot = state.clone();
    assert!(
        model
            .advance(&mut state, 86400)
            .unwrap_err()
            .contains("substep limit")
    );
    assert_eq!(state, snapshot);
}

#[test]
fn batching_fixed_coupled_ticks_does_not_change_the_physical_state() {
    let model = model(Settings::default());
    let mut daily = model.initial_state();
    let mut hourly = daily.clone();
    for _ in 0..3 {
        model.advance(&mut daily, 86400).unwrap();
        for _ in 0..24 {
            model.advance(&mut hourly, 3600).unwrap();
        }
    }
    assert_eq!(daily, hourly);
}
