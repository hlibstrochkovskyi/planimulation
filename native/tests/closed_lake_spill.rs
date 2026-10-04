use planimulation_core::{
    Recipe, Surface, World,
    basins::Basins,
    drainage::Drainage,
    seasonal_moisture::closed_lake::spill::{Destination, first_connections},
};

fn world(seed: &str) -> World {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.seed = seed.into();
    recipe.subdivision = 2;
    World::generate(recipe).unwrap()
}
fn chain(heights: &[f64], bodies: &[u32]) -> World {
    // Constructed reciprocal geometry only; never a recipe-compatible checkpoint.
    let mut world = world("spill-chain-geometry");
    let n = heights.len();
    let mut offsets = vec![0];
    let mut neighbors = Vec::new();
    for i in 0..n {
        if i > 0 {
            neighbors.push((i - 1) as u32);
        }
        if i + 1 < n {
            neighbors.push((i + 1) as u32);
        }
        offsets.push(neighbors.len() as u32);
    }
    world.surface = Surface {
        centers: vec![],
        faces: vec![],
        distances: vec![1.; neighbors.len()],
        offsets,
        neighbors,
        areas: vec![1.; n],
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    world.terrain.elevation = heights.to_vec();
    world.water.body_ids = bodies.to_vec();
    world.basins = Basins::build(&world.surface, heights).unwrap();
    world.drainage = Drainage::build(&world.surface, heights, bodies);
    world
}

#[test]
fn generated_routes_are_real_threshold_and_downhill_paths_with_actual_owners() {
    let mut checked = 0;
    for seed in ["seasonal-reference", "moisture-coast", "moisture-interior"] {
        let w = world(seed);
        let hierarchy = w.basins.clone();
        let drainage = w.drainage.clone();
        for c in first_connections(&w).unwrap() {
            assert_eq!(c.has_unique_route, c.routes.len() == 1);
            for r in c.routes {
                checked += 1;
                let h = c.first_connection_level_meters.unwrap();
                let passage = &r.sill_passage_regions;
                assert!(passage.len() >= 3);
                for &i in &passage[1..passage.len() - 1] {
                    assert_eq!(w.terrain.elevation[i as usize], h);
                }
                assert!(w.terrain.elevation[passage[0] as usize] < h);
                assert!(w.terrain.elevation[*passage.last().unwrap() as usize] < h);
                for p in passage.windows(2) {
                    let i = p[0] as usize;
                    assert!(
                        w.surface.neighbors
                            [w.surface.offsets[i] as usize..w.surface.offsets[i + 1] as usize]
                            .contains(&p[1])
                    );
                }
                assert_eq!(passage.last(), r.downhill_regions.first());
                for p in r.downhill_regions.windows(2) {
                    assert_eq!(w.drainage.receivers[p[0] as usize], p[1]);
                }
                let terminal = *r.downhill_regions.last().unwrap() as usize;
                assert_eq!(w.drainage.receivers[terminal] as usize, terminal);
                assert_ne!(w.basins.region_nodes()[terminal], c.basin_node);
                match r.destination {
                    Destination::ReferenceBody {
                        body_id,
                        contact_region,
                    } => {
                        assert_eq!(contact_region, terminal);
                        assert_eq!(body_id, w.water.body_ids[terminal]);
                        assert!(body_id > 0);
                    }
                    Destination::ClosedTerminal { terminal_region } => {
                        assert_eq!(terminal_region, terminal);
                        assert_eq!(w.water.body_ids[terminal], 0);
                    }
                }
            }
        }
        assert_eq!(hierarchy, w.basins);
        assert_eq!(drainage, w.drainage);
    }
    assert!(checked > 0);
}

#[test]
fn chain_retains_ambiguity_and_does_not_skip_an_unfilled_middle_leaf() {
    let w = chain(&[0., 4., 1., 4., 2.], &[0; 5]);
    let c = first_connections(&w).unwrap();
    let left = c.iter().find(|c| c.terminal_region == 0).unwrap();
    assert!(left.has_unique_route);
    assert_eq!(left.routes[0].sill_passage_regions, [0, 1, 2]);
    assert_eq!(
        left.routes[0].destination,
        Destination::ClosedTerminal { terminal_region: 2 }
    );
    let middle = c.iter().find(|c| c.terminal_region == 2).unwrap();
    assert!(!middle.has_unique_route);
    assert_eq!(middle.routes.len(), 2);
    assert_eq!(
        middle
            .routes
            .iter()
            .map(|r| r.destination.clone())
            .collect::<Vec<_>>(),
        [
            Destination::ClosedTerminal { terminal_region: 0 },
            Destination::ClosedTerminal { terminal_region: 4 }
        ]
    );
}

#[test]
fn nested_entry_and_reference_body_contact_are_not_canonical_teleports() {
    let w = chain(&[0., 2., 1., 5., -1.], &[0; 5]);
    let connections = first_connections(&w).unwrap();
    let c = connections.iter().find(|c| c.terminal_region == 4).unwrap();
    assert_eq!(c.routes[0].sill_passage_regions, [4, 3, 2]);
    assert_eq!(c.routes[0].downhill_regions, [2]);
    assert_eq!(
        c.routes[0].destination,
        Destination::ClosedTerminal { terminal_region: 2 }
    );
    // Synthetic wet-body mask: its canonical minimum ID is 0, but water would
    // arrive at contact 1. This fixture tests metadata, not a generated wet level.
    let w = chain(&[0., 1., 2., 0.], &[1, 1, 0, 0]);
    let connections = first_connections(&w).unwrap();
    let c = connections.iter().find(|c| c.terminal_region == 3).unwrap();
    assert_eq!(c.routes[0].sill_passage_regions, [3, 2, 1]);
    assert_eq!(w.drainage.outlets[1], 0);
    assert_eq!(
        c.routes[0].destination,
        Destination::ReferenceBody {
            body_id: 1,
            contact_region: 1
        }
    );
}

#[test]
fn closed_root_has_no_external_drain_and_stale_or_corrupt_geometry_rejects() {
    let w = chain(&[16.; 4], &[0; 4]);
    let connections = first_connections(&w).unwrap();
    assert_eq!(connections.len(), 1);
    let c = &connections[0];
    assert_eq!(c.first_connection_level_meters, None);
    assert_eq!(c.capacity_cubic_meters, None);
    assert!(c.routes.is_empty());
    assert!(!c.has_unique_route);
    let mut w = chain(&[0., 4., 1.], &[0; 3]);
    w.drainage.receivers[2] = 0;
    assert!(first_connections(&w).is_err());
    let mut w = world("seasonal-reference");
    w.terrain.elevation.fill(0.);
    assert!(first_connections(&w).is_err());
}
