use planimulation_core::{
    Recipe, World,
    prescribed_water_inventory::{Checkpoint, PrescribedWaterInventory},
    spill_connections::SpillConnections,
    water::WaterSettings,
};

const ONE_CUBIC_METER: i128 = 1_i128 << 56;

fn recipe() -> Recipe {
    serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap()
}

#[test]
fn generated_runoff_changes_exact_basin_stocks_and_replays() {
    let world = World::generate(recipe()).unwrap();
    let mut state = PrescribedWaterInventory::from_world(&world).unwrap();
    let initial = state.checkpoint();
    let mut first_input = vec![0; world.surface.areas.len()];
    let dry_source = world
        .water
        .body_ids
        .iter()
        .position(|&body| body == 0)
        .unwrap();
    first_input[dry_source] = ONE_CUBIC_METER;
    let first = state.apply_runoff(&first_input).unwrap();
    assert_eq!(first.input_units, ONE_CUBIC_METER);
    assert_eq!(first.affected_branches.len(), 1);
    let after_first = state.checkpoint();
    assert_eq!(after_first.step, 1);
    assert_eq!(
        after_first.accepted_input_units,
        ONE_CUBIC_METER.to_string()
    );
    let slot = after_first
        .stocks
        .iter()
        .position(|stock| stock.branch == first.affected_branches[0])
        .unwrap();
    let before: i128 = initial.stocks[slot].volume_units.parse().unwrap();
    let after: i128 = after_first.stocks[slot].volume_units.parse().unwrap();
    assert_eq!(after - before, ONE_CUBIC_METER);
    assert!(
        state
            .level_meters(first.affected_branches[0])
            .unwrap()
            .is_some()
    );

    let mut second_input = vec![0; first_input.len()];
    second_input[dry_source] = ONE_CUBIC_METER;
    let mut uninterrupted = state;
    uninterrupted.apply_runoff(&second_input).unwrap();
    let json = serde_json::to_string(&after_first).unwrap();
    let restored: Checkpoint = serde_json::from_str(&json).unwrap();
    let mut resumed = PrescribedWaterInventory::restore(restored).unwrap();
    resumed.apply_runoff(&second_input).unwrap();
    assert_eq!(resumed.checkpoint(), uninterrupted.checkpoint());
    assert_eq!(resumed.checkpoint().step, 2);
}

#[test]
fn closed_ocean_retains_all_runoff_and_tiny_inputs_in_its_stock() {
    let mut input = recipe();
    input.water = WaterSettings::Coverage { fraction: 1. };
    let world = World::generate(input).unwrap();
    let mut state = PrescribedWaterInventory::from_world(&world).unwrap();
    let initial = state.checkpoint();
    assert_eq!(initial.stocks.len(), 1);
    let initial_level = state.level_meters(world.basins.root()).unwrap().unwrap();
    let runoff = vec![1_i128; world.surface.areas.len()];
    let result = state.apply_runoff(&runoff).unwrap();
    assert_eq!(result.input_units, runoff.len() as i128);
    assert_eq!(result.affected_branches, vec![world.basins.root()]);
    let after = state.checkpoint();
    let initial_units: i128 = initial.stocks[0].volume_units.parse().unwrap();
    let after_units: i128 = after.stocks[0].volume_units.parse().unwrap();
    assert_eq!(after_units - initial_units, runoff.len() as i128);
    assert_eq!(after.accepted_input_units, runoff.len().to_string());
    assert!(state.level_meters(world.basins.root()).unwrap().is_some());

    let mut larger_runoff = vec![0; runoff.len()];
    larger_runoff[0] = 1_000_000_000_000_i128 * ONE_CUBIC_METER;
    state.apply_runoff(&larger_runoff).unwrap();
    assert!(state.level_meters(world.basins.root()).unwrap().unwrap() > initial_level);
}

#[test]
fn generated_seed_sample_accepts_small_subspill_inputs() {
    let base = recipe();
    for seed in 0..20 {
        let mut input = base.clone();
        input.seed = format!("prescribed-inventory-{seed}");
        let world = World::generate(input).unwrap();
        let mut state = PrescribedWaterInventory::from_world(&world).unwrap();
        let source = world
            .water
            .body_ids
            .iter()
            .position(|&body| body == world.water.main_ocean_id)
            .unwrap();
        let mut runoff = vec![0; world.surface.areas.len()];
        runoff[source] = ONE_CUBIC_METER;
        state.apply_runoff(&runoff).unwrap();
        let checkpoint = state.checkpoint();
        assert_eq!(
            PrescribedWaterInventory::restore(checkpoint.clone())
                .unwrap()
                .checkpoint(),
            checkpoint
        );
    }
}

