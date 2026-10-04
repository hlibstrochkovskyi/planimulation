use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Model, ReferenceWaterPool, Settings, SoilNumerics, State, SurfaceNumerics,
        TerminalNumerics,
        body_preparation::{Criteria, Monitor, YEAR_SECONDS},
        preparation,
    },
};

fn settings() -> Settings {
    Settings {
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        max_coupled_step_seconds: 900,
        ..Default::default()
    }
}
fn recipe() -> Recipe {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.subdivision = 1;
    recipe.radius_meters = 1_000_000.;
    recipe
}
fn build(recipe: Recipe, settings: Settings) -> (World, Model) {
    let world = World::generate(recipe).unwrap();
    let model =
        Model::from_world(&world, settings, Default::default(), Default::default()).unwrap();
    (world, model)
}
fn year(model: &Model, state: &mut State) {
    for _ in 0..365 {
        model.advance(state, 86400).unwrap();
    }
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}

#[test]
fn observed_and_unobserved_trajectories_keep_identical_complete_state_and_body_ownership() {
    let (world, model) = build(recipe(), settings());
    let mut state = model.initial_state();
    let mut control = state.clone();
    let mut monitor = Monitor::new(&model, &state, Default::default()).unwrap();
    for y in 1..=2 {
        let start = state.checkpoint();
        year(&model, &mut state);
        year(&model, &mut control);
        let text = serde_json::to_string(&state.checkpoint()).unwrap();
        let result = monitor.observe_year(&state).unwrap();
        assert_eq!(text, serde_json::to_string(&state.checkpoint()).unwrap());
        assert_eq!(state, control);
        assert_eq!(result.end_seconds, y * YEAR_SECONDS);
        assert_eq!(
            result.meets_recorded_stationarity_criteria.is_none(),
            y == 1
        );
        assert!(result.positive_atmospheric_cycling_observed);
        let cp = state.checkpoint();
        for (b, body) in result.reference_bodies.iter().enumerate() {
            let area: f64 = world
                .water
                .body_ids
                .iter()
                .enumerate()
                .filter(|(_, id)| **id == body.reference_body_id)
                .map(|(i, _)| world.surface.areas[i])
                .sum();
            close(area, body.reference_area_square_meters);
            assert_eq!(
                body.stock_high_kilograms,
                cp.reference_body_high_kilograms.as_ref().unwrap()[b]
            );
            assert_eq!(
                body.stock_low_kilograms,
                cp.reference_body_low_kilograms.as_ref().unwrap()[b]
            );
            let high_delta = cp.reference_body_high_kilograms.as_ref().unwrap()[b]
                - start.reference_body_high_kilograms.as_ref().unwrap()[b];
            let low_delta = cp.reference_body_low_kilograms.as_ref().unwrap()[b]
                - start.reference_body_low_kilograms.as_ref().unwrap()[b];
            close(
                body.component_l1_change_kilograms,
                high_delta.abs() + low_delta.abs(),
            );
            close(body.signed_stock_change_kilograms, high_delta + low_delta);
            let f = body.annual_flow_kilograms;
            close(
                body.signed_stock_change_kilograms,
                f[0] + f[1] + f[2] - f[3],
            );
        }
        let regions: f64 = result
            .regional_component_l1_change_by_stock_kilograms
            .iter()
            .sum();
        close(
            result.relative_total_inventory_component_l1_change,
            (regions + result.reference_body_component_l1_change_kilograms)
                / model
                    .budget(&state)
                    .unwrap()
                    .initial_mobile_water_kilograms
                    .max(1.),
        );
        close(
            result.closed_dry_terminal_water_kilograms,
            model.budget(&state).unwrap().terminal_water_kilograms,
        );
        for (i, &id) in world.water.body_ids.iter().enumerate() {
            if id != 0 {
                assert_eq!(cp.surface_kilograms[i], 0.);
                assert_eq!(cp.terminal_water_kilograms[i], 0.);
            }
        }
    }
    let (restored_model, mut restored) = Model::restore(
        serde_json::from_str(&serde_json::to_string(&state.checkpoint()).unwrap()).unwrap(),
    )
    .unwrap();
    model.advance(&mut state, 3600).unwrap();
    restored_model.advance(&mut restored, 3600).unwrap();
    assert_eq!(state, restored);
}

