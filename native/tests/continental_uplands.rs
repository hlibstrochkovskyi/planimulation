use planimulation_core::{Recipe, World, continental_uplands, geography, hash, terrain, wire};

fn recipe(version: &str) -> Recipe {
    serde_json::from_value(serde_json::json!({"schemaVersion":1,"modelVersion":version,
        "randomVersion":"fnv1a-utf8-mulberry32-1","seed":"first-light","subdivision":3,
        "radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,
        "continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,
        "boundaryWidthKm":300.,"detailAmplitudeMeters":300.,"terrainPreparationPasses":0,
        "water":{"mode":"coverage","fraction":0.71}}))
    .unwrap()
}
#[test]
fn matched_structure_changes_only_initial_columns_and_their_baseline_before_preparation() {
    let old = World::generate(recipe(geography::MODEL_VERSION)).unwrap();
    let new = World::generate(recipe(continental_uplands::MODEL_VERSION)).unwrap();
    assert_eq!(old.tectonics, new.tectonics);
    assert_eq!(old.crust.potential, new.crust.potential);
    assert_eq!(old.crust.continentality, new.crust.continentality);
    assert_eq!(old.crust.threshold, new.crust.threshold);
    assert_eq!(
        old.crust.density_kg_per_cubic_meter,
        new.crust.density_kg_per_cubic_meter
    );
    assert_eq!(old.terrain.convergence, new.terrain.convergence);
    assert_eq!(old.terrain.divergence, new.terrain.divergence);
    assert_eq!(old.terrain.detail, new.terrain.detail);
    assert!(new.terrain.baseline.iter().any(|&v| v > 167.));
    for i in 0..new.surface.centers.len() {
        assert_eq!(
            new.terrain.baseline[i],
            terrain::crust_baseline(
                new.crust.thickness_meters[i],
                new.crust.density_kg_per_cubic_meter[i]
            )
        );
        assert!((7000. ..=50000.).contains(&new.crust.thickness_meters[i]));
        assert!(
            (new.terrain.elevation[i]
                - old.terrain.elevation[i]
                - (new.terrain.baseline[i] - old.terrain.baseline[i]))
                .abs()
                < 1e-8
        );
    }
}
#[test]
fn repeat_and_older_version_pins_survive_candidate_generation() {
    let mut r = recipe(geography::MODEL_VERSION);
    r.terrain_preparation_passes = Some(4);
    let old = World::generate(r.clone()).unwrap();
    let mut bytes = serde_json::to_vec(&r).unwrap();
    bytes.extend(wire::arrays(&old));
    assert_eq!(format!("{:08x}", hash(&bytes)), "0727ddc8");
    r.model_version = continental_uplands::MODEL_VERSION.into();
    let a = World::generate(r.clone()).unwrap();
    let mut pin = serde_json::to_vec(&r).unwrap();
    pin.extend(wire::arrays(&a));
    assert_eq!(format!("{:08x}", hash(&pin)), "d4d48032");
    let b = World::generate(r).unwrap();
    assert_eq!(wire::arrays(&a), wire::arrays(&b));
    assert_eq!(
        wire::arrays(&old),
        wire::arrays(&World::generate(old.recipe.clone()).unwrap())
    );
}
#[test]
fn extreme_material_and_water_inputs_keep_explicit_version_and_finite_fields() {
    for fraction in [0., 1.] {
        for water in [0., 1.] {
            let mut r = recipe(continental_uplands::MODEL_VERSION);
            r.subdivision = 2;
            r.continental_fraction = fraction;
            r.water = planimulation_core::water::WaterSettings::Coverage { fraction: water };
            let w = World::generate(r).unwrap();
            assert!(w.crust.continentality.iter().all(|&c| c == fraction));
            assert!(w.terrain.elevation.iter().all(|h| h.is_finite()));
            if fraction == 0. {
                assert!(w.crust.thickness_meters.iter().all(|&t| t == 7000.));
            }
            assert_eq!(w.water.depth_meters.iter().all(|&d| d > 0.), water == 1.);
        }
    }
    let mut r = recipe(continental_uplands::MODEL_VERSION);
    r.terrain_preparation_passes = None;
    assert!(World::generate(r).is_err());
}
