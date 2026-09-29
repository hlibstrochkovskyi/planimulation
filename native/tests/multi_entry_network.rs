use planimulation_core::{
    Random, Recipe, World,
    nested_reservoir::{Edge, Geometry, Input},
    reservoir::Column,
    spill_junction::Weight,
    spill_network::simultaneous::{
        self, SimultaneousNetwork,
        multi_entry::{self, EntryWeight, MultiEntryNetwork, Setup},
    },
    spill_readiness::Screening,
};

fn geometry(join_sills: bool) -> Geometry {
    let mut edges = vec![[0, 1], [1, 2], [2, 3], [3, 4], [4, 5], [5, 6]];
    if join_sills {
        edges.push([1, 5]);
    }
    Geometry {
        columns: [0., 4., 1., 2., 0.5, 4., 3.]
            .iter()
            .map(|&bed_meters| Column {
                bed_meters,
                area_square_meters: 1.,
            })
            .collect(),
        edges: edges
            .into_iter()
            .map(|regions| Edge {
                regions,
                distance_meters: 1.,
            })
            .collect(),
    }
}
fn setup(g: Geometry) -> Setup {
    let targets = multi_entry::entry_targets(&g).unwrap();
    let count = targets
        .iter()
        .map(|t| t.branch)
        .max()
        .map_or(0, |id| id + 1);
    Setup {
        geometry: g,
        policy_version: multi_entry::POLICY_VERSION.into(),
        weights: (0..count)
            .map(|branch| Weight { branch, weight: 1. })
            .collect(),
        entry_weights: targets
            .into_iter()
            .map(|t| EntryWeight {
                branch: t.branch,
                leaf: t.leaf,
                weight: 1.,
            })
            .collect(),
    }
}
fn input(region: usize, volume_cubic_meters: f64) -> Input {
    Input {
        region,
        volume_cubic_meters,
    }
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-8_f64.max(b.abs() * 1e-10), "{a} != {b}");
}
fn stock(m: &MultiEntryNetwork, region: usize) -> f64 {
    let branch = m.connections().basins().region_nodes()[region];
    m.checkpoint()
        .inventory
        .active
        .iter()
        .find(|s| s.branch == branch)
        .unwrap()
        .volume_cubic_meters
}
fn weighted() -> Setup {
    let mut s = setup(geometry(true));
    // IDs are analysis-local: r4 is leaf 1, r2 is leaf 2, their parent is 3.
    s.entry_weights
        .iter_mut()
        .find(|w| w.branch == 3 && w.leaf == 1)
        .unwrap()
        .weight = 3.;
    s
}
fn compare(a: &MultiEntryNetwork, b: &MultiEntryNetwork) {
    let a = a.snapshot().unwrap();
    let b = b.snapshot().unwrap();
    assert_eq!(a.active.len(), b.active.len());
    for (a, b) in a.active.iter().zip(b.active) {
        assert_eq!(a.stock.branch, b.stock.branch);
        near(a.stock.volume_cubic_meters, b.stock.volume_cubic_meters);
    }
}

#[test]
fn reachable_entries_split_after_branch_share_not_per_contact() {
    let mut m = MultiEntryNetwork::new(weighted()).unwrap();
    m.add_interval(vec![input(0, 4.)]).unwrap();
    let receivers = m.receivers(0).unwrap();
    assert_eq!(
        receivers
            .iter()
            .map(|r| (r.branch, r.entry_leaf))
            .collect::<Vec<_>>(),
        [(3, 1), (3, 2), (4, 4)]
    );
    m.add_interval(vec![input(0, 1.)]).unwrap();
    assert_eq!(stock(&m, 2), 0.125);
    assert_eq!(stock(&m, 4), 0.375);
    assert_eq!(stock(&m, 6), 0.5);
    // Repeat contact to near leaf on the same plateau; it must not gain weight.
    let mut s = weighted();
    s.geometry.columns.push(Column {
        bed_meters: 4.,
        area_square_meters: 1.,
    });
    for regions in [[0, 7], [7, 1], [7, 2]] {
        s.geometry.edges.push(Edge {
            regions,
            distance_meters: 1.,
        });
    }
    let mut repeated = MultiEntryNetwork::new(s).unwrap();
    repeated.add_interval(vec![input(0, 5.)]).unwrap();
    compare(&m, &repeated);
    let old = simultaneous::Setup {
        geometry: geometry(true),
        policy_version: simultaneous::POLICY_VERSION.into(),
        weights: weighted().weights,
    };
    assert!(
        SimultaneousNetwork::new(old)
            .unwrap_err()
            .contains("Alternative entries")
    );
}

