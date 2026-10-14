use planimulation_core::{
    Recipe, Surface, World,
    basins::Basins,
    sediment_deposition::deposit,
    water::{Water, WaterSettings},
};

fn chain(areas: &[f64]) -> Surface {
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
    for i in 0..areas.len() {
        if i > 0 {
            s.neighbors.push((i - 1) as u32);
        }
        if i + 1 < areas.len() {
            s.neighbors.push((i + 1) as u32);
        }
        s.offsets.push(s.neighbors.len() as u32);
    }
    s.distances.resize(s.neighbors.len(), 1.);
    s
}
fn dry(s: &Surface, bed: &[f64]) -> Water {
    Water::generate(s, bed, &WaterSettings::Coverage { fraction: 0. }).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= b.abs().max(1e-300) * 1e-11, "{a} != {b}");
}

#[test]
fn weighted_nested_leaves_stop_at_first_merge_and_keep_excess_inventory() {
    let s = chain(&[2., 3., 5., 7., 11.]);
    let bed = [0., 2., 1., 5., -1.];
    let pending = [10., 0., 20., 0., 100.];
    let p = deposit(&s, &bed, &dry(&s, &bed), &pending, 10.).unwrap();
    assert_eq!(p.elevation_meters, [2., 2., 2., 5., 5.]);
    assert_eq!(p.deposited_thickness_meters, [2., 0., 1., 0., 6.]);
    assert_eq!(p.pending_volume_cubic_meters, [6., 0., 15., 0., 34.]);
    assert_eq!(p.accepted_volume_cubic_meters, 75.);
    assert_eq!(p.debited_volume_cubic_meters, 75.);
    assert_eq!(p.relative_inventory_ledger_residual, 0.);
    assert_eq!(p.eligible_leaf_count, 3);
    assert_eq!(p.changed_regions, 3);
    assert_eq!(bed, [0., 2., 1., 5., -1.]);
    assert_eq!(pending, [10., 0., 20., 0., 100.]);
}

#[test]
fn sibling_spill_is_not_an_ocean_outlet_and_wet_bed_never_receives_deposition() {
    let s = chain(&[2., 3., 5., 7., 11.]);
    let bed = [0., 2., 1., 5., -1.];
    let water = Water::generate(
        &s,
        &bed,
        &WaterSettings::Volume {
            volume_cubic_meters: 11.,
        },
    )
    .unwrap();
    assert_eq!(water.level_meters, 0.);
    let p = deposit(&s, &bed, &water, &[100., 0., 0., 0., 0.], 10.).unwrap();
    assert_eq!(p.elevation_meters, [2., 2., 1., 5., -1.]);
    assert_eq!(p.accepted_volume_cubic_meters, 4.);
    assert_eq!(p.pending_volume_cubic_meters, [96., 0., 0., 0., 0.]);
    assert!(deposit(&s, &bed, &water, &[0., 0., 0., 0., 1.], 1.).is_err());
}

#[test]
fn minimum_leaf_footprint_rises_only_where_level_reaches_it_and_obeys_rate_bound() {
    let s = chain(&[2., 3., 5., 7.]);
    let bed = [0., 1., 3., -1.];
    let water = dry(&s, &bed);
    let p = deposit(&s, &bed, &water, &[6., 0., 0., 0.], 1.5).unwrap();
    assert_eq!(p.elevation_meters, [1.5, 1.5, 3., -1.]);
    assert_eq!(p.deposited_thickness_meters, [1.5, 0.5, 0., 0.]);
    assert_eq!(p.accepted_volume_cubic_meters, 4.5);
    assert_eq!(p.pending_volume_cubic_meters, [1.5, 0., 0., 0.]);
    let inventory_bound = deposit(&s, &bed, &water, &[6., 0., 0., 0.], 10.).unwrap();
    close(inventory_bound.elevation_meters[0], 1.8);
    close(inventory_bound.elevation_meters[1], 1.8);
    close(inventory_bound.accepted_volume_cubic_meters, 6.);
    assert!(inventory_bound.pending_volume_cubic_meters[0] >= 0.);
}

