use planimulation_core::{Random, Surface, basins::Basins};

fn graph(n: usize, edges: &[(usize, usize)]) -> Surface {
    let mut adjacent = vec![Vec::new(); n];
    for &(a, b) in edges {
        adjacent[a].push(b as u32);
        adjacent[b].push(a as u32);
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
    for mut neighbors in adjacent {
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
fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-10 * b.abs().max(1.), "{a} != {b}");
}

#[test]
fn weighted_nested_bowls_have_physical_thresholds_and_total_storage() {
    let mut s = chain(5);
    s.areas = vec![2., 3., 5., 7., 11.];
    let h = [0., 2., 1., 5., -1.];
    let b = Basins::build(&s, &h).unwrap();
    assert_eq!(b.nodes().len(), 5);
    assert_eq!(b.region_nodes(), [1, 3, 2, 4, 0]);
    assert_eq!(b.root(), 4);
    assert_eq!(b.nodes()[3].children, [1, 2]);
    assert_eq!(b.nodes()[4].children, [0, 3]);
    for (node, spill, capacity) in [(0, 5., 66.), (1, 2., 4.), (2, 2., 5.), (3, 5., 39.)] {
        assert_eq!(b.nodes()[node].spill_level_meters, Some(spill));
        assert_eq!(b.nodes()[node].capacity_cubic_meters, Some(capacity));
        near(b.volume_at_level(node, spill).unwrap(), capacity);
    }
    near(b.volume_at_level(3, 2.).unwrap(), 9.);
    near(b.volume_at_level(3, 3.).unwrap(), 19.);
    near(b.volume_at_level(4, 5.).unwrap(), 105.);
    near(b.volume_at_level(4, 6.).unwrap(), 133.);
    assert_eq!(b.nodes()[4].spill_level_meters, None);
    assert_eq!(b.nodes()[4].capacity_cubic_meters, None);
    assert_eq!(h, [0., 2., 1., 5., -1.]);
    assert!(b.volume_at_level(3, 1.9).is_err());
    assert!(b.volume_at_level(3, 5.1).is_err());
    assert!(b.volume_at_level(4, f64::INFINITY).is_err());
    assert!(b.volume_at_level(100, 5.).is_err());
}

#[test]
fn simultaneous_sills_form_one_multiway_merge_not_an_ordered_binary_ladder() {
    for (mut s, h) in [
        (chain(5), vec![0., 4., 1., 4., 2.]),
        (graph(4, &[(0, 1), (0, 2), (0, 3)]), vec![4., 0., 1., 2.]),
    ] {
        let b = Basins::build(&s, &h).unwrap();
        assert_eq!(b.nodes().len(), 4);
        assert_eq!(b.nodes()[b.root()].children.len(), 3);
        for child in &b.nodes()[b.root()].children {
            assert_eq!(b.nodes()[*child].spill_level_meters, Some(4.));
        }
        for i in 0..h.len() {
            s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize].reverse();
        }
        assert_eq!(b, Basins::build(&s, &h).unwrap());
    }
}

#[test]
fn flats_slopes_and_closed_global_root_do_not_invent_outlets() {
    for h in [
        vec![0.; 5],
        vec![0., 1., 2., 3., 4.],
        vec![4., 1., 0., 1., 4.],
    ] {
        let b = Basins::build(&chain(5), &h).unwrap();
        assert_eq!(b.nodes().len(), 1);
        assert_eq!(b.nodes()[0].parent, None);
        assert_eq!(b.nodes()[0].spill_edge, None);
        assert_eq!(b.nodes()[0].capacity_cubic_meters, None);
        near(
            b.volume_at_level(0, 10.).unwrap(),
            h.iter().map(|z| 10. - z).sum(),
        );
    }
    let s = chain(4);
    assert_eq!(
        Basins::build(&s, &[-0., 0., -0., 0.]).unwrap(),
        Basins::build(&s, &[0.; 4]).unwrap()
    );
}

