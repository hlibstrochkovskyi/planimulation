use planimulation_core::{
    Recipe, Surface, World,
    terrain_preparation::{MAX_CHANGE_PER_PASS_METERS, MAX_PASSES, prepare},
};

fn chain() -> Surface {
    Surface {
        centers: vec![[1., 0., 0.]; 3],
        faces: vec![],
        offsets: vec![0, 1, 3, 4],
        neighbors: vec![1, 0, 2, 1],
        distances: vec![1000.; 4],
        areas: vec![2., 1., 3.],
        boundary_offsets: vec![],
        boundaries: vec![],
    }
}

fn material(surface: &Surface, heights: &[f64]) -> f64 {
    heights
        .iter()
        .zip(&surface.areas)
        .map(|(height, area)| height * area)
        .sum()
}

#[test]
fn one_pass_moves_only_excess_slope_and_accounts_for_both_sides() {
    let s = chain();
    let before = [100., 0., 0.];
    let result = prepare(&s, &before, 1).unwrap();
    // 96 m excess above the 4 m edge threshold; the harmonic-area transfer
    // is 0.25 * 96 / (1/2 + 1/1) = 16 reference-area m³.
    assert_eq!(result.elevation_meters, vec![92., 16., 0.]);
    assert_eq!(result.eroded_meters, vec![8., 0., 0.]);
    assert_eq!(result.deposited_meters, vec![0., 16., 0.]);
    assert_eq!(result.transported_cubic_meters, 16.);
    assert_eq!((result.requested_passes, result.applied_passes), (1, 1));
    assert_eq!(
        material(&s, &before),
        material(&s, &result.elevation_meters)
    );
    let flat = prepare(&s, &[4., 0., 0.], 16).unwrap();
    assert_eq!(flat.transported_cubic_meters, 0.);
    assert_eq!((flat.requested_passes, flat.applied_passes), (16, 0));
    assert_eq!(prepare(&s, &before, 0).unwrap().elevation_meters, before);
}

#[test]
fn shared_donor_and_recipient_caps_bound_all_simultaneous_transfers() {
    let s = chain();
    let before = [10_000., 0., -10_000.];
    let result = prepare(&s, &before, 1).unwrap();
    for (i, original) in before.iter().enumerate() {
        assert!(
            (result.elevation_meters[i] - *original).abs() <= MAX_CHANGE_PER_PASS_METERS + 1e-12
        );
    }
    assert!(result.transported_cubic_meters > 0.);
    assert!((material(&s, &before) - material(&s, &result.elevation_meters)).abs() < 1e-9);
    assert!(
        (result
            .eroded_meters
            .iter()
            .zip(&s.areas)
            .map(|(h, a)| h * a)
            .sum::<f64>()
            - result.transported_cubic_meters)
            .abs()
            < 1e-9
    );
    assert!(
        (result
            .deposited_meters
            .iter()
            .zip(&s.areas)
            .map(|(h, a)| h * a)
            .sum::<f64>()
            - result.transported_cubic_meters)
            .abs()
            < 1e-9
    );
    assert_eq!(
        prepare(&s, &before, 8).unwrap(),
        prepare(&s, &before, 8).unwrap()
    );
}

#[test]
fn area_scaling_datum_shifts_and_region_relabeling_preserve_the_physical_result() {
    let s = chain();
    let base = prepare(&s, &[100., 0., -20.], 4).unwrap();
    let mut scaled = chain();
    for area in &mut scaled.areas {
        *area *= 10.;
    }
    let larger = prepare(&scaled, &[100., 0., -20.], 4).unwrap();
    for (a, b) in base.elevation_meters.iter().zip(&larger.elevation_meters) {
        assert!((a - b).abs() < 1e-12);
    }
    assert!((larger.transported_cubic_meters - 10. * base.transported_cubic_meters).abs() < 1e-9);
    let shifted = prepare(&s, &[1100., 1000., 980.], 4).unwrap();
    for (a, b) in base.elevation_meters.iter().zip(&shifted.elevation_meters) {
        assert!((a + 1000. - b).abs() < 1e-11);
    }
    let mut reversed = chain();
    reversed.areas.reverse();
    let relabeled = prepare(&reversed, &[-20., 0., 100.], 4).unwrap();
    for (a, b) in base
        .elevation_meters
        .iter()
        .rev()
        .zip(&relabeled.elevation_meters)
    {
        assert!((a - b).abs() < 1e-12);
    }
}

#[test]
fn malformed_input_rejects_without_modifying_the_supplied_heights() {
    let s = chain();
    let heights = [100., 0., 0.];
    assert!(prepare(&s, &heights, MAX_PASSES + 1).is_err());
    assert!(prepare(&s, &[f64::NAN, 0., 0.], 1).is_err());
    assert!(prepare(&s, &[100., 0.], 1).is_err());
    let mut bad = chain();
    bad.distances[0] = 0.;
    assert!(prepare(&bad, &heights, 1).is_err());
    let mut asymmetric = chain();
    asymmetric.distances[0] = 999.;
    assert!(prepare(&asymmetric, &heights, 1).is_err());
    assert_eq!(heights, [100., 0., 0.]);
}

#[test]
fn seeded_worlds_keep_a_reproducible_reference_area_material_ledger() {
    let mut changed_worlds = 0;
    for (seed, level) in [
        ("preparation-a", 2),
        ("preparation-b", 3),
        ("preparation-c", 4),
    ] {
        let recipe: Recipe = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1, "modelVersion": "basins-1", "randomVersion": "fnv1a-utf8-mulberry32-1",
            "seed": seed, "subdivision": level, "radiusMeters": 6371000., "plateCount": 12,
            "maxPlateSpeedCmPerYear": 8., "continentalFraction": 0.38, "continentalScale": 1.,
            "reliefScale": 1., "boundaryWidthKm": 300., "detailAmplitudeMeters": 300.,
            "water": { "mode": "coverage", "fraction": 0.71 }
        })).unwrap();
        let world = World::generate(recipe).unwrap();
        let result = prepare(&world.surface, &world.terrain.elevation, 8).unwrap();
        if result.transported_cubic_meters > 0. {
            changed_worlds += 1;
        }
        assert_eq!(
            result,
            prepare(&world.surface, &world.terrain.elevation, 8).unwrap()
        );
        assert!(
            result
                .elevation_meters
                .iter()
                .all(|height| height.is_finite())
        );
        let original = material(&world.surface, &world.terrain.elevation);
        let prepared = material(&world.surface, &result.elevation_meters);
        let reference = world
            .surface
            .areas
            .iter()
            .zip(&world.terrain.elevation)
            .map(|(area, height)| (area * height).abs())
            .sum::<f64>();
        assert!((prepared - original).abs() / reference < 1e-13);
        let moved = result
            .eroded_meters
            .iter()
            .zip(&world.surface.areas)
            .map(|(height, area)| height * area)
            .sum::<f64>();
        assert!(
            (moved - result.transported_cubic_meters).abs()
                / result.transported_cubic_meters.max(1.)
                < 1e-12
        );
        // This preparation remains separate: the accepted generated world's
        // elevation, water, and drainage are not changed by a probe call.
        assert_eq!(world.terrain.elevation.len(), result.elevation_meters.len());
        assert_eq!(
            world.water.depth_meters.len(),
            result.elevation_meters.len()
        );
    }
    assert!(
        changed_worlds > 0,
        "At least one seeded world must actually prepare steep terrain."
    );
}