#[test]
fn separate_sills_do_not_teleport_input_to_a_remote_internal_entry() {
    let s = setup(geometry(false));
    let mut a = MultiEntryNetwork::new(s.clone()).unwrap();
    a.add_interval(vec![input(0, 4.)]).unwrap();
    assert_eq!(
        a.receivers(0)
            .unwrap()
            .iter()
            .map(|r| r.entry_region)
            .collect::<Vec<_>>(),
        [2]
    );
    a.add_interval(vec![input(0, 0.5)]).unwrap();
    assert_eq!(stock(&a, 2), 0.5);
    assert_eq!(stock(&a, 4), 0.);
    let mut b = MultiEntryNetwork::new(s).unwrap();
    b.add_interval(vec![input(6, 1.)]).unwrap();
    b.add_interval(vec![input(6, 0.5)]).unwrap();
    assert_eq!(stock(&b, 2), 0.);
    assert_eq!(stock(&b, 4), 0.5);
    let mut both = MultiEntryNetwork::new(setup(geometry(false))).unwrap();
    both.add_interval(vec![input(0, 4.), input(6, 1.)]).unwrap();
    let mut reversed = both.clone();
    let report = both
        .add_interval(vec![input(0, 0.5), input(6, 0.5)])
        .unwrap();
    assert_eq!(
        report,
        reversed
            .add_interval(vec![input(6, 0.5), input(0, 0.5)])
            .unwrap()
    );
    assert_eq!(stock(&both, 2), 0.5);
    assert_eq!(stock(&both, 4), 0.5);
}

#[test]
fn inner_thresholds_merge_and_checkpoint_continuation_preserve_budgets() {
    let mut m = MultiEntryNetwork::new(weighted()).unwrap();
    for v in [4., 1., 1., 2., 12.5] {
        let before = serde_json::to_vec(&m.checkpoint()).unwrap();
        let mut restored =
            MultiEntryNetwork::restore(serde_json::from_slice(&before).unwrap()).unwrap();
        let a = m.add_interval(vec![input(0, v)]).unwrap();
        assert_eq!(a, restored.add_interval(vec![input(0, v)]).unwrap());
        assert_eq!(m.checkpoint(), restored.checkpoint());
    }
    assert_eq!(m.snapshot().unwrap().total_stored_cubic_meters, 20.5);
    assert_eq!(m.snapshot().unwrap().active[0].water_level_meters, Some(5.));
    assert!(m.receivers(usize::MAX).is_err());
    assert!(m.receivers(m.connections().basins().root()).is_err());
    let mut full = MultiEntryNetwork::new(weighted()).unwrap();
    full.add_interval(vec![input(0, 8.)]).unwrap();
    let mut partitioned = MultiEntryNetwork::new(weighted()).unwrap();
    for _ in 0..32 {
        partitioned.add_interval(vec![input(0, 0.25)]).unwrap();
    }
    compare(&full, &partitioned);
    // The inner parent is now active; two entry leaves feed the same reservoir.
    assert_eq!(
        full.checkpoint()
            .inventory
            .active
            .iter()
            .find(|s| s.branch == 3)
            .unwrap()
            .volume_cubic_meters,
        3.
    );
}

#[test]
fn seeded_weights_match_independent_two_stage_sharing_and_scaling() {
    let mut rng = Random::stream("multi-entry", "weights");
    let mut rand = || 0.5 + f64::from(rng.next_u32()) / 4294967296.;
    for case in 0..100 {
        let (wb, wd, wn, wf) = (rand(), rand(), rand(), rand());
        let scale = 2_f64.powi(case % 9 - 4);
        let mut s = setup(geometry(true));
        for c in &mut s.geometry.columns {
            c.area_square_meters = scale;
        }
        s.weights[3].weight = wb;
        s.weights[4].weight = wd;
        for w in &mut s.entry_weights {
            if w.branch == 3 {
                w.weight = if w.leaf == 1 { wf } else { wn };
            }
        }
        let mut m = MultiEntryNetwork::new(s.clone()).unwrap();
        m.add_interval(vec![input(0, 4.5 * scale)]).unwrap();
        let inner = 0.5 * scale * wb / (wb + wd);
        near(stock(&m, 2), inner * wn / (wn + wf));
        near(stock(&m, 4), inner * wf / (wn + wf));
        near(stock(&m, 6), 0.5 * scale - inner);
        for w in &mut s.weights {
            w.weight *= 8.;
        }
        for w in &mut s.entry_weights {
            w.weight *= 4.;
        }
        let mut scaled = MultiEntryNetwork::new(s).unwrap();
        scaled.add_interval(vec![input(0, 4.5 * scale)]).unwrap();
        compare(&m, &scaled);
    }
}