#[test]
fn flat_minimum_pools_real_donor_stock_but_slope_stock_stays_pending() {
    let s = chain(&[2., 3., 5., 7., 11.]);
    let bed = [0., 0., 1., 3., -1.];
    let water = dry(&s, &bed);
    let p = deposit(&s, &bed, &water, &[2., 3., 99., 0., 0.], 10.).unwrap();
    assert_eq!(p.elevation_meters, [1., 1., 1., 3., -1.]);
    assert_eq!(p.accepted_volume_cubic_meters, 5.);
    assert_eq!(p.pending_volume_cubic_meters, [0., 0., 99., 0., 0.]);
}

#[test]
fn root_with_no_finite_spill_never_invents_a_deposition_threshold() {
    for bed in [[2., 0., 1.], [0.; 3]] {
        let s = chain(&[2., 3., 5.]);
        assert_eq!(Basins::build(&s, &bed).unwrap().nodes().len(), 1);
        let pending = [10., 20., 30.];
        let p = deposit(&s, &bed, &dry(&s, &bed), &pending, 100.).unwrap();
        assert_eq!(p.elevation_meters, bed);
        assert_eq!(p.pending_volume_cubic_meters, pending);
        assert_eq!(p.accepted_volume_cubic_meters, 0.);
        assert_eq!(p.eligible_leaf_count, 0);
    }
}

#[test]
fn zero_supply_zero_rate_and_all_wet_fields_are_exactly_unchanged() {
    let s = chain(&[1.; 3]);
    let bed = [0., 2., 1.];
    let water = dry(&s, &bed);
    for (pending, bound) in [([0.; 3], 10.), ([5., 0., 3.], 0.)] {
        let p = deposit(&s, &bed, &water, &pending, bound).unwrap();
        assert_eq!(p.elevation_meters, bed);
        assert_eq!(p.pending_volume_cubic_meters, pending);
        assert_eq!(p.accepted_volume_cubic_meters, 0.);
        assert_eq!(p.debited_volume_cubic_meters, 0.);
    }
    let wet = Water::generate(&s, &bed, &WaterSettings::Coverage { fraction: 1. }).unwrap();
    let p = deposit(&s, &bed, &wet, &[0.; 3], 1.).unwrap();
    assert_eq!(p.elevation_meters, bed);
    assert_eq!(p.accepted_volume_cubic_meters, 0.);
}

#[test]
fn large_area_fractional_debit_rounding_is_measured_not_repaired() {
    let s = chain(&[4.3e10, 6.1e10, 8.7e10]);
    let bed = [1000., 1004., 999.];
    let water = dry(&s, &bed);
    let pending = [123456789012345.67, 0., 0.];
    let p = deposit(&s, &bed, &water, &pending, 0.13).unwrap();
    let actual_debit = pending[0] - p.pending_volume_cubic_meters[0];
    let actual_prism = (p.elevation_meters[0] - bed[0]) * s.areas[0];
    assert_eq!(actual_debit, p.debited_volume_cubic_meters);
    assert_eq!(actual_prism, p.accepted_volume_cubic_meters);
    assert_ne!(actual_debit, actual_prism);
    assert!(p.relative_inventory_ledger_residual > 0.);
    assert!(p.relative_inventory_ledger_residual <= 1e-11);
    assert!(p.deposited_thickness_meters[0] <= 0.13);
    assert!(p.pending_volume_cubic_meters[0] >= 0.);
}

#[test]
fn rounded_rate_addition_cannot_exceed_requested_rise() {
    let s = chain(&[1.; 3]);
    let bed = [1., 2., 0.];
    let p = deposit(&s, &bed, &dry(&s, &bed), &[1., 0., 0.], 1.5e-16).unwrap();
    assert_eq!(p.elevation_meters, bed);
    assert_eq!(p.pending_volume_cubic_meters, [1., 0., 0.]);
    assert_eq!(p.accepted_volume_cubic_meters, 0.);
}

