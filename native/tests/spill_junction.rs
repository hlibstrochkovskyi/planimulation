use planimulation_core::{
    Random,
    nested_reservoir::{Edge, Geometry, Stock},
    reservoir::Column,
    spill_junction::{POLICY_VERSION, Setup, SpillJunction, Storage, Weight},
};

fn setup() -> Setup {
    Setup {
        geometry: Geometry {
            columns: [0., 1., 2., 4.]
                .into_iter()
                .map(|bed_meters| Column {
                    bed_meters,
                    area_square_meters: 1.,
                })
                .collect(),
            edges: (0..3)
                .map(|i| Edge {
                    regions: [i, 3],
                    distance_meters: 1.,
                })
                .collect(),
        },
        policy_version: POLICY_VERSION.into(),
        weights: [1., 1., 3.]
            .into_iter()
            .enumerate()
            .map(|(branch, weight)| Weight { branch, weight })
            .collect(),
    }
}
fn stocks(values: &[f64]) -> Vec<Stock> {
    values
        .iter()
        .enumerate()
        .map(|(branch, &volume_cubic_meters)| Stock {
            branch,
            volume_cubic_meters,
        })
        .collect()
}
fn values(j: &SpillJunction) -> Vec<f64> {
    match j.checkpoint().inventory.storage {
        Storage::Separate(s) => s.iter().map(|v| v.volume_cubic_meters).collect(),
        Storage::Merged(s) => vec![s.volume_cubic_meters],
    }
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8_f64.max(b.abs() * 1e-10), "{a} != {b}");
}
fn audit(j: &SpillJunction) {
    let c = j.checkpoint();
    let snapshot = j.snapshot().unwrap();
    let b = j.connections().basins();
    let active = match &c.inventory.storage {
        Storage::Separate(s) => s.clone(),
        Storage::Merged(s) => vec![s.clone()],
    };
    let mut total = 0.;
    for (stock, level) in active.iter().zip(snapshot.levels_meters) {
        let reconstructed: f64 = c
            .setup
            .geometry
            .columns
            .iter()
            .enumerate()
            .filter(|(r, _)| stock.branch == b.root() || stock.branch == b.region_nodes()[*r])
            .map(|(_, c)| c.area_square_meters * level.map_or(0., |h| (h - c.bed_meters).max(0.)))
            .sum();
        near(stock.volume_cubic_meters, reconstructed);
        total += reconstructed;
    }
    near(total, c.inventory.input_cubic_meters);
    near(total, snapshot.total_stored_cubic_meters);
}
fn restored(j: &SpillJunction) -> SpillJunction {
    let checkpoint = serde_json::from_slice(&serde_json::to_vec(&j.checkpoint()).unwrap()).unwrap();
    let r = SpillJunction::restore(checkpoint).unwrap();
    assert_eq!(*j, r);
    r
}

#[test]
fn weighted_split_caps_receivers_and_merges_only_after_all_fill() {
    let mut j = SpillJunction::new(setup()).unwrap();
    assert_eq!(j.capacities(), stocks(&[4., 3., 2.]));
    j.add_input(&stocks(&[4., 0., 0.])).unwrap();
    assert_eq!(values(&j), [4., 0., 0.]);
    let pulse = j.add_input(&stocks(&[2., 0., 0.])).unwrap();
    assert_eq!(values(&j), [4., 0.5, 1.5]);
    assert_eq!(pulse.spill_supplied, stocks(&[2., 0., 0.]));
    assert_eq!(pulse.spill_received, stocks(&[0., 0.5, 1.5]));
    j.add_input(&stocks(&[2., 0., 0.])).unwrap();
    assert_eq!(values(&j), [4., 2., 2.]);
    j.add_input(&stocks(&[1., 0., 0.])).unwrap();
    assert_eq!(j.snapshot().unwrap().phase, "atSill");
    assert_eq!(j.snapshot().unwrap().levels_meters, [Some(4.)]);
    j.add_input(&stocks(&[4., 0., 0.])).unwrap();
    assert_eq!(j.snapshot().unwrap().phase, "merged");
    assert_eq!(j.snapshot().unwrap().levels_meters, [Some(5.)]);
    assert_eq!(values(&j), [13.]);
    audit(&j);
}

