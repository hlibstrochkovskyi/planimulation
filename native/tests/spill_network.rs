use planimulation_core::{
    Random,
    nested_reservoir::{Edge, Geometry, Input, NestedReservoir, Stock},
    reservoir::Column,
    spill_connections::SpillConnections,
    spill_junction::{SpillJunction, Weight},
    spill_network::{POLICY_VERSION, Setup, SpillNetwork},
};

fn geometry(h: &[f64], edges: &[[usize; 2]]) -> Geometry {
    Geometry {
        columns: h
            .iter()
            .map(|&bed_meters| Column {
                bed_meters,
                area_square_meters: 1.,
            })
            .collect(),
        edges: edges
            .iter()
            .map(|&regions| Edge {
                regions,
                distance_meters: 1.,
            })
            .collect(),
    }
}
fn setup(g: Geometry, count: usize) -> Setup {
    Setup {
        geometry: g,
        policy_version: POLICY_VERSION.into(),
        weights: (0..count)
            .map(|branch| Weight { branch, weight: 1. })
            .collect(),
    }
}
fn chain() -> Setup {
    setup(
        geometry(&[0., 4., 1., 4., 2.], &[[0, 1], [1, 2], [2, 3], [3, 4]]),
        3,
    )
}
fn fork() -> Setup {
    setup(
        geometry(
            &[0., 4., 3., 4., 2., 4., 1.],
            &[[0, 1], [1, 2], [2, 3], [3, 4], [0, 5], [5, 6]],
        ),
        4,
    )
}
fn nested() -> Setup {
    setup(
        geometry(
            &[0., 5., 1., 2., 0.5, 5., 2., 5., 3.],
            &[
                [0, 1],
                [1, 2],
                [2, 3],
                [3, 4],
                [2, 5],
                [5, 6],
                [0, 7],
                [7, 8],
            ],
        ),
        6,
    )
}
fn pulse(
    n: &mut SpillNetwork,
    region: usize,
    volume_cubic_meters: f64,
) -> planimulation_core::spill_network::Pulse {
    n.add_input(Input {
        region,
        volume_cubic_meters,
    })
    .unwrap()
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-8_f64.max(b.abs() * 1e-10), "{a} != {b}");
}
fn under(b: &SpillConnections, mut child: usize, parent: usize) -> bool {
    loop {
        if child == parent {
            return true;
        }
        let Some(p) = b.basins().nodes()[child].parent else {
            return false;
        };
        child = p;
    }
}
fn depths(n: &SpillNetwork) -> Vec<f64> {
    let c = n.checkpoint();
    let b = n.connections().basins();
    let s = n.snapshot().unwrap();
    let mut result = vec![0.; c.setup.geometry.columns.len()];
    let mut total = 0.;
    for active in s.active {
        let mut measured = 0.;
        for (r, col) in c.setup.geometry.columns.iter().enumerate() {
            if under(n.connections(), b.region_nodes()[r], active.stock.branch) {
                result[r] = active
                    .water_level_meters
                    .map_or(0., |h| (h - col.bed_meters).max(0.));
                measured += result[r] * col.area_square_meters;
            }
        }
        near(measured, active.stock.volume_cubic_meters);
        total += measured;
    }
    near(total, c.inventory.input_cubic_meters);
    result
}
fn replay(n: &SpillNetwork) -> SpillNetwork {
    let bytes = serde_json::to_vec(&n.checkpoint()).unwrap();
    let restored = SpillNetwork::restore(serde_json::from_slice(&bytes).unwrap()).unwrap();
    assert_eq!(*n, restored);
    restored
}

#[test]
fn chain_blocks_unfilled_transit_then_exposes_the_next_receiver() {
    let mut n = SpillNetwork::new(chain()).unwrap();
    pulse(&mut n, 0, 4.);
    assert_eq!(depths(&n), [4., 0., 0., 0., 0.]);
    pulse(&mut n, 0, 1.);
    assert_eq!(depths(&n), [4., 0., 1., 0., 0.]);
    pulse(&mut n, 0, 2.);
    assert_eq!(depths(&n), [4., 0., 3., 0., 0.]);
    let p = pulse(&mut n, 0, 1.);
    assert_eq!(depths(&n), [4., 0., 3., 0., 1.]);
    assert_eq!(p.stages[0].receivers[0].transit_branches, [0, 1]);
    assert_eq!(p.stages[0].receivers[0].plateaus.len(), 2);
    assert_eq!(p.stages[0].receivers[0].entry_region, 4);
    pulse(&mut n, 0, 1.);
    assert_eq!(n.checkpoint().inventory.active.len(), 1);
    pulse(&mut n, 0, 5.);
    assert_eq!(depths(&n), [5., 1., 4., 1., 3.]);
    replay(&n);
}

