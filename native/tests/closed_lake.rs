use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Model, ReferenceWaterPool, Settings, SoilNumerics, SurfaceNumerics, TerminalNumerics,
        closed_lake::{Demand, Layout, Liquid},
    },
};

fn recipe(seed: &str, coverage: f64) -> Recipe {
    let mut r: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    r.seed = seed.into();
    r.subdivision = 2;
    r.radius_meters = 6_371_000.;
    r.water = planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
    r
}
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
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-10 * a.abs().max(b.abs()).max(1.),
        "{a} != {b}"
    );
}

#[test]
fn generated_leaf_footprints_are_exclusive_dry_and_reconstruct_direct_prisms() {
    let mut tested = 0;
    for seed in ["seasonal-reference", "moisture-coast", "moisture-interior"] {
        let world = World::generate(recipe(seed, 0.71)).unwrap();
        let before = serde_json::to_string(&world.basins).unwrap();
        let layout = Layout::from_world(&world).unwrap();
        let mut seen = vec![false; world.surface.areas.len()];
        let closed = world
            .drainage
            .receivers
            .iter()
            .enumerate()
            .filter(|(i, r)| **r as usize == *i && world.water.body_ids[*i] == 0)
            .count();
        assert_eq!(layout.lakes().len(), closed);
        for lake in layout.lakes() {
            tested += 1;
            let node = &world.basins.nodes()[lake.basin_node()];
            assert!(node.children.is_empty());
            for &i in lake.regions() {
                assert!(!seen[i]);
                seen[i] = true;
                assert_eq!(world.water.body_ids[i], 0);
                assert_eq!(world.basins.region_nodes()[i], lake.basin_node());
            }
            let volume = lake.capacity_cubic_meters().unwrap() * 0.25;
            let liquid = Liquid {
                high_kilograms: volume * 1000.,
                low_kilograms: 0.,
            };
            let s = lake.surface(liquid).unwrap();
            let h = s.height_above_minimum_meters.unwrap();
            let z = world.terrain.elevation[lake.terminal_region()];
            let mut expected = 0.;
            let mut area = 0.;
            let mut support = Vec::new();
            for &i in lake.regions() {
                let depth = (h - (world.terrain.elevation[i] - z)).max(0.);
                expected += world.surface.areas[i] * depth;
                if depth > 0. {
                    area += world.surface.areas[i];
                    support.push(i);
                }
            }
            close(expected, s.volume_cubic_meters);
            close(area, s.exposed_area_square_meters);
            assert_eq!(support, s.exposed_regions);
            let forcing = lake
                .regions()
                .iter()
                .map(|_| Demand {
                    temperature_celsius: 15.,
                    potential_kilograms_per_square_meter: 0.25,
                })
                .collect::<Vec<_>>();
            let e = lake.evaporate(liquid, &forcing, true).unwrap();
            close(e.actual_evaporation_kilograms, area * 0.25);
            close(
                e.remaining_liquid.high_kilograms
                    + e.remaining_liquid.low_kilograms
                    + e.actual_evaporation_kilograms,
                liquid.high_kilograms,
            );
            for (&i, &grant) in lake.regions().iter().zip(&e.regional_grants_kilograms) {
                assert_eq!(grant > 0., support.contains(&i));
            }
        }
        assert_eq!(before, serde_json::to_string(&world.basins).unwrap());
    }
    assert!(tested > 0);
}

