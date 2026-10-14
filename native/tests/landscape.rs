use planimulation_core::{
    Recipe, Surface, World,
    drainage::Drainage,
    landscape::{self, Settings},
    moisture_transport::total_mass,
    water::{Water, WaterSettings},
};

fn world(seed: &str, model: &str) -> World {
    let recipe: Recipe = serde_json::from_value(serde_json::json!({
        "schemaVersion":1,"modelVersion":model,
        "randomVersion":"fnv1a-utf8-mulberry32-1","seed":seed,"subdivision":3,
        "radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,
        "continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,
        "boundaryWidthKm":300.,"detailAmplitudeMeters":300.,"terrainPreparationPasses":0,
        "water":{"mode":"coverage","fraction":0.71}
    }))
    .unwrap();
    World::generate(recipe).unwrap()
}

fn links(surface: &Surface, receivers: &[u32]) -> Vec<f64> {
    receivers
        .iter()
        .enumerate()
        .map(|(i, &r)| {
            if r as usize == i {
                return 0.;
            }
            let edge = (surface.offsets[i] as usize..surface.offsets[i + 1] as usize)
                .find(|&k| surface.neighbors[k] == r)
                .unwrap();
            surface.distances[edge]
        })
        .collect()
}

#[test]
fn generated_spherical_incisions_preserve_terminals_and_close_independent_ledgers() {
    for model in ["continuous-plates-1", "multiscale-geography-2"] {
        for seed in ["first-light", "4836187e", "landscape-02", "landscape-03"] {
            let w = world(seed, model);
            let original = w.terrain.elevation.clone();
            let mut bed = original.clone();
            let mut volumes = Vec::new();
            let settings = Settings {
                erodibility_per_year: 1e-5,
                step_years: 10000.,
                base_level_meters: Some(w.water.level_meters),
            };
            for _ in 0..8 {
                let graph = Drainage::build(&w.surface, &bed, &w.water.body_ids);
                let distance = links(&w.surface, &graph.receivers);
                let incision = landscape::incise(
                    &bed,
                    &w.surface.areas,
                    &graph.receivers,
                    &distance,
                    settings,
                )
                .unwrap();
                assert_eq!(
                    serde_json::to_vec(&incision).unwrap(),
                    serde_json::to_vec(
                        &landscape::incise(
                            &bed,
                            &w.surface.areas,
                            &graph.receivers,
                            &distance,
                            settings
                        )
                        .unwrap()
                    )
                    .unwrap()
                );
                for (i, &old) in bed.iter().enumerate() {
                    let new = incision.elevation_meters[i];
                    assert!(new.is_finite() && new <= old);
                    if graph.receivers[i] as usize == i || old <= w.water.level_meters {
                        assert_eq!(new, old);
                    } else {
                        assert!(new >= w.water.level_meters);
                    }
                }
                volumes.push(incision.removed_material_cubic_meters);
                bed = incision.elevation_meters;
            }
            let independent = total_mass(
                &original
                    .iter()
                    .zip(&bed)
                    .zip(&w.surface.areas)
                    .map(|((a, b), area)| (a - b) * area)
                    .collect::<Vec<_>>(),
            );
            let removed = total_mass(&volumes);
            assert!(removed > 0.);
            assert!((removed - independent).abs() / removed < 1e-12);
            let water = Water::generate(
                &w.surface,
                &bed,
                &WaterSettings::Volume {
                    volume_cubic_meters: w.water.resolved_volume_cubic_meters,
                },
            )
            .unwrap();
            assert!(
                (water.resolved_volume_cubic_meters - w.water.resolved_volume_cubic_meters).abs()
                    / w.water.resolved_volume_cubic_meters
                    < 1e-12
            );
            assert_eq!(w.terrain.elevation, original);
            assert_eq!(w.terrain.elevation, world(seed, model).terrain.elevation);
        }
    }
}

#[test]
fn generation_probe_reuses_exact_supported_geometry_and_refuses_before_allocation() {
    for level in 0..=3 {
        let a = Surface::build(level, 6371000.);
        let b = Surface::build_generation_probe(level, 6371000.).unwrap();
        assert_eq!(a.centers, b.centers);
        assert_eq!(a.faces, b.faces);
        assert_eq!(a.offsets, b.offsets);
        assert_eq!(a.neighbors, b.neighbors);
        assert_eq!(a.distances, b.distances);
        assert_eq!(a.areas, b.areas);
        assert_eq!(a.boundary_offsets, b.boundary_offsets);
        assert_eq!(a.boundaries, b.boundaries);
    }
    for (level, radius) in [
        (9, 6371000.),
        (u32::MAX, 1.),
        (1, 0.),
        (1, -1.),
        (1, f64::NAN),
        (1, f64::INFINITY),
        (1, f64::MAX),
        (1, 1e-200),
    ] {
        assert!(Surface::build_generation_probe(level, radius).is_err());
    }
    let mut unsupported = world("limits", "continuous-plates-1").recipe;
    unsupported.subdivision = 7;
    assert!(unsupported.validate().is_err());
    assert!(World::generate(unsupported).is_err());
}
