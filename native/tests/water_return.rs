use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Model, Settings, SoilNumerics, SurfaceNumerics, TerminalNumerics, water_return::Audit,
    },
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

#[test]
fn ownership_partitions_actual_reference_bodies_and_closed_land_without_changing_state() {
    let (world, model) = build(recipe(), precise());
    let mut state = model.initial_state();
    let mut independent = state.clone();
    for _ in 0..32 {
        model.advance(&mut state, 86400).unwrap();
        let before = serde_json::to_string(&state.checkpoint()).unwrap();
        let report = Audit::capture(&model, &state, 300).unwrap();
        assert_eq!(before, serde_json::to_string(&state.checkpoint()).unwrap());
        model.advance(&mut independent, 86400).unwrap();
        assert_eq!(state, independent);
        let groups: Vec<_> = report
            .reference_bodies
            .iter()
            .map(|b| &b.inventory)
            .chain([&report.closed_dry_terminals, &report.other_dry_land])
            .collect();
        assert_eq!(
            groups.iter().map(|g| g.region_count).sum::<usize>(),
            world.surface.areas.len()
        );
        close(
            groups.iter().map(|g| g.area_square_meters).sum(),
            world.surface.areas.iter().sum(),
        );
        let budget = model.budget(&state).unwrap();
        for (k, expected) in [
            budget.surface_kilograms,
            budget.snow_kilograms,
            budget.soil_kilograms,
            budget.pending_runoff_kilograms,
            budget.terminal_water_kilograms,
            budget.vapor_kilograms,
        ]
        .into_iter()
        .enumerate()
        {
            close(
                groups.iter().map(|g| g.owned_stock_kilograms[k]).sum(),
                expected,
            );
        }
        let cp = state.checkpoint();
        for body in report.reference_bodies {
            let indices: Vec<_> = world
                .water
                .body_ids
                .iter()
                .enumerate()
                .filter_map(|(i, &id)| (id == body.reference_body_id).then_some(i))
                .collect();
            assert_eq!(indices.len(), body.inventory.region_count);
            close(
                body.inventory.area_square_meters,
                indices.iter().map(|&i| world.surface.areas[i]).sum(),
            );
            close(
                body.inventory.owned_stock_kilograms[4],
                indices
                    .iter()
                    .map(|&i| {
                        cp.terminal_water_kilograms[i]
                            + cp.terminal_low_kilograms.as_ref().unwrap()[i]
                    })
                    .sum(),
            );
            close(
                body.inventory.cumulative_transfer_kilograms[0],
                indices
                    .iter()
                    .map(|&i| cp.cumulative_runoff_transfers[i].terminal_delivery)
                    .sum(),
            );
            assert_eq!(body.frozen_monthly_liquid_probes.len(), 12);
            for p in body.frozen_monthly_liquid_probes {
                assert_eq!(p.probe_seconds, 300);
                assert!(
                    p.local_liquid_grant_kilograms <= p.potential_demand_kilograms * (1. + 1e-14)
                );
                assert!(p.perfect_body_sharing_cap_kilograms <= p.potential_demand_kilograms);
                assert!(
                    p.perfect_body_sharing_cap_kilograms
                        <= body.inventory.owned_stock_kilograms[0]
                            + body.inventory.owned_stock_kilograms[4]
                );
                close(
                    p.additional_sharing_cap_kilograms,
                    p.perfect_body_sharing_cap_kilograms - p.local_liquid_grant_kilograms,
                );
            }
        }
        assert_eq!(report.other_dry_land.owned_stock_kilograms[4], 0.);
        assert_eq!(report.other_dry_land.cumulative_transfer_kilograms[0], 0.);
    }
}

#[test]
fn empty_dry_and_disabled_evaporation_do_not_invent_access_or_flow() {
    for coverage in [0., 1.] {
        for disabled in [false, true] {
            let mut recipe = recipe();
            recipe.water =
                planimulation_core::water::WaterSettings::Coverage { fraction: coverage };
            let (world, model) = build(
                recipe,
                Settings {
                    initial_active_surface_depth_meters: 0.,
                    evaporation_enabled: !disabled,
                    ..Default::default()
                },
            );
            let state = model.initial_state();
            let report = Audit::capture(&model, &state, 300).unwrap();
            assert_eq!(report.reference_bodies.is_empty(), coverage == 0.);
            if coverage == 1. {
                assert_eq!(
                    report.reference_bodies[0].inventory.region_count,
                    world.surface.areas.len()
                );
            }
            for body in report.reference_bodies {
                assert_eq!(body.inventory.owned_stock_kilograms, [0.; 6]);
                assert_eq!(body.inventory.maximum_terminal_region, None);
                for p in body.frozen_monthly_liquid_probes {
                    assert_eq!(p.local_liquid_grant_kilograms, 0.);
                    assert_eq!(p.perfect_body_sharing_cap_kilograms, 0.);
                    if disabled {
                        assert_eq!(p.potential_demand_kilograms, 0.);
                    }
                }
            }
        }
    }
}

#[test]
fn checkpoint_restore_rebuilds_identical_immutable_body_ownership() {
    let (_, model) = build(recipe(), precise());
    let mut state = model.initial_state();
    for _ in 0..7 {
        model.advance(&mut state, 86400).unwrap();
    }
    let report = Audit::capture(&model, &state, 300).unwrap();
    let (restored_model, mut restored) = Model::restore(
        serde_json::from_str(&serde_json::to_string(&state.checkpoint()).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(
        report,
        Audit::capture(&restored_model, &restored, 300).unwrap()
    );
    model.advance(&mut state, 3600).unwrap();
    restored_model.advance(&mut restored, 3600).unwrap();
    assert_eq!(state, restored);
}

#[test]
fn invalid_probe_and_foreign_state_reject_without_mutation() {
    let (_, model) = build(recipe(), precise());
    let state = model.initial_state();
    let before = state.clone();
    for seconds in [0, 3601, u32::MAX] {
        assert!(Audit::capture(&model, &state, seconds).is_err());
    }
    let (_, other) = build(recipe(), Settings::default());
    assert!(Audit::capture(&other, &state, 300).is_err());
    assert_eq!(state, before);
}

#[test]
fn multiple_generated_water_bodies_have_separate_liquid_caps() {
    // Bounded structural fixture search, not a climate-success seed ensemble.
    let mut checked = 0;
    for fraction in [0.025, 0.05, 0.1, 0.15, 0.2, 0.25] {
        let mut recipe = recipe();
        recipe.subdivision = 2;
        recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction };
        let (world, model) = build(recipe, precise());
        if world.water.body_ids.iter().copied().max().unwrap() <= 1 {
            continue;
        }
        let state = model.initial_state();
        let report = Audit::capture(&model, &state, 300).unwrap();
        assert!(report.reference_bodies.len() > 1);
        for body in &report.reference_bodies {
            let expected: f64 = world
                .water
                .body_ids
                .iter()
                .enumerate()
                .filter_map(|(i, &id)| {
                    (id == body.reference_body_id).then_some(state.surface_kilograms()[i])
                })
                .sum();
            close(expected, body.inventory.owned_stock_kilograms[0]);
            for p in &body.frozen_monthly_liquid_probes {
                close(
                    p.perfect_body_sharing_cap_kilograms,
                    p.potential_demand_kilograms.min(expected),
                );
            }
        }
        checked += 1;
    }
    assert!(
        checked > 0,
        "The directed coverage fixtures must exercise disconnected reference water."
    );
}
