use planimulation_core::{
    Random, Surface,
    basins::Basins,
    reservoir::{Boundary, Reservoir},
};

fn chain(heights: &[f64], areas: &[f64]) -> Basins {
    let n = heights.len();
    let mut s = Surface {
        centers: vec![[1., 0., 0.]; n],
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
    Basins::build(&s, heights).unwrap()
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9_f64.max(b.abs() * 1e-12), "{a} != {b}");
}

fn audit(r: &Reservoir) {
    let c = r.checkpoint();
    let i = r.inventory();
    near(
        i.initial_volume_cubic_meters + i.input_volume_cubic_meters,
        i.stored_volume_cubic_meters + i.outflow_volume_cubic_meters,
    );
    if let Some(level) = r.water_level_meters().unwrap() {
        // Independent direct column sum, not the indexed storage curve.
        let direct: f64 = c
            .columns
            .iter()
            .map(|c| c.area_square_meters * (level - c.bed_meters).max(0.))
            .sum();
        assert!(
            (direct - i.stored_volume_cubic_meters).abs()
                <= 1e-9_f64.max(i.stored_volume_cubic_meters * 1e-10)
        );
        if let Boundary::ExternalCollector { spill_level_meters } = c.boundary {
            assert!(level <= spill_level_meters);
        }
    } else {
        assert_eq!(i.stored_volume_cubic_meters, 0.);
    }
}

#[test]
fn weighted_bowl_fills_crosses_area_breaks_and_spills_only_excess() {
    let b = chain(&[0., 1., 3., -1.], &[2., 3., 7., 11.]);
    let mut r = b.isolated_leaf_reservoir(b.region_nodes()[0], 0.).unwrap();
    assert_eq!(r.capacity_cubic_meters(), Some(12.));
    assert_eq!(r.water_level_meters().unwrap(), None);
    for (input, level, stored, outflow) in [
        (1., 0.5, 1., 0.),
        (1., 1., 2., 0.),
        (5., 2., 7., 0.),
        (5., 3., 12., 0.),
        (4., 3., 12., 4.),
        (0., 3., 12., 0.),
    ] {
        let pulse = r.add_input(input).unwrap();
        assert_eq!(pulse.water_level_meters, Some(level));
        assert_eq!(pulse.stored_cubic_meters, stored);
        assert_eq!(pulse.outflow_cubic_meters, outflow);
        audit(&r);
    }
    assert_eq!(r.inventory().outflow_volume_cubic_meters, 4.);
    let mut from_wet = b.isolated_leaf_reservoir(b.region_nodes()[0], 7.).unwrap();
    let pulse = from_wet.add_input(9.).unwrap();
    assert_eq!(pulse.outflow_cubic_meters, 4.);
    assert_eq!(pulse.water_level_meters, Some(3.));
    audit(&from_wet);
}

#[test]
fn closed_bowls_slopes_and_flats_keep_all_input_without_invented_drains() {
    for h in [
        vec![3., 1., 0., 1., 3.],
        vec![0., 1., 2., 3., 4.],
        vec![0.; 5],
    ] {
        let b = chain(&h, &[2., 3., 5., 7., 11.]);
        let mut r = b.isolated_leaf_reservoir(b.root(), 0.).unwrap();
        assert_eq!(r.capacity_cubic_meters(), None);
        for pulse in [0., 0.25, 1.75, 10., 1000.] {
            assert_eq!(r.add_input(pulse).unwrap().outflow_cubic_meters, 0.);
            audit(&r);
        }
        assert!(r.water_level_meters().unwrap().unwrap() > 4.);
    }
    let b = chain(&[0., 0., 2., -1.], &[2., 3., 7., 11.]);
    let mut r = b.isolated_leaf_reservoir(b.region_nodes()[0], 0.).unwrap();
    assert_eq!(r.add_input(11.).unwrap().outflow_cubic_meters, 1.);
    assert_eq!(r.water_level_meters().unwrap(), Some(2.));
    audit(&r);
}

#[test]
fn nested_and_multiway_basins_are_not_prematurely_combined() {
    for h in [vec![0., 2., 1., 5., -1.], vec![0., 4., 1., 4., 2.]] {
        let b = chain(&h, &vec![1.; h.len()]);
        let original = b.clone();
        for (id, node) in b.nodes().iter().enumerate() {
            if node.children.is_empty() {
                let mut r = b.isolated_leaf_reservoir(id, 0.).unwrap();
                let cap = node.capacity_cubic_meters.unwrap();
                near(r.capacity_cubic_meters().unwrap(), cap);
                let p = r.add_input(cap + 10.).unwrap();
                near(p.outflow_cubic_meters, 10.);
                near(p.stored_cubic_meters, cap);
                audit(&r);
            } else {
                assert!(b.isolated_leaf_reservoir(id, 0.).is_err());
            }
        }
        assert_eq!(b, original);
    }
}

