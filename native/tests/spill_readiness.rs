use planimulation_core::{
    Random, Recipe, Surface, World,
    drainage::Drainage,
    nested_reservoir::{Edge, Geometry},
    reservoir::Column,
    spill_connections::SpillConnections,
    spill_junction::Weight,
    spill_network::simultaneous::{POLICY_VERSION, Setup, SimultaneousNetwork},
    spill_readiness::Screening,
    water::WaterSettings,
    wire,
};
use std::collections::BTreeSet;

fn graph(n: usize, edges: &[(usize, usize)]) -> Surface {
    let mut s = Surface {
        centers: vec![],
        faces: vec![],
        offsets: vec![0],
        neighbors: vec![],
        distances: vec![],
        areas: vec![1.; n],
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    for i in 0..n {
        let mut neighbors: Vec<_> = edges
            .iter()
            .filter_map(|&(a, b)| {
                if a == i {
                    Some(b as u32)
                } else if b == i {
                    Some(a as u32)
                } else {
                    None
                }
            })
            .collect();
        neighbors.sort_unstable();
        s.neighbors.extend(neighbors);
        s.offsets.push(s.neighbors.len() as u32);
    }
    s.distances = vec![1.; s.neighbors.len()];
    s
}
fn chain(n: usize) -> Surface {
    graph(n, &(1..n).map(|i| (i - 1, i)).collect::<Vec<_>>())
}
fn reference(s: &Surface, heights: &[f64], screen: &Screening) {
    let connections = SpillConnections::build(s, heights).unwrap();
    let basins = connections.basins();
    let d = Drainage::build(s, heights, &vec![0; heights.len()]);
    let mut leaves = vec![BTreeSet::new(); basins.nodes().len()];
    for p in connections.plateaus() {
        if p.contacts
            .iter()
            .map(|c| c.child_branch)
            .collect::<BTreeSet<_>>()
            .len()
            < 2
        {
            continue;
        }
        for c in &p.contacts {
            let mut region = c.edge[1];
            let mut length = 0;
            while region != d.receivers[region as usize] {
                region = d.receivers[region as usize];
                length += 1;
                assert!(length < heights.len());
            }
            leaves[c.child_branch].insert(basins.region_nodes()[region as usize]);
        }
    }
    assert_eq!(
        screen
            .ambiguous_entries
            .iter()
            .map(|w| w.branch)
            .collect::<Vec<_>>(),
        leaves
            .iter()
            .enumerate()
            .filter(|(_, set)| set.len() > 1)
            .map(|(id, _)| id)
            .collect::<Vec<_>>()
    );
    for conflict in &screen.ambiguous_entries {
        assert_ne!(
            conflict.first.terminal_leaf,
            conflict.different.terminal_leaf
        );
        for w in [&conflict.first, &conflict.different] {
            let p = &connections.plateaus()[w.plateau];
            assert_eq!(p.parent_branch, conflict.parent_branch);
            assert!(
                p.contacts
                    .iter()
                    .any(|c| c.child_branch == conflict.branch && c.edge == w.edge)
            );
            assert_eq!(w.terminal_region, d.outlets[w.edge[1] as usize]);
            assert_eq!(
                w.terminal_leaf,
                basins.region_nodes()[w.terminal_region as usize]
            );
            let mut branch = w.terminal_leaf;
            while branch != conflict.branch {
                branch = basins.nodes()[branch].parent.unwrap();
            }
        }
    }
    // Independent ancestor walks count the curve columns without production's
    // bottom-up subtree totals or Euler intervals.
    let mut references = 0;
    let mut max_depth = 0;
    for &owner in basins.region_nodes() {
        let mut id = owner;
        let mut depth = 0;
        references += 1;
        while let Some(parent) = basins.nodes()[id].parent {
            id = parent;
            depth += 1;
            references += 1;
        }
        max_depth = max_depth.max(depth);
    }
    assert_eq!(screen.laboratory_curve_column_references, references);
    assert_eq!(screen.maximum_branch_depth, max_depth);
}

#[test]
fn nested_conflict_reports_both_real_entry_paths() {
    let heights = [0., 4., 1., 2., 0.5, 4., 3.];
    let s = chain(7);
    let report = Screening::build(&s, &heights).unwrap();
    assert!(report.within_laboratory_region_limit);
    assert!(!report.unambiguous_nested_entries);
    assert_eq!(report.ambiguous_entries.len(), 1);
    let conflict = &report.ambiguous_entries[0];
    assert_eq!(conflict.first.edge, [1, 2]);
    assert_eq!(conflict.different.edge, [5, 4]);
    assert_eq!(conflict.first.terminal_region, 2);
    assert_eq!(conflict.different.terminal_region, 4);
    reference(&s, &heights, &report);
}

#[test]
fn dead_end_contacts_do_not_create_a_false_entry_conflict() {
    // The outer dead end touches the far inner leaf; only the near leaf is a
    // receiving entry from another outer sibling.
    let h = [0., 2., 1., 5., -1., 5.];
    let s = graph(6, &[(0, 1), (1, 2), (2, 3), (3, 4), (0, 5)]);
    let a = Screening::build(&s, &h).unwrap();
    assert!(a.unambiguous_nested_entries);
    assert_eq!(a.dead_end_plateau_count, 1);
    assert_eq!(a.connecting_plateau_count, 2);
    reference(&s, &h, &a);
    let flat = Screening::build(&chain(3), &[2.; 3]).unwrap();
    assert_eq!(flat.branch_count, 1);
    assert_eq!(flat.maximum_branch_depth, 0);
    assert_eq!(flat.checked_contact_count, 0);
    assert!(flat.unambiguous_nested_entries);
    assert_eq!(flat.laboratory_curve_column_references, 3);
}

#[test]
fn seeded_small_surfaces_match_actual_solver_admission_and_independent_witnesses() {
    for case in 0..60 {
        let mut s = Surface::build(1, 1.);
        s.areas.fill(1.);
        let mut rng = Random::stream(&format!("screening-{case}"), "heights");
        let h: Vec<_> = (0..s.areas.len())
            .map(|_| (rng.next_u32() % 9) as f64)
            .collect();
        let screen = Screening::build(&s, &h).unwrap();
        reference(&s, &h, &screen);
        let mut edges = Vec::new();
        for i in 0..h.len() {
            for k in s.offsets[i] as usize..s.offsets[i + 1] as usize {
                let j = s.neighbors[k] as usize;
                if i < j {
                    edges.push(Edge {
                        regions: [i, j],
                        distance_meters: s.distances[k],
                    });
                }
            }
        }
        let model = SimultaneousNetwork::new(Setup {
            policy_version: POLICY_VERSION.into(),
            geometry: Geometry {
                columns: h
                    .iter()
                    .map(|&bed_meters| Column {
                        bed_meters,
                        area_square_meters: 1.,
                    })
                    .collect(),
                edges,
            },
            weights: (0..screen.branch_count - 1)
                .map(|branch| Weight { branch, weight: 1. })
                .collect(),
        });
        assert_eq!(
            model.is_ok(),
            screen.unambiguous_nested_entries,
            "case {case}: {model:?}"
        );
        if let Err(message) = model {
            assert!(message.contains("Alternative entries"));
        }
        let mut reverse = s;
        for range in reverse.offsets.windows(2) {
            reverse.neighbors[range[0] as usize..range[1] as usize].reverse();
            reverse.distances[range[0] as usize..range[1] as usize].reverse();
        }
        assert_eq!(screen, Screening::build(&reverse, &h).unwrap());
    }
}

#[test]
fn size_and_depth_are_screened_without_building_the_quadratic_solver() {
    // Deep repeated mergers exercise iterative analysis above laboratory size.
    let n = 2001;
    let s = chain(n);
    let h: Vec<_> = (0..n)
        .map(|i| if i % 2 == 0 { 0. } else { (i / 2 + 1) as f64 })
        .collect();
    let a = Screening::build(&s, &h).unwrap();
    assert!(!a.within_laboratory_region_limit);
    assert_eq!(a.laboratory_region_limit, 128);
    assert_eq!(a.maximum_branch_depth, 1000);
    assert_eq!(a.branch_count, 2001);
    assert_eq!(a.laboratory_membership_matrix_entries, 2001 * 2001);
    assert!(a.unambiguous_nested_entries);
    reference(&s, &h, &a);
    let a = Screening::build(&chain(129), &[0.; 129]).unwrap();
    assert!(!a.within_laboratory_region_limit);
    assert!(a.unambiguous_nested_entries);
}

#[test]
fn generated_worlds_are_unchanged_and_water_settings_do_not_affect_dry_screening() {
    for subdivision in [0, 2, 5, 6] {
        let mut recipe: Recipe =
            serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json"))
                .unwrap();
        recipe.subdivision = subdivision;
        let world = World::generate(recipe.clone()).unwrap();
        let bytes = wire::arrays(&world);
        let a = Screening::build(&world.surface, &world.terrain.elevation).unwrap();
        assert_eq!(a.region_count, world.surface.areas.len());
        assert_eq!(a.branch_count, world.basins.nodes().len());
        assert_eq!(bytes, wire::arrays(&world));
        recipe.water = WaterSettings::Coverage { fraction: 0. };
        let mut dry = World::generate(recipe).unwrap();
        dry.advance(1).unwrap();
        assert_eq!(
            a,
            Screening::build(&dry.surface, &dry.terrain.elevation).unwrap()
        );
    }
}

#[test]
fn invalid_fields_fail_without_treating_them_as_a_supported_screen() {
    let s = chain(3);
    for h in [vec![], vec![0.; 2], vec![0., f64::NAN, 1.]] {
        assert!(Screening::build(&s, &h).is_err());
    }
    let mut bad = chain(3);
    bad.distances.pop();
    assert!(Screening::build(&bad, &[0.; 3]).is_err());
    let mut bad = s;
    bad.distances[0] = 0.;
    assert!(Screening::build(&bad, &[0.; 3]).is_err());
    assert!(Screening::build(&graph(2, &[]), &[0.; 2]).is_err());
}