#[test]
fn equal_vs_prescribed_weights_are_explicit_and_scale_invariant() {
    let weighted = setup();
    let mut equal = weighted.clone();
    for w in &mut equal.weights {
        w.weight = 1.;
    }
    let mut a = SpillJunction::new(weighted.clone()).unwrap();
    let mut b = SpillJunction::new(equal).unwrap();
    a.add_input(&stocks(&[6., 0., 0.])).unwrap();
    b.add_input(&stocks(&[6., 0., 0.])).unwrap();
    assert_eq!(values(&a), [4., 0.5, 1.5]);
    assert_eq!(values(&b), [4., 1., 1.]);
    for factor in [0.125, 8., 1e200] {
        let mut scaled = weighted.clone();
        for w in &mut scaled.weights {
            w.weight *= factor;
        }
        let mut scaled = SpillJunction::new(scaled).unwrap();
        scaled.add_input(&stocks(&[6., 0., 0.])).unwrap();
        assert_eq!(scaled.snapshot().unwrap(), a.snapshot().unwrap());
    }
}

#[test]
fn simultaneous_inputs_fill_locally_before_pooling_and_replay_exactly() {
    let mut j = SpillJunction::new(setup()).unwrap();
    let p = j.add_input(&stocks(&[5., 4., 0.])).unwrap();
    assert_eq!(p.local_retained, stocks(&[4., 3., 0.]));
    assert_eq!(p.spill_supplied, stocks(&[1., 1., 0.]));
    assert_eq!(p.spill_received, stocks(&[0., 0., 2.]));
    assert_eq!(p.snapshot.phase, "atSill");
    let mut fresh = SpillJunction::new(setup()).unwrap();
    for input in [
        [0., 0., 0.],
        [4., 0., 0.],
        [0.12345678901234568, 0., 0.],
        [1., 0., 0.],
        [5., 2., 0.],
        [0., 0.25, 0.],
    ] {
        let mut resumed = restored(&fresh);
        assert_eq!(
            fresh.add_input(&stocks(&input)).unwrap(),
            resumed.add_input(&stocks(&input)).unwrap()
        );
        assert_eq!(fresh.checkpoint(), resumed.checkpoint());
        audit(&fresh);
    }
    restored(&fresh);
    // Equal final stock does not imply equal pulse-local transfer attribution.
    let mut batch = SpillJunction::new(setup()).unwrap();
    let together = batch.add_input(&stocks(&[6., 3., 0.])).unwrap();
    let mut serial = SpillJunction::new(setup()).unwrap();
    let first = serial.add_input(&stocks(&[6., 0., 0.])).unwrap();
    let second = serial.add_input(&stocks(&[0., 3., 0.])).unwrap();
    assert_eq!(batch.snapshot().unwrap(), serial.snapshot().unwrap());
    let accepted = |p: &planimulation_core::spill_junction::Pulse| {
        p.spill_received
            .iter()
            .map(|v| v.volume_cubic_meters)
            .sum::<f64>()
    };
    assert_eq!(accepted(&together), 2.);
    assert_eq!(accepted(&first) + accepted(&second), 2.5);
}

