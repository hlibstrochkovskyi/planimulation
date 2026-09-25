use planimulation_core::{
    Random,
    nested_reservoir::{Checkpoint, Edge, Geometry, Input, NestedReservoir},
    reservoir::Column,
};

fn chain(heights: &[f64], areas: &[f64]) -> Geometry {
    Geometry {
        columns: heights
            .iter()
            .zip(areas)
            .map(|(&bed_meters, &area_square_meters)| Column {
                bed_meters,
                area_square_meters,
            })
            .collect(),
        edges: (1..heights.len())
            .map(|r| Edge {
                regions: [r - 1, r],
                distance_meters: 1.,
            })
            .collect(),
    }
}
fn fixture() -> Geometry {
    chain(&[0., 2., 1., 5., -1.], &[1.; 5])
}
fn pulse(e: &mut NestedReservoir, region: usize, volume_cubic_meters: f64) {
    e.add_input(Input {
        region,
        volume_cubic_meters,
    })
    .unwrap();
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-8_f64.max(b.abs() * 1e-10), "{a} != {b}");
}
fn is_under(e: &NestedReservoir, mut owner: usize, ancestor: usize) -> bool {
    loop {
        if owner == ancestor {
            return true;
        }
        let Some(parent) = e.connections().basins().nodes()[owner].parent else {
            return false;
        };
        owner = parent;
    }
}
// Independent column sums and an ancestor walk, not the prepared storage index
// or its membership matrix. Return physical region depths for comparison.
fn audit(e: &NestedReservoir) -> Vec<f64> {
    let c = e.checkpoint();
    let b = e.connections().basins();
    let s = e.snapshot().unwrap();
    let mut depths = vec![0.; c.geometry.columns.len()];
    let mut total = 0.;
    for active in s.active {
        let mut volume = 0.;
        for (r, column) in c.geometry.columns.iter().enumerate() {
            if is_under(e, b.region_nodes()[r], active.stock.branch) {
                let depth = active
                    .water_level_meters
                    .map_or(0., |h| (h - column.bed_meters).max(0.));
                depths[r] = depth;
                volume += depth * column.area_square_meters;
            }
        }
        near(volume, active.stock.volume_cubic_meters);
        total += volume;
    }
    near(total, c.inventory.input_cubic_meters);
    near(total, s.total_stored_cubic_meters);
    depths
}
fn roundtrip(e: &NestedReservoir) -> NestedReservoir {
    let json = serde_json::to_vec(&e.checkpoint()).unwrap();
    let restored = NestedReservoir::restore(serde_json::from_slice(&json).unwrap()).unwrap();
    assert_eq!(restored, *e);
    restored
}

#[test]
fn outer_spill_enters_the_near_nested_leaf_before_the_far_one() {
    let mut e = NestedReservoir::new(fixture()).unwrap();
    let owners = e.connections().basins().region_nodes().to_vec();
    let route = e.routes()[owners[4]].as_ref().unwrap();
    assert_eq!(route.passage_regions, [4, 3, 2]);
    assert_eq!(route.entry_leaf, owners[2]);
    pulse(&mut e, 4, 6.);
    assert_eq!(audit(&e), [0., 0., 0., 0., 6.]);
    let p = e
        .add_input(Input {
            region: 4,
            volume_cubic_meters: 0.5,
        })
        .unwrap();
    assert_eq!(audit(&e), [0., 0., 0.5, 0., 6.]);
    assert_eq!(p.transfers.len(), 1);
    assert_eq!(p.transfers[0].volume_cubic_meters, 0.5);
    pulse(&mut e, 4, 0.5);
    assert_eq!(audit(&e), [0., 0., 1., 0., 6.]);
    pulse(&mut e, 4, 1.);
    assert_eq!(audit(&e), [1., 0., 1., 0., 6.]);
    pulse(&mut e, 4, 1.);
    assert_eq!(audit(&e), [2., 0., 1., 0., 6.]);
    assert_eq!(e.checkpoint().inventory.active.len(), 2);
    pulse(&mut e, 4, 9.);
    assert_eq!(audit(&e), [5., 3., 4., 0., 6.]);
    assert_eq!(e.checkpoint().inventory.active.len(), 1);
    pulse(&mut e, 4, 5.);
    assert_eq!(audit(&e), [6., 4., 5., 1., 7.]);
    roundtrip(&e);
}

