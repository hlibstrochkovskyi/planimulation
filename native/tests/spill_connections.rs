use planimulation_core::{Random, Surface, basins::Basins, spill_connections::SpillConnections};
use std::collections::BTreeSet;

fn graph(n: usize, edges: &[(usize, usize)]) -> Surface {
    let mut lists = vec![Vec::new(); n];
    for &(a, b) in edges {
        lists[a].push(b as u32);
        lists[b].push(a as u32);
    }
    let mut s = Surface {
        centers: vec![[1., 0., 0.]; n],
        faces: vec![],
        offsets: vec![0],
        neighbors: vec![],
        distances: vec![],
        areas: vec![1.; n],
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    for mut list in lists {
        list.sort_unstable();
        s.neighbors.extend(list);
        s.offsets.push(s.neighbors.len() as u32);
    }
    s.distances = vec![1.; s.neighbors.len()];
    s
}
fn chain(n: usize) -> Surface {
    graph(n, &(1..n).map(|i| (i - 1, i)).collect::<Vec<_>>())
}
fn adjacent(s: &Surface, id: usize) -> &[u32] {
    &s.neighbors[s.offsets[id] as usize..s.offsets[id + 1] as usize]
}
fn child_of(b: &Basins, mut node: usize, parent: usize) -> Option<usize> {
    loop {
        let p = b.nodes()[node].parent?;
        if p == parent {
            return Some(node);
        }
        node = p;
    }
}

// Independent raw-height flood from every edge of the source's lower component.
// It traverses only sill-height cells, never a different lower component.
fn reference_receivers(s: &Surface, h: &[f64], b: &Basins, source: usize) -> BTreeSet<usize> {
    let Some(parent) = b.nodes()[source].parent else {
        return BTreeSet::new();
    };
    let level = b.nodes()[parent].birth_level_meters;
    let mut seen = vec![false; h.len()];
    let mut queue = Vec::new();
    for i in 0..h.len() {
        if h[i] >= level || child_of(b, b.region_nodes()[i], parent) != Some(source) {
            continue;
        }
        for &j in adjacent(s, i) {
            if h[j as usize] == level && !seen[j as usize] {
                seen[j as usize] = true;
                queue.push(j as usize);
            }
        }
    }
    let mut found = BTreeSet::new();
    let mut cursor = 0;
    while cursor < queue.len() {
        let i = queue[cursor];
        cursor += 1;
        for &j in adjacent(s, i) {
            let j = j as usize;
            if h[j] < level {
                let child = child_of(b, b.region_nodes()[j], parent).unwrap();
                if child != source {
                    found.insert(child);
                }
            } else if h[j] == level && !seen[j] {
                seen[j] = true;
                queue.push(j);
            }
        }
    }
    found
}

fn audit(s: &Surface, h: &[f64], a: &SpillConnections) {
    let b = a.basins();
    for source in 0..b.nodes().len() {
        let receivers = a.receivers(source).unwrap();
        assert_eq!(
            receivers.iter().map(|r| r.branch).collect::<BTreeSet<_>>(),
            reference_receivers(s, h, b, source)
        );
        for r in receivers {
            assert_eq!(b.nodes()[source].parent, b.nodes()[r.branch].parent);
            for plateau in r.plateaus {
                let p = &a.plateaus()[plateau];
                let path = a.passage(source, r.branch, plateau).unwrap().regions;
                assert!(path.len() >= 3);
                assert_eq!(path.iter().collect::<BTreeSet<_>>().len(), path.len());
                for pair in path.windows(2) {
                    assert!(adjacent(s, pair[0] as usize).contains(&pair[1]));
                }
                assert!(
                    h[path[0] as usize] < p.level_meters
                        && h[*path.last().unwrap() as usize] < p.level_meters
                );
                assert_eq!(
                    child_of(b, b.region_nodes()[path[0] as usize], p.parent_branch),
                    Some(source)
                );
                assert_eq!(
                    child_of(
                        b,
                        b.region_nodes()[*path.last().unwrap() as usize],
                        p.parent_branch
                    ),
                    Some(r.branch)
                );
                for &i in &path[1..path.len() - 1] {
                    assert_eq!(h[i as usize], p.level_meters);
                    assert!(p.regions.contains(&i));
                }
            }
        }
    }
    // The stored representation stays linear; no all-pairs sibling clique.
    assert!(a.plateaus().iter().map(|p| p.contacts.len()).sum::<usize>() <= s.neighbors.len());
    let mut reversed = Surface {
        centers: s.centers.clone(),
        faces: s.faces.clone(),
        offsets: s.offsets.clone(),
        neighbors: s.neighbors.clone(),
        distances: s.distances.clone(),
        areas: s.areas.clone(),
        boundary_offsets: s.boundary_offsets.clone(),
        boundaries: s.boundaries.clone(),
    };
    for pair in reversed.offsets.windows(2) {
        reversed.neighbors[pair[0] as usize..pair[1] as usize].reverse();
    }
    assert_eq!(*a, SpillConnections::build(&reversed, h).unwrap());
}

#[test]
fn three_way_chain_cannot_skip_an_unfilled_middle_child() {
    let s = chain(5);
    let h = [0., 4., 1., 4., 2.];
    let a = SpillConnections::build(&s, &h).unwrap();
    let owners = a.basins().region_nodes();
    let (left, middle, right) = (owners[0], owners[2], owners[4]);
    assert_eq!(a.basins().nodes()[a.basins().root()].children.len(), 3);
    assert_eq!(a.plateaus().len(), 2);
    assert_eq!(
        a.receivers(left)
            .unwrap()
            .iter()
            .map(|r| r.branch)
            .collect::<Vec<_>>(),
        [middle]
    );
    assert_eq!(
        a.receivers(middle)
            .unwrap()
            .iter()
            .map(|r| r.branch)
            .collect::<Vec<_>>(),
        [left, right]
    );
    assert!(a.passage(left, right, 0).is_err());
    assert!(a.passage(left, right, 1).is_err());
    assert_eq!(a.passage(left, middle, 0).unwrap().regions, [0, 1, 2]);
    audit(&s, &h, &a);
}

#[test]
fn one_plateau_exposes_multiple_receivers_without_choosing_one() {
    let s = graph(4, &[(0, 1), (0, 2), (0, 3)]);
    let h = [4., 0., 1., 2.];
    let a = SpillConnections::build(&s, &h).unwrap();
    let source = a.basins().region_nodes()[1];
    assert_eq!(a.plateaus().len(), 1);
    assert_eq!(a.receivers(source).unwrap().len(), 2);
    assert_eq!(
        a.passage(source, a.basins().region_nodes()[3], 0)
            .unwrap()
            .regions,
        [1, 0, 3]
    );
    audit(&s, &h, &a);
}

#[test]
fn nested_receiver_keeps_the_actual_entry_region_instead_of_choosing_a_leaf() {
    let s = chain(5);
    let h = [0., 2., 1., 5., -1.];
    let a = SpillConnections::build(&s, &h).unwrap();
    let b = a.basins();
    let source = b.region_nodes()[4];
    let receiving_parent = b.nodes()[b.region_nodes()[2]].parent.unwrap();
    let r = a.receivers(source).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].branch, receiving_parent);
    let path = a
        .passage(source, r[0].branch, r[0].plateaus[0])
        .unwrap()
        .regions;
    assert_eq!(path, [4, 3, 2]);
    assert_ne!(b.region_nodes()[2], b.region_nodes()[0]);
    audit(&s, &h, &a);
}

