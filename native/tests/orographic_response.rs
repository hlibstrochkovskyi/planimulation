use planimulation_core::{
    Recipe, Surface, World,
    orographic_response::{Settings, deposition, terrain_gradients, uplift},
    seasonal_moisture::{self, Model},
    seasonal_temperature, seasonal_wind,
};

const RADIUS: f64 = 1_000_000.;
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(x, y)| x * y).sum()
}
fn model(world: &World, settings: seasonal_moisture::Settings) -> Model {
    Model::from_world(
        world,
        settings,
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings::default(),
    )
    .unwrap()
}
fn world() -> World {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.subdivision = 2;
    World::generate(recipe).unwrap()
}
fn enabled() -> seasonal_moisture::Settings {
    seasonal_moisture::Settings {
        orography: Some(Settings::default()),
        ..Default::default()
    }
}

#[test]
fn spherical_gradient_converges_to_an_analytic_tangent_field() {
    let mut previous = f64::INFINITY;
    for level in 1..=4 {
        let surface = Surface::build(level, RADIUS);
        let heights: Vec<_> = surface.centers.iter().map(|p| 4096. * p[2]).collect();
        let gradients = terrain_gradients(&surface, RADIUS, &heights).unwrap();
        let mut error = 0.;
        for ((&p, &g), &area) in surface.centers.iter().zip(&gradients).zip(&surface.areas) {
            let exact: [f64; 3] =
                std::array::from_fn(|k| 4096. / RADIUS * (f64::from(k == 2) - p[2] * p[k]));
            error += dot(
                std::array::from_fn(|k| g[k] - exact[k]),
                std::array::from_fn(|k| g[k] - exact[k]),
            ) * area;
            assert!(dot(p, g).abs() < 1e-16);
        }
        let relative_rms = (error / surface.areas.iter().sum::<f64>()).sqrt() / (4096. / RADIUS);
        assert!(
            relative_rms < previous * 0.65,
            "level {level}: {relative_rms}, previous {previous}"
        );
        previous = relative_rms;
    }
    assert!(previous < 0.005);
}

#[test]
fn gradients_ignore_datum_and_visual_projection_and_scale_with_physical_radius() {
    let mut surface = Surface::build(3, RADIUS);
    let heights: Vec<_> = surface.centers.iter().map(|p| 4096. * p[2]).collect();
    let original = terrain_gradients(&surface, RADIUS, &heights).unwrap();
    let translated = terrain_gradients(
        &surface,
        RADIUS,
        &heights.iter().map(|h| h + 8192.).collect::<Vec<_>>(),
    )
    .unwrap();
    let scaled = terrain_gradients(&surface, RADIUS * 2., &heights).unwrap();
    for ((a, b), c) in original.iter().zip(translated).zip(scaled) {
        for k in 0..3 {
            assert!((a[k] - b[k]).abs() < 1e-17);
            assert_eq!(a[k] / 2., c[k]);
        }
    }
    // Rotate through a coordinate permutation, including original poles/seam.
    for p in &mut surface.centers {
        *p = [p[2], p[0], p[1]];
    }
    let rotated = terrain_gradients(&surface, RADIUS, &heights).unwrap();
    for (a, b) in original.iter().zip(rotated) {
        for (x, y) in [a[2], a[0], a[1]].into_iter().zip(b) {
            assert!((x - y).abs() < 1e-17);
        }
    }
    assert!(
        terrain_gradients(&surface, RADIUS, &vec![500.; heights.len()])
            .unwrap()
            .iter()
            .flatten()
            .all(|v| *v == 0.)
    );
}

#[test]
fn uplift_is_directional_bounded_and_rejects_nonphysical_vectors() {
    let settings = Settings::default();
    let p = [1., 0., 0.];
    let slope = [0., 0.01, 0.];
    let wind = [0., 20., 0.];
    let w = uplift(p, slope, wind).unwrap();
    assert_eq!(w, 0.2);
    assert_eq!(settings.additional_rate_per_second(w).unwrap(), 0.0002);
    assert_eq!(settings.additional_rate_per_second(-w).unwrap(), 0.);
    assert_eq!(
        settings
            .additional_rate_per_second(uplift(p, slope, [0.; 3]).unwrap())
            .unwrap(),
        0.
    );
    assert_eq!(
        Settings {
            strength: 0.,
            ..settings
        }
        .additional_rate_per_second(w)
        .unwrap(),
        0.
    );
    assert!(uplift(p, slope, [1., 20., 0.]).is_err());
    assert!(uplift([2., 0., 0.], slope, wind).is_err());
    assert!(settings.additional_rate_per_second(f64::NAN).is_err());
    assert!(uplift(p, [0., f64::MAX, f64::MAX], wind).is_err());
    for bad in [
        Settings {
            strength: -1.,
            ..settings
        },
        Settings {
            strength: f64::INFINITY,
            ..settings
        },
        Settings {
            uplift_response_height_meters: 0.,
            ..settings
        },
    ] {
        assert!(bad.validate().is_err());
    }
}