#[test]
fn reverse_spill_and_oversized_pulses_preserve_each_threshold() {
    let mut e = NestedReservoir::new(fixture()).unwrap();
    pulse(&mut e, 0, 2.);
    assert_eq!(audit(&e), [2., 0., 0., 0., 0.]);
    pulse(&mut e, 0, 1.);
    assert_eq!(audit(&e), [2., 0., 1., 0., 0.]);
    pulse(&mut e, 0, 9.);
    assert_eq!(audit(&e), [5., 3., 4., 0., 0.]);
    pulse(&mut e, 0, 2.);
    assert_eq!(audit(&e), [5., 3., 4., 0., 2.]);
    pulse(&mut e, 0, 9.);
    assert_eq!(audit(&e), [6., 4., 5., 1., 7.]);
    for entry in 0..5 {
        let mut single = NestedReservoir::new(fixture()).unwrap();
        pulse(&mut single, entry, 23.);
        assert_eq!(single.snapshot().unwrap(), e.snapshot().unwrap());
    }
}

#[test]
fn checkpoints_resume_before_at_and_after_nested_merges() {
    let mut e = NestedReservoir::new(fixture()).unwrap();
    for volume in [0., 6., 1., 1., 1., 9., 5., 0.1, 0.12345678901234568] {
        let mut restored = roundtrip(&e);
        let input = Input {
            region: 4,
            volume_cubic_meters: volume,
        };
        assert_eq!(
            e.add_input(input.clone()).unwrap(),
            restored.add_input(input).unwrap()
        );
        assert_eq!(e.checkpoint(), restored.checkpoint());
        audit(&e);
    }
}