#[test]
fn dry_sink_begins_to_hold_water_without_spilling() {
    let mut input = recipe();
    input.water = WaterSettings::Coverage { fraction: 0. };
    let world = World::generate(input).unwrap();
    let mut state = PrescribedWaterInventory::from_world(&world).unwrap();
    let initial = state.checkpoint();
    let source = world
        .drainage
        .receivers
        .iter()
        .enumerate()
        .find_map(|(region, &receiver)| (region == receiver as usize).then_some(region))
        .unwrap();
    let mut runoff = vec![0; world.surface.areas.len()];
    runoff[source] = ONE_CUBIC_METER;
    let report = state.apply_runoff(&runoff).unwrap();
    assert_eq!(report.affected_branches.len(), 1);
    let branch = report.affected_branches[0];
    let slot = initial
        .stocks
        .iter()
        .position(|stock| stock.branch == branch)
        .unwrap();
    assert_eq!(initial.stocks[slot].volume_units, "0");
    assert_eq!(
        state.checkpoint().stocks[slot].volume_units,
        ONE_CUBIC_METER.to_string()
    );
    assert!(state.level_meters(branch).unwrap().is_some());
}

#[test]
fn invalid_inputs_and_corrupt_checkpoints_are_atomic() {
    let mut input = recipe();
    input.water = WaterSettings::Coverage { fraction: 0. };
    let world = World::generate(input).unwrap();
    let mut state = PrescribedWaterInventory::from_world(&world).unwrap();
    let before = state.checkpoint();
    assert!(state.apply_runoff(&[1]).is_err());
    let mut invalid = vec![0; world.surface.areas.len()];
    invalid[0] = -1;
    assert!(state.apply_runoff(&invalid).is_err());
    invalid[0] = i128::MAX / 2;
    assert!(state.apply_runoff(&invalid).is_err());
    assert_eq!(state.checkpoint(), before);

    let mut bad = before.clone();
    bad.inventory_version = "future".into();
    assert!(PrescribedWaterInventory::restore(bad).is_err());
    let mut bad = before.clone();
    bad.step = 1_u64 << 53;
    assert!(PrescribedWaterInventory::restore(bad).is_err());
    let mut bad = before.clone();
    bad.stocks[0].volume_units = "01".into();
    assert!(PrescribedWaterInventory::restore(bad).is_err());
    let mut bad = before.clone();
    bad.stocks[0].volume_units = "1".into();
    assert!(PrescribedWaterInventory::restore(bad).is_err());
    let mut bad = before.clone();
    bad.stocks[0].branch = usize::MAX;
    assert!(PrescribedWaterInventory::restore(bad).is_err());
    let mut bad = before.clone();
    bad.origin.origin_recipe.seed = "unrelated".into();
    assert!(PrescribedWaterInventory::restore(bad).is_err());
    let mut bad = before;
    bad.accepted_input_units = "1".into();
    assert!(PrescribedWaterInventory::restore(bad).is_err());
}