#[test]
fn allocation_stops_at_first_saturation_and_recomputes_the_frontier() {
    let mut n = SpillNetwork::new(fork()).unwrap();
    let p = pulse(&mut n, 0, 7.);
    // A's 3 m³ excess initially reaches B and D. B fills after 1 m³,
    // opening C before the final 1 m³ is divided between C and D.
    assert_eq!(depths(&n), [4., 0., 1., 0., 0.5, 0., 1.5]);
    assert_eq!(p.stages.len(), 2);
    assert_eq!(p.stages[0].newly_saturated, [3]);
    assert_eq!(
        p.stages[1]
            .receivers
            .iter()
            .map(|r| r.branch)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    let c = p.stages[1]
        .receivers
        .iter()
        .find(|r| r.branch == 2)
        .unwrap();
    assert_eq!(c.transit_branches, [0, 3]);
    assert_eq!(c.entry_region, 4);
    let mut split = SpillNetwork::new(fork()).unwrap();
    for _ in 0..28 {
        pulse(&mut split, 0, 0.25);
    }
    assert_eq!(depths(&n), depths(&split));
}

#[test]
fn nested_receiver_uses_actual_entry_and_opens_outer_transit_only_at_outer_capacity() {
    let mut n = SpillNetwork::new(nested()).unwrap();
    for (input, expected) in [
        (5., vec![5., 0., 0., 0., 0., 0., 0., 0., 0.]),
        (1., vec![5., 0., 0.5, 0., 0., 0., 0., 0., 0.5]),
        (1., vec![5., 0., 1., 0., 0., 0., 0., 0., 1.]),
        (2., vec![5., 0., 1., 0., 1., 0., 0., 0., 2.]),
        (9.5, vec![5., 0., 4., 3., 4.5, 0., 0., 0., 2.]),
        (3., vec![5., 0., 4., 3., 4.5, 0., 3., 0., 2.]),
        (9., vec![6., 1., 5., 4., 5.5, 1., 4., 1., 3.]),
    ] {
        let mut restored = replay(&n);
        assert_eq!(pulse(&mut n, 0, input), pulse(&mut restored, 0, input));
        assert_eq!(depths(&n), expected);
    }
    assert_eq!(n.checkpoint().inventory.active.len(), 1);
    near(n.snapshot().unwrap().total_stored_cubic_meters, 30.5);
}

#[test]
fn cycles_and_alternative_sills_do_not_multiply_receiver_weights_or_loop() {
    let g = geometry(
        &[0., 1., 2., 4., 4., 4.],
        &[[0, 3], [3, 1], [1, 4], [4, 2], [2, 5], [5, 0]],
    );
    let mut n = SpillNetwork::new(setup(g, 3)).unwrap();
    pulse(&mut n, 0, 6.);
    assert_eq!(depths(&n), [4., 1., 1., 0., 0., 0.]);
    pulse(&mut n, 0, 3.);
    assert_eq!(n.checkpoint().inventory.active.len(), 1);
    let mut n = SpillNetwork::new(setup(
        geometry(&[0., 4., 1., 4.], &[[0, 1], [1, 2], [2, 3], [3, 0]]),
        2,
    ))
    .unwrap();
    pulse(&mut n, 0, 5.);
    assert_eq!(depths(&n), [4., 0., 1., 0.]);
    assert_eq!(n.connections().plateaus().len(), 2);
}

#[test]
fn binary_and_shared_sill_limits_match_previous_experiments() {
    let g = geometry(&[0., 2., 1., 5., -1.], &[[0, 1], [1, 2], [2, 3], [3, 4]]);
    let mut binary = NestedReservoir::new(g.clone()).unwrap();
    let mut network = SpillNetwork::new(setup(g, 4)).unwrap();
    for region in [4, 0, 2, 4, 3, 4, 1] {
        let input = Input {
            region,
            volume_cubic_meters: 2.5,
        };
        binary.add_input(input.clone()).unwrap();
        network.add_input(input).unwrap();
        assert_eq!(binary.snapshot().unwrap(), network.snapshot().unwrap());
    }
    let g = geometry(&[0., 1., 2., 4.], &[[0, 3], [1, 3], [2, 3]]);
    let mut s = setup(g.clone(), 3);
    s.weights[2].weight = 3.;
    let mut junction = SpillJunction::new(planimulation_core::spill_junction::Setup {
        geometry: g,
        policy_version: planimulation_core::spill_junction::POLICY_VERSION.into(),
        weights: s.weights.clone(),
    })
    .unwrap();
    let mut network = SpillNetwork::new(s).unwrap();
    for input in [4., 2., 2., 1., 4.] {
        pulse(&mut network, 0, input);
        let p = junction
            .add_input(&[
                Stock {
                    branch: 0,
                    volume_cubic_meters: input,
                },
                Stock {
                    branch: 1,
                    volume_cubic_meters: 0.,
                },
                Stock {
                    branch: 2,
                    volume_cubic_meters: 0.,
                },
            ])
            .unwrap();
        near(
            network.snapshot().unwrap().total_stored_cubic_meters,
            p.snapshot.total_stored_cubic_meters,
        );
        assert_eq!(
            network
                .snapshot()
                .unwrap()
                .active
                .iter()
                .map(|s| s.water_level_meters)
                .collect::<Vec<_>>(),
            p.snapshot.levels_meters
        );
    }
}

#[test]
fn ambiguous_nested_entries_invalid_weights_and_corrupt_checkpoints_fail_atomically() {
    let g = geometry(
        &[0., 4., 1., 2., 0.5, 4., 3.],
        &[[0, 1], [1, 2], [2, 3], [3, 4], [4, 5], [5, 6]],
    );
    assert!(
        SpillNetwork::new(setup(g, 5))
            .unwrap_err()
            .contains("Alternative entries")
    );
    for weights in [
        vec![],
        vec![
            Weight {
                branch: 0,
                weight: 0.
            };
            3
        ],
        vec![
            Weight {
                branch: 0,
                weight: 1.
            };
            3
        ],
    ] {
        let mut bad = chain();
        bad.weights = weights;
        assert!(SpillNetwork::new(bad).is_err());
    }
    let mut n = SpillNetwork::new(nested()).unwrap();
    pulse(&mut n, 0, 7.);
    let good = n.checkpoint();
    for (r, v) in [
        (999, 1.),
        (0, -1.),
        (0, f64::INFINITY),
        (0, f64::NAN),
        (0, f64::MIN_POSITIVE),
    ] {
        assert!(
            n.add_input(Input {
                region: r,
                volume_cubic_meters: v
            })
            .is_err()
        );
        assert_eq!(n.checkpoint(), good);
    }
    let mut bad = good.clone();
    bad.experiment_version = "future".into();
    assert!(SpillNetwork::restore(bad).is_err());
    let mut bad = good.clone();
    bad.setup.policy_version = "future".into();
    assert!(SpillNetwork::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.input_cubic_meters += 1.;
    assert!(SpillNetwork::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.active.pop();
    assert!(SpillNetwork::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.active.reverse();
    assert!(SpillNetwork::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.active[0].volume_cubic_meters = -1.;
    assert!(SpillNetwork::restore(bad).is_err());
    let mut bad = good.clone();
    bad.inventory.pulse_count = u64::MAX;
    let mut max = SpillNetwork::restore(bad.clone()).unwrap();
    assert!(
        max.add_input(Input {
            region: 0,
            volume_cubic_meters: 0.
        })
        .is_err()
    );
    assert_eq!(max.checkpoint(), bad);
}

#[test]
fn weighted_fork_matches_independent_piecewise_event_oracle() {
    let mut rng = Random::stream("network-fork", "tests");
    let mut random = || f64::from(rng.next_u32()) / 4294967296.;
    for _ in 0..40 {
        let mut s = fork();
        let scale = 0.5 + random() * 3.;
        for col in &mut s.geometry.columns {
            col.area_square_meters *= scale;
        }
        let wb = 0.5 + random() * 2.;
        let wd = 0.5 + random() * 2.;
        let wc = 0.5 + random() * 2.;
        s.weights[3].weight = wb;
        s.weights[1].weight = wd;
        s.weights[2].weight = wc;
        // Restrict D so that B is the first gate to open. Oracle knows the
        // two exact stages analytically and uses no graph or indexed curves.
        s.geometry.columns[6].area_square_meters = 10. * scale;
        let mut n = SpillNetwork::new(s).unwrap();
        let event = scale * (wb + wd) / wb;
        for k in 0..30 {
            // Do not ask arbitrary floating geometry to hit the analytical
            // threshold exactly; that representability case is tested below.
            let extra = event * (k as f64 + 0.75) / 20.;
            let target = 4. * scale + extra;
            let delta = target - n.checkpoint().inventory.input_cubic_meters;
            n.add_input(Input { region: 0, volume_cubic_meters: delta }).unwrap_or_else(|e|
                panic!("{e}; step={k}, scale={scale}, weights={wb},{wd},{wc}, delta={delta}, state={:?}",n.checkpoint().inventory));
            let (b, c, d) = if extra <= event {
                (extra * wb / (wb + wd), 0., extra * wd / (wb + wd))
            } else {
                (
                    scale,
                    (extra - event) * wc / (wc + wd),
                    event * wd / (wb + wd) + (extra - event) * wd / (wc + wd),
                )
            };
            let depth = depths(&n);
            near(depth[2] * scale, b);
            near(depth[4] * scale, c);
            near(depth[6] * 10. * scale, d);
        }
        replay(&n);
    }
}

#[test]
fn long_transit_chain_and_closed_flat_remain_bounded_and_reproducible() {
    let h: Vec<_> = (0..127).map(|r| if r % 2 == 0 { 0. } else { 1. }).collect();
    let edges: Vec<_> = (1..127).map(|i| [i - 1, i]).collect();
    let mut n = SpillNetwork::new(setup(geometry(&h, &edges), 64)).unwrap();
    let p = pulse(&mut n, 0, 63.5);
    assert_eq!(p.stages.len(), 63);
    assert_eq!(
        p.stages.last().unwrap().receivers[0].transit_branches.len(),
        63
    );
    depths(&n);
    replay(&n);
    pulse(&mut n, 0, 0.5);
    assert_eq!(n.checkpoint().inventory.active.len(), 1);
    let mut flat = SpillNetwork::new(setup(geometry(&[0., 0., 0.], &[[0, 1], [1, 2]]), 0)).unwrap();
    pulse(&mut flat, 2, 3.);
    assert_eq!(depths(&flat), [1., 1., 1.]);
    replay(&flat);
}

#[test]
fn unrepresentable_threshold_remainder_is_rejected_without_discarding_water() {
    let scale = 1.8164103149902076;
    let wb = 1.8751503708772361;
    let wd = 1.468324460554868;
    let mut s = fork();
    for c in &mut s.geometry.columns {
        c.area_square_meters *= scale;
    }
    s.geometry.columns[6].area_square_meters = 10. * scale;
    s.weights[3].weight = wb;
    s.weights[1].weight = wd;
    s.weights[2].weight = 1.6618032548576593;
    let mut n = SpillNetwork::new(s).unwrap();
    let event = scale * (wb + wd) / wb;
    for k in 0..19 {
        let target = 4. * scale + event * (k as f64 + 1.) / 20.;
        let delta = target - n.checkpoint().inventory.input_cubic_meters;
        pulse(&mut n, 0, delta);
    }
    let before = n.checkpoint();
    let target = 4. * scale + event;
    let result = n.add_input(Input {
        region: 0,
        volume_cubic_meters: target - before.inventory.input_cubic_meters,
    });
    assert!(result.unwrap_err().contains("precision"));
    assert_eq!(n.checkpoint(), before);
    // A later resolvable input can still cross the event and open the new route.
    pulse(&mut n, 0, 1.);
    depths(&n);
    replay(&n);
}

#[test]
fn dynamic_frontiers_match_independent_raw_region_floods_through_only_full_bowls() {
    use std::collections::BTreeSet;
    let mut rng = Random::stream("network-frontiers", "tests");
    for _ in 0..60 {
        let n = 8;
        let mut links: BTreeSet<_> = (1..n).map(|i| [i - 1, i]).collect();
        for _ in 0..8 {
            let a = rng.next_u32() as usize % n;
            let b = rng.next_u32() as usize % n;
            if a != b {
                links.insert([a.min(b), a.max(b)]);
            }
        }
        let mut heights = vec![0.; n];
        let mut edges = Vec::new();
        for [a, b] in links {
            let sill = heights.len();
            heights.push(4.);
            edges.push([a, sill]);
            edges.push([sill, b]);
        }
        let model = SpillNetwork::new(setup(geometry(&heights, &edges), n)).unwrap();
        let mut checkpoint = model.checkpoint();
        for s in &mut checkpoint.inventory.active {
            s.volume_cubic_meters = (rng.next_u32() % 3) as f64 * 2.;
        }
        checkpoint.inventory.active[0].volume_cubic_meters = 4.;
        checkpoint.inventory.active[n - 1].volume_cubic_meters = 0.;
        checkpoint.inventory.input_cubic_meters = checkpoint
            .inventory
            .active
            .iter()
            .map(|s| s.volume_cubic_meters)
            .sum();
        checkpoint.inventory.pulse_count = 1;
        let model = SpillNetwork::restore(checkpoint.clone()).unwrap();
        for source in 0..n {
            if checkpoint.inventory.active[source].volume_cubic_meters != 4. {
                assert!(model.receivers(source).is_err());
                continue;
            }
            let mut queue = vec![source];
            let mut seen = vec![false; heights.len()];
            seen[source] = true;
            let mut expected = BTreeSet::new();
            let mut cursor = 0;
            while cursor < queue.len() {
                let r = queue[cursor];
                cursor += 1;
                for [a, b] in &edges {
                    let next = if *a == r {
                        *b
                    } else if *b == r {
                        *a
                    } else {
                        continue;
                    };
                    if seen[next] {
                        continue;
                    }
                    seen[next] = true;
                    if next < n && checkpoint.inventory.active[next].volume_cubic_meters < 4. {
                        expected.insert(next);
                    } else {
                        queue.push(next);
                    }
                }
            }
            let actual = model.receivers(source).unwrap();
            assert_eq!(
                actual.iter().map(|r| r.branch).collect::<BTreeSet<_>>(),
                expected
            );
            for r in actual {
                assert_eq!(r.transit_branches[0], source);
                assert_eq!(r.transit_branches.len(), r.plateaus.len());
                for (i, &from) in r.transit_branches.iter().enumerate() {
                    assert_eq!(checkpoint.inventory.active[from].volume_cubic_meters, 4.);
                    let to = r.transit_branches.get(i + 1).copied().unwrap_or(r.branch);
                    let path = model
                        .connections()
                        .passage(from, to, r.plateaus[i])
                        .unwrap();
                    for edge in path.regions.windows(2) {
                        assert!(
                            edges
                                .iter()
                                .any(|e| *e == [edge[0] as usize, edge[1] as usize]
                                    || *e == [edge[1] as usize, edge[0] as usize])
                        );
                    }
                    if i + 1 == r.transit_branches.len() {
                        assert_eq!(*path.regions.last().unwrap(), r.entry_region);
                    }
                }
            }
        }
        assert!(model.receivers(n).is_err());
        assert!(model.receivers(usize::MAX).is_err());
        assert_eq!(checkpoint, model.checkpoint());
    }
}

#[test]
fn ordered_pulses_preserve_history_when_a_gate_opens_at_different_times() {
    let mut ab = SpillNetwork::new(fork()).unwrap();
    pulse(&mut ab, 0, 5.);
    let mut resumed = replay(&ab);
    assert_eq!(pulse(&mut ab, 2, 1.), pulse(&mut resumed, 2, 1.));
    let mut ba = SpillNetwork::new(fork()).unwrap();
    pulse(&mut ba, 2, 1.);
    pulse(&mut ba, 0, 5.);
    assert_eq!(depths(&ab), [4., 0., 1., 0., 0.25, 0., 0.75]);
    assert_eq!(depths(&ba), [4., 0., 1., 0., 0.5, 0., 0.5]);
    assert_eq!(ab.checkpoint().inventory.input_cubic_meters, 6.);
    assert_eq!(ba.checkpoint().inventory.input_cubic_meters, 6.);
    // Same total inputs, different timing of access to C. This policy does not
    // turn a list of ordered region pulses into simultaneous precipitation.
}