#[test]
fn malformed_frontiers_ledgers_and_unrepresentable_pulses_are_atomic() {
    let mut e = NestedReservoir::new(fixture()).unwrap();
    pulse(&mut e, 4, 7.);
    let good = e.checkpoint();
    let mut bads: Vec<Checkpoint> = vec![];
    let mut bad = good.clone();
    bad.experiment_version = "future".into();
    bads.push(bad);
    let mut bad = good.clone();
    bad.inventory.active.pop();
    bads.push(bad);
    let mut bad = good.clone();
    bad.inventory.active.push(bad.inventory.active[0].clone());
    bads.push(bad);
    let mut bad = good.clone();
    bad.inventory.active.reverse();
    bads.push(bad);
    let mut bad = good.clone();
    bad.inventory.active[0].branch = 999;
    bads.push(bad);
    let mut bad = good.clone();
    bad.inventory.input_cubic_meters += 1.;
    bads.push(bad);
    let mut bad = good.clone();
    bad.inventory.pulse_count = 0;
    bads.push(bad);
    let mut bad = good.clone();
    bad.inventory.active[0].volume_cubic_meters = f64::NAN;
    bads.push(bad);
    let mut bad = good.clone();
    bad.inventory.active[0].volume_cubic_meters = -1.;
    bads.push(bad);
    // Replace two separate, not-yet-filled children by a premature common lake.
    let mut bad = good.clone();
    let b = e.connections().basins().clone();
    let parent = b.nodes()[b.region_nodes()[0]].parent.unwrap();
    bad.inventory
        .active
        .retain(|s| !is_under(&e, s.branch, parent));
    bad.inventory
        .active
        .push(planimulation_core::nested_reservoir::Stock {
            branch: parent,
            volume_cubic_meters: 1.,
        });
    bads.push(bad);
    // Both the parent and descendants are present, with an otherwise matching ledger.
    let mut overlap = good.clone();
    overlap
        .inventory
        .active
        .push(planimulation_core::nested_reservoir::Stock {
            branch: parent,
            volume_cubic_meters: 3.,
        });
    overlap.inventory.input_cubic_meters += 3.;
    bads.push(overlap);
    for bad in bads {
        assert!(NestedReservoir::restore(bad).is_err());
    }
    for input in [
        Input {
            region: 999,
            volume_cubic_meters: 1.,
        },
        Input {
            region: 0,
            volume_cubic_meters: -1.,
        },
        Input {
            region: 0,
            volume_cubic_meters: f64::NAN,
        },
        Input {
            region: 0,
            volume_cubic_meters: f64::INFINITY,
        },
        Input {
            region: 0,
            volume_cubic_meters: f64::MIN_POSITIVE,
        },
    ] {
        assert!(e.add_input(input).is_err());
        assert_eq!(e.checkpoint(), good);
    }
    let mut max_count = good.clone();
    max_count.inventory.pulse_count = u64::MAX;
    let mut e = NestedReservoir::restore(max_count.clone()).unwrap();
    assert!(
        e.add_input(Input {
            region: 0,
            volume_cubic_meters: 0.
        })
        .is_err()
    );
    assert_eq!(e.checkpoint(), max_count);
    let mut huge = NestedReservoir::new(fixture()).unwrap();
    pulse(&mut huge, 0, f64::MAX);
    let before = huge.checkpoint();
    assert!(
        huge.add_input(Input {
            region: 0,
            volume_cubic_meters: f64::MAX
        })
        .is_err()
    );
    assert_eq!(huge.checkpoint(), before);
    // A full sibling pair saved separately is noncanonical, even with a valid budget.
    let mut full = NestedReservoir::new(fixture()).unwrap().checkpoint();
    for s in &mut full.inventory.active {
        s.volume_cubic_meters = b.nodes()[s.branch].capacity_cubic_meters.unwrap();
    }
    full.inventory.input_cubic_meters = full
        .inventory
        .active
        .iter()
        .map(|s| s.volume_cubic_meters)
        .sum();
    full.inventory.pulse_count = 1;
    assert!(NestedReservoir::restore(full).is_err());
}

#[test]
fn unsupported_ambiguities_and_invalid_graphs_are_rejected_not_arbitrarily_routed() {
    assert!(NestedReservoir::new(chain(&[0., 4., 1., 4., 2.], &[1.; 5])).is_err());
    let mut alternative = chain(&[0., 4., 1., 4.], &[1.; 4]);
    alternative.edges.push(Edge {
        regions: [3, 0],
        distance_meters: 1.,
    });
    assert!(NestedReservoir::new(alternative).is_err());
    // One connecting plateau, but two different lower contacts on one side.
    let mut contacts = chain(&[0., 4., 1., 4.], &[1.; 4]);
    contacts.edges.push(Edge {
        regions: [1, 3],
        distance_meters: 1.,
    });
    assert!(NestedReservoir::new(contacts).is_err());
    let mut bads = vec![chain(&[], &[]), chain(&[0.; 129], &[1.; 129])];
    let mut g = fixture();
    g.edges.pop();
    bads.push(g);
    let mut g = fixture();
    g.edges.push(g.edges[0].clone());
    bads.push(g);
    let mut g = fixture();
    g.edges[0].regions = [0, 0];
    bads.push(g);
    let mut g = fixture();
    g.edges[0].regions = [0, 999];
    bads.push(g);
    let mut g = fixture();
    g.edges[0].distance_meters = 0.;
    bads.push(g);
    let mut g = fixture();
    g.columns[0].area_square_meters = -1.;
    bads.push(g);
    let mut g = fixture();
    g.columns[0].bed_meters = f64::NAN;
    bads.push(g);
    let mut g = fixture();
    g.edges[0].distance_meters = f64::from_bits(1);
    bads.push(g);
    for g in bads {
        assert!(NestedReservoir::new(g).is_err());
    }
}