#[test]
fn unique_entry_limit_keeps_existing_concurrent_reports_exact() {
    let raw: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/scenarios/simultaneous-network.json"
    ))
    .unwrap();
    let old: simultaneous::Setup = serde_json::from_value(raw["start"]["dry"].clone()).unwrap();
    let mut new = setup(old.geometry.clone());
    new.weights = old.weights.clone();
    let mut m = MultiEntryNetwork::new(new).unwrap();
    let mut reference = SimultaneousNetwork::new(old).unwrap();
    for batch in [
        vec![input(0, 5.), input(2, 1.)],
        vec![input(0, 11.)],
        vec![],
    ] {
        assert_eq!(
            m.add_interval(batch.clone()).unwrap(),
            reference.add_interval(batch).unwrap()
        );
    }
}

#[test]
fn relabeled_geography_keeps_entry_allocation_in_physical_locations() {
    let mut original = MultiEntryNetwork::new(weighted()).unwrap();
    original.add_interval(vec![input(0, 5.)]).unwrap();
    let permutation = [6, 2, 4, 0, 3, 1, 5];
    let source = geometry(true);
    let mut relabeled = source.clone();
    for (old, &new) in permutation.iter().enumerate() {
        relabeled.columns[new] = source.columns[old].clone();
    }
    relabeled.edges = source
        .edges
        .iter()
        .rev()
        .map(|e| Edge {
            regions: [permutation[e.regions[1]], permutation[e.regions[0]]],
            distance_meters: e.distance_meters,
        })
        .collect();
    let mut setup = setup(relabeled);
    let far_leaf = permutation[4];
    let temporary = MultiEntryNetwork::new(setup.clone()).unwrap();
    let basin_leaf = temporary.connections().basins().region_nodes()[far_leaf];
    let parent = temporary.connections().basins().nodes()[basin_leaf]
        .parent
        .unwrap();
    for weight in &mut setup.entry_weights {
        if weight.branch == parent && weight.leaf == basin_leaf {
            weight.weight = 3.;
        }
    }
    let mut model = MultiEntryNetwork::new(setup).unwrap();
    model.add_interval(vec![input(permutation[0], 5.)]).unwrap();
    for region in [0, 2, 4, 6] {
        near(stock(&original, region), stock(&model, permutation[region]));
    }
}

#[test]
fn full_width_and_cycles_remain_bounded() {
    let heights: Vec<_> = (0..127)
        .map(|region| if region % 2 == 0 { 0. } else { 1. })
        .collect();
    let wide_geometry = Geometry {
        columns: heights
            .iter()
            .map(|&bed_meters| Column {
                bed_meters,
                area_square_meters: 1.,
            })
            .collect(),
        edges: (0..126)
            .map(|region| Edge {
                regions: [region, region + 1],
                distance_meters: 1.,
            })
            .collect(),
    };
    let mut model = MultiEntryNetwork::new(setup(wide_geometry)).unwrap();
    let report = model.add_interval(vec![input(0, 127.)]).unwrap();
    assert_eq!(report.snapshot.total_stored_cubic_meters, 127.);
    assert_eq!(report.snapshot.active.len(), 1);
    assert!(report.events.len() <= 65);

    let mut cyclic = geometry(true);
    cyclic.edges.push(Edge {
        regions: [2, 5],
        distance_meters: 1.,
    });
    let mut model = MultiEntryNetwork::new(setup(cyclic)).unwrap();
    model.add_interval(vec![input(0, 4.)]).unwrap();
    let entries = model.receivers(0).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|r| (r.branch, r.entry_leaf))
            .collect::<Vec<_>>(),
        [(3, 1), (3, 2), (4, 4)]
    );
    let report = model.add_interval(vec![input(0, 1.)]).unwrap();
    assert_eq!(report.snapshot.total_stored_cubic_meters, 5.);
}