#[test]
fn malformed_geometry_is_rejected_without_panics() {
    let surface = Surface::build(1, RADIUS);
    let heights = vec![0.; surface.centers.len()];
    assert!(terrain_gradients(&surface, 0., &heights).is_err());
    let coarse = Surface::build(0, RADIUS);
    assert!(terrain_gradients(&coarse, f64::MAX, &vec![0.; coarse.centers.len()]).is_err());
    assert!(terrain_gradients(&surface, RADIUS, &heights[1..]).is_err());
    let mut bad = Surface::build(1, RADIUS);
    bad.offsets[2] = u32::MAX;
    assert!(terrain_gradients(&bad, RADIUS, &heights).is_err());
    let mut bad = Surface::build(1, RADIUS);
    bad.neighbors[0] = u32::MAX;
    assert!(terrain_gradients(&bad, RADIUS, &heights).is_err());
    let mut bad = Surface::build(1, RADIUS);
    bad.centers[0][0] = f64::NAN;
    assert!(terrain_gradients(&bad, RADIUS, &heights).is_err());
    let mut overflowing = heights;
    overflowing[0] = f64::MAX;
    overflowing[surface.neighbors[0] as usize] = -f64::MAX;
    assert!(terrain_gradients(&surface, RADIUS, &overflowing).is_err());
}

#[test]
fn deposition_is_exact_excess_relaxation_with_a_single_finite_donor() {
    let rain = deposition(100., 30., 0.001, 0.002, 100.).unwrap();
    assert!((rain - 70. * (1. - (-0.3_f64).exp())).abs() < 1e-14);
    assert!(rain > deposition(100., 30., 0.001, 0., 100.).unwrap());
    let mut vapor = 100.;
    let mut recipient = 0.;
    for _ in 0..100 {
        let p = deposition(vapor, 30., 0.001, 0.002, 1.).unwrap();
        vapor -= p;
        recipient += p;
    }
    assert!((recipient - rain).abs() < 1e-12);
    assert!((vapor + recipient - 100.).abs() < 1e-12);
    assert_eq!(deposition(29., 30., 0.001, 1., 100.).unwrap(), 0.);
    assert_eq!(deposition(100., 30., 0., 0., 100.).unwrap(), 0.);
    assert!(deposition(f64::NAN, 30., 0.001, 0., 100.).is_err());
    assert!(deposition(100., 30., f64::MAX, f64::MAX, 1.).is_err());
}

