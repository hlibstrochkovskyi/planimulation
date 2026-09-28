use planimulation_core::{
    Random,
    nested_reservoir::{Edge, Geometry, Input},
    reservoir::Column,
    spill_junction::{SpillJunction, Weight},
    spill_network::{
        self, SpillNetwork,
        simultaneous::{Checkpoint, POLICY_VERSION, Setup, SimultaneousNetwork},
    },
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
fn fork() -> Setup {
    setup(
        geometry(
            &[0., 4., 3., 4., 2., 4., 1.],
            &[[0, 1], [1, 2], [2, 3], [3, 4], [0, 5], [5, 6]],
        ),
        4,
    )
}
fn input(region: usize, volume_cubic_meters: f64) -> Input {
    Input {
        region,
        volume_cubic_meters,
    }
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9_f64.max(b.abs() * 1e-10), "{a} != {b}");
}
fn region_stock(model: &SimultaneousNetwork, region: usize) -> f64 {
    let branch = model.connections().basins().region_nodes()[region];
    model
        .checkpoint()
        .inventory
        .active
        .iter()
        .find(|s| s.branch == branch)
        .unwrap()
        .volume_cubic_meters
}
fn ordered(s: &Setup) -> SpillNetwork {
    SpillNetwork::new(spill_network::Setup {
        geometry: s.geometry.clone(),
        policy_version: spill_network::POLICY_VERSION.into(),
        weights: s.weights.clone(),
    })
    .unwrap()
}
fn compare(a: &SimultaneousNetwork, b: &SimultaneousNetwork) {
    let a = a.snapshot().unwrap();
    let b = b.snapshot().unwrap();
    assert_eq!(a.active.len(), b.active.len());
    for (a, b) in a.active.iter().zip(&b.active) {
        assert_eq!(a.stock.branch, b.stock.branch);
        near(a.stock.volume_cubic_meters, b.stock.volume_cubic_meters);
        match (a.water_level_meters, b.water_level_meters) {
            (Some(a), Some(b)) => near(a, b),
            (None, None) => {}
            _ => panic!("level presence"),
        }
    }
}
fn restore(model: &SimultaneousNetwork) -> SimultaneousNetwork {
    let bytes = serde_json::to_vec(&model.checkpoint()).unwrap();
    SimultaneousNetwork::restore(serde_json::from_slice(&bytes).unwrap()).unwrap()
}

#[test]
fn simultaneous_fork_is_neither_ordered_sequence_and_is_permutation_invariant() {
    let mut a = SimultaneousNetwork::new(fork()).unwrap();
    let mut b = a.clone();
    let report = a.add_interval(vec![input(0, 5.), input(2, 1.)]).unwrap();
    assert_eq!(
        report,
        b.add_interval(vec![input(2, 1.), input(0, 5.)]).unwrap()
    );
    assert_eq!(a.checkpoint(), b.checkpoint());
    near(region_stock(&a, 0), 4.);
    near(region_stock(&a, 2), 1.);
    near(region_stock(&a, 4), 3. / 7.);
    near(region_stock(&a, 6), 4. / 7.);
    // Preserve non-binary fractions, not just dry or integer common stocks.
    let mut continued = a.clone();
    let mut resumed = restore(&a);
    assert_eq!(
        continued.add_interval(vec![input(0, 0.75)]).unwrap(),
        resumed.add_interval(vec![input(0, 0.75)]).unwrap()
    );
    assert_eq!(continued.checkpoint(), resumed.checkpoint());
    assert_eq!(report.events.len(), 3);
    near(report.events.iter().map(|e| e.input_cubic_meters).sum(), 6.);
    near(report.events.iter().map(|e| e.duration_fraction).sum(), 1.);
    for event in &report.events {
        near(
            event.retained.iter().map(|s| s.volume_cubic_meters).sum(),
            event.input_cubic_meters,
        );
    }
    near(report.events[0].duration_fraction, 0.8);
    near(report.events[1].duration_fraction, 2. / 35.);
    near(report.events[2].duration_fraction, 1. / 7.);
    let mut ab = ordered(&fork());
    let mut ba = ordered(&fork());
    ab.add_input(input(0, 5.)).unwrap();
    ab.add_input(input(2, 1.)).unwrap();
    ba.add_input(input(2, 1.)).unwrap();
    ba.add_input(input(0, 5.)).unwrap();
    assert_ne!(
        a.checkpoint().inventory.active,
        ab.checkpoint().inventory.active
    );
    assert_ne!(
        a.checkpoint().inventory.active,
        ba.checkpoint().inventory.active
    );
}

