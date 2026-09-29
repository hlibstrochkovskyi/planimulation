use planimulation_core::{
    Recipe, Surface, World,
    basins::Basins,
    initial_water_inventory::InitialWaterInventory,
    nested_reservoir::{Edge, Geometry, Input},
    reservoir::Column,
    spill_junction::Weight,
    spill_network::simultaneous::multi_entry::{self, EntryWeight, Setup, seeded},
    water::{Water, WaterSettings},
};

fn geometry(heights: &[f64]) -> Geometry {
    Geometry {
        columns: heights
            .iter()
            .map(|&bed_meters| Column {
                bed_meters,
                area_square_meters: 1.,
            })
            .collect(),
        edges: (1..heights.len())
            .map(|i| Edge {
                regions: [i - 1, i],
                distance_meters: 1.,
            })
            .collect(),
    }
}

fn surface(g: &Geometry) -> Surface {
    let n = g.columns.len();
    let mut s = Surface {
        centers: vec![],
        faces: vec![],
        offsets: vec![0],
        neighbors: vec![],
        distances: vec![],
        areas: g.columns.iter().map(|c| c.area_square_meters).collect(),
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    for i in 0..n {
        if i > 0 {
            s.neighbors.push((i - 1) as u32);
        }
        if i + 1 < n {
            s.neighbors.push((i + 1) as u32);
        }
        s.offsets.push(s.neighbors.len() as u32);
    }
    s.distances = vec![1.; s.neighbors.len()];
    s
}

fn setup(g: Geometry) -> Setup {
    let s = surface(&g);
    let heights: Vec<_> = g.columns.iter().map(|c| c.bed_meters).collect();
    let basins = Basins::build(&s, &heights).unwrap();
    let entry_weights = multi_entry::entry_targets(&g)
        .unwrap()
        .into_iter()
        .map(|target| EntryWeight {
            branch: target.branch,
            leaf: target.leaf,
            weight: 1.,
        })
        .collect();
    Setup {
        geometry: g,
        policy_version: multi_entry::POLICY_VERSION.into(),
        weights: (0..basins.nodes().len() - 1)
            .map(|branch| Weight { branch, weight: 1. })
            .collect(),
        entry_weights,
    }
}

fn imported(g: &Geometry, volume: f64) -> InitialWaterInventory {
    let s = surface(g);
    let heights: Vec<_> = g.columns.iter().map(|c| c.bed_meters).collect();
    let water = Water::generate(
        &s,
        &heights,
        &WaterSettings::Volume {
            volume_cubic_meters: volume,
        },
    )
    .unwrap();
    let basins = Basins::build(&s, &heights).unwrap();
    InitialWaterInventory::from_water(&s, &heights, &water, &basins).unwrap()
}

fn input(region: usize, volume: f64) -> Input {
    Input {
        region,
        volume_cubic_meters: volume,
    }
}

#[test]
fn initial_and_external_ledgers_remain_distinct_through_replay() {
    let g = geometry(&[0., 4., 1., 2., 0.5, 4., 3.]);
    let initial = imported(&g, 2.5);
    let mut continuous = seeded::SeededNetwork::new(setup(g), initial.clone()).unwrap();
    assert_eq!(continuous.checkpoint().initial_water, initial);
    assert_eq!(continuous.checkpoint().inventory.input_cubic_meters, 0.);
    assert_eq!(continuous.checkpoint().inventory.pulse_count, 0);
    assert_eq!(
        continuous.snapshot().unwrap().total_stored_cubic_meters,
        2.5
    );
    for inputs in [vec![input(0, 0.2), input(2, 0.1)], vec![input(4, 0.1)]] {
        let bytes = serde_json::to_vec(&continuous.checkpoint()).unwrap();
        let mut restored =
            seeded::SeededNetwork::restore(serde_json::from_slice(&bytes).unwrap()).unwrap();
        assert_eq!(
            continuous.add_interval(inputs.clone()).unwrap(),
            restored.add_interval(inputs).unwrap()
        );
        assert_eq!(continuous.checkpoint(), restored.checkpoint());
    }
    let checkpoint = continuous.checkpoint();
    assert_eq!(checkpoint.initial_water.initial_volume_cubic_meters, 2.5);
    assert_eq!(checkpoint.inventory.input_cubic_meters, 0.4);
    assert_eq!(checkpoint.inventory.pulse_count, 2);
    assert!((continuous.snapshot().unwrap().total_stored_cubic_meters - 2.9).abs() < 1e-12);
    let value = serde_json::to_value(checkpoint).unwrap();
    assert!(serde_json::from_value::<multi_entry::Checkpoint>(value).is_err());
}

#[test]
fn exact_dry_sills_and_corrupt_initial_checkpoints_fail_explicitly() {
    let g = geometry(&[0., 2., 0.]);
    let at_sill = imported(&g, 4.);
    assert_eq!(at_sill.source_level_meters, 2.);
    assert!(
        seeded::SeededNetwork::new(setup(g.clone()), at_sill)
            .unwrap_err()
            .contains("dry merging sill")
    );
    let initial = imported(&g, 1.);
    let fresh = seeded::SeededNetwork::new(setup(g), initial).unwrap();
    let mut checkpoint = fresh.checkpoint();
    checkpoint.inventory.input_cubic_meters = 1.;
    assert!(seeded::SeededNetwork::restore(checkpoint).is_err());
    let mut checkpoint = fresh.checkpoint();
    checkpoint.inventory.active[0].volume_cubic_meters += 0.25;
    assert!(seeded::SeededNetwork::restore(checkpoint).is_err());
    let mut checkpoint = fresh.checkpoint();
    checkpoint.initial_water.initial_volume_cubic_meters += 0.5;
    assert!(seeded::SeededNetwork::restore(checkpoint).is_err());
    let mut checkpoint = fresh.checkpoint();
    checkpoint.experiment_version = "future".into();
    assert!(seeded::SeededNetwork::restore(checkpoint).is_err());
}

#[test]
fn generated_world_advances_headlessly_without_refitting_initial_water() {
    let mut recipe: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    recipe.subdivision = 1;
    recipe.water = WaterSettings::Coverage { fraction: 0.25 };
    let world = World::generate(recipe).unwrap();
    let setup = seeded::generated_unit_setup(&world).unwrap();
    let initial = InitialWaterInventory::from_water(
        &world.surface,
        &world.terrain.elevation,
        &world.water,
        &world.basins,
    )
    .unwrap();
    let initial_depths = world.water.depth_meters.clone();
    let initial_bodies = world.water.body_ids.clone();
    let mut model = seeded::SeededNetwork::from_generated(&world, setup).unwrap();
    let before = model.snapshot().unwrap().total_stored_cubic_meters;
    let before_failure = model.checkpoint();
    assert!(
        model
            .add_interval(vec![input(3, 1e-4)])
            .unwrap_err()
            .contains("precision")
    );
    assert_eq!(model.checkpoint(), before_failure);
    let interval = model.add_interval(vec![input(3, 2e14)]).unwrap();
    let after = interval.snapshot.total_stored_cubic_meters;
    assert!((after - before - 2e14).abs() <= 2e14 * 1e-10);
    assert_eq!(interval.events.len(), 3);
    assert_eq!(interval.events[1].merged, [7]);
    assert_eq!(model.checkpoint().initial_water, initial);
    assert_eq!(
        model.checkpoint().origin_recipe.unwrap().seed,
        world.recipe.seed
    );
    assert_eq!(model.checkpoint().inventory.input_cubic_meters, 2e14);
    assert_eq!(world.water.depth_meters, initial_depths);
    assert_eq!(world.water.body_ids, initial_bodies);
    let mut false_origin = model.checkpoint();
    false_origin.origin_recipe.as_mut().unwrap().seed = "different-world".into();
    assert!(seeded::SeededNetwork::restore(false_origin).is_err());
    let mut oversized = world.recipe.clone();
    oversized.subdivision = 6;
    assert!(seeded::generated_unit_setup(&World::generate(oversized).unwrap()).is_err());
}

#[test]
fn generated_seed_and_coverage_ensemble_preserves_separate_budgets() {
    let baseline: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    for case in 0..12 {
        for fraction in [0.25, 0.55, 0.85] {
            let mut recipe = baseline.clone();
            recipe.seed = format!("seeded-{case:02}");
            recipe.subdivision = 1;
            recipe.water = WaterSettings::Coverage { fraction };
            let world = World::generate(recipe).unwrap();
            let setup = seeded::generated_unit_setup(&world).unwrap();
            let imported = InitialWaterInventory::from_water(
                &world.surface,
                &world.terrain.elevation,
                &world.water,
                &world.basins,
            )
            .unwrap();
            let mut model = seeded::SeededNetwork::from_generated(&world, setup)
                .unwrap_or_else(|e| panic!("seeded-{case:02} fraction {fraction}: {e}"));
            let before = model.snapshot().unwrap().total_stored_cubic_meters;
            model
                .add_interval(vec![input(0, 1e12)])
                .unwrap_or_else(|e| panic!("seeded-{case:02} fraction {fraction}: {e}"));
            let after = model.snapshot().unwrap().total_stored_cubic_meters;
            assert!((after - before - 1e12).abs() <= 1e12 * 1e-10);
            assert_eq!(model.checkpoint().initial_water, imported);
            assert_eq!(model.checkpoint().inventory.input_cubic_meters, 1e12);
        }
    }
}

#[test]
fn generated_dry_and_full_worlds_keep_the_initial_ledger() {
    let baseline: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    for fraction in [0., 1.] {
        let mut recipe = baseline.clone();
        recipe.subdivision = 1;
        recipe.water = WaterSettings::Coverage { fraction };
        let world = World::generate(recipe).unwrap();
        let setup = seeded::generated_unit_setup(&world).unwrap();
        let initial = InitialWaterInventory::from_water(
            &world.surface,
            &world.terrain.elevation,
            &world.water,
            &world.basins,
        )
        .unwrap();
        let mut model = seeded::SeededNetwork::from_generated(&world, setup).unwrap();
        model.add_interval(vec![input(0, 1e12)]).unwrap();
        assert_eq!(model.checkpoint().inventory.input_cubic_meters, 1e12);
        assert_eq!(
            model.checkpoint().initial_water.initial_volume_cubic_meters,
            world.water.resolved_volume_cubic_meters
        );
        assert_eq!(model.checkpoint().initial_water, initial);
    }
}

#[test]
fn expanded_generated_worlds_preserve_versions_and_budgets() {
    let baseline: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    for subdivision in [2, 3, 5] {
        let mut recipe = baseline.clone();
        recipe.subdivision = subdivision;
        recipe.water = WaterSettings::Coverage { fraction: 0.25 };
        let world = World::generate(recipe).unwrap();
        let setup = seeded::generated_unit_setup(&world).unwrap();
        let mut model = seeded::SeededNetwork::from_generated(&world, setup).unwrap();
        assert_eq!(
            model.experiment_version(),
            seeded::EXACT_LIMIT_EXPERIMENT_VERSION
        );
        let initial = model.snapshot().unwrap().total_stored_cubic_meters;
        let original_checkpoint = model.checkpoint();
        let interval = model.add_interval(vec![input(3, 2e14)]).unwrap();
        let stored = interval.snapshot.total_stored_cubic_meters;
        assert!((stored - initial - 2e14).abs() <= 2e14 * 1e-10);
        assert_eq!(model.checkpoint().inventory.input_cubic_meters, 2e14);
        assert!(
            (world.water.resolved_volume_cubic_meters - initial).abs()
                <= world.water.resolved_volume_cubic_meters * 1e-12
        );
        let saved = serde_json::to_vec(&model.checkpoint()).unwrap();
        let resumed =
            seeded::SeededNetwork::restore(serde_json::from_slice(&saved).unwrap()).unwrap();
        assert_eq!(resumed.checkpoint(), model.checkpoint());
        if subdivision == 2 {
            let mut legacy = original_checkpoint;
            legacy.experiment_version = seeded::EXPANDED_EXPERIMENT_VERSION.into();
            let mut legacy = seeded::SeededNetwork::restore(legacy).unwrap();
            assert_eq!(legacy.add_interval(vec![input(3, 2e14)]).unwrap(), interval);
            let mut downgraded = model.checkpoint();
            downgraded.experiment_version = seeded::EXPERIMENT_VERSION.into();
            assert!(seeded::SeededNetwork::restore(downgraded).is_err());
        }
    }
}

#[test]
fn measured_expanded_restrictions_are_explicit_and_atomic() {
    let baseline: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    let mut numeric_recipe = baseline.clone();
    numeric_recipe.seed = "profile-01".into();
    numeric_recipe.subdivision = 5;
    numeric_recipe.water = WaterSettings::Coverage { fraction: 0.25 };
    let world = World::generate(numeric_recipe).unwrap();
    let setup = seeded::generated_unit_setup(&world).unwrap();
    let mut model = seeded::SeededNetwork::from_generated(&world, setup).unwrap();
    let before = model.checkpoint();
    let mut old_checkpoint = before.clone();
    old_checkpoint.experiment_version = seeded::EXPANDED_EXPERIMENT_VERSION.into();
    let mut old_model = seeded::SeededNetwork::restore(old_checkpoint).unwrap();
    let old_before = old_model.checkpoint();
    assert_eq!(
        old_model.add_interval(vec![input(3, 2e14)]).unwrap_err(),
        "Concurrent update exceeds capacity."
    );
    assert_eq!(old_model.checkpoint(), old_before);
    let interval = model.add_interval(vec![input(3, 2e14)]).unwrap();
    assert!(
        interval
            .events
            .iter()
            .any(|event| event.saturated.contains(&254))
    );
    assert_eq!(model.checkpoint().inventory.input_cubic_meters, 2e14);
    assert!(interval.snapshot.budget_residual_cubic_meters.abs() <= 2e14 * 1e-12);
    let saved = serde_json::to_vec(&before).unwrap();
    let mut replay =
        seeded::SeededNetwork::restore(serde_json::from_slice(&saved).unwrap()).unwrap();
    assert_eq!(replay.add_interval(vec![input(3, 2e14)]).unwrap(), interval);
    assert_eq!(replay.checkpoint(), model.checkpoint());

    let mut bounded_recipe = baseline;
    bounded_recipe.seed = "profile-06".into();
    bounded_recipe.subdivision = 5;
    bounded_recipe.water = WaterSettings::Coverage { fraction: 0.25 };
    let world = World::generate(bounded_recipe).unwrap();
    let setup = seeded::generated_unit_setup(&world).unwrap();
    assert!(
        seeded::SeededNetwork::from_generated(&world, setup)
            .unwrap_err()
            .contains("duplicated curve-column budget")
    );
}
