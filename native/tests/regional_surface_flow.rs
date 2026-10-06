use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Checkpoint, ClosedLakeExchange, Model, ReferenceWaterPool, Settings, SoilNumerics,
        SurfaceNumerics, TerminalNumerics, surface_flow,
    },
};

fn settings(quiet: bool) -> Settings {
    Settings {
        initial_active_surface_depth_meters: 10.,
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(ClosedLakeExchange::FrozenRegionalSurfaceFlow(
            Default::default(),
        )),
        evaporation_enabled: !quiet,
        precipitation_enabled: !quiet,
        routing_enabled: !quiet,
        max_coupled_step_seconds: 900,
        ..Default::default()
    }
}
fn fixture(quiet: bool) -> (World, Model) {
    let mut r: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    r.water = planimulation_core::water::WaterSettings::Coverage { fraction: 0.3 };
    let world = World::generate(r).unwrap();
    let m = Model::from_world(
        &world,
        settings(quiet),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    (world, m)
}
fn credit(h: &mut f64, l: &mut f64, v: f64) {
    let s = *h + v;
    let virtual_v = s - *h;
    let error = (*h - (s - virtual_v)) + (v - virtual_v);
    let tail = *l + error;
    let high = s + tail;
    let virtual_tail = high - s;
    *h = high;
    *l = (s - (high - virtual_tail)) + (tail - virtual_tail);
}
// Synthetic regional rain, funded by actual finite reference liquid.
fn fund(mut cp: Checkpoint, world: &World, inputs: &[(usize, f64)]) -> Checkpoint {
    cp.elapsed_seconds = cp.elapsed_seconds.max(900);
    let mut ids = world.water.body_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| id > 0);
    let mut used = vec![false; world.surface.areas.len()];
    for &(r, amount) in inputs {
        assert_eq!(world.water.body_ids[r], 0);
        let body = cp
            .reference_body_high_kilograms
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        let contact = world
            .water
            .body_ids
            .iter()
            .enumerate()
            .find_map(|(i, &id)| (id == ids[body] && !used[i]).then_some(i))
            .unwrap();
        used[contact] = true;
        credit(
            &mut cp.reference_body_high_kilograms.as_mut().unwrap()[body],
            &mut cp.reference_body_low_kilograms.as_mut().unwrap()[body],
            -amount,
        );
        assert!(cp.reference_body_high_kilograms.as_ref().unwrap()[body] > 0.);
        cp.cumulative_surface_transfers[contact].liquid_evaporation += amount;
        cp.cumulative_evaporation_kilograms[contact] += amount;
        cp.cumulative_surface_transfers[r].rain += amount;
        cp.cumulative_precipitation_kilograms[r] += amount;
        credit(
            &mut cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[r],
            &mut cp.cumulative_lake_capture_low_kilograms.as_mut().unwrap()[r],
            amount,
        );
        credit(
            &mut cp.terminal_water_kilograms[r],
            &mut cp.terminal_low_kilograms.as_mut().unwrap()[r],
            amount,
        );
    }
    cp
}
fn funded() -> (Model, planimulation_core::seasonal_moisture::State) {
    let (world, m) = fixture(true);
    Model::restore(fund(
        m.initial_state().checkpoint(),
        &world,
        &[(8, 1.4e17), (156, 2.4e17)],
    ))
    .unwrap()
}

