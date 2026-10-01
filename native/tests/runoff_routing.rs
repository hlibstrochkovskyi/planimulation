use planimulation_core::{Recipe, World, drainage::Drainage};

fn example_drainage() -> Drainage {
    // Regions 2 and 3 are separate wet terminals in the same water body.
    // Region 4 is an independent closed dry sink.
    Drainage {
        receivers: vec![2, 2, 2, 3, 4],
        outlets: vec![2, 2, 2, 2, 4],
        flat_steps: vec![0; 5],
        contributing_area: vec![0.; 5],
    }
}

#[test]
fn directed_runoff_reaches_wet_body_and_closed_sink_without_loss() {
    let drainage = example_drainage();
    let input = [3, 5, 7, 11, 13];
    let result = drainage.route_runoff_units(&input).unwrap();
    assert_eq!(result.through_region_units, [3, 5, 15, 11, 13]);
    assert_eq!(result.terminal_units, [0, 0, 26, 0, 13]);
    assert_eq!(result.total_input_units, 39);
    assert_eq!(
        drainage
            .route_runoff_units(&[0; 5])
            .unwrap()
            .total_input_units,
        0
    );

    let first = drainage.route_runoff_units(&[3, 5, 0, 0, 0]).unwrap();
    let second = drainage.route_runoff_units(&[0, 0, 7, 11, 13]).unwrap();
    for (region, &volume) in result.terminal_units.iter().enumerate() {
        assert_eq!(
            volume,
            first.terminal_units[region] + second.terminal_units[region]
        );
    }
}

#[test]
fn generated_worlds_match_independent_terminal_traces() {
    let base: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    for seed in 0..5 {
        let mut recipe = base.clone();
        recipe.seed = format!("runoff-routing-{seed}");
        let world = World::generate(recipe).unwrap();
        let original_depths = world.water.depth_meters.clone();
        let inputs: Vec<i128> = (0..world.surface.areas.len())
            .map(|region| (region % 7) as i128)
            .collect();
        let result = world.drainage.route_runoff_units(&inputs).unwrap();
        let mut expected = vec![0_i128; inputs.len()];
        for (start, &volume) in inputs.iter().enumerate() {
            let mut region = start;
            for _ in 0..inputs.len() {
                let next = world.drainage.receivers[region] as usize;
                if next == region {
                    break;
                }
                region = next;
            }
            assert_eq!(world.drainage.receivers[region] as usize, region);
            expected[world.drainage.outlets[region] as usize] += volume;
        }
        assert_eq!(result.terminal_units, expected);
        assert_eq!(result.total_input_units, inputs.iter().sum());
        assert_eq!(world.drainage.route_runoff_units(&inputs).unwrap(), result);
        assert_eq!(world.water.depth_meters, original_depths);
    }
}

#[test]
fn dry_and_fully_wet_worlds_keep_all_prescribed_runoff() {
    let base: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    for fraction in [0., 1.] {
        let mut recipe = base.clone();
        recipe.subdivision = 0;
        recipe.water = planimulation_core::water::WaterSettings::Coverage { fraction };
        let world = World::generate(recipe).unwrap();
        let input = vec![1; world.surface.areas.len()];
        let routed = world.drainage.route_runoff_units(&input).unwrap();
        assert_eq!(routed.total_input_units, input.len() as i128);
        assert_eq!(
            routed.terminal_units.iter().sum::<i128>(),
            input.len() as i128
        );
        if fraction == 1. {
            assert_eq!(routed.terminal_units[0], input.len() as i128);
            assert_eq!(routed.terminal_units.iter().filter(|&&v| v > 0).count(), 1);
        }
    }
}

#[test]
fn invalid_inputs_and_graphs_reject_without_mutating_drainage() {
    let drainage = example_drainage();
    let original = drainage.clone();
    assert!(drainage.route_runoff_units(&[1; 4]).is_err());
    assert!(drainage.route_runoff_units(&[1, -1, 1, 1, 1]).is_err());
    assert!(
        drainage
            .route_runoff_units(&[i128::MAX, 1, 0, 0, 0])
            .is_err()
    );
    assert!(
        drainage
            .route_runoff_units(&[0, 0, i128::MAX, 1, 0])
            .is_err()
    );
    assert_eq!(drainage, original);

    let mut bad = original.clone();
    bad.receivers[0] = 5;
    assert!(bad.route_runoff_units(&[1; 5]).is_err());
    let mut bad = original.clone();
    bad.receivers[0] = 1;
    bad.receivers[1] = 0;
    assert!(bad.route_runoff_units(&[1; 5]).is_err());
    let mut bad = original.clone();
    bad.outlets[0] = 4;
    assert!(bad.route_runoff_units(&[1; 5]).is_err());
    let mut bad = original;
    bad.outlets[3] = 9;
    assert!(bad.route_runoff_units(&[1; 5]).is_err());
}
