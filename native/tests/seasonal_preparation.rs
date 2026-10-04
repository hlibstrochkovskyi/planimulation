use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Model, Settings, SoilNumerics, SurfaceNumerics, TerminalNumerics,
        preparation::{Criteria, Monitor, ThermalRegime, YEAR_SECONDS},
    },
};

fn model(settings: Settings) -> Model {
    model_with_temperature(settings, 15., 23.44)
}
fn model_with_temperature(settings: Settings, reference_temperature: f64, tilt: f64) -> Model {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.subdivision = 1;
    recipe.radius_meters = 1_000_000.;
    Model::from_world(
        &World::generate(recipe).unwrap(),
        settings,
        planimulation_core::seasonal_temperature::Settings {
            reference_temperature_celsius: reference_temperature,
            axial_tilt_degrees: tilt,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap()
}
fn precise() -> Settings {
    Settings {
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        max_coupled_step_seconds: 900,
        ..Default::default()
    }
}

#[test]
fn supported_model_representations_preserve_their_complete_state() {
    let mut settings = Settings {
        initial_active_surface_depth_meters: 0.,
        ..Default::default()
    };
    for version in 3..=7 {
        match version {
            4 => settings.orography = Some(Default::default()),
            5 => settings.soil_numerics = Some(SoilNumerics::Compensated),
            6 => settings.surface_numerics = Some(SurfaceNumerics::Compensated),
            7 => settings.terminal_numerics = Some(TerminalNumerics::Compensated),
            _ => {}
        }
        let model = model(settings);
        assert_eq!(
            model.model_version(),
            format!("seasonal-moisture-{version}")
        );
        let mut state = model.initial_state();
        let mut monitor = Monitor::new(&model, &state, Default::default()).unwrap();
        advance_year(&model, &mut state);
        let before = state.clone();
        let report = monitor.observe_year(&state).unwrap();
        assert_eq!(state, before);
        assert_eq!(report.annual_flow_kilograms, [0.; 5]);
        assert_eq!(
            report.state_change.relative_inventory_component_l1_change,
            0.
        );
        assert_eq!(report.meets_recorded_stationarity_criteria, None);
    }
}
fn advance_year(model: &Model, state: &mut planimulation_core::seasonal_moisture::State) {
    for _ in 0..365 {
        model.advance(state, 86400).unwrap();
    }
}

#[test]
fn observations_preserve_complete_state_and_identify_cold_stock_ownership() {
    // Zero tilt gives genuinely always-cold polar forcing and warm low latitudes;
    // the stock-ownership test must not assume a random world contains cold traps.
    let model = model_with_temperature(precise(), 15., 0.);
    let mut state = model.initial_state();
    let mut monitor = Monitor::new(&model, &state, Default::default()).unwrap();
    assert!(
        monitor
            .thermal_regimes()
            .contains(&ThermalRegime::AlwaysCold)
    );
    advance_year(&model, &mut state);
    let text = serde_json::to_string(&state.checkpoint()).unwrap();
    let report = monitor.observe_year(&state).unwrap();
    assert_eq!(serde_json::to_string(&state.checkpoint()).unwrap(), text);
    assert_eq!(report.end_seconds, YEAR_SECONDS);
    assert_eq!(report.meets_recorded_stationarity_criteria, None);
    assert!(!report.stationarity_candidate);
    assert!(report.positive_atmospheric_cycling_observed);
    assert!(report.structurally_cold_locked_kilograms > 0.);
    let fraction = report
        .structurally_cold_locked_fraction_of_initial_water
        .unwrap();
    assert!((0. ..=1.).contains(&fraction));
    let check = |a: f64, b: f64| assert!((a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.));
    check(
        report.cold_snow_stock_change_kilograms,
        report.annual_cold_snowfall_kilograms,
    );
    check(
        report.cold_terminal_stock_change_kilograms,
        report.annual_cold_terminal_delivery_kilograms,
    );
    let cp = state.checkpoint();
    let expected: f64 = (0..state.surface_kilograms().len())
        .filter(|&i| monitor.thermal_regimes()[i] == ThermalRegime::AlwaysCold)
        .map(|i| cp.snow_kilograms[i] + cp.snow_low_kilograms.as_ref().unwrap()[i])
        .sum();
    check(
        expected,
        report.thermal_inventories[0].owned_stock_kilograms[1],
    );
    let total: f64 = report
        .thermal_inventories
        .iter()
        .flat_map(|g| g.owned_stock_kilograms)
        .sum();
    check(
        total,
        model.budget(&state).unwrap().initial_mobile_water_kilograms,
    );
    let (resumed_model, mut resumed) =
        Model::restore(serde_json::from_str(&text).unwrap()).unwrap();
    model.advance(&mut state, 3600).unwrap();
    resumed_model.advance(&mut resumed, 3600).unwrap();
    assert_eq!(state, resumed);
}

#[test]
fn quiet_and_zero_water_cycles_can_be_stationary_but_are_not_active_climates() {
    for depth in [0., 1.] {
        let model = model(Settings {
            initial_active_surface_depth_meters: depth,
            evaporation_enabled: false,
            precipitation_enabled: false,
            ..Default::default()
        });
        let mut state = model.initial_state();
        let criteria = Criteria {
            required_consecutive_comparisons: 2,
            ..Default::default()
        };
        let mut monitor = Monitor::new(&model, &state, criteria).unwrap();
        for year in 1..=3 {
            advance_year(&model, &mut state);
            let report = monitor.observe_year(&state).unwrap();
            assert_eq!(
                report.state_change.relative_inventory_component_l1_change,
                0.
            );
            assert_eq!(report.annual_flow_kilograms, [0.; 5]);
            assert!(!report.positive_atmospheric_cycling_observed);
            assert_eq!(report.stationarity_candidate, year == 3);
            assert_eq!(report.consecutive_matching_comparisons, year - 1);
            assert_eq!(
                report
                    .structurally_cold_locked_fraction_of_initial_water
                    .is_none(),
                depth == 0.
            );
        }
    }
}

#[test]
fn bad_observation_clocks_origins_and_criteria_are_transactional() {
    let model = model(precise());
    let mut state = model.initial_state();
    for criteria in [
        Criteria {
            relative_inventory_component_change: f64::NAN,
            ..Default::default()
        },
        Criteria {
            maximum_local_component_change_millimeters: -1.,
            ..Default::default()
        },
        Criteria {
            relative_annual_flow_change: 2.,
            ..Default::default()
        },
        Criteria {
            absolute_annual_flow_change_millimeters: f64::INFINITY,
            ..Default::default()
        },
        Criteria {
            required_consecutive_comparisons: 0,
            ..Default::default()
        },
    ] {
        assert!(Monitor::new(&model, &state, criteria).is_err());
    }
    let mut monitor = Monitor::new(&model, &state, Default::default()).unwrap();
    assert!(monitor.observe_year(&state).is_err());
    assert_eq!(monitor.last_observed_seconds(), 0);
    advance_year(&model, &mut state);
    let one_year = state.clone();
    advance_year(&model, &mut state);
    assert!(monitor.observe_year(&state).is_err());
    assert_eq!(monitor.last_observed_seconds(), 0);
    let first = monitor.observe_year(&one_year).unwrap();
    assert!(monitor.observe_year(&one_year).is_err());
    assert_eq!(monitor.last_observed_seconds(), YEAR_SECONDS);
    let second = monitor.observe_year(&state).unwrap();
    assert!(second.meets_recorded_stationarity_criteria.is_some());
    let mut separate = Monitor::new(&model, &model.initial_state(), Default::default()).unwrap();
    assert_eq!(first, separate.observe_year(&one_year).unwrap());
    assert_eq!(second, separate.observe_year(&state).unwrap());
    model.advance(&mut state, 3600).unwrap();
    assert!(Monitor::new(&model, &state, Default::default()).is_err());
    let other = model_with_other_settings();
    assert!(Monitor::new(&other, &one_year, Default::default()).is_err());
}
fn model_with_other_settings() -> Model {
    model(Settings::default())
}

#[test]
fn reanchoring_at_a_saved_year_does_not_invent_previous_flux_comparisons() {
    let model = model(Settings {
        initial_active_surface_depth_meters: 0.,
        ..Default::default()
    });
    let mut state = model.initial_state();
    advance_year(&model, &mut state);
    let saved = state.clone();
    let mut monitor = Monitor::new(&model, &saved, Default::default()).unwrap();
    advance_year(&model, &mut state);
    let report = monitor.observe_year(&state).unwrap();
    assert_eq!(report.start_seconds, YEAR_SECONDS);
    assert_eq!(report.flow_change_from_previous_year, None);
    assert_eq!(report.meets_recorded_stationarity_criteria, None);
    assert_eq!(report.consecutive_matching_comparisons, 0);
}