/// Retained model-15 limitation, not an expectation for a future coupling.
/// Equal total water, with only a tiny redistribution from local to pooled liquid.
#[test]
fn positive_film_freezes_the_existing_soil_column_in_model_15() {
    let (world, _) = fixture(true);
    let region = world.water.body_ids.iter().position(|&id| id == 0).unwrap();
    let area = world.surface.areas[region];
    let mut configuration = settings(true);
    configuration.closed_lake_exchange = Some(ClosedLakeExchange::FrozenRegionalSurfaceFlow(
        surface_flow::Settings {
            maximum_diffusivity_square_meters_per_second: 0.,
            ..Default::default()
        },
    ));
    let temperature = planimulation_core::seasonal_temperature::Settings {
        reference_temperature_celsius: 30.,
        sensitivity_celsius_per_watt_per_square_meter: 0.,
        lapse_rate_celsius_per_meter: 0.,
        ..Default::default()
    };
    let m = Model::from_world(&world, configuration, temperature, Default::default()).unwrap();
    let mut cp = fund(
        m.initial_state().checkpoint(),
        &world,
        &[(region, area * 130.)],
    );
    // The same funded rain was infiltrated or remains local, rather than captured.
    cp.terminal_water_kilograms[region] = 0.;
    cp.terminal_low_kilograms.as_mut().unwrap()[region] = 0.;
    cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[region] = 0.;
    cp.cumulative_lake_capture_low_kilograms.as_mut().unwrap()[region] = 0.;
    cp.soil_kilograms[region] = area * 120.;
    cp.surface_kilograms[region] = area * 130. - cp.soil_kilograms[region];
    cp.cumulative_surface_transfers[region].infiltration = cp.soil_kilograms[region];
    let baseline = cp.clone();
    let column = planimulation_core::surface_water::ponded_soil::State {
        liquid: planimulation_core::surface_water::ponded_soil::Mass {
            high: baseline.surface_kilograms[region],
            low: 0.,
        },
        soil: planimulation_core::surface_water::ponded_soil::Mass {
            high: baseline.soil_kilograms[region],
            low: 0.,
        },
        ..Default::default()
    };
    let candidate = planimulation_core::surface_water::ponded_soil::advance(
        column,
        area,
        30.,
        0.,
        900.,
        Default::default(),
    )
    .unwrap();
    assert!(candidate.transfers.infiltration_kilograms > 0.);
    assert!(candidate.transfers.soil_drainage_kilograms > 0.);
    let (dry_model, mut dry_state) = Model::restore(cp).unwrap();
    let dry_step = dry_model.advance(&mut dry_state, 900).unwrap();
    let dry = dry_step.surface_transfers[region];
    assert!(dry.infiltration > 0.);
    assert!(dry.soil_drainage > 0.);
    println!(
        "model15 film witness: region={region} area={area} dry_infiltration_per_area={} dry_drainage_per_area={}",
        dry.infiltration / area,
        dry.soil_drainage / area
    );
    for film in [1., 1e-6, 1e-12] {
        let mut cp = baseline.clone();
        credit(
            &mut cp.surface_kilograms[region],
            &mut cp.surface_low_kilograms.as_mut().unwrap()[region],
            -film,
        );
        credit(
            &mut cp.terminal_water_kilograms[region],
            &mut cp.terminal_low_kilograms.as_mut().unwrap()[region],
            film,
        );
        credit(
            &mut cp.cumulative_lake_capture_kilograms.as_mut().unwrap()[region],
            &mut cp.cumulative_lake_capture_low_kilograms.as_mut().unwrap()[region],
            film,
        );
        // Compare the standalone operator only, not a new seasonal integration.
        // Its one liquid owner recombines both provenance partitions and tails.
        let mut combined = planimulation_core::surface_water::ponded_soil::Mass {
            high: cp.surface_kilograms[region],
            low: cp.surface_low_kilograms.as_ref().unwrap()[region],
        };
        credit(
            &mut combined.high,
            &mut combined.low,
            cp.terminal_water_kilograms[region],
        );
        credit(
            &mut combined.high,
            &mut combined.low,
            cp.terminal_low_kilograms.as_ref().unwrap()[region],
        );
        let result = planimulation_core::surface_water::ponded_soil::advance(
            planimulation_core::surface_water::ponded_soil::State {
                liquid: combined,
                ..column
            },
            area,
            30.,
            0.,
            900.,
            Default::default(),
        )
        .unwrap();
        assert_eq!(result, candidate);
        let (wet_model, mut wet_state) = Model::restore(cp).unwrap();
        let step = wet_model.advance(&mut wet_state, 900).unwrap();
        assert_eq!(step.surface_transfers[region].infiltration, 0.);
        assert_eq!(step.surface_transfers[region].soil_drainage, 0.);
        assert_eq!(
            wet_state.checkpoint().soil_kilograms[region],
            baseline.soil_kilograms[region]
        );
        assert!(step.budget.maximum_relative_local_surface_ledger_residual < 1e-12);
    }
}