#[test]
fn invalid_entry_weights_versions_and_updates_are_atomic() {
    let good = weighted();
    for mode in 0..5 {
        let mut s = good.clone();
        match mode {
            0 => {
                s.entry_weights.pop();
            }
            1 => s.entry_weights.reverse(),
            2 => s.entry_weights[0].weight = 0.,
            3 => s.entry_weights[0].leaf = 999,
            _ => s.policy_version = "future".into(),
        };
        assert!(MultiEntryNetwork::new(s).is_err());
    }
    let mut m = MultiEntryNetwork::new(good).unwrap();
    m.add_interval(vec![input(0, 1.)]).unwrap();
    for batch in [
        vec![input(0, -1.)],
        vec![input(99, 1.)],
        vec![input(0, 1e-30)],
        vec![input(0, 1.), input(0, 1.)],
    ] {
        let before = m.clone();
        assert!(m.add_interval(batch).is_err());
        assert_eq!(m, before);
    }
    let mut bad = m.checkpoint();
    bad.experiment_version = simultaneous::EXPERIMENT_VERSION.into();
    assert!(MultiEntryNetwork::restore(bad).is_err());
    let mut bad = m.checkpoint();
    bad.setup.entry_weights.pop();
    assert!(MultiEntryNetwork::restore(bad).is_err());
    let mut bad = m.checkpoint();
    bad.inventory.active.pop();
    assert!(MultiEntryNetwork::restore(bad).is_err());
}

#[test]
fn generated_conflicting_subtrees_are_admitted_without_modifying_the_world() {
    let mut checked = 0;
    for seed in [
        "first-light",
        "readiness-00",
        "readiness-01",
        "readiness-02",
    ] {
        let mut recipe: Recipe =
            serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json"))
                .unwrap();
        recipe.seed = seed.into();
        let world = World::generate(recipe).unwrap();
        let before = planimulation_core::wire::arrays(&world);
        let screen = Screening::build(&world.surface, &world.terrain.elevation).unwrap();
        for conflict in screen.ambiguous_entries {
            let parent = conflict.parent_branch;
            let inside = |mut id: usize| {
                loop {
                    if id == parent {
                        return true;
                    }
                    match world.basins.nodes()[id].parent {
                        Some(p) => id = p,
                        None => return false,
                    }
                }
            };
            let regions: Vec<_> = world
                .basins
                .region_nodes()
                .iter()
                .enumerate()
                .filter(|(_, id)| inside(**id))
                .map(|(r, _)| r)
                .collect();
            if regions.len() > 128 {
                continue;
            }
            let mut map = vec![usize::MAX; world.surface.areas.len()];
            for (local, &global) in regions.iter().enumerate() {
                map[global] = local;
            }
            let mut edges = Vec::new();
            for &a in &regions {
                for k in world.surface.offsets[a] as usize..world.surface.offsets[a + 1] as usize {
                    let b = world.surface.neighbors[k] as usize;
                    if a < b && map[b] != usize::MAX {
                        edges.push(Edge {
                            regions: [map[a], map[b]],
                            distance_meters: world.surface.distances[k],
                        });
                    }
                }
            }
            let g = Geometry {
                columns: regions
                    .iter()
                    .map(|&r| Column {
                        bed_meters: world.terrain.elevation[r],
                        area_square_meters: world.surface.areas[r],
                    })
                    .collect(),
                edges,
            };
            let s = setup(g);
            let old = simultaneous::Setup {
                geometry: s.geometry.clone(),
                policy_version: simultaneous::POLICY_VERSION.into(),
                weights: s.weights.clone(),
            };
            assert!(
                SimultaneousNetwork::new(old)
                    .unwrap_err()
                    .contains("Alternative entries")
            );
            let mut model = MultiEntryNetwork::new(s).unwrap();
            // Small distributed volume exercises real generated areas/beds without
            // claiming external-boundary fidelity or threshold-scale robustness.
            let inputs = regions
                .iter()
                .enumerate()
                .map(|(local, &global)| input(local, world.surface.areas[global] * 0.01))
                .collect();
            model.add_interval(inputs).unwrap();
            checked += 1;
        }
        assert_eq!(before, planimulation_core::wire::arrays(&world));
    }
    assert!(checked >= 4, "only {checked} bounded conflicting subtrees");
    eprintln!("Generated conflicting subtree cases: {checked}");
}