#[test]
fn unrepresentable_stock_debit_is_observed_without_deposition_or_stock_loss() {
    let s = chain(&[1.; 3]);
    let bed = [0., 1., -1.];
    let pending = [f64::MAX, 0., 0.];
    let p = deposit(&s, &bed, &dry(&s, &bed), &pending, 0.5).unwrap();
    assert_eq!(p.elevation_meters, bed);
    assert_eq!(p.pending_volume_cubic_meters, pending);
    assert_eq!(p.accepted_volume_cubic_meters, 0.);
    assert_eq!(p.debited_volume_cubic_meters, 0.);
    assert_eq!(p.unresolved_positive_deposition_leaves, 1);
    assert_eq!(p.unresolved_candidate_volume_cubic_meters, 0.5);
}

#[test]
fn partially_feasible_plateau_debits_are_atomic_and_overshoot_stays_pending() {
    let s = chain(&[1.; 4]);
    let bed = [0., 0., 1., -1.];
    let pending = [0.1, 1e20, 0., 0.];
    let p = deposit(&s, &bed, &dry(&s, &bed), &pending, 0.5).unwrap();
    assert_eq!(p.elevation_meters, bed);
    assert_eq!(p.pending_volume_cubic_meters, pending);
    assert_eq!(p.accepted_volume_cubic_meters, 0.);
    assert_eq!(p.unresolved_positive_deposition_leaves, 1);
    assert_eq!(p.unresolved_candidate_volume_cubic_meters, 1.);
    let s = chain(&[1.; 3]);
    let bed = [0., 3., -1.];
    let pending = [1e16, 0., 1.];
    let p = deposit(&s, &bed, &dry(&s, &bed), &pending, 3.).unwrap();
    // First donor would debit 4 for a proposed prism of 3; it is unchanged.
    assert_eq!(p.elevation_meters[..2], [0., 3.]);
    assert!(p.elevation_meters[2].abs() <= f64::EPSILON);
    assert_eq!(p.pending_volume_cubic_meters, [1e16, 0., 0.]);
    assert_eq!(p.accepted_volume_cubic_meters, 1.);
    assert_eq!(p.debited_volume_cubic_meters, 1.);
    assert_eq!(p.unresolved_positive_deposition_leaves, 1);
    assert_eq!(p.unresolved_candidate_volume_cubic_meters, 3.);
}

#[test]
fn observed_micrometre_spill_rounding_skips_conservatively() {
    let s = chain(&[186058461748.80878, 1e11, 1e11]);
    let bed = [861.2709417758092, 861.2709700064912, 850.];
    let pending = [18189286812184.355, 0., 0.];
    let p = deposit(&s, &bed, &dry(&s, &bed), &pending, 625.).unwrap();
    assert_eq!(p.elevation_meters, bed);
    assert_eq!(p.pending_volume_cubic_meters, pending);
    assert_eq!(p.accepted_volume_cubic_meters, 0.);
    assert_eq!(p.unresolved_positive_deposition_leaves, 1);
    assert!(p.unresolved_candidate_volume_cubic_meters > 5e6);
}

#[test]
fn positive_prism_underflow_still_refuses() {
    let tiny = chain(&[1e-300, 1., 1.]);
    let tiny_bed = [0., 1., -1e-300];
    assert!(
        deposit(
            &tiny,
            &tiny_bed,
            &dry(&tiny, &tiny_bed),
            &[1e-300, 0., 0.],
            1e-300
        )
        .is_err()
    );
}