#[test]
fn concurrent_generated_columns_flow_with_complete_replay_and_no_basin_parent_state() {
    let (m, mut s) = funded();
    let before = s.clone();
    let step = m.advance(&mut s, 900).unwrap();
    let cp = s.checkpoint();
    assert_eq!(cp.schema_version, 15);
    assert!(cp.merged_lake_state.is_none());
    assert!(cp.leaf_spill_state.is_none());
    let flow = cp.regional_surface_flow.as_ref().unwrap();
    assert!(
        flow.directed_transfers
            .high_kilograms
            .iter()
            .any(|&v| v > 0.)
    );
    assert!(
        cp.terminal_water_kilograms
            .iter()
            .enumerate()
            .any(|(r, &v)| r != 8 && r != 156 && v > 0.)
    );
    assert!(step.budget.maximum_relative_local_surface_ledger_residual < 1e-12);
    let (resumed, mut a) =
        Model::restore(serde_json::from_str(&serde_json::to_string(&cp).unwrap()).unwrap())
            .unwrap();
    assert_eq!(s, a);
    m.advance(&mut s, 900).unwrap();
    resumed.advance(&mut a, 900).unwrap();
    assert_eq!(s, a);
    let mut direct = before.clone();
    let mut split = before;
    m.advance(&mut direct, 1800).unwrap();
    m.advance(&mut split, 900).unwrap();
    m.advance(&mut split, 900).unwrap();
    assert_eq!(direct, split);
}
#[test]
fn strict_save_graph_identity_and_obsolete_observers_reject_without_publishing_state() {
    let (m, mut s) = funded();
    m.advance(&mut s, 900).unwrap();
    let cp = s.checkpoint();
    for variant in 0..4 {
        let mut bad = cp.clone();
        match variant {
            0 => bad.regional_surface_flow = None,
            1 => bad.regional_surface_flow.as_mut().unwrap().model_version = "wrong".into(),
            2 => {
                bad.regional_surface_flow
                    .as_mut()
                    .unwrap()
                    .directed_transfers
                    .low_kilograms
                    .pop();
            }
            _ => {
                bad.regional_surface_flow
                    .as_mut()
                    .unwrap()
                    .directed_transfers
                    .high_kilograms
                    .fill(0.);
                bad.regional_surface_flow
                    .as_mut()
                    .unwrap()
                    .directed_transfers
                    .low_kilograms
                    .fill(0.);
            }
        }
        assert!(Model::restore(bad).is_err());
    }
    let mut json = serde_json::to_value(&cp).unwrap();
    json["regionalSurfaceFlow"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<Checkpoint>(json).is_err());
    let before = s.clone();
    let view = m.regional_surface_observation(&s).unwrap();
    assert_eq!(view.elapsed_seconds, cp.elapsed_seconds);
    assert_eq!(
        view.regional_liquid_high_kilograms,
        s.terminal_water_kilograms()
    );
    assert!(m.closed_lake_frontier(&s).is_err());
    assert!(
        m.advance_observed(&mut s, 1, |_| panic!("Unsupported observer invoked."))
            .is_err()
    );
    let mut bytes = Vec::new();
    assert!(planimulation_core::wire::seasonal_checkpoint(&mut bytes, &m, &s).is_err());
    assert!(planimulation_core::wire::seasonal_moisture(&mut bytes, &m, &s, None, 0).is_err());
    assert!(bytes.is_empty());
    assert_eq!(before, s);
}
#[test]
fn invalid_configuration_and_excessive_work_refuse_whole_caller() {
    let (world, m) = fixture(true);
    let mut bad = settings(true);
    bad.closed_lake_exchange = Some(ClosedLakeExchange::FrozenRegionalSurfaceFlow(
        surface_flow::Settings {
            roughness: 0.,
            ..Default::default()
        },
    ));
    assert!(Model::from_world(&world, bad, Default::default(), Default::default()).is_err());
    let mut cp = m.initial_state().checkpoint();
    cp.regional_surface_flow
        .as_mut()
        .unwrap()
        .directed_transfers
        .high_kilograms[0] = 1.;
    assert!(Model::restore(cp).is_err());
    let mut r = world.recipe.clone();
    r.radius_meters = 100_000.;
    r.subdivision = 5;
    let world = World::generate(r).unwrap();
    let mut configuration = settings(true);
    configuration.orography.as_mut().unwrap().strength = 0.;
    configuration.closed_lake_exchange = Some(ClosedLakeExchange::FrozenRegionalSurfaceFlow(
        surface_flow::Settings {
            maximum_diffusivity_square_meters_per_second: 1e8,
            ..Default::default()
        },
    ));
    let m = Model::from_world(
        &world,
        configuration,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut s = m.initial_state();
    let before = s.clone();
    assert!(
        m.advance(&mut s, 900)
            .unwrap_err()
            .contains("explicit work bound")
    );
    assert_eq!(s, before);
}
#[test]
fn ordinary_generated_forty_days_have_finite_stocks_exchange_and_exact_continuation() {
    let (_, m) = fixture(false);
    let mut s = m.initial_state();
    let before = s.clone();
    for _ in 0..39 {
        m.advance(&mut s, 86400).unwrap();
    }
    let step = m.advance(&mut s, 86400).unwrap();
    assert!(step.budget.cumulative_precipitation_kilograms > 0.);
    assert!(step.budget.maximum_relative_local_surface_ledger_residual < 1e-12);
    assert_ne!(before, s);
    let (r, mut a) = Model::restore(
        serde_json::from_str(&serde_json::to_string(&s.checkpoint()).unwrap()).unwrap(),
    )
    .unwrap();
    m.advance(&mut s, 86400).unwrap();
    r.advance(&mut a, 86400).unwrap();
    assert_eq!(s, a);
}