#[test]
fn quiet_empty_and_all_dry_cycles_can_be_candidates_without_climate_readiness() {
    for (coverage, depth) in [(0.71, 0.), (0.71, 1.), (0., 1.)] {
        let mut recipe = recipe();
        recipe.subdivision = 0;
        recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
        let (_, model) = build(
            recipe,
            Settings {
                initial_active_surface_depth_meters: depth,
                evaporation_enabled: false,
                precipitation_enabled: false,
                ..settings()
            },
        );
        let mut state = model.initial_state();
        let mut monitor = Monitor::new(&model, &state, Default::default()).unwrap();
        for y in 1..=4 {
            year(&model, &mut state);
            let report = monitor.observe_year(&state).unwrap();
            assert_eq!(report.relative_total_inventory_component_l1_change, 0.);
            assert_eq!(report.maximum_body_change_id, None);
            assert_eq!(report.annual_flow_kilograms, [0.; 5]);
            assert!(!report.positive_atmospheric_cycling_observed);
            assert_eq!(report.stationarity_candidate, y == 4);
            assert_eq!(report.consecutive_matching_comparisons, y - 1);
            assert_eq!(report.reference_bodies.is_empty(), coverage == 0.);
        }
    }
}

#[test]
fn invalid_observations_and_restored_anchors_do_not_invent_or_change_history() {
    let (_, model) = build(recipe(), settings());
    let mut state = model.initial_state();
    assert!(
        Monitor::new(
            &model,
            &state,
            Criteria {
                relative_inventory_component_change: f64::NAN,
                ..Default::default()
            }
        )
        .is_err()
    );
    let mut monitor = Monitor::new(&model, &state, Default::default()).unwrap();
    assert!(monitor.observe_year(&state).is_err());
    year(&model, &mut state);
    let saved = state.clone();
    year(&model, &mut state);
    assert!(monitor.observe_year(&state).is_err());
    assert_eq!(monitor.last_observed_seconds(), 0);
    let first = monitor.observe_year(&saved).unwrap();
    assert!(monitor.observe_year(&saved).is_err());
    let second = monitor.observe_year(&state).unwrap();
    let mut fresh = Monitor::new(&model, &model.initial_state(), Default::default()).unwrap();
    assert_eq!(first, fresh.observe_year(&saved).unwrap());
    assert_eq!(second, fresh.observe_year(&state).unwrap());
    let mut foreign_recipe = recipe();
    foreign_recipe.seed = "body-diagnostic-foreign".into();
    let (_, foreign_model) = build(foreign_recipe, settings());
    let mut foreign = foreign_model.initial_state();
    year(&foreign_model, &mut foreign);
    let mut empty_history =
        Monitor::new(&model, &model.initial_state(), Default::default()).unwrap();
    assert!(empty_history.observe_year(&foreign).is_err());
    assert_eq!(empty_history.last_observed_seconds(), 0);
    let (resumed_model, resumed) = Model::restore(saved.checkpoint()).unwrap();
    let mut reanchored = Monitor::new(&resumed_model, &resumed, Default::default()).unwrap();
    let report = reanchored.observe_year(&state).unwrap();
    assert_eq!(report.start_seconds, YEAR_SECONDS);
    assert_eq!(report.flow_change_from_previous_year, None);
    assert_eq!(report.meets_recorded_stationarity_criteria, None);
    model.advance(&mut state, 3600).unwrap();
    assert!(Monitor::new(&model, &state, Default::default()).is_err());
    assert!(monitor.observe_year(&state).is_err());
}

#[test]
fn diagnostic_versions_cannot_silently_inherit_incompatible_owner_interpretations() {
    for version in 3..=8 {
        let mut settings = Settings::default();
        if version >= 4 {
            settings.orography = Some(Default::default());
        }
        if version >= 5 {
            settings.soil_numerics = Some(SoilNumerics::Compensated);
        }
        if version >= 6 {
            settings.surface_numerics = Some(SurfaceNumerics::Compensated);
        }
        if version >= 7 {
            settings.terminal_numerics = Some(TerminalNumerics::Compensated);
        }
        if version >= 8 {
            settings.reference_water_pool = Some(ReferenceWaterPool::FastConnectedBody);
        }
        let (_, model) = build(recipe(), settings);
        let state = model.initial_state();
        assert_eq!(
            Monitor::new(&model, &state, Default::default()).is_ok(),
            version == 8
        );
        assert_eq!(
            preparation::Monitor::new(&model, &state, Default::default()).is_ok(),
            version <= 7
        );
    }
}
