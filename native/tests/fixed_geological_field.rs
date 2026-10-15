use planimulation_core::{
    Recipe, Surface, World,
    fixed_geological_field::Field,
    geodesic_belts::{self, Origin},
};
fn recipe() -> Recipe {
    serde_json::from_value(serde_json::json!({"schemaVersion":1,"modelVersion":"multiscale-geography-2",
        "randomVersion":"fnv1a-utf8-mulberry32-1","seed":"first-light","subdivision":2,"radiusMeters":6371000.,
        "plateCount":12,"maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,
        "boundaryWidthKm":300.,"detailAmplitudeMeters":300.,"terrainPreparationPasses":0,"water":{"mode":"coverage","fraction":0.71}})).unwrap()
}
#[test]
fn direct_samples_equal_existing_fixed_origin_composition_at_every_shared_direction() {
    let r = recipe();
    let w = World::generate(r.clone()).unwrap();
    let f = Field::new(r).unwrap();
    let a = geodesic_belts::build(
        &w.surface,
        &w.crust,
        &w.tectonics,
        &w.recipe,
        &w.terrain,
        Origin::FixedBoundaryArcs,
    )
    .unwrap();
    assert_eq!(a.source_identity.as_ref().unwrap(), f.source_identity());
    for (i, &p) in w.surface.centers.iter().enumerate() {
        let s = f.sample(p).unwrap();
        assert_eq!(s.elevation_meters, a.terrain.elevation[i]);
        assert_eq!(s.baseline_meters, a.terrain.baseline[i]);
        assert_eq!(s.detail_meters, a.terrain.detail[i]);
        assert_eq!(s.convergence_meters, a.terrain.convergence[i]);
        assert_eq!(s.divergence_meters, a.terrain.divergence[i]);
    }
}
#[test]
fn sampling_is_stateless_and_does_not_depend_on_target_grid_or_camera() {
    let r = recipe();
    let a = Field::new(r.clone()).unwrap();
    let mut other = r.clone();
    other.subdivision = 6;
    let b = Field::new(other).unwrap();
    assert_eq!(a.source_identity(), b.source_identity());
    let directions = Surface::build(1, r.radius_meters).centers;
    let values: Vec<_> = directions.iter().map(|&p| a.sample(p).unwrap()).collect();
    for (i, &p) in directions.iter().enumerate().rev() {
        assert_eq!(a.sample(p).unwrap(), values[i]);
        assert_eq!(b.sample(p).unwrap(), values[i]);
    }
    for p in [[0.; 3], [f64::NAN, 0., 0.], [2., 0., 0.]] {
        assert!(a.sample(p).is_err());
    }
    assert_eq!(a.reference_recipe(), &r);
}
