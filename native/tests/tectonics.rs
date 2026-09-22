use planimulation_core::{Recipe, World, tectonics::*};
use std::collections::{BTreeMap, BTreeSet};

fn recipe(level: u32, count: u32, seed: u32) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: "crust-1".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: format!("plates-{seed}"),
        subdivision: level,
        radius_meters: 6_371_000.,
        plate_count: count,
        max_plate_speed_cm_per_year: 8.,
        continental_fraction: 0.38,
        continental_scale: 1.,
    }
}

#[test]
fn controlled_boundary_motion_and_reference_frame_invariance() {
    assert_eq!(classify(QUIET_METERS_PER_YEAR, 0.), QUIET);
    assert_eq!(classify(QUIET_METERS_PER_YEAR * 2., 0.), DIVERGENT);
    let p = [1., 0., 0.];
    let normal = [0., 1., 0.];
    let zero = [0.; 3];
    for (omega, expected, kind) in [
        ([0., 0., 0.5], (1., 0.), DIVERGENT),
        ([0., 0., -0.5], (-1., 0.), CONVERGENT),
        ([0., -0.5, 0.], (0., 1.), TRANSFORM),
        ([0., 0., 0.], (0., 0.), QUIET),
        ([0., -1., 0.5], (1., 2.), TRANSFORM),
    ] {
        let actual = relative_motion(zero, omega, p, normal, 2.);
        assert_eq!(actual, expected);
        assert_eq!(classify(actual.0, actual.1), kind);
        let base = [2., 3., 4.];
        let shifted = [omega[0] + base[0], omega[1] + base[1], omega[2] + base[2]];
        assert_eq!(actual, relative_motion(base, shifted, p, normal, 2.));
        assert_eq!(
            actual,
            relative_motion(omega, zero, p, normal.map(|v| -v), 2.)
        );
    }
    let v = velocity([0.1, 0.2, 0.3], p, 100.);
    assert_eq!(v[0], 0.);
    assert_eq!(velocity([0.1, 0., 0.], p, 100.), zero);
}

#[test]
fn seed_ensemble_partitions_cover_the_sphere_and_every_plate_is_connected() {
    for level in [0, 2, 4] {
        for count in [2, 7, if level == 0 { 12 } else { 32 }] {
            for seed in 0..20 {
                let w = World::generate(recipe(level, count, seed)).unwrap();
                let t = &w.tectonics;
                let s = &w.surface;
                assert_eq!(t.seeds.len(), count as usize);
                assert_eq!(
                    t.seeds.iter().copied().collect::<BTreeSet<_>>().len(),
                    count as usize
                );
                for (plate, &root) in t.seeds.iter().enumerate() {
                    let mut seen = BTreeSet::from([root]);
                    let mut queue = vec![root];
                    let mut index = 0;
                    while index < queue.len() {
                        let i = queue[index] as usize;
                        index += 1;
                        for &j in &s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize] {
                            if t.owners[j as usize] == plate as u32 && seen.insert(j) {
                                queue.push(j);
                            }
                        }
                    }
                    assert_eq!(
                        seen.len(),
                        t.owners.iter().filter(|&&p| p == plate as u32).count()
                    );
                    assert_eq!(t.owners[root as usize], plate as u32);
                }
                let mut expected = BTreeMap::new();
                for i in 0..t.owners.len() {
                    for &j in &s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize] {
                        if i < j as usize && t.owners[i] != t.owners[j as usize] {
                            expected.insert((i as u32, j), 2);
                        }
                    }
                    let v = velocity(
                        t.angular_velocities[t.owners[i] as usize],
                        s.centers[i],
                        w.recipe.radius_meters,
                    );
                    assert!(
                        v.iter()
                            .zip(s.centers[i])
                            .map(|(a, b)| a * b)
                            .sum::<f64>()
                            .abs()
                            < 1e-15
                    );
                    assert!(v.iter().map(|v| v * v).sum::<f64>().sqrt() <= 0.08 + 1e-15);
                }
                let mut actual = BTreeMap::new();
                for (i, pair) in t.boundary_cells.as_chunks::<2>().0.iter().enumerate() {
                    *actual.entry((pair[0], pair[1])).or_insert(0) += 1;
                    let (opening, shear) = (t.boundary_motion[i * 2], t.boundary_motion[i * 2 + 1]);
                    assert!(opening.is_finite() && shear.is_finite());
                    assert!(opening.hypot(shear) <= 0.16 + 1e-15);
                    assert_eq!(t.boundary_types[i], classify(opening, shear));
                    for endpoint in &t.boundary_directions[i * 2..i * 2 + 2] {
                        assert!((endpoint.iter().map(|v| v * v).sum::<f64>() - 1.).abs() < 1e-14);
                        for &cell in pair {
                            let ring = &s.boundaries[s.boundary_offsets[cell as usize] as usize
                                ..s.boundary_offsets[cell as usize + 1] as usize];
                            assert!(
                                ring.contains(endpoint),
                                "Actual shared dual-boundary endpoint"
                            );
                        }
                    }
                }
                assert_eq!(actual, expected);
            }
        }
    }
}

#[test]
fn parameters_are_independent_and_have_controlled_effects() {
    let r = recipe(3, 12, 42);
    let w = World::generate(r.clone()).unwrap();
    let mut half = r.clone();
    half.max_plate_speed_cm_per_year *= 0.5;
    let half = World::generate(half).unwrap();
    assert_eq!(w.field, half.field);
    assert_eq!(w.tectonics.owners, half.tectonics.owners);
    assert_eq!(
        w.tectonics.boundary_directions,
        half.tectonics.boundary_directions
    );
    for (a, b) in w
        .tectonics
        .boundary_motion
        .iter()
        .zip(&half.tectonics.boundary_motion)
    {
        assert_eq!(*a * 0.5, *b);
    }
    let mut stopped = r.clone();
    stopped.max_plate_speed_cm_per_year = 0.;
    let stopped = World::generate(stopped).unwrap();
    assert_eq!(w.tectonics.owners, stopped.tectonics.owners);
    assert!(stopped.tectonics.boundary_motion.iter().all(|v| *v == 0.));
    assert!(stopped.tectonics.boundary_types.iter().all(|v| *v == QUIET));
    let mut larger = r.clone();
    larger.radius_meters *= 2.;
    let larger = World::generate(larger).unwrap();
    assert_eq!(w.tectonics.owners, larger.tectonics.owners);
    assert_eq!(
        w.tectonics.boundary_motion,
        larger.tectonics.boundary_motion
    );
    let mut more = r;
    more.plate_count = 20;
    let more = World::generate(more).unwrap();
    assert_eq!(w.field, more.field);
    assert_eq!(w.surface.centers, more.surface.centers);
    assert_eq!(w.tectonics.seeds, &more.tectonics.seeds[..12]);
    assert_eq!(
        w.tectonics.angular_velocities,
        &more.tectonics.angular_velocities[..12]
    );
    let original = w.tectonics.clone();
    let mut running = w;
    running.advance(100).unwrap();
    assert_eq!(
        original, running.tectonics,
        "Diagnostic evolution must not move plates."
    );
}

#[test]
fn invalid_parameters_are_rejected() {
    for (level, count, speed) in [
        (0, 13, 8.),
        (1, 0, 8.),
        (1, 33, 8.),
        (1, 12, -1.),
        (1, 12, 21.),
        (1, 12, f64::NAN),
    ] {
        let mut r = recipe(level, count, 1);
        r.max_plate_speed_cm_per_year = speed;
        assert!(r.validate().is_err());
    }
}