// Independent cumulative-input oracle: find lambda such that
// sum(min(cap_i, direct_i + lambda * weight_i)) = total supplied.
// This uses bisection, not the event-based saturation allocator.
fn oracle(caps: &[f64], direct: &[f64], weights: &[Weight]) -> Vec<f64> {
    let total: f64 = direct.iter().sum();
    if total >= caps.iter().sum() {
        return vec![total];
    }
    let mut lo = 0.;
    let mut hi = caps
        .iter()
        .zip(weights)
        .map(|(c, w)| c / w.weight)
        .fold(0., f64::max);
    for _ in 0..100 {
        let mid = (lo + hi) * 0.5;
        let stored: f64 = caps
            .iter()
            .zip(direct)
            .zip(weights)
            .map(|((&c, &d), w)| c.min(d + mid * w.weight))
            .sum();
        if stored < total {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    caps.iter()
        .zip(direct)
        .zip(weights)
        .map(|((&c, &d), w)| c.min(d + (lo + hi) * 0.5 * w.weight))
        .collect()
}

#[test]
fn seeded_batches_match_independent_oracle_and_order_partition_comparisons() {
    let mut rng = Random::stream("shared-junction", "tests");
    let mut random = || f64::from(rng.next_u32()) / 4294967296.;
    for case in 0..40 {
        let n = 3 + case % 6;
        let mut s = setup();
        s.geometry.columns = (0..n)
            .map(|i| Column {
                bed_meters: i as f64,
                area_square_meters: 0.5 + random() * 3.,
            })
            .collect();
        s.geometry.columns.push(Column {
            bed_meters: 12.,
            area_square_meters: 2.,
        });
        s.geometry.edges = (0..n)
            .map(|i| Edge {
                regions: [i, n],
                distance_meters: 1.,
            })
            .collect();
        s.weights = (0..n)
            .map(|branch| Weight {
                branch,
                weight: 0.2 + random() * 4.,
            })
            .collect();
        let caps: Vec<_> = s.geometry.columns[..n]
            .iter()
            .map(|c| c.area_square_meters * (12. - c.bed_meters))
            .collect();
        let mut j = SpillJunction::new(s.clone()).unwrap();
        let mut direct = vec![0.; n];
        let mut inputs = Vec::new();
        for _ in 0..60 {
            let mut pulse = vec![0.; n];
            for i in 0..n {
                pulse[i] = random() * 2.;
                direct[i] += pulse[i];
            }
            j.add_input(&stocks(&pulse)).unwrap();
            let expected = oracle(&caps, &direct, &s.weights);
            assert_eq!(values(&j).len(), expected.len());
            for (a, b) in values(&j).iter().zip(expected) {
                near(*a, b);
            }
            audit(&j);
            inputs.push(pulse);
        }
        let mut reverse = SpillJunction::new(s.clone()).unwrap();
        for input in inputs.iter().rev() {
            reverse.add_input(&stocks(input)).unwrap();
        }
        let mut one = SpillJunction::new(s.clone()).unwrap();
        one.add_input(&stocks(&direct)).unwrap();
        for other in [&reverse, &one] {
            assert_eq!(values(&j).len(), values(other).len());
            for (a, b) in values(&j).iter().zip(values(other)) {
                near(*a, b);
            }
        }
        // Separate partial-state comparison, before global merging can hide bias.
        let amounts: Vec<_> = caps
            .iter()
            .enumerate()
            .map(|(i, &c)| if i == 0 { c * 1.5 } else { c * 0.125 })
            .collect();
        let mut batch = SpillJunction::new(s.clone()).unwrap();
        batch.add_input(&stocks(&amounts)).unwrap();
        assert_eq!(batch.snapshot().unwrap().phase, "separate");
        for order in [(0..n).collect::<Vec<_>>(), (0..n).rev().collect()] {
            let mut serial = SpillJunction::new(s.clone()).unwrap();
            for id in order {
                let mut pulse = vec![0.; n];
                pulse[id] = amounts[id];
                serial.add_input(&stocks(&pulse)).unwrap();
            }
            for (a, b) in values(&batch).iter().zip(values(&serial)) {
                near(*a, b);
            }
        }
        restored(&j);
    }
}

#[test]
fn duplicate_contacts_do_not_multiply_receiver_weights_and_paths_remain_real() {
    let mut s = setup();
    s.geometry.columns.push(Column {
        bed_meters: 4.,
        area_square_meters: 1.,
    });
    s.geometry.edges.push(Edge {
        regions: [3, 4],
        distance_meters: 1.,
    });
    s.geometry.edges.push(Edge {
        regions: [2, 4],
        distance_meters: 1.,
    });
    let mut j = SpillJunction::new(s.clone()).unwrap();
    j.add_input(&stocks(&[6., 0., 0.])).unwrap();
    assert_eq!(values(&j), [4., 0.5, 1.5]);
    for a in 0..3 {
        for b in 0..3 {
            if a == b {
                continue;
            }
            let p = j.connections().passage(a, b, j.plateau()).unwrap();
            for edge in p.regions.windows(2) {
                assert!(
                    s.geometry
                        .edges
                        .iter()
                        .any(|e| e.regions == [edge[0] as usize, edge[1] as usize]
                            || e.regions == [edge[1] as usize, edge[0] as usize])
                );
            }
        }
    }
    s.geometry.edges.reverse();
    let reordered = SpillJunction::new(s).unwrap();
    assert_eq!(j.connections(), reordered.connections());
}

#[test]
fn separate_sills_nested_children_and_invalid_policies_are_rejected() {
    let mut bads = Vec::new();
    let mut s = setup();
    s.policy_version = "future".into();
    bads.push(s);
    let mut s = setup();
    s.weights.pop();
    bads.push(s);
    let mut s = setup();
    s.weights[0].branch = 1;
    bads.push(s);
    let mut s = setup();
    s.weights.reverse();
    bads.push(s);
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        let mut s = setup();
        s.weights[0].weight = value;
        bads.push(s);
    }
    let mut s = setup();
    s.weights[0].weight = f64::MIN_POSITIVE;
    s.weights[1].weight = f64::MAX;
    bads.push(s);
    for heights in [[0., 4., 1., 4., 2.], [0., 2., 1., 5., -1.]] {
        let mut s = setup();
        s.geometry.columns = heights
            .iter()
            .map(|&bed_meters| Column {
                bed_meters,
                area_square_meters: 1.,
            })
            .collect();
        s.geometry.edges = (1..5)
            .map(|i| Edge {
                regions: [i - 1, i],
                distance_meters: 1.,
            })
            .collect();
        bads.push(s);
    }
    // Two alternative connecting plateaus between the same bowls.
    let mut s = setup();
    s.geometry.columns = [0., 4., 1., 4.]
        .iter()
        .map(|&bed_meters| Column {
            bed_meters,
            area_square_meters: 1.,
        })
        .collect();
    s.geometry.edges = (0..4)
        .map(|i| Edge {
            regions: [i, (i + 1) % 4],
            distance_meters: 1.,
        })
        .collect();
    bads.push(s);
    for s in bads {
        assert!(SpillJunction::new(s).is_err());
    }
}

#[test]
fn malformed_checkpoints_and_failed_pulses_leave_the_last_state_unchanged() {
    let mut j = SpillJunction::new(setup()).unwrap();
    j.add_input(&stocks(&[6., 0., 0.])).unwrap();
    let good = j.checkpoint();
    let mut bad = good.clone();
    bad.experiment_version = "future".into();
    assert!(SpillJunction::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.input_cubic_meters += 1.;
    assert!(SpillJunction::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.pulse_count = 0;
    assert!(SpillJunction::restore(bad).is_err());
    for storage in [
        Storage::Separate(stocks(&[4., 3., 2.])),
        Storage::Separate(stocks(&[4., 0.5])),
        Storage::Separate(stocks(&[4., -1., 3.])),
        Storage::Merged(Stock {
            branch: 3,
            volume_cubic_meters: 6.,
        }),
        Storage::Merged(Stock {
            branch: 0,
            volume_cubic_meters: 9.,
        }),
    ] {
        let mut bad = good.clone();
        bad.inventory.storage = storage;
        assert!(SpillJunction::restore(bad).is_err());
    }
    for input in [
        stocks(&[1., 0.]),
        stocks(&[-1., 0., 0.]),
        stocks(&[f64::NAN, 0., 0.]),
        stocks(&[f64::INFINITY, 0., 0.]),
        stocks(&[f64::MIN_POSITIVE, 0., 0.]),
        stocks(&[f64::MAX, f64::MAX, 0.]),
    ] {
        assert!(j.add_input(&input).is_err());
        assert_eq!(j.checkpoint(), good);
    }
    let mut bad = good.clone();
    bad.inventory.pulse_count = u64::MAX;
    let mut max = SpillJunction::restore(bad.clone()).unwrap();
    assert!(max.add_input(&stocks(&[0., 0., 0.])).is_err());
    assert_eq!(max.checkpoint(), bad);
    restored(&j);
}

#[test]
fn region_relabeling_area_scaling_and_wide_junctions_preserve_physical_results() {
    let mut base = SpillJunction::new(setup()).unwrap();
    base.add_input(&stocks(&[6., 0., 0.])).unwrap();
    let mut s = setup();
    s.geometry.columns.reverse();
    for edge in &mut s.geometry.edges {
        edge.regions = edge.regions.map(|id| 3 - id);
    }
    let mut mirrored = SpillJunction::new(s).unwrap();
    mirrored.add_input(&stocks(&[6., 0., 0.])).unwrap();
    assert_eq!(mirrored.snapshot().unwrap(), base.snapshot().unwrap());
    let mut s = setup();
    for c in &mut s.geometry.columns {
        c.area_square_meters *= 3.;
        c.bed_meters += 1000.;
    }
    let mut scaled = SpillJunction::new(s).unwrap();
    scaled.add_input(&stocks(&[18., 0., 0.])).unwrap();
    for (a, b) in values(&base).iter().zip(values(&scaled)) {
        near(*a * 3., b);
    }
    audit(&scaled);
    let n = 127;
    let mut s = setup();
    s.geometry.columns = (0..n)
        .map(|_| Column {
            bed_meters: 0.,
            area_square_meters: 1.,
        })
        .collect();
    s.geometry.columns.push(Column {
        bed_meters: 1.,
        area_square_meters: 1.,
    });
    s.geometry.edges = (0..n)
        .map(|i| Edge {
            regions: [i, n],
            distance_meters: 1.,
        })
        .collect();
    s.weights = (0..n).map(|branch| Weight { branch, weight: 1. }).collect();
    let mut wide = SpillJunction::new(s).unwrap();
    let mut input = vec![0.; n];
    input[0] = 64.;
    wide.add_input(&stocks(&input)).unwrap();
    for v in &values(&wide)[1..] {
        near(*v, 0.5);
    }
    audit(&wide);
    restored(&wide);
}

#[test]
fn two_leaf_limit_matches_the_existing_pair_with_banks_and_weighted_columns() {
    use planimulation_core::reservoir_pair::{Geometry as PairGeometry, ReservoirPair, Volumes};
    let columns: Vec<_> = [(0., 2.), (1., 3.), (-1., 4.), (4., 1.), (6., 2.)]
        .into_iter()
        .map(|(bed_meters, area_square_meters)| Column {
            bed_meters,
            area_square_meters,
        })
        .collect();
    let mut pair = ReservoirPair::new(
        PairGeometry {
            left: columns[..2].to_vec(),
            right: vec![columns[2].clone()],
            connection: columns[3..].to_vec(),
            sill_meters: 4.,
        },
        Volumes {
            left: 0.,
            right: 0.,
        },
    )
    .unwrap();
    let mut j = SpillJunction::new(Setup {
        geometry: Geometry {
            columns,
            edges: [[0, 1], [1, 3], [2, 3], [3, 4]]
                .into_iter()
                .map(|regions| Edge {
                    regions,
                    distance_meters: 1.,
                })
                .collect(),
        },
        policy_version: POLICY_VERSION.into(),
        weights: vec![
            Weight {
                branch: 0,
                weight: 0.25,
            },
            Weight {
                branch: 1,
                weight: 5.,
            },
        ],
    })
    .unwrap();
    for (left, right) in [
        (1., 2.),
        (16., 0.),
        (3., 0.),
        (0., 15.),
        (20., 2.),
        (5., 6.),
    ] {
        let p = pair.add_input(Volumes { left, right }).unwrap();
        // Lower right minimum receives branch 0; left receives branch 1.
        let q = j.add_input(&stocks(&[right, left])).unwrap();
        near(
            q.snapshot.total_stored_cubic_meters,
            p.snapshot.total_stored_cubic_meters,
        );
        assert_eq!(q.snapshot.phase, p.snapshot.phase);
        if q.snapshot.phase == "separate" {
            assert_eq!(
                q.snapshot.levels_meters,
                [p.snapshot.levels_meters[1], p.snapshot.levels_meters[0]]
            );
        } else {
            assert_eq!(q.snapshot.levels_meters, [p.snapshot.levels_meters[0]]);
        }
        audit(&j);
    }
}