// Independent positive-depth BFS, not the production elevation sweep or union-find.
fn audit_level(s: &Surface, h: &[f64], b: &Basins, level: f64) {
    let mut visited = vec![false; h.len()];
    let mut branches = std::collections::BTreeSet::new();
    let mut total = 0.;
    for start in 0..h.len() {
        if visited[start] || h[start] >= level {
            continue;
        }
        let mut queue = vec![start];
        visited[start] = true;
        let mut cursor = 0;
        let mut volume = 0.;
        let mut branch = b.region_nodes()[start];
        while let Some(parent) = b.nodes()[branch].parent {
            if b.nodes()[parent].birth_level_meters >= level {
                break;
            }
            branch = parent;
        }
        assert!(
            branches.insert(branch),
            "Disconnected components cannot share a live branch."
        );
        while cursor < queue.len() {
            let i = queue[cursor];
            cursor += 1;
            volume += s.areas[i] * (level - h[i]);
            let mut node = b.region_nodes()[i];
            while node != branch {
                node = b.nodes()[node].parent.expect("Wrong branch membership.");
            }
            for &j in &s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize] {
                let j = j as usize;
                if !visited[j] && h[j] < level {
                    visited[j] = true;
                    queue.push(j);
                }
            }
        }
        near(b.volume_at_level(branch, level).unwrap(), volume);
        total += volume;
    }
    near(
        total,
        h.iter()
            .zip(&s.areas)
            .map(|(z, a)| a * (level - z).max(0.))
            .sum(),
    );
}

fn audit(s: &Surface, h: &[f64], b: &Basins) {
    assert_eq!(b, &Basins::build(s, h).unwrap());
    assert!(b.nodes().len() < 2 * h.len());
    for (id, node) in b.nodes().iter().enumerate() {
        if let Some(parent) = node.parent {
            assert!(parent > id);
            assert!(b.nodes()[parent].children.contains(&id));
            let spill = node.spill_level_meters.unwrap();
            assert!(spill > node.birth_level_meters);
            assert_eq!(spill, b.nodes()[parent].birth_level_meters);
            let [a, c] = node.spill_edge.unwrap().map(|i| i as usize);
            assert_eq!(h[a], spill);
            assert!(h[c] < spill);
            assert!(
                s.neighbors[s.offsets[a] as usize..s.offsets[a + 1] as usize].contains(&(c as u32))
            );
            let mut inside = b.region_nodes()[c];
            while inside != id {
                inside = b.nodes()[inside].parent.unwrap();
            }
            near(
                b.volume_at_level(id, spill).unwrap(),
                node.capacity_cubic_meters.unwrap(),
            );
        } else {
            assert_eq!(id, b.root());
        }
        if !node.children.is_empty() {
            assert!(node.children.len() >= 2);
            near(
                b.volume_at_level(id, node.birth_level_meters).unwrap(),
                node.children
                    .iter()
                    .map(|&c| b.nodes()[c].capacity_cubic_meters.unwrap())
                    .sum(),
            );
        }
    }
    near(
        b.nodes()[b.root()].support_area_square_meters,
        s.areas.iter().sum(),
    );
    let mut levels = h.to_vec();
    levels.sort_by(f64::total_cmp);
    levels.dedup();
    for pair in levels.windows(2) {
        audit_level(s, h, b, pair[0] + (pair[1] - pair[0]) * 0.5);
    }
    audit_level(s, h, b, levels[levels.len() - 1] + 1.);
}

#[test]
fn independent_threshold_components_verify_quantized_and_continuous_spherical_ensembles() {
    for level in [0, 1, 2] {
        let s = Surface::build(level, 6371000.);
        for seed in 0..20 {
            let mut rng = Random::stream(&format!("basins-{seed}"), "test-bed");
            for quantized in [false, true] {
                let h: Vec<f64> = (0..s.areas.len())
                    .map(|_| {
                        let v = rng.next_u32();
                        if quantized {
                            f64::from(v % 7) - 3.
                        } else {
                            f64::from(v) / 1e6
                        }
                    })
                    .collect();
                audit(&s, &h, &Basins::build(&s, &h).unwrap());
            }
        }
    }
}