#[test]
fn captures_and_candidate_evaporation_do_not_change_native_trajectory_or_replay() {
    let world = World::generate(recipe("seasonal-reference", 0.71)).unwrap();
    let layout = Layout::from_world(&world).unwrap();
    let model =
        Model::from_world(&world, settings(), Default::default(), Default::default()).unwrap();
    let mut state = model.initial_state();
    let mut control = state.clone();
    let mut positive = false;
    for _ in 0..40 {
        model.advance(&mut state, 86400).unwrap();
        model.advance(&mut control, 86400).unwrap();
        let text = serde_json::to_string(&state.checkpoint()).unwrap();
        let observed = layout.capture(&model, &state).unwrap();
        for (lake, o) in layout.lakes().iter().zip(observed) {
            if o.liquid.high_kilograms > 0. {
                positive = true;
            }
            let cp = state.checkpoint();
            assert_eq!(
                o.liquid.high_kilograms,
                cp.terminal_water_kilograms[o.terminal_region]
            );
            assert_eq!(
                o.liquid.low_kilograms,
                cp.terminal_low_kilograms.unwrap()[o.terminal_region]
            );
            if o.surface.is_some() {
                let demand = vec![
                    Demand {
                        temperature_celsius: 10.,
                        potential_kilograms_per_square_meter: 1.
                    };
                    lake.regions().len()
                ];
                lake.evaporate(o.liquid, &demand, true).unwrap();
            }
        }
        assert_eq!(text, serde_json::to_string(&state.checkpoint()).unwrap());
        assert_eq!(state, control);
    }
    assert!(positive);
    let normals = planimulation_core::seasonal_temperature::Normals::from_world(
        &world,
        model.temperature_settings(),
    )
    .unwrap();
    let checkpoint = state.checkpoint();
    for (index, lake) in layout.lakes().iter().enumerate() {
        for month in [0, 3, 6, 9] {
            let response = -(-300_f64 / model.settings().evaporation_response_seconds).exp_m1();
            let forcing:Vec<_> = lake.regions().iter().map(|&i| {
                let t = normals.monthly_temperature_celsius[month][i];
                let capacity = planimulation_core::seasonal_moisture::saturation_column_kilograms_per_square_meter(t, model.settings().effective_vapor_depth_meters).unwrap()*world.surface.areas[i];
                Demand { temperature_celsius:t, potential_kilograms_per_square_meter:(capacity-checkpoint.vapor_kilograms[i]).max(0.)*response/world.surface.areas[i] }
            }).collect();
            let i = lake.terminal_region();
            let expected = lake
                .evaporate(
                    Liquid {
                        high_kilograms: checkpoint.terminal_water_kilograms[i],
                        low_kilograms: checkpoint.terminal_low_kilograms.as_ref().unwrap()[i],
                    },
                    &forcing,
                    true,
                )
                .unwrap();
            assert_eq!(
                layout.probe(&model, &state, index, month, 300).unwrap(),
                expected
            );
        }
    }
    for (lake, month, seconds) in [(usize::MAX, 0, 300), (0, 12, 300), (0, 0, 0), (0, 0, 3601)] {
        assert!(layout.probe(&model, &state, lake, month, seconds).is_err());
    }
    assert_eq!(checkpoint, state.checkpoint());
    let (resumed_model, mut resumed) = Model::restore(
        serde_json::from_str(&serde_json::to_string(&state.checkpoint()).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        layout.capture(&model, &state).unwrap(),
        layout.capture(&resumed_model, &resumed).unwrap()
    );
    model.advance(&mut state, 3600).unwrap();
    resumed_model.advance(&mut resumed, 3600).unwrap();
    assert_eq!(state, resumed);
    let other = World::generate(recipe("foreign-lake", 0.71)).unwrap();
    let foreign =
        Model::from_world(&other, settings(), Default::default(), Default::default()).unwrap();
    assert!(layout.capture(&foreign, &foreign.initial_state()).is_err());
    let legacy = Model::from_world(
        &world,
        Settings::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert!(layout.capture(&legacy, &legacy.initial_state()).is_err());
}

#[test]
fn dry_and_fully_wet_worlds_do_not_invent_ocean_or_duplicate_owners() {
    for coverage in [0., 1.] {
        let world = World::generate(recipe("lake-empty", coverage)).unwrap();
        let layout = Layout::from_world(&world).unwrap();
        let model =
            Model::from_world(&world, settings(), Default::default(), Default::default()).unwrap();
        let observations = layout.capture(&model, &model.initial_state()).unwrap();
        assert_eq!(observations.len(), layout.lakes().len());
        assert_eq!(observations.is_empty(), coverage == 1.);
        for o in observations {
            assert_eq!(o.liquid.high_kilograms, 0.);
            assert_eq!(o.surface.unwrap().exposed_area_square_meters, 0.);
        }
    }
}

#[test]
fn constructed_minimum_plateau_has_one_closed_owner_and_no_fictitious_outlet() {
    // Explicitly synthetic geometry, not a recipe-compatible seasonal checkpoint.
    let mut world = World::generate(recipe("lake-flat-geometry", 0.)).unwrap();
    world.terrain.elevation.fill(16.);
    world.basins =
        planimulation_core::basins::Basins::build(&world.surface, &world.terrain.elevation)
            .unwrap();
    world.drainage = planimulation_core::drainage::Drainage::build(
        &world.surface,
        &world.terrain.elevation,
        &world.water.body_ids,
    );
    let layout = Layout::from_world(&world).unwrap();
    assert_eq!(layout.lakes().len(), 1);
    let lake = &layout.lakes()[0];
    assert_eq!(lake.terminal_region(), 0);
    assert_eq!(lake.capacity_cubic_meters(), None);
    assert_eq!(lake.regions().len(), world.surface.areas.len());
    let area = planimulation_core::moisture_transport::total_mass(&world.surface.areas);
    let s = lake
        .surface(Liquid {
            high_kilograms: area * 1000.,
            low_kilograms: 0.,
        })
        .unwrap();
    close(s.height_above_minimum_meters.unwrap(), 1.);
    assert_eq!(s.exposed_area_square_meters, area);
    assert_eq!(s.exposed_regions.len(), world.surface.areas.len());
    // A wet and dry minimum in one leaf cannot be assigned separate lake/ocean
    // ownership by this bounded component. Refuse the synthetic overlap.
    world.water.body_ids[1] = 1;
    world.drainage = planimulation_core::drainage::Drainage::build(
        &world.surface,
        &world.terrain.elevation,
        &world.water.body_ids,
    );
    assert!(Layout::from_world(&world).is_err());
}