#[test]
fn weighted_forks_match_independent_concurrent_event_equations() {
    let mut rng = Random::stream("20260928", "simultaneous-forks");
    let mut random = || f64::from(rng.next_u32()) / 4294967296.;
    for case in 0..100 {
        let scale = 2_f64.powi(case % 9 - 4);
        let wb = 0.8 + 0.4 * random();
        let wc = 0.8 + 0.4 * random();
        let wd = 0.8 + 0.4 * random();
        let mut s = fork();
        for col in &mut s.geometry.columns {
            col.area_square_meters = scale;
        }
        let dry = SimultaneousNetwork::new(s.clone()).unwrap();
        for (region, weight) in [(2, wb), (4, wc), (6, wd)] {
            s.weights[dry.connections().basins().region_nodes()[region]].weight = weight;
        }
        let mut model = SimultaneousNetwork::new(s.clone()).unwrap();
        let mut reversed = model.clone();
        let report = model
            .add_interval(vec![input(0, 5. * scale), input(2, scale)])
            .unwrap();
        assert_eq!(
            report,
            reversed
                .add_interval(vec![input(2, scale), input(0, 5. * scale)])
                .unwrap()
        );
        let middle = 0.2 / (1. + 5. * wb / (wb + wd));
        let tail = 0.2 - middle;
        near(region_stock(&model, 4), 6. * wc / (wc + wd) * tail * scale);
        near(
            region_stock(&model, 6),
            (5. * wd / (wb + wd) * middle + 6. * wd / (wc + wd) * tail) * scale,
        );
        near(report.snapshot.total_stored_cubic_meters, 6. * scale);
        for weight in &mut s.weights {
            weight.weight *= 8.;
        }
        let mut rescaled = SimultaneousNetwork::new(s).unwrap();
        rescaled
            .add_interval(vec![input(0, 5. * scale), input(2, scale)])
            .unwrap();
        compare(&model, &rescaled);
    }
}

#[test]
fn proportional_time_partition_and_exact_checkpoint_continuation() {
    let mut whole = SimultaneousNetwork::new(fork()).unwrap();
    whole
        .add_interval(vec![input(0, 5.), input(2, 1.)])
        .unwrap();
    for count in [2, 4, 8, 16] {
        let mut split = SimultaneousNetwork::new(fork()).unwrap();
        for _ in 0..count {
            let mut resumed = restore(&split);
            let inputs = vec![input(0, 5. / count as f64), input(2, 1. / count as f64)];
            assert_eq!(
                split.add_interval(inputs.clone()).unwrap(),
                resumed.add_interval(inputs).unwrap()
            );
            assert_eq!(split.checkpoint(), resumed.checkpoint());
        }
        compare(&whole, &split);
    }
}

#[test]
fn nested_entry_and_single_source_limit_match_ordered_experiment() {
    let request: serde_json::Value =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-network.json")).unwrap();
    let original: spill_network::Setup =
        serde_json::from_value(request["start"]["dry"].clone()).unwrap();
    let s = Setup {
        geometry: original.geometry.clone(),
        weights: original.weights.clone(),
        policy_version: POLICY_VERSION.into(),
    };
    let mut model = SimultaneousNetwork::new(s.clone()).unwrap();
    let mut reference = SpillNetwork::new(original).unwrap();
    for volume in [5., 1., 1., 2., 9.5, 3., 9.] {
        let report = model
            .add_interval(vec![input(0, volume)])
            .unwrap_or_else(|e| panic!("input {volume}: {e}"));
        let other = reference.add_input(input(0, volume)).unwrap();
        assert_eq!(report.snapshot.active.len(), other.snapshot.active.len());
        for (a, b) in report.snapshot.active.iter().zip(&other.snapshot.active) {
            assert_eq!(a.stock.branch, b.stock.branch);
            near(a.stock.volume_cubic_meters, b.stock.volume_cubic_meters);
        }
    }
    near(model.snapshot().unwrap().total_stored_cubic_meters, 30.5);
    near(
        model.snapshot().unwrap().active[0]
            .water_level_meters
            .unwrap(),
        6.,
    );
    let mut concurrent = SimultaneousNetwork::new(s.clone()).unwrap();
    let batch = vec![input(0, 8.), input(4, 3.), input(6, 1.)];
    concurrent.add_interval(batch.clone()).unwrap();
    let mut split = SimultaneousNetwork::new(s).unwrap();
    for _ in 0..4 {
        split
            .add_interval(
                batch
                    .iter()
                    .map(|i| input(i.region, i.volume_cubic_meters / 4.))
                    .collect(),
            )
            .unwrap();
    }
    compare(&concurrent, &split);
    assert!(concurrent.checkpoint().inventory.active.len() > 1);
}