#[test]
fn malformed_inputs_bad_water_disconnection_and_workload_bounds_refuse() {
    let s = chain(&[1.; 3]);
    let bed = [0., 2., 1.];
    let water = dry(&s, &bed);
    for bound in [-1., f64::NAN, f64::INFINITY] {
        assert!(deposit(&s, &bed, &water, &[0.; 3], bound).is_err());
    }
    for pending in [
        vec![0.],
        vec![-1., 0., 0.],
        vec![f64::NAN; 3],
        vec![f64::MAX; 3],
    ] {
        assert!(deposit(&s, &bed, &water, &pending, 1.).is_err());
    }
    let mut bad = s;
    bad.neighbors.clear();
    bad.distances.clear();
    bad.offsets = vec![0; 4];
    assert!(deposit(&bad, &bed, &water, &[0.; 3], 1.).is_err());
    let bad_water = Water {
        depth_meters: vec![1.; 3],
        ..water.clone()
    };
    assert!(deposit(&chain(&[1.; 3]), &bed, &bad_water, &[0.; 3], 1.).is_err());
    let huge = vec![0.; 1_000_001];
    assert!(deposit(&bad, &huge, &water, &huge, 1.).is_err());
    assert!(deposit(&bad, &[], &water, &[], 1.).is_err());
}

#[test]
fn generated_leaves_independently_audit_footprints_actual_debits_and_repeat() {
    for seed in [
        "first-light",
        "sediment-deposition-a",
        "sediment-deposition-b",
    ] {
        let recipe: Recipe = serde_json::from_value(serde_json::json!({
            "schemaVersion":1, "modelVersion":"multiscale-geography-2",
            "randomVersion":"fnv1a-utf8-mulberry32-1", "seed":seed,
            "subdivision":2, "radiusMeters":6371000., "plateCount":12,
            "maxPlateSpeedCmPerYear":8., "continentalFraction":0.4,
            "continentalScale":1., "reliefScale":1., "boundaryWidthKm":300.,
            "detailAmplitudeMeters":180., "terrainPreparationPasses":0,
            "water":{"mode":"coverage", "fraction":0.71}
        }))
        .unwrap();
        let world = World::generate(recipe).unwrap();
        let bed = &world.terrain.elevation;
        let tree = Basins::build(&world.surface, bed).unwrap();
        let pending: Vec<_> = bed
            .iter()
            .enumerate()
            .map(|(i, &h)| {
                let branch = &tree.nodes()[tree.region_nodes()[i]];
                if branch.children.is_empty()
                    && h == branch.birth_level_meters
                    && world.water.body_ids[i] == 0
                {
                    world.surface.areas[i] * 15.
                } else {
                    0.
                }
            })
            .collect();
        let before = (bed.clone(), pending.clone(), world.water.clone());
        let p = deposit(&world.surface, bed, &world.water, &pending, 20.).unwrap();
        let mut prisms = 0.;
        let mut debits = 0.;
        for i in 0..bed.len() {
            let increment = p.elevation_meters[i] - bed[i];
            assert_eq!(increment, p.deposited_thickness_meters[i]);
            assert!((0. ..=20.).contains(&increment));
            prisms += increment * world.surface.areas[i];
            debits += pending[i] - p.pending_volume_cubic_meters[i];
            if increment > 0. {
                let branch = &tree.nodes()[tree.region_nodes()[i]];
                assert!(branch.children.is_empty());
                assert!(p.elevation_meters[i] <= branch.spill_level_meters.unwrap());
                assert_eq!(world.water.body_ids[i], 0);
            }
        }
        close(prisms, p.accepted_volume_cubic_meters);
        close(debits, p.debited_volume_cubic_meters);
        close(prisms, debits);
        let repeat = deposit(&world.surface, bed, &world.water, &pending, 20.).unwrap();
        assert_eq!(
            serde_json::to_string(&p).unwrap(),
            serde_json::to_string(&repeat).unwrap()
        );
        assert_eq!(before, (bed.clone(), pending, world.water.clone()));
    }
}
