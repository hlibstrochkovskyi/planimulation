use planimulation_core::{
    Recipe, Surface, World,
    basins::Basins,
    initial_water_inventory::InitialWaterInventory,
    water::{Water, WaterSettings},
};

fn chain(areas: &[f64]) -> Surface {
    let n = areas.len();
    let mut s = Surface {
        centers: vec![],
        faces: vec![],
        offsets: vec![0],
        neighbors: vec![],
        distances: vec![],
        areas: areas.to_vec(),
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

fn imported(
    surface: &Surface,
    heights: &[f64],
    setting: WaterSettings,
) -> (Water, InitialWaterInventory) {
    let water = Water::generate(surface, heights, &setting).unwrap();
    let basins = Basins::build(surface, heights).unwrap();
    let inventory = InitialWaterInventory::from_water(surface, heights, &water, &basins).unwrap();
    let restored = inventory.reconstruct(surface, heights, &basins).unwrap();
    assert_eq!(restored.depth_meters, water.depth_meters);
    assert_eq!(restored.body_ids, water.body_ids);
    assert_eq!(restored.main_ocean_id, water.main_ocean_id);
    assert_eq!(
        restored.volume_cubic_meters,
        water.resolved_volume_cubic_meters
    );
    (water, inventory)
}

#[test]
fn nested_bowls_use_only_exclusive_active_storage() {
    let s = chain(&[2., 3., 5., 7., 11.]);
    let h = [0., 2., 1., 5., -1.];
    for setting in [
        WaterSettings::Coverage { fraction: 0. },
        WaterSettings::Volume {
            volume_cubic_meters: 8.,
        },
        WaterSettings::Volume {
            volume_cubic_meters: 42.,
        },
        WaterSettings::Volume {
            volume_cubic_meters: 63.,
        },
        WaterSettings::Volume {
            volume_cubic_meters: 105.,
        },
        WaterSettings::Coverage { fraction: 1. },
    ] {
        imported(&s, &h, setting);
    }
    let (_, at_sill) = imported(
        &s,
        &h,
        WaterSettings::Volume {
            volume_cubic_meters: 42.,
        },
    );
    assert_eq!(at_sill.source_level_meters, 2.);
    assert_eq!(
        at_sill.stocks.iter().map(|s| s.branch).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(
        at_sill.stocks.iter().map(|s| s.body_id).collect::<Vec<_>>(),
        [3, 1, 2]
    );
    let (_, above_sill) = imported(
        &s,
        &h,
        WaterSettings::Volume {
            volume_cubic_meters: 63.,
        },
    );
    assert_eq!(
        above_sill
            .stocks
            .iter()
            .map(|s| s.branch)
            .collect::<Vec<_>>(),
        [0, 3]
    );
    assert_eq!(
        above_sill
            .stocks
            .iter()
            .map(|s| s.body_id)
            .collect::<Vec<_>>(),
        [2, 1]
    );
}

#[test]
fn invalid_metadata_and_mixed_geometry_are_rejected() {
    let s = chain(&[1.; 3]);
    let h = [0., 2., 0.];
    let water = Water::generate(
        &s,
        &h,
        &WaterSettings::Volume {
            volume_cubic_meters: 4.,
        },
    )
    .unwrap();
    let basins = Basins::build(&s, &h).unwrap();
    let good = InitialWaterInventory::from_water(&s, &h, &water, &basins).unwrap();
    assert_eq!(good.stocks.len(), 2);
    let mut changed = good.clone();
    changed.stocks[0].volume_cubic_meters += 1.;
    assert!(changed.reconstruct(&s, &h, &basins).is_err());
    let mut changed = good.clone();
    changed.initial_volume_cubic_meters += 1.;
    assert!(changed.reconstruct(&s, &h, &basins).is_err());
    let mut changed = good.clone();
    changed.stocks[0].body_id = 2;
    assert!(changed.reconstruct(&s, &h, &basins).is_err());
    let mut changed = good.clone();
    changed.stocks[0].branch = basins.root();
    assert!(changed.reconstruct(&s, &h, &basins).is_err());
    let mut changed = good.clone();
    changed.import_version = "future".into();
    assert!(changed.reconstruct(&s, &h, &basins).is_err());
    let mut changed_water = water.clone();
    changed_water.body_ids[2] = 1;
    assert!(InitialWaterInventory::from_water(&s, &h, &changed_water, &basins).is_err());
    let other = Basins::build(&s, &[0., 3., 0.]).unwrap();
    assert!(InitialWaterInventory::from_water(&s, &h, &water, &other).is_err());
}

#[test]
fn generated_worlds_preserve_initial_water_and_replay() {
    let baseline: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    for (seed, subdivision, fraction) in [
        ("import-dry", 0, 0.),
        ("import-low", 2, 0.2),
        ("import-mid", 3, 0.65),
        ("import-high", 5, 1.),
    ] {
        let mut recipe = baseline.clone();
        recipe.seed = seed.into();
        recipe.subdivision = subdivision;
        recipe.water = WaterSettings::Coverage { fraction };
        let world = World::generate(recipe).unwrap();
        let initial = InitialWaterInventory::from_water(
            &world.surface,
            &world.terrain.elevation,
            &world.water,
            &world.basins,
        )
        .unwrap();
        assert_eq!(
            initial,
            InitialWaterInventory::from_water(
                &world.surface,
                &world.terrain.elevation,
                &world.water,
                &world.basins
            )
            .unwrap()
        );
        let bytes = serde_json::to_vec(&initial).unwrap();
        let replay: InitialWaterInventory = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(replay, initial);
        let restored = replay
            .reconstruct(&world.surface, &world.terrain.elevation, &world.basins)
            .unwrap();
        assert_eq!(restored.depth_meters, world.water.depth_meters);
        assert_eq!(restored.body_ids, world.water.body_ids);
    }
}

#[test]
fn generated_seed_and_coverage_ensemble_preserves_region_states() {
    let baseline: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    for case in 0..12 {
        for fraction in [0.1, 0.5, 0.9] {
            let mut recipe = baseline.clone();
            recipe.seed = format!("inventory-{case:02}");
            recipe.subdivision = 2;
            recipe.water = WaterSettings::Coverage { fraction };
            let world = World::generate(recipe).unwrap();
            let inventory = InitialWaterInventory::from_water(
                &world.surface,
                &world.terrain.elevation,
                &world.water,
                &world.basins,
            )
            .unwrap();
            let restored = inventory
                .reconstruct(&world.surface, &world.terrain.elevation, &world.basins)
                .unwrap();
            assert_eq!(restored.depth_meters, world.water.depth_meters);
            assert_eq!(restored.body_ids, world.water.body_ids);
            assert_eq!(
                restored.volume_cubic_meters,
                world.water.resolved_volume_cubic_meters
            );
        }
    }
}