#[test]
fn pulse_partition_and_independent_instances_preserve_inventory() {
    let b = chain(&[0., 1., 3., -1.], &[2., 3., 7., 11.]);
    let original = b.isolated_leaf_reservoir(b.region_nodes()[0], 0.).unwrap();
    let mut single = original.clone();
    let mut split = original.clone();
    single.add_input(17.).unwrap();
    for _ in 0..68 {
        split.add_input(0.25).unwrap();
        audit(&split);
    }
    assert_eq!(
        single.inventory().stored_volume_cubic_meters,
        split.inventory().stored_volume_cubic_meters
    );
    assert_eq!(
        single.inventory().outflow_volume_cubic_meters,
        split.inventory().outflow_volume_cubic_meters
    );
    assert_eq!(original.inventory().stored_volume_cubic_meters, 0.);
}

#[test]
fn checkpoint_json_restores_exact_continuation_and_rejects_corruption() {
    let b = chain(&[0., 1., 3., -1.], &[2., 3., 7., 11.]);
    let mut a = b
        .isolated_leaf_reservoir(b.region_nodes()[0], 0.13)
        .unwrap();
    for input in [0.27, 1.19, 0., 2.33] {
        a.add_input(input).unwrap();
    }
    let encoded = serde_json::to_vec(&a.checkpoint()).unwrap();
    let mut restored = Reservoir::restore(serde_json::from_slice(&encoded).unwrap()).unwrap();
    assert_eq!(a, restored);
    for input in [0.01, 3.71, 22.91, 0., 4.13] {
        assert_eq!(
            a.add_input(input).unwrap(),
            restored.add_input(input).unwrap()
        );
    }
    assert_eq!(a, restored);
    let good = a.checkpoint();
    let mut bad = good.clone();
    bad.experiment_version = "future".into();
    assert!(Reservoir::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.stored_volume_cubic_meters -= 1.;
    assert!(Reservoir::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.input_volume_cubic_meters += 1.;
    assert!(Reservoir::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.pulse_count = 0;
    assert!(Reservoir::restore(bad).is_err());
    let mut bad = good.clone();
    bad.columns[0].area_square_meters = -1.;
    assert!(Reservoir::restore(bad).is_err());
    let mut bad = good.clone();
    bad.boundary = Boundary::Closed;
    assert!(Reservoir::restore(bad).is_err());
    let mut json = serde_json::to_value(&good).unwrap();
    json["unknown"] = true.into();
    assert!(serde_json::from_value::<planimulation_core::reservoir::Checkpoint>(json).is_err());
}

#[test]
fn invalid_inputs_overflow_and_unresolvable_precision_are_transactional() {
    let b = chain(&[0., 1., 3., -1.], &[2., 3., 7., 11.]);
    let node = b.region_nodes()[0];
    for v in [-1., f64::NAN, f64::INFINITY, 12.1] {
        assert!(b.isolated_leaf_reservoir(node, v).is_err());
    }
    assert!(b.isolated_leaf_reservoir(999, 0.).is_err());
    let mut r = b.isolated_leaf_reservoir(node, 0.).unwrap();
    for input in [-1., f64::NAN, f64::INFINITY] {
        let before = r.clone();
        assert!(r.add_input(input).is_err());
        assert_eq!(r, before);
    }
    let b = chain(&[1e20], &[1.]);
    let mut r = b.isolated_leaf_reservoir(0, 0.).unwrap();
    let before = r.clone();
    assert!(r.add_input(1.).is_err());
    assert_eq!(r, before);
    let b = chain(&[0.], &[1.]);
    let mut r = b.isolated_leaf_reservoir(0, f64::MAX).unwrap();
    for input in [1., f64::MAX] {
        let before = r.clone();
        assert!(r.add_input(input).is_err());
        assert_eq!(r, before);
    }
    let mut checkpoint = r.checkpoint();
    checkpoint.inventory.pulse_count = u64::MAX;
    let mut r = Reservoir::restore(checkpoint).unwrap();
    let before = r.clone();
    assert!(r.add_input(0.).is_err());
    assert_eq!(r, before);
}

#[test]
fn area_and_datum_scaling_and_seeded_pulses_match_direct_prism_sums() {
    for seed in 0..30 {
        let mut rng = Random::stream(&format!("reservoir-{seed}"), "test-pulses");
        let heights: Vec<f64> = (0..32).map(|i| i as f64 * 0.25).chain([10., -1.]).collect();
        let areas: Vec<f64> = heights
            .iter()
            .map(|_| 1. + f64::from(rng.next_u32() % 20))
            .collect();
        let b = chain(&heights, &areas);
        let shifted = chain(
            &heights.iter().map(|h| h + 1e6).collect::<Vec<_>>(),
            &areas.iter().map(|a| a * 4.).collect::<Vec<_>>(),
        );
        let mut a = b.isolated_leaf_reservoir(b.region_nodes()[0], 0.).unwrap();
        let mut c = shifted
            .isolated_leaf_reservoir(shifted.region_nodes()[0], 0.)
            .unwrap();
        // Audit varied inputs at the original datum. Only the final comparison
        // uses the large shifted datum and an exactly representable spill height.
        for _ in 0..100 {
            let input = f64::from(rng.next_u32() % 200) * 0.25;
            a.add_input(input).unwrap();
            audit(&a);
        }
        // Compare scaling at the exactly representable spill threshold.
        let input = a.capacity_cubic_meters().unwrap() + 17.;
        let mut a = b.isolated_leaf_reservoir(b.region_nodes()[0], 0.).unwrap();
        a.add_input(input).unwrap();
        c.add_input(input * 4.).unwrap();
        near(
            c.inventory().stored_volume_cubic_meters,
            a.inventory().stored_volume_cubic_meters * 4.,
        );
        near(
            c.inventory().outflow_volume_cubic_meters,
            a.inventory().outflow_volume_cubic_meters * 4.,
        );
        assert_eq!(
            c.water_level_meters().unwrap().unwrap(),
            a.water_level_meters().unwrap().unwrap() + 1e6
        );
        audit(&c);
    }
}

#[test]
fn detached_generated_leaves_preserve_world_arrays_and_analysis_capacity() {
    use planimulation_core::{Recipe, World, water::WaterSettings, wire};
    for subdivision in [0, 3] {
        for seed in 0..3 {
            let world = World::generate(Recipe {
                schema_version: 1,
                model_version: "basins-1".into(),
                random_version: "fnv1a-utf8-mulberry32-1".into(),
                seed: format!("reservoir-world-{seed}"),
                subdivision,
                radius_meters: 6371000.,
                plate_count: 12,
                max_plate_speed_cm_per_year: 8.,
                continental_fraction: 0.38,
                continental_scale: 1.,
                relief_scale: 1.,
                boundary_width_km: 300.,
                detail_amplitude_meters: 300.,
                water: WaterSettings::Coverage { fraction: 0.71 },
            })
            .unwrap();
            let before = wire::arrays(&world);
            for (id, node) in world.basins.nodes().iter().enumerate() {
                if !node.children.is_empty() {
                    continue;
                }
                let mut r = world.basins.isolated_leaf_reservoir(id, 0.).unwrap();
                if let Some(cap) = node.capacity_cubic_meters {
                    near(r.capacity_cubic_meters().unwrap(), cap);
                    // These experiments do not adopt or reset the world's water.
                    for input in [cap * 0.25, cap * 0.25, cap] {
                        r.add_input(input).unwrap_or_else(|error| panic!("{error}: seed={seed}, subdivision={subdivision}, branch={id}, cap={cap}, input={input}, checkpoint={:?}", r.checkpoint()));
                        audit(&r);
                        let bytes = serde_json::to_vec(&r.checkpoint()).unwrap();
                        let restored =
                            Reservoir::restore(serde_json::from_slice(&bytes).unwrap()).unwrap();
                        assert_eq!(r, restored);
                    }
                    near(r.inventory().outflow_volume_cubic_meters, cap * 0.5);
                }
            }
            assert_eq!(before, wire::arrays(&world));
        }
    }
}

#[test]
fn finest_flat_leaf_uses_iterative_indexing_and_keeps_all_input() {
    let surface = Surface::build(6, 6371000.);
    let b = Basins::build(&surface, &vec![0.; surface.areas.len()]).unwrap();
    let mut r = b.isolated_leaf_reservoir(b.root(), 0.).unwrap();
    let total_area: f64 = surface.areas.iter().sum();
    let pulse = r.add_input(total_area * 2.).unwrap();
    assert_eq!(pulse.water_level_meters, Some(2.));
    assert_eq!(pulse.outflow_cubic_meters, 0.);
    audit(&r);
}