// Independent weighted three-bowl oracle for input from the outer right bowl.
// Direct prism sums and bisection, no hierarchy storage/fill implementation.
fn inverse(columns: &[Column], ids: &[usize], volume: f64) -> f64 {
    let mut lo = ids
        .iter()
        .map(|&i| columns[i].bed_meters)
        .fold(f64::INFINITY, f64::min);
    let mut hi = ids
        .iter()
        .map(|&i| columns[i].bed_meters)
        .fold(f64::NEG_INFINITY, f64::max)
        + volume
            / ids
                .iter()
                .map(|&i| columns[i].area_square_meters)
                .sum::<f64>()
        + 1.;
    for _ in 0..100 {
        let mid = (lo + hi) / 2.;
        let v: f64 = ids
            .iter()
            .map(|&i| columns[i].area_square_meters * (mid - columns[i].bed_meters).max(0.))
            .sum();
        if v < volume {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.
}
fn expected(g: &Geometry, total: f64) -> Vec<f64> {
    let c = &g.columns;
    let inner = c[1].bed_meters;
    let outer = c[3].bed_meters;
    let cap = |id: usize, h: f64| c[id].area_square_meters * (h - c[id].bed_meters);
    let a = cap(0, inner);
    let b = cap(2, inner);
    let right = cap(4, outer);
    let left: f64 = [0, 1, 2].iter().map(|&id| cap(id, outer)).sum();
    let mut result = vec![0.; 5];
    if total <= right {
        result[4] = total / c[4].area_square_meters;
    } else if total <= right + b {
        result[4] = outer - c[4].bed_meters;
        result[2] = (total - right) / c[2].area_square_meters;
    } else if total <= right + b + a {
        result[4] = outer - c[4].bed_meters;
        result[2] = inner - c[2].bed_meters;
        result[0] = (total - right - b) / c[0].area_square_meters;
    } else if total <= right + left {
        result[4] = outer - c[4].bed_meters;
        let level = inverse(c, &[0, 1, 2], total - right);
        for id in 0..3 {
            result[id] = (level - c[id].bed_meters).max(0.);
        }
    } else {
        let level = inverse(c, &[0, 1, 2, 3, 4], total);
        for id in 0..5 {
            result[id] = (level - c[id].bed_meters).max(0.);
        }
    }
    result
}

#[test]
fn seeded_weighted_nested_filling_matches_independent_piecewise_oracle() {
    let mut rng = Random::stream("nested-weighted", "tests");
    let mut random = || f64::from(rng.next_u32()) / 4294967296.;
    for _ in 0..40 {
        let datum = random() * 100. - 50.;
        let areas: Vec<_> = (0..5).map(|_| 0.5 + random() * 5.).collect();
        let heights = [datum, datum + 3., datum + 1., datum + 8., datum - 2.];
        let g = chain(&heights, &areas);
        let mut e = NestedReservoir::new(g.clone()).unwrap();
        for _ in 0..100 {
            pulse(&mut e, 4, 0.1 + random() * 5.);
            let want = expected(&g, e.checkpoint().inventory.input_cubic_meters);
            for (a, b) in audit(&e).iter().zip(want) {
                near(*a, b);
            }
        }
        roundtrip(&e);
    }
}

#[test]
fn deeper_hierarchies_pulse_partitioning_and_relabeling_preserve_physical_results() {
    let g = chain(
        &[0., 2., 1., 4., 0., 6., -1.],
        &[2., 1., 3., 2., 4., 1., 5.],
    );
    let mut full = NestedReservoir::new(g.clone()).unwrap();
    pulse(&mut full, 6, 200.);
    let mut split = NestedReservoir::new(g.clone()).unwrap();
    for _ in 0..200 {
        pulse(&mut split, 6, 1.);
        audit(&split);
    }
    assert_eq!(audit(&full), audit(&split));
    // Reverse region IDs and edge order; physical routing is unique here.
    let mut reversed = g.clone();
    reversed.columns.reverse();
    for edge in &mut reversed.edges {
        edge.regions = edge.regions.map(|r| 6 - r);
    }
    reversed.edges.reverse();
    let mut e = NestedReservoir::new(reversed).unwrap();
    pulse(&mut e, 0, 200.);
    let mut depths = audit(&e);
    depths.reverse();
    assert_eq!(depths, audit(&full));
    let heights: Vec<_> = (0..127)
        .map(|r| if r % 2 == 0 { 0. } else { (r / 2 + 1) as f64 })
        .collect();
    let mut deep = NestedReservoir::new(chain(&heights, &[1.; 127])).unwrap();
    pulse(&mut deep, 126, 10000.);
    assert_eq!(deep.checkpoint().inventory.active.len(), 1);
    audit(&deep);
    roundtrip(&deep);
}

#[test]
fn flat_closed_storage_and_dry_bed_descent_are_explicit() {
    for g in [
        chain(&[3.], &[2.]),
        chain(&[3.; 128], &[1.; 128]),
        chain(&[3., 2., 0.], &[1.; 3]),
    ] {
        let mut e = NestedReservoir::new(g).unwrap();
        assert!(e.snapshot().unwrap().active[0].water_level_meters.is_none());
        pulse(&mut e, 0, 10.);
        audit(&e);
        roundtrip(&e);
    }
    // Outer sill meets a bank in the receiving subtree, not its minimum cell.
    let mut e = NestedReservoir::new(chain(&[0., 2., 1., 3., 5., -1.], &[1.; 6])).unwrap();
    let owner = e.connections().basins().region_nodes()[5];
    assert_eq!(e.routes()[owner].as_ref().unwrap().descent_regions, [3, 2]);
    pulse(&mut e, 5, 6.5);
    assert_eq!(audit(&e), [0., 0., 0.5, 0., 0., 6.]);
}

#[test]
fn mixed_entries_scaling_and_partial_relabeling_preserve_stock_and_continuation() {
    let mut independent = NestedReservoir::new(fixture()).unwrap();
    pulse(&mut independent, 0, 0.5);
    pulse(&mut independent, 2, 0.25);
    pulse(&mut independent, 4, 1.);
    assert_eq!(audit(&independent), [0.5, 0., 0.25, 0., 1.]);
    pulse(&mut independent, 4, 5.);
    pulse(&mut independent, 4, 0.75);
    assert_eq!(audit(&independent), [0.5, 0., 1., 0., 6.]);
    pulse(&mut independent, 4, 0.5);
    assert_eq!(audit(&independent), [1., 0., 1., 0., 6.]);

    let g = chain(
        &[0., 2., 1., 4., 0., 6., -1.],
        &[2., 1., 3., 2., 4., 1., 5.],
    );
    let mut reversed = g.clone();
    reversed.columns.reverse();
    for edge in &mut reversed.edges {
        edge.regions = edge.regions.map(|r| 6 - r);
    }
    reversed.edges.reverse();
    let mut scaled = g.clone();
    for column in &mut scaled.columns {
        column.bed_meters += 1000.;
        column.area_square_meters *= 3.;
    }
    let mut original = NestedReservoir::new(g).unwrap();
    let mut mirror = NestedReservoir::new(reversed).unwrap();
    let mut scaled = NestedReservoir::new(scaled).unwrap();
    let mut rng = Random::stream("nested-mixed", "tests");
    for index in 0..500 {
        // Minima only: relabeling equal-slope dry ridge ties is not an invariant.
        let region = (rng.next_u32() as usize % 4) * 2;
        let volume = 0.125 * (1 + rng.next_u32() % 8) as f64;
        pulse(&mut original, region, volume);
        pulse(&mut mirror, 6 - region, volume);
        pulse(&mut scaled, region, volume * 3.);
        let depth = audit(&original);
        let mut mirrored = audit(&mirror);
        mirrored.reverse();
        for ((a, b), c) in depth.iter().zip(mirrored).zip(audit(&scaled)) {
            near(*a, b);
            near(*a, c);
        }
        if index % 11 == 0 {
            original = roundtrip(&original);
        }
    }
}