#[test]
fn area_scaling_and_datum_shifts_preserve_topology_and_scale_capacity() {
    let mut s = chain(7);
    let h = [0., 2., 1., 5., -1., 8., 3.];
    let b = Basins::build(&s, &h).unwrap();
    s.areas.iter_mut().for_each(|a| *a *= 4.);
    let shifted = Basins::build(&s, &h.map(|z| z + 1e9)).unwrap();
    assert_eq!(b.region_nodes(), shifted.region_nodes());
    for (a, c) in b.nodes().iter().zip(shifted.nodes()) {
        assert_eq!(a.parent, c.parent);
        assert_eq!(a.children, c.children);
        assert_eq!(a.spill_edge, c.spill_edge);
        near(
            a.support_area_square_meters * 4.,
            c.support_area_square_meters,
        );
        if let Some(v) = a.capacity_cubic_meters {
            near(v * 4., c.capacity_cubic_meters.unwrap());
        }
    }
    near(
        b.volume_at_level(b.root(), 10.).unwrap() * 4.,
        shifted.volume_at_level(shifted.root(), 1e9 + 10.).unwrap(),
    );
}

#[test]
fn finest_flat_sphere_and_deep_merge_chain_are_iterative() {
    let s = Surface::build(6, 6371000.);
    let h = vec![0.; s.areas.len()];
    let b = Basins::build(&s, &h).unwrap();
    assert_eq!(b.nodes().len(), 1);
    near(
        b.volume_at_level(0, 2.).unwrap(),
        s.areas.iter().sum::<f64>() * 2.,
    );
    let h: Vec<f64> = (0..12001)
        .map(|i| if i % 2 == 0 { 0. } else { i as f64 })
        .collect();
    let s = chain(h.len());
    let b = Basins::build(&s, &h).unwrap();
    assert_eq!(b.nodes().len(), 12001);
    near(
        b.volume_at_level(b.root(), 12002.).unwrap(),
        h.iter().map(|z| 12002. - z).sum(),
    );
}

#[test]
fn invalid_fields_disconnected_graph_and_overflow_are_rejected() {
    let mut s = chain(2);
    for h in [
        vec![],
        vec![0.],
        vec![0., f64::NAN],
        vec![f64::NEG_INFINITY, 0.],
    ] {
        assert!(Basins::build(&s, &h).is_err());
    }
    s.areas[0] = 0.;
    assert!(Basins::build(&s, &[0., 1.]).is_err());
    s.areas[0] = f64::INFINITY;
    assert!(Basins::build(&s, &[0., 1.]).is_err());
    assert!(Basins::build(&graph(2, &[]), &[0., 1.]).is_err());
    assert!(Basins::build(&chain(2), &[-f64::MAX, f64::MAX]).is_err());
    let b = Basins::build(&chain(2), &[0.; 2]).unwrap();
    assert!(b.volume_at_level(b.root(), f64::MAX).is_err());
}

#[test]
fn generated_terrain_retains_water_and_drainage_and_matches_threshold_budgets() {
    use planimulation_core::{Recipe, World, water::WaterSettings};
    for level in [0, 2, 4, 6] {
        for seed in 0..if level == 6 { 2 } else { 10 } {
            let mut world = World::generate(Recipe {
                schema_version: 1,
                model_version: "basins-1".into(),
                random_version: "fnv1a-utf8-mulberry32-1".into(),
                seed: format!("basin-world-{seed}"),
                subdivision: level,
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
            let original = planimulation_core::wire::arrays(&world);
            let b = Basins::build(&world.surface, &world.terrain.elevation).unwrap();
            assert_eq!(original, planimulation_core::wire::arrays(&world));
            let low = world
                .terrain
                .elevation
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min);
            let high = world
                .terrain
                .elevation
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            for k in 1..=8 {
                audit_level(
                    &world.surface,
                    &world.terrain.elevation,
                    &b,
                    low + (high - low) * k as f64 / 8. + 0.01,
                );
            }
            for (id, node) in b.nodes().iter().enumerate() {
                if let Some(spill) = node.spill_level_meters {
                    near(
                        b.volume_at_level(id, spill).unwrap(),
                        node.capacity_cubic_meters.unwrap(),
                    );
                }
            }
            world.advance(1).unwrap();
            assert_eq!(
                b,
                Basins::build(&world.surface, &world.terrain.elevation).unwrap()
            );
            let mut recipe = world.recipe.clone();
            recipe.water = WaterSettings::Coverage { fraction: 0. };
            let dry = World::generate(recipe).unwrap();
            assert_eq!(
                b,
                Basins::build(&dry.surface, &dry.terrain.elevation).unwrap()
            );
        }
    }
}