#[test]
fn zero_strength_keeps_legacy_physics_and_legacy_checkpoint_shape_exact() {
    let world = world();
    let legacy = model(&world, Default::default());
    let control = model(
        &world,
        seasonal_moisture::Settings {
            orography: Some(Settings {
                strength: 0.,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    let mut a = legacy.initial_state();
    let mut b = control.initial_state();
    for _ in 0..40 {
        let x = legacy.advance(&mut a, 86400).unwrap();
        let y = control.advance(&mut b, 86400).unwrap();
        assert_eq!(
            a.owned_stocks().collect::<Vec<_>>(),
            b.owned_stocks().collect::<Vec<_>>()
        );
        assert_eq!(x.budget, y.budget);
        assert_eq!(x.precipitation_kilograms, y.precipitation_kilograms);
    }
    let cp = serde_json::to_value(a.checkpoint()).unwrap();
    assert_eq!(cp["schemaVersion"], 3);
    assert!(cp.get("orographicModelVersion").is_none());
    assert!(cp["settings"].get("orography").is_none());
}

#[test]
fn enabled_model_preserves_water_batching_and_exact_checkpoint_continuation() {
    let world = world();
    let model = model(&world, enabled());
    assert_eq!(
        model.model_version(),
        seasonal_moisture::OROGRAPHIC_MODEL_VERSION
    );
    let mut daily = model.initial_state();
    let mut hourly = daily.clone();
    model.advance(&mut daily, 86400).unwrap();
    for _ in 0..24 {
        model.advance(&mut hourly, 3600).unwrap();
    }
    assert_eq!(daily, hourly);
    for _ in 1..365 {
        let step = model.advance(&mut daily, 86400).unwrap();
        assert!(
            step.budget.residual_kilograms.abs() / step.budget.initial_mobile_water_kilograms
                < 1e-12
        );
        assert!(step.budget.maximum_relative_local_surface_ledger_residual < 1e-12);
    }
    assert!(
        model
            .budget(&daily)
            .unwrap()
            .cumulative_precipitation_kilograms
            > 0.
    );
    let encoded = serde_json::to_vec(&daily.checkpoint()).unwrap();
    let (restored_model, mut restored) =
        Model::restore(serde_json::from_slice(&encoded).unwrap()).unwrap();
    assert_eq!(daily, restored);
    model.advance(&mut daily, 86400).unwrap();
    restored_model.advance(&mut restored, 86400).unwrap();
    assert_eq!(daily, restored);
}

#[test]
fn checkpoint_requires_matching_schema_settings_and_all_model_pins() {
    let model = model(&world(), enabled());
    let original = serde_json::to_value(model.initial_state().checkpoint()).unwrap();
    assert_eq!(original["schemaVersion"], 4);
    for (path, value) in [
        ("schemaVersion", serde_json::json!(3)),
        (
            "orographicModelVersion",
            serde_json::json!("orographic-response-2"),
        ),
        ("modelVersion", serde_json::json!("seasonal-moisture-3")),
        ("orographicModelVersion", serde_json::Value::Null),
    ] {
        let mut bad = original.clone();
        bad[path] = value;
        let parsed = serde_json::from_value(bad);
        assert!(parsed.is_err() || Model::restore(parsed.unwrap()).is_err());
    }
    let mut bad = original.clone();
    bad.as_object_mut()
        .unwrap()
        .remove("orographicModelVersion");
    assert!(Model::restore(serde_json::from_value(bad).unwrap()).is_err());
    let mut bad = original;
    bad["settings"]["orography"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<seasonal_moisture::Checkpoint>(bad).is_err());
}

#[test]
fn precipitation_switch_disables_upslope_exchange_as_well() {
    let model = model(
        &world(),
        seasonal_moisture::Settings {
            precipitation_enabled: false,
            ..enabled()
        },
    );
    let mut state = model.initial_state();
    for _ in 0..40 {
        let step = model.advance(&mut state, 86400).unwrap();
        assert!(step.precipitation_kilograms.iter().all(|v| *v == 0.));
        assert_eq!(step.budget.cumulative_precipitation_kilograms, 0.);
    }
}

#[test]
fn air_facing_water_surface_does_not_treat_bathymetry_as_mountains() {
    let mut recipe = world().recipe;
    recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction: 1. };
    let wet = World::generate(recipe).unwrap();
    let response = model(&wet, enabled());
    assert!(
        response
            .orographic_uplift()
            .unwrap()
            .iter()
            .flatten()
            .all(|w| *w == 0.)
    );
    let legacy = model(&wet, Default::default());
    let mut a = response.initial_state();
    let mut b = legacy.initial_state();
    response.advance(&mut a, 86400).unwrap();
    legacy.advance(&mut b, 86400).unwrap();
    assert!(a.owned_stocks().eq(b.owned_stocks()));
}

#[test]
fn extreme_upslope_response_rejects_excessive_work_without_state_changes() {
    let mut recipe = world().recipe;
    recipe.subdivision = 4;
    recipe.radius_meters = 100_000.;
    recipe.detail_amplitude_meters = 1000.;
    let world = World::generate(recipe).unwrap();
    let model = Model::from_world(
        &world,
        seasonal_moisture::Settings {
            orography: Some(Settings {
                strength: 2.,
                uplift_response_height_meters: 100.,
            }),
            ..Default::default()
        },
        seasonal_temperature::Settings::default(),
        seasonal_wind::Settings {
            trade_easterly_meters_per_second: 40.,
            midlatitude_westerly_meters_per_second: 40.,
            polar_easterly_meters_per_second: 40.,
            tropical_convergence_meters_per_second: 40.,
            midlatitude_poleward_meters_per_second: 40.,
            polar_equatorward_meters_per_second: 40.,
            ..Default::default()
        },
    )
    .unwrap();
    let mut state = model.initial_state();
    let before = state.clone();
    let error = model.advance(&mut state, 86400).unwrap_err();
    assert!(
        error.contains("4096") || error.contains("sub-second"),
        "{error}"
    );
    assert_eq!(state, before);
}