#[test]
fn multiple_full_sources_combine_at_one_receiver_and_merge_at_exact_endpoint() {
    let s = setup(
        geometry(&[0., 1., 0., 1., 0.], &[[0, 1], [1, 2], [2, 3], [3, 4]]),
        3,
    );
    let mut model = SimultaneousNetwork::new(s).unwrap();
    model
        .add_interval(vec![input(0, 1.), input(4, 1.)])
        .unwrap();
    let report = model
        .add_interval(vec![input(0, 0.5), input(4, 0.5)])
        .unwrap();
    assert_eq!(report.events.len(), 1);
    assert_eq!(report.events[0].transfers.len(), 2);
    assert_eq!(report.events[0].merged.len(), 1);
    assert_eq!(report.snapshot.active.len(), 1);
    assert_eq!(report.snapshot.total_stored_cubic_meters, 3.);
    let next = model
        .add_interval(vec![input(0, 2.), input(4, 3.)])
        .unwrap();
    assert_eq!(next.snapshot.active[0].water_level_meters, Some(2.));
}

#[test]
fn shared_sill_limit_matches_local_first_batch_model() {
    let g = geometry(&[0., 1., 2., 4.], &[[0, 3], [1, 3], [2, 3]]);
    let mut s = setup(g.clone(), 3);
    s.weights[2].weight = 3.;
    let mut model = SimultaneousNetwork::new(s.clone()).unwrap();
    let mut reference = SpillJunction::new(planimulation_core::spill_junction::Setup {
        geometry: g,
        policy_version: planimulation_core::spill_junction::POLICY_VERSION.into(),
        weights: s.weights,
    })
    .unwrap();
    for volumes in [
        [5., 0.5, 0.25],
        [0.5, 0.25, 0.125],
        [2., 1., 1.],
        [1., 1., 1.],
    ] {
        let report = model
            .add_interval(
                volumes
                    .iter()
                    .enumerate()
                    .map(|(r, &v)| input(r, v))
                    .collect(),
            )
            .unwrap();
        let other = reference
            .add_input(
                &volumes
                    .iter()
                    .enumerate()
                    .map(|(branch, &volume_cubic_meters)| {
                        planimulation_core::nested_reservoir::Stock {
                            branch,
                            volume_cubic_meters,
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        near(
            report.snapshot.total_stored_cubic_meters,
            other.snapshot.total_stored_cubic_meters,
        );
        let stocks = match &other.snapshot.storage {
            planimulation_core::spill_junction::Storage::Separate(stocks) => stocks.clone(),
            planimulation_core::spill_junction::Storage::Merged(stock) => vec![stock.clone()],
        };
        assert_eq!(report.snapshot.active.len(), stocks.len());
        for (a, b) in report.snapshot.active.iter().zip(stocks) {
            assert_eq!(a.stock.branch, b.branch);
            near(a.stock.volume_cubic_meters, b.volume_cubic_meters);
        }
    }
}

#[test]
fn cyclic_networks_match_an_independent_full_component_rate_oracle() {
    let mut rng = Random::stream("concurrent-components", "network-test");
    for case in 0..60 {
        let count = 8;
        let caps: Vec<_> = (0..count)
            .map(|_| 1. + (rng.next_u32() % 4) as f64)
            .collect();
        let weights: Vec<_> = (0..count)
            .map(|_| 1. + (rng.next_u32() % 4) as f64)
            .collect();
        let mut pairs: Vec<_> = (0..count - 1).map(|i| [i, i + 1]).collect();
        for _ in 0..12 {
            let a = rng.next_u32() as usize % count;
            let b = rng.next_u32() as usize % count;
            let pair = [a.min(b), a.max(b)];
            if a != b && !pairs.contains(&pair) {
                pairs.push(pair);
            }
        }
        let mut heights: Vec<_> = caps.iter().map(|c| 4. - c).collect();
        let mut edges = Vec::new();
        for [a, b] in &pairs {
            let sill = heights.len();
            heights.push(4.);
            edges.extend([[*a, sill], [sill, *b]]);
        }
        let mut s = setup(geometry(&heights, &edges), count);
        let dry = SimultaneousNetwork::new(s.clone()).unwrap();
        for (r, &weight) in weights.iter().enumerate() {
            s.weights[dry.connections().basins().region_nodes()[r]].weight = weight;
        }
        let mut model = SimultaneousNetwork::new(s).unwrap();
        // Fresh forcing is below total capacity, but concentrated enough to
        // cross several thresholds. No common-lake result can hide a bias.
        let forcing: Vec<_> = (0..count)
            .map(|r| if r % 3 == case % 3 { 3.25 } else { 0.125 })
            .collect();
        let mut expected = vec![0.; count];
        let mut remaining = 1.;
        for _ in 0..=count {
            if remaining == 0. {
                break;
            }
            let full: Vec<_> = expected.iter().zip(&caps).map(|(v, c)| v == c).collect();
            let mut rates: Vec<_> = forcing
                .iter()
                .enumerate()
                .map(|(i, &v)| if full[i] { 0. } else { v })
                .collect();
            let mut visited = vec![false; count];
            for seed in 0..count {
                if !full[seed] || visited[seed] {
                    continue;
                }
                let mut component = vec![seed];
                let mut frontier = vec![false; count];
                visited[seed] = true;
                let mut cursor = 0;
                while cursor < component.len() {
                    let from = component[cursor];
                    cursor += 1;
                    for &[a, b] in &pairs {
                        let to = if a == from {
                            b
                        } else if b == from {
                            a
                        } else {
                            continue;
                        };
                        if full[to] {
                            if !visited[to] {
                                visited[to] = true;
                                component.push(to);
                            }
                        } else {
                            frontier[to] = true;
                        }
                    }
                }
                let pool: f64 = component.iter().map(|&i| forcing[i]).sum();
                let sum: f64 = (0..count)
                    .filter(|&i| frontier[i])
                    .map(|i| weights[i])
                    .sum();
                assert!(sum > 0.);
                for i in 0..count {
                    if frontier[i] {
                        rates[i] += pool * weights[i] / sum;
                    }
                }
            }
            let times: Vec<_> = (0..count)
                .map(|i| {
                    if rates[i] > 0. {
                        (caps[i] - expected[i]) / rates[i]
                    } else {
                        f64::INFINITY
                    }
                })
                .collect();
            let dt = times.iter().copied().fold(remaining, f64::min);
            for i in 0..count {
                expected[i] = if times[i] == dt {
                    caps[i]
                } else {
                    expected[i] + dt * rates[i]
                };
            }
            remaining -= dt;
        }
        assert_eq!(remaining, 0.);
        let batch: Vec<_> = forcing
            .iter()
            .enumerate()
            .map(|(r, &v)| input(r, v))
            .collect();
        let mut reversed = model.clone();
        let mut reverse = batch.clone();
        reverse.reverse();
        let report = model
            .add_interval(batch)
            .unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(report, reversed.add_interval(reverse).unwrap());
        for (r, &v) in expected.iter().enumerate() {
            near(region_stock(&model, r), v);
        }
        // Independently reconstruct every level's volume from its raw column.
        for active in &report.snapshot.active {
            let r = model
                .connections()
                .basins()
                .region_nodes()
                .iter()
                .position(|&branch| branch == active.stock.branch)
                .unwrap();
            near(
                active.water_level_meters.unwrap() - heights[r],
                active.stock.volume_cubic_meters,
            );
        }
    }
}

#[test]
fn bounded_long_transit_cycles_and_simultaneous_ties_terminate() {
    let h: Vec<_> = (0..127).map(|i| if i % 2 == 0 { 0. } else { 1. }).collect();
    let edges: Vec<_> = (0..126).map(|i| [i, i + 1]).collect();
    let s = setup(geometry(&h, &edges), 64);
    let mut model = SimultaneousNetwork::new(s.clone()).unwrap();
    let report = model.add_interval(vec![input(0, 127.)]).unwrap();
    assert!(report.events.len() <= 65);
    near(report.snapshot.total_stored_cubic_meters, 127.);
    near(
        report.snapshot.active[0].water_level_meters.unwrap(),
        1. + 63. / 127.,
    );
    let mut tied = SimultaneousNetwork::new(s).unwrap();
    let report = tied
        .add_interval((0..127).step_by(2).map(|r| input(r, 1.)).collect())
        .unwrap();
    assert_eq!(report.events.len(), 1);
    assert_eq!(report.events[0].saturated.len(), 64);
    assert_eq!(report.snapshot.active.len(), 1);
    let cycle = setup(
        geometry(
            &[0., 1., 0., 1., 0., 1.],
            &[[0, 1], [1, 2], [2, 3], [3, 4], [4, 5], [5, 0]],
        ),
        3,
    );
    let mut model = SimultaneousNetwork::new(cycle).unwrap();
    model
        .add_interval(vec![input(0, 1.), input(2, 1.)])
        .unwrap();
    let report = model
        .add_interval(vec![input(0, 0.25), input(2, 0.25)])
        .unwrap();
    assert_eq!(report.events[0].transfers.len(), 2);
    near(region_stock(&model, 4), 0.5);
}

#[test]
fn invalid_inputs_versions_checkpoints_and_precision_fail_atomically() {
    let mut model = SimultaneousNetwork::new(fork()).unwrap();
    for batch in [
        vec![input(0, 1.), input(0, 2.)],
        vec![input(99, 1.)],
        vec![input(0, -1.)],
        vec![input(0, f64::NAN)],
        vec![input(0, f64::INFINITY)],
    ] {
        let before = model.clone();
        assert!(model.add_interval(batch).is_err());
        assert_eq!(model, before);
    }
    let mut bad = model.checkpoint();
    bad.experiment_version = "spill-network-1".into();
    assert!(SimultaneousNetwork::restore(bad).is_err());
    let mut bad = model.checkpoint();
    bad.setup.policy_version = "frontier-weighted-events-1".into();
    assert!(SimultaneousNetwork::restore(bad).is_err());
    let mut bad = model.checkpoint();
    bad.inventory.active.pop();
    assert!(SimultaneousNetwork::restore(bad).is_err());
    let mut bad = model.checkpoint();
    bad.inventory.pulse_count = u64::MAX;
    let mut full = SimultaneousNetwork::restore(bad).unwrap();
    let before = full.clone();
    assert!(full.add_interval(vec![]).is_err());
    assert_eq!(full, before);
    model.add_interval(vec![input(0, 1.)]).unwrap();
    let before = model.clone();
    assert!(model.add_interval(vec![input(0, 1e-30)]).is_err());
    assert_eq!(model, before);
    // The total interval is resolvable, but one concurrently growing stock is
    // not. No other source may commit while that recipient's update fails.
    assert!(
        model
            .add_interval(vec![input(0, 1e-30), input(2, 1.)])
            .is_err()
    );
    assert_eq!(model, before);
    let report = model.add_interval(vec![]).unwrap();
    assert!(report.events.is_empty());
    let mut flat = SimultaneousNetwork::new(setup(geometry(&[0.], &[]), 0)).unwrap();
    assert_eq!(
        flat.add_interval(vec![input(0, 2.)])
            .unwrap()
            .snapshot
            .active[0]
            .water_level_meters,
        Some(2.)
    );
    let mut raw = serde_json::to_value(flat.checkpoint()).unwrap();
    raw["unexpected"] = true.into();
    assert!(serde_json::from_value::<Checkpoint>(raw).is_err());
}

#[test]
fn relabeling_regions_and_reversing_edges_do_not_assign_physical_priority() {
    let s = fork();
    let mut original = SimultaneousNetwork::new(s.clone()).unwrap();
    original
        .add_interval(vec![input(0, 5.), input(2, 1.)])
        .unwrap();
    let permutation = [6, 4, 2, 0, 5, 3, 1];
    let mut g = s.geometry.clone();
    for (old, &new) in permutation.iter().enumerate() {
        g.columns[new] = s.geometry.columns[old].clone();
    }
    g.edges = s
        .geometry
        .edges
        .iter()
        .rev()
        .map(|e| Edge {
            regions: [permutation[e.regions[1]], permutation[e.regions[0]]],
            distance_meters: e.distance_meters,
        })
        .collect();
    let mut relabeled = SimultaneousNetwork::new(setup(g, 4)).unwrap();
    relabeled
        .add_interval(vec![input(permutation[0], 5.), input(permutation[2], 1.)])
        .unwrap();
    for r in [0, 2, 4, 6] {
        near(
            region_stock(&original, r),
            region_stock(&relabeled, permutation[r]),
        );
    }
}
