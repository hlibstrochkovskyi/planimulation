use planimulation_core::{Recipe, World, geography, hash, wire};

fn recipe(seed: &str, version: &str) -> Recipe {
    serde_json::from_value(serde_json::json!({
        "schemaVersion":1,"modelVersion":version,"randomVersion":"fnv1a-utf8-mulberry32-1",
        "seed":seed,"subdivision":3,"radiusMeters":6371000.,"plateCount":12,
        "maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,"continentalScale":1.,
        "reliefScale":1.,"boundaryWidthKm":300.,"detailAmplitudeMeters":300.,
        "terrainPreparationPasses":4,"water":{"mode":"coverage","fraction":0.71}
    }))
    .unwrap()
}
fn fingerprint(w: &World) -> u32 {
    let mut bytes = serde_json::to_vec(&w.recipe).unwrap();
    bytes.extend(wire::arrays(w));
    hash(&bytes)
}

#[test]
fn opt_in_fields_repeat_and_keep_plate_streams_and_old_worlds_intact() {
    for seed in ["first-light", "4836187e", "geography-00"] {
        let old = World::generate(recipe(seed, "continuous-plates-1")).unwrap();
        let candidate = World::generate(recipe(seed, geography::MODEL_VERSION)).unwrap();
        if seed == "first-light" {
            assert_eq!(format!("{:08x}", fingerprint(&candidate)), "0727ddc8");
        }
        let repeated = World::generate(candidate.recipe.clone()).unwrap();
        assert_eq!(wire::arrays(&candidate), wire::arrays(&repeated));
        assert_eq!(candidate.tectonics, old.tectonics);
        assert_ne!(candidate.crust.potential, old.crust.potential);
        assert_ne!(fingerprint(&candidate), fingerprint(&old));
        assert_eq!(
            wire::arrays(&old),
            wire::arrays(&World::generate(old.recipe.clone()).unwrap())
        );
        for i in 0..candidate.terrain.elevation.len() {
            let t = &candidate.terrain;
            assert!(
                (t.elevation[i]
                    - t.baseline[i]
                    - t.convergence[i]
                    - t.divergence[i]
                    - t.detail[i]
                    - t.preparation[i])
                    .abs()
                    < 1e-8
            );
            assert!(t.convergence[i] >= 0. && t.convergence[i] <= 12000.);
            assert!((-3000. ..=5000.).contains(&t.divergence[i]));
            assert!(t.detail[i].abs() <= candidate.recipe.detail_amplitude_meters);
            assert!(t.preparation[i].abs() <= 400. + 1e-8);
        }
    }
}

#[test]
fn zero_controls_do_not_invent_plate_relief_or_detail() {
    let mut r = recipe("zero", geography::MODEL_VERSION);
    r.max_plate_speed_cm_per_year = 0.;
    r.detail_amplitude_meters = 0.;
    r.terrain_preparation_passes = Some(0);
    let w = World::generate(r.clone()).unwrap();
    assert!(w.terrain.convergence.iter().all(|&v| v == 0.));
    assert!(w.terrain.divergence.iter().all(|&v| v == 0.));
    assert!(w.terrain.detail.iter().all(|&v| v == 0.));
    assert_eq!(w.terrain.baseline, w.terrain.elevation);
    r.max_plate_speed_cm_per_year = 8.;
    r.relief_scale = 0.;
    let w = World::generate(r).unwrap();
    assert_eq!(w.terrain.baseline, w.terrain.elevation);
}

#[test]
fn independent_new_crust_controls_are_monotone_and_preserve_relief_motion_streams() {
    let mut r = recipe("fraction", geography::MODEL_VERSION);
    r.subdivision = 2;
    r.terrain_preparation_passes = Some(0);
    let mut previous = vec![0.; 162];
    let mut motion = None;
    for fraction in [0., 0.1, 0.38, 0.7, 1.] {
        r.continental_fraction = fraction;
        let w = World::generate(r.clone()).unwrap();
        if let Some(ref values) = motion {
            assert_eq!(&w.tectonics.angular_velocities, values);
        } else {
            motion = Some(w.tectonics.angular_velocities.clone());
        }
        for (&a, &b) in previous.iter().zip(&w.crust.continentality) {
            assert!(b >= a);
        }
        if fraction == 0. || fraction == 1. {
            assert!(w.crust.continentality.iter().all(|&v| v == fraction));
        }
        previous = w.crust.continentality;
    }
}

#[test]
fn native_candidate_recipe_refuses_missing_or_excess_preparation_and_future_pins() {
    let r = recipe("validation", geography::MODEL_VERSION);
    r.validate().unwrap();
    let mut bad = r.clone();
    bad.terrain_preparation_passes = None;
    assert!(bad.validate().is_err());
    bad.terrain_preparation_passes = Some(17);
    assert!(bad.validate().is_err());
    bad = r.clone();
    bad.model_version = "multiscale-geography-3".into();
    assert!(bad.validate().is_err());
    bad = r;
    bad.subdivision = 7;
    assert!(World::generate(bad).is_err());
}
