use planimulation_core::{
    Recipe, World,
    geodesic_belts::{Origin, build},
};
fn recipe(seed: &str) -> Recipe {
    serde_json::from_value(serde_json::json!({"schemaVersion":1,"modelVersion":"multiscale-geography-2","randomVersion":"fnv1a-utf8-mulberry32-1","seed":seed,"subdivision":3,"radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,"detailAmplitudeMeters":300.,"terrainPreparationPasses":0,"water":{"mode":"coverage","fraction":0.71}})).unwrap()
}
#[test]
fn both_geometries_keep_owned_columns_detail_and_inputs_unchanged() {
    for seed in ["first-light", "4836187e", "geography-00"] {
        let w = World::generate(recipe(seed)).unwrap();
        let old = w.terrain.clone();
        for origin in [
            Origin::RegionSeeds,
            Origin::BoundaryArcs,
            Origin::FixedBoundaryArcs,
        ] {
            let a = build(&w.surface, &w.crust, &w.tectonics, &w.recipe, &old, origin).unwrap();
            let b = build(&w.surface, &w.crust, &w.tectonics, &w.recipe, &old, origin).unwrap();
            assert_eq!(a.terrain, b.terrain);
            assert_eq!(a.terrain.baseline, old.baseline);
            assert_eq!(a.terrain.detail, old.detail);
            assert_eq!(a.terrain.preparation, old.preparation);
            assert_eq!(w.terrain, old);
            for i in 0..old.elevation.len() {
                assert_eq!(
                    a.terrain.elevation[i],
                    old.baseline[i]
                        + a.terrain.convergence[i]
                        + a.terrain.divergence[i]
                        + old.detail[i]
                );
                assert!((0. ..=6000.).contains(&a.terrain.convergence[i]));
                assert!((-1500. ..=2500.).contains(&a.terrain.divergence[i]));
                if matches!(origin, Origin::RegionSeeds) {
                    assert!(a.terrain.convergence[i] + 1e-9 >= old.convergence[i]);
                }
            }
        }
    }
}
#[test]
fn stopped_or_zero_strength_has_no_structural_height_change() {
    for no_motion in [true, false] {
        let mut r = recipe("zero");
        if no_motion {
            r.max_plate_speed_cm_per_year = 0.;
        } else {
            r.relief_scale = 0.;
        }
        let w = World::generate(r).unwrap();
        for origin in [
            Origin::RegionSeeds,
            Origin::BoundaryArcs,
            Origin::FixedBoundaryArcs,
        ] {
            let a = build(
                &w.surface,
                &w.crust,
                &w.tectonics,
                &w.recipe,
                &w.terrain,
                origin,
            )
            .unwrap();
            assert_eq!(a.terrain.elevation, w.terrain.elevation);
            assert!(a.terrain.convergence.iter().all(|&v| v == 0.));
            assert!(a.terrain.divergence.iter().all(|&v| v == 0.));
        }
    }
}

#[test]
fn fixed_origins_and_shared_initial_contributions_do_not_depend_on_target_grid() {
    let mut r = recipe("first-light");
    r.subdivision = 2;
    let coarse = World::generate(r.clone()).unwrap();
    r.subdivision = 3;
    let fine = World::generate(r).unwrap();
    let apply = |w: &World| {
        build(
            &w.surface,
            &w.crust,
            &w.tectonics,
            &w.recipe,
            &w.terrain,
            Origin::FixedBoundaryArcs,
        )
        .unwrap()
    };
    let a = apply(&coarse);
    let b = apply(&fine);
    assert_ne!(
        coarse.tectonics.boundary_types.len(),
        fine.tectonics.boundary_types.len()
    );
    assert_eq!(a.source_identity, b.source_identity);
    assert!(a.source_identity.is_some());
    assert_eq!(
        serde_json::to_vec(&a.sources).unwrap(),
        serde_json::to_vec(&b.sources).unwrap()
    );
    let shared = coarse.surface.centers.len();
    assert_eq!(coarse.surface.centers, fine.surface.centers[..shared]);
    assert_eq!(a.terrain.baseline, b.terrain.baseline[..shared]);
    assert_eq!(a.terrain.detail, b.terrain.detail[..shared]);
    assert_eq!(a.terrain.convergence, b.terrain.convergence[..shared]);
    assert_eq!(a.terrain.divergence, b.terrain.divergence[..shared]);
    assert_eq!(a.terrain.elevation, b.terrain.elevation[..shared]);
    // This is initial-field consistency, not convergence of discretized erosion,
    // water coverage fitting, downstream connectivity or prepared landscapes.
}
#[test]
fn malformed_fields_and_noninitial_provenance_refuse_without_mutation() {
    let w = World::generate(recipe("invalid")).unwrap();
    let mut plates = w.tectonics.clone();
    plates.boundary_cells[0] = u32::MAX;
    assert!(
        build(
            &w.surface,
            &w.crust,
            &plates,
            &w.recipe,
            &w.terrain,
            Origin::BoundaryArcs
        )
        .is_err()
    );
    let mut old = w.terrain.clone();
    old.preparation[0] = 1.;
    assert!(
        build(
            &w.surface,
            &w.crust,
            &w.tectonics,
            &w.recipe,
            &old,
            Origin::RegionSeeds
        )
        .is_err()
    );
    assert_eq!(old.preparation[0], 1.);
    let mut overflow = w.terrain.clone();
    overflow.baseline[0] = 1e308;
    overflow.convergence[0] = -1e308;
    overflow.divergence[0] = 0.;
    overflow.detail[0] = 1e308;
    overflow.elevation[0] = 1e308;
    let snapshot = overflow.clone();
    assert!(
        build(
            &w.surface,
            &w.crust,
            &w.tectonics,
            &w.recipe,
            &overflow,
            Origin::BoundaryArcs
        )
        .is_err()
    );
    assert_eq!(overflow, snapshot);
}