#[test]
fn multiple_sills_and_dead_end_contacts_are_retained_without_false_passages() {
    let s = graph(4, &[(0, 1), (1, 2), (2, 3), (3, 0)]);
    let h = [0., 4., 1., 4.];
    let a = SpillConnections::build(&s, &h).unwrap();
    let source = a.basins().region_nodes()[0];
    let r = a.receivers(source).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].plateaus, [0, 1]);
    assert_eq!(
        a.passage(source, r[0].branch, 0).unwrap().regions,
        [0, 1, 2]
    );
    assert_eq!(
        a.passage(source, r[0].branch, 1).unwrap().regions,
        [0, 3, 2]
    );
    audit(&s, &h, &a);
    let s = graph(5, &[(0, 2), (1, 2), (1, 3), (4, 3)]);
    let h = [4., 4., 0., 1., 4.];
    let a = SpillConnections::build(&s, &h).unwrap();
    let source = a.basins().region_nodes()[2];
    assert_eq!(a.basins().nodes()[source].spill_edge, Some([0, 2]));
    let r = a.receivers(source).unwrap();
    assert_eq!(r[0].plateaus, [1]);
    assert!(a.passage(source, r[0].branch, 0).is_err());
    assert_eq!(
        a.passage(source, r[0].branch, 1).unwrap().regions,
        [2, 1, 3]
    );
    audit(&s, &h, &a);
}

#[test]
fn plateau_paths_have_stable_shortest_hop_ties_and_preserve_all_contact_edges() {
    let s = graph(6, &[(0, 1), (1, 2), (1, 3), (2, 4), (3, 4), (4, 5)]);
    let h = [0., 3., 3., 3., 3., 1.];
    let a = SpillConnections::build(&s, &h).unwrap();
    let owners = a.basins().region_nodes();
    assert_eq!(a.plateaus()[0].regions, [1, 2, 3, 4]);
    assert_eq!(
        a.passage(owners[0], owners[5], 0).unwrap().regions,
        [0, 1, 2, 4, 5]
    );
    audit(&s, &h, &a);
    let s = graph(4, &[(0, 1), (0, 2), (1, 2), (1, 3), (2, 3)]);
    let h = [0., 3., 3., 1.];
    let a = SpillConnections::build(&s, &h).unwrap();
    assert_eq!(a.plateaus()[0].contacts.len(), 4);
    audit(&s, &h, &a);
}