#[test]
fn generated_basin_spills_to_one_neighbor_then_merges_and_replays() {
    let mut input = recipe();
    input.water = WaterSettings::Coverage { fraction: 0. };
    let world = World::generate(input).unwrap();
    let passages = SpillConnections::build(&world.surface, &world.terrain.elevation).unwrap();
    let mut state = PrescribedWaterInventory::from_world(&world).unwrap();
    let (source, receiver, parent, terminal, source_capacity, receiver_capacity) =
        world
            .basins
            .nodes()
            .iter()
            .enumerate()
            .find_map(|(parent, node)| {
                if node.children.len() != 2
                    || !node
                        .children
                        .iter()
                        .all(|&child| world.basins.nodes()[child].children.is_empty())
                {
                    return None;
                }
                let source = node.children[0];
                let receiver = node.children[1];
                if passages.receivers(source).ok()?.len() != 1 {
                    return None;
                }
                let terminal = world.drainage.receivers.iter().enumerate().find_map(
                    |(region, &downhill)| {
                        (region == downhill as usize
                            && world.basins.region_nodes()[region] == source)
                            .then_some(region)
                    },
                )?;
                let a = state.remaining_before_spill_units(source).ok()??;
                let b = state.remaining_before_spill_units(receiver).ok()??;
                (a > 0 && b > 1).then_some((source, receiver, parent, terminal, a, b))
            })
            .expect("generated world needs a directed two-leaf spill fixture");
    let initial = state.checkpoint();
    let receiver_terminal = world
        .drainage
        .receivers
        .iter()
        .enumerate()
        .find_map(|(region, &downhill)| {
            (region == downhill as usize && world.basins.region_nodes()[region] == receiver)
                .then_some(region)
        })
        .unwrap();
    let mut simultaneous = vec![0; world.surface.areas.len()];
    simultaneous[terminal] = source_capacity;
    simultaneous[receiver_terminal] = receiver_capacity;
    assert_eq!(
        state.apply_runoff(&simultaneous),
        Err("Simultaneous spill sources need an allocation policy.".into())
    );
    assert_eq!(state.checkpoint(), initial);
    let mut first_input = vec![0; world.surface.areas.len()];
    first_input[terminal] = source_capacity + 1;
    state.apply_runoff(&first_input).unwrap();
    let after_first = state.checkpoint();
    let stock = |checkpoint: &Checkpoint, branch| -> i128 {
        checkpoint
            .stocks
            .iter()
            .find(|stock| stock.branch == branch)
            .unwrap()
            .volume_units
            .parse()
            .unwrap()
    };
    assert_eq!(stock(&after_first, source), source_capacity);
    assert_eq!(stock(&after_first, receiver), 1);
    assert_eq!(
        after_first.accepted_input_units,
        (source_capacity + 1).to_string()
    );

    let mut second_input = vec![0; first_input.len()];
    second_input[terminal] = receiver_capacity;
    let mut uninterrupted = state;
    uninterrupted.apply_runoff(&second_input).unwrap();
    let after_merge = uninterrupted.checkpoint();
    assert!(
        after_merge
            .stocks
            .iter()
            .any(|stock| stock.branch == parent)
    );
    assert!(
        after_merge
            .stocks
            .iter()
            .all(|stock| stock.branch != source && stock.branch != receiver)
    );
    assert_eq!(
        stock(&after_merge, parent),
        source_capacity + receiver_capacity + 1
    );
    assert_eq!(
        after_merge.accepted_input_units,
        (source_capacity + receiver_capacity + 1).to_string()
    );
    assert_ne!(after_merge.stocks, initial.stocks);
    let mut one_shot = PrescribedWaterInventory::from_world(&world).unwrap();
    let mut combined_input = vec![0; first_input.len()];
    combined_input[terminal] = source_capacity + receiver_capacity + 1;
    one_shot.apply_runoff(&combined_input).unwrap();
    assert_eq!(one_shot.checkpoint().stocks, after_merge.stocks);

    let mut overlapping = after_merge.clone();
    overlapping
        .stocks
        .push(planimulation_core::prescribed_water_inventory::BasinStock {
            branch: source,
            volume_units: "0".into(),
        });
    overlapping.stocks.sort_by_key(|stock| stock.branch);
    assert!(PrescribedWaterInventory::restore(overlapping).is_err());
    let mut unbalanced = after_merge.clone();
    unbalanced
        .stocks
        .iter_mut()
        .find(|stock| stock.branch == parent)
        .unwrap()
        .volume_units = (source_capacity + receiver_capacity).to_string();
    assert!(PrescribedWaterInventory::restore(unbalanced).is_err());

    let json = serde_json::to_string(&after_first).unwrap();
    let restored: Checkpoint = serde_json::from_str(&json).unwrap();
    let mut resumed = PrescribedWaterInventory::restore(restored).unwrap();
    resumed.apply_runoff(&second_input).unwrap();
    assert_eq!(resumed.checkpoint(), after_merge);
    assert_eq!(
        PrescribedWaterInventory::restore(after_merge.clone())
            .unwrap()
            .checkpoint(),
        after_merge
    );
}
