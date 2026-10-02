use planimulation_core::{Recipe, Surface, World, hash, plate_roots, tectonics::Tectonics, wire};
use std::collections::BTreeSet;

fn angular_error(root: [f64; 3], center: [f64; 3]) -> f64 {
    let dot: f64 = root.iter().zip(center).map(|(a, b)| a * b).sum();
    dot.clamp(-1., 1.).acos()
}

#[test]
fn continuous_roots_are_reproducible_bounded_and_count_prefix_stable() {
    let roots = plate_roots::directions("root-candidate", 32).unwrap();
    assert_eq!(
        roots,
        plate_roots::directions("root-candidate", 32).unwrap()
    );
    assert_eq!(
        &roots[..12],
        plate_roots::directions("root-candidate", 12).unwrap()
    );
    assert_ne!(roots, plate_roots::directions("another-seed", 32).unwrap());
    for root in roots {
        let norm: f64 = root.iter().map(|value| value * value).sum();
        assert!((norm - 1.).abs() < 1e-14);
    }
    assert!(plate_roots::directions(" ", 12).is_err());
    assert!(plate_roots::directions("x", 1).is_err());
    assert!(plate_roots::directions("x", 33).is_err());
}

#[test]
fn projection_keeps_distinct_roots_and_improves_nearest_error_with_refinement() {
    for seed in ["root-a", "root-b", "root-c"] {
        let roots = plate_roots::directions(seed, 12).unwrap();
        let mut prior = vec![f64::INFINITY; roots.len()];
        for level in 2..=6 {
            let surface = Surface::build(level, 6_371_000.);
            let cells = plate_roots::project(&surface, &roots).unwrap();
            assert_eq!(cells.len(), roots.len());
            assert_eq!(
                cells.iter().copied().collect::<BTreeSet<_>>().len(),
                roots.len()
            );
            for (plate, &cell) in cells.iter().enumerate() {
                let error = angular_error(roots[plate], surface.centers[cell as usize]);
                assert!(error <= prior[plate] + 1e-12);
                prior[plate] = error;
            }
        }
    }
    let coarse = Surface::build(0, 6_371_000.);
    let roots = plate_roots::directions("root-candidate", 32).unwrap();
    assert!(plate_roots::project(&coarse, &roots).is_err());
    assert!(plate_roots::project(&coarse, &[[f64::NAN, 0., 0.]]).is_err());
    assert!(plate_roots::project(&coarse, &[[1., 0., 0.], [1., 0., 0.]]).is_ok());
}

#[test]
fn candidate_partition_is_connected_and_keeps_motion_stream_independent() {
    for level in [1, 3, 5] {
        let surface = Surface::build(level, 6_371_000.);
        let candidate =
            Tectonics::build_with_continuous_roots(&surface, "root-candidate", 12, 8., 6_371_000.)
                .unwrap();
        let repeated =
            Tectonics::build_with_continuous_roots(&surface, "root-candidate", 12, 8., 6_371_000.)
                .unwrap();
        let legacy = Tectonics::build(&surface, "root-candidate", 12, 8., 6_371_000.);
        assert_eq!(candidate, repeated);
        assert_eq!(candidate.angular_velocities, legacy.angular_velocities);
        for (plate, &root) in candidate.seeds.iter().enumerate() {
            assert_eq!(candidate.owners[root as usize], plate as u32);
            let expected = candidate
                .owners
                .iter()
                .filter(|&&owner| owner == plate as u32)
                .count();
            let mut seen = vec![false; surface.centers.len()];
            let mut queue = vec![root as usize];
            seen[root as usize] = true;
            let mut index = 0;
            while index < queue.len() {
                let region = queue[index];
                for edge in surface.offsets[region] as usize..surface.offsets[region + 1] as usize {
                    let neighbor = surface.neighbors[edge] as usize;
                    if !seen[neighbor] && candidate.owners[neighbor] == plate as u32 {
                        seen[neighbor] = true;
                        queue.push(neighbor);
                    }
                }
                index += 1;
            }
            assert_eq!(queue.len(), expected);
        }
    }
    let surface = Surface::build(1, 6_371_000.);
    assert!(Tectonics::build_with_continuous_roots(&surface, "x", 12, -1., 6_371_000.).is_err());
}

#[test]
fn candidate_accepts_supported_coarse_and_dense_plate_counts() {
    for (level, count) in [(0, 2), (0, 12), (1, 32), (4, 32), (6, 32)] {
        let surface = Surface::build(level, 6_371_000.);
        let candidate =
            Tectonics::build_with_continuous_roots(&surface, "count-bounds", count, 8., 6_371_000.)
                .unwrap();
        assert_eq!(candidate.seeds.len(), count as usize);
        assert_eq!(candidate.owners.len(), surface.centers.len());
        assert!(candidate.owners.iter().all(|&owner| owner < count));
        assert_eq!(
            candidate
                .seeds
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len(),
            count as usize
        );
        for (plate, &root) in candidate.seeds.iter().enumerate() {
            assert_eq!(candidate.owners[root as usize], plate as u32);
        }
    }
}

#[test]
fn legacy_plate_seeds_remain_pinned() {
    let surface = Surface::build(5, 6_371_000.);
    let legacy = Tectonics::build(&surface, "first-light", 12, 8., 6_371_000.);
    assert_eq!(
        legacy.seeds,
        [
            1888, 3264, 9639, 9348, 8374, 4217, 9115, 4643, 6849, 2247, 5803, 1430
        ]
    );
    let recipe: Recipe = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1, "modelVersion": "basins-1", "randomVersion": "fnv1a-utf8-mulberry32-1",
        "seed": "first-light", "subdivision": 5, "radiusMeters": 6371000., "plateCount": 12,
        "maxPlateSpeedCmPerYear": 8., "continentalFraction": 0.38, "continentalScale": 1.,
        "reliefScale": 1., "boundaryWidthKm": 300., "detailAmplitudeMeters": 300.,
        "water": { "mode": "coverage", "fraction": 0.71 }
    }))
    .unwrap();
    let world = World::generate(recipe).unwrap();
    let mut fingerprint = serde_json::to_vec(&world.recipe).unwrap();
    fingerprint.extend(wire::arrays(&world));
    assert_eq!(format!("{:08x}", hash(&fingerprint)), "2e66ac09");
}