#[test]
fn flat_closed_world_and_invalid_queries_do_not_invent_outlets() {
    for h in [[0.; 5], [0., 1., 2., 3., 4.], [4., 1., 0., 1., 4.]] {
        let a = SpillConnections::build(&chain(5), &h).unwrap();
        assert!(a.plateaus().is_empty());
        assert!(a.receivers(a.basins().root()).unwrap().is_empty());
        assert!(a.receivers(999).is_err());
        assert!(a.passage(0, 1, 0).is_err());
    }
    let a = SpillConnections::build(&chain(3), &[0., 3., 1.]).unwrap();
    assert!(a.passage(0, 0, 0).is_err());
    assert!(a.passage(999, 1, 0).is_err());
    assert!(a.passage(0, 999, 0).is_err());
    assert!(SpillConnections::build(&chain(3), &[0., f64::NAN, 1.]).is_err());
}

#[test]
fn seeded_spheres_match_independent_raw_height_reachability() {
    for level in [0, 1, 2] {
        let s = Surface::build(level, 6371000.);
        for seed in 0..12 {
            let mut rng = Random::stream(&format!("spill-{seed}"), "test-bed");
            for quantized in [false, true] {
                let h: Vec<_> = (0..s.areas.len())
                    .map(|_| {
                        let v = rng.next_u32();
                        if quantized {
                            f64::from(v % 7)
                        } else {
                            f64::from(v) / 1e6
                        }
                    })
                    .collect();
                audit(&s, &h, &SpillConnections::build(&s, &h).unwrap());
            }
        }
    }
}

#[test]
fn deep_hierarchy_and_large_plateau_are_iterative_and_do_not_materialize_cliques() {
    let n = 12001;
    let s = chain(n);
    let h: Vec<_> = (0..n)
        .map(|i| if i % 2 == 0 { 0. } else { i as f64 })
        .collect();
    let a = SpillConnections::build(&s, &h).unwrap();
    assert_eq!(a.plateaus().len(), 6000);
    let a = SpillConnections::build(&s, &vec![0.; n]).unwrap();
    assert!(a.plateaus().is_empty());
    let mut h = vec![4.; n];
    h[0] = 0.;
    h[n - 1] = 1.;
    let a = SpillConnections::build(&s, &h).unwrap();
    let p = a
        .passage(
            a.basins().region_nodes()[0],
            a.basins().region_nodes()[n - 1],
            0,
        )
        .unwrap();
    assert_eq!(p.regions.len(), n);
    assert_eq!(a.plateaus()[0].contacts.len(), 2);
    let n = 2001;
    let s = graph(n, &(1..n).map(|i| (0, i)).collect::<Vec<_>>());
    let mut h = vec![0.; n];
    h[0] = 4.;
    let a = SpillConnections::build(&s, &h).unwrap();
    assert_eq!(a.plateaus()[0].contacts.len(), n - 1);
    assert_eq!(
        a.receivers(a.basins().region_nodes()[1]).unwrap().len(),
        n - 2
    );
}

#[test]
fn generated_worlds_keep_their_water_hierarchy_and_wire_state_unchanged() {
    use planimulation_core::{Recipe, World, water::WaterSettings, wire};
    for subdivision in [0, 2, 5, 6] {
        let mut recipe: Recipe =
            serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json"))
                .unwrap();
        recipe.subdivision = subdivision;
        let world = World::generate(recipe.clone()).unwrap();
        let original = wire::arrays(&world);
        let a = SpillConnections::build(&world.surface, &world.terrain.elevation).unwrap();
        assert_eq!(a.basins(), &world.basins);
        assert_eq!(original, wire::arrays(&world));
        for source in 0..a.basins().nodes().len().min(24) {
            for receiver in a.receivers(source).unwrap() {
                for plateau in receiver.plateaus {
                    let p = a.passage(source, receiver.branch, plateau).unwrap();
                    for edge in p.regions.windows(2) {
                        assert!(adjacent(&world.surface, edge[0] as usize).contains(&edge[1]));
                    }
                    for &region in &p.regions[1..p.regions.len() - 1] {
                        assert_eq!(
                            world.terrain.elevation[region as usize],
                            a.plateaus()[plateau].level_meters
                        );
                    }
                }
            }
        }
        recipe.water = WaterSettings::Coverage { fraction: 0. };
        let mut dry = World::generate(recipe).unwrap();
        dry.advance(1).unwrap();
        assert_eq!(
            a,
            SpillConnections::build(&dry.surface, &dry.terrain.elevation).unwrap()
        );
    }
}
