use planimulation_core::{
    Recipe, Surface, World,
    water::{Water, WaterSettings},
};

fn chain(areas: &[f64]) -> Surface {
    let n = areas.len();
    let mut offsets = vec![0];
    let mut neighbors = Vec::new();
    for id in 0..n {
        if id > 0 {
            neighbors.push(id as u32 - 1);
        }
        if id + 1 < n {
            neighbors.push(id as u32 + 1);
        }
        offsets.push(neighbors.len() as u32);
    }
    Surface {
        centers: vec![[1., 0., 0.]; n],
        faces: vec![],
        offsets,
        neighbors,
        distances: vec![],
        areas: areas.to_vec(),
        boundary_offsets: vec![],
        boundaries: vec![],
    }
}
fn coverage(fraction: f64) -> WaterSettings {
    WaterSettings::Coverage { fraction }
}
fn volume(volume_cubic_meters: f64) -> WaterSettings {
    WaterSettings::Volume {
        volume_cubic_meters,
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= (b.abs() * 1e-10).max(1e-9), "{a} != {b}");
}

#[test]
fn weighted_basins_have_separate_ids_and_largest_area_ocean() {
    let s = chain(&[2., 1., 4., 1., 2.]);
    let h = [0., 10., 0., 10., 0.];
    let water = Water::generate(&s, &h, &volume(40.)).unwrap();
    assert_eq!(water.level_meters, 5.);
    assert_eq!(water.depth_meters, [5., 0., 5., 0., 5.]);
    assert_eq!(water.body_ids, [1, 0, 2, 0, 3]);
    assert_eq!(water.main_ocean_id, 2);
    assert_eq!(water.resolved_volume_cubic_meters, 40.);
    let filled = Water::generate(&s, &h, &volume(90.)).unwrap();
    assert_eq!(filled.level_meters, 11.);
    assert_eq!(filled.body_ids, [1; 5]);
    // Exactly at a sill, zero-depth regions remain dry: no premature connection.
    let sill = Water::generate(&s, &h, &volume(80.)).unwrap();
    assert_eq!(sill.body_ids, water.body_ids);
    let tie = Water::generate(&chain(&[1.; 3]), &[0., 10., 0.], &volume(2.)).unwrap();
    assert_eq!(tie.main_ocean_id, 1);
}

#[test]
fn coverage_is_area_weighted_nearest_prefix_and_never_splits_plateaus() {
    let s = chain(&[1., 8., 1.]);
    let h = [0., 1., 2.];
    let low = Water::generate(&s, &h, &coverage(0.4)).unwrap();
    assert_eq!(low.depth_meters, [0.5, 0., 0.]);
    let high = Water::generate(&s, &h, &coverage(0.6)).unwrap();
    assert_eq!(high.depth_meters, [1.5, 0.5, 0.]);
    let tie = Water::generate(&s, &h, &coverage(0.5)).unwrap();
    assert_eq!(tie, low);
    for fraction in [0., 0.49, 0.5, 0.51, 1.] {
        let water = Water::generate(&s, &[7.; 3], &coverage(fraction)).unwrap();
        assert_eq!(
            water.depth_meters,
            if fraction <= 0.5 { [0.; 3] } else { [1.; 3] }
        );
    }
    let dry = Water::generate(&s, &h, &coverage(0.)).unwrap();
    assert_eq!(dry.resolved_volume_cubic_meters, 0.);
    assert_eq!(dry.main_ocean_id, 0);
    let full = Water::generate(&s, &h, &coverage(1.)).unwrap();
    assert_eq!(full.level_meters, 3.);
    assert!(full.depth_meters.iter().all(|d| *d > 0.));
}

#[test]
fn volume_fit_preserves_stock_under_bed_changes_and_has_consistent_units() {
    let s = chain(&[2., 3., 4., 5.]);
    for requested in [0., 0.001, 1., 10., 100., 1000.] {
        for h in [
            [-3., 2., -1., 6.],
            [10., 10., 10., 10.],
            [-5., 3., 2., -10.],
        ] {
            let w = Water::generate(&s, &h, &volume(requested)).unwrap();
            close(w.resolved_volume_cubic_meters, requested);
            let shifted = Water::generate(&s, &h.map(|h| h + 500.), &volume(requested)).unwrap();
            close(shifted.level_meters, w.level_meters + 500.);
            for (a, b) in shifted.depth_meters.iter().zip(&w.depth_meters) {
                close(*a, *b);
            }
            let scaled =
                Water::generate(&chain(&[8., 12., 16., 20.]), &h, &volume(requested * 4.)).unwrap();
            close(w.level_meters, scaled.level_meters);
        }
    }
    assert!(Water::generate(&s, &[10.; 4], &volume(1e-8)).is_ok());
    // A requested stock below resolvable depth cannot silently disappear.
    assert!(Water::generate(&chain(&[1e14; 4]), &[-4000.; 4], &volume(1.)).is_err());
}

fn recipe(seed: usize, subdivision: u32) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: "basins-1".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: format!("water-{seed}"),
        subdivision,
        radius_meters: 6371000.,
        plate_count: 12,
        max_plate_speed_cm_per_year: 8.,
        continental_fraction: 0.38,
        continental_scale: 1.,
        relief_scale: 1.,
        boundary_width_km: 300.,
        detail_amplitude_meters: 300.,
        water: coverage(0.71),
    }
}

#[test]
fn water_ensemble_reproduces_conserves_and_leaves_prior_layers_unchanged() {
    for level in [0, 2, 4] {
        for seed in 0..20 {
            let mut r = recipe(seed, level);
            let base = World::generate(r.clone()).unwrap();
            for settings in [
                coverage(0.),
                coverage(0.3),
                coverage(0.71),
                coverage(1.),
                volume(0.),
                volume(1e18),
            ] {
                r.water = settings;
                let w = World::generate(r.clone()).unwrap();
                assert_eq!(w.water, World::generate(r.clone()).unwrap().water);
                assert_eq!(w.terrain, base.terrain);
                assert_eq!(w.crust, base.crust);
                assert_eq!(w.tectonics, base.tectonics);
                assert_eq!(w.field, base.field);
                let water = &w.water;
                close(
                    water.resolved_volume_cubic_meters,
                    water
                        .depth_meters
                        .iter()
                        .zip(&w.surface.areas)
                        .map(|(d, a)| d * a)
                        .sum(),
                );
                let mut wet_area = 0.;
                for i in 0..water.body_ids.len() {
                    assert!(water.depth_meters[i].is_finite() && water.depth_meters[i] >= 0.);
                    assert_eq!(water.body_ids[i] != 0, water.depth_meters[i] > 0.);
                    if water.body_ids[i] == 0 {
                        continue;
                    }
                    wet_area += w.surface.areas[i];
                    for k in w.surface.offsets[i]..w.surface.offsets[i + 1] {
                        let other = water.body_ids[w.surface.neighbors[k as usize] as usize];
                        assert!(other == 0 || other == water.body_ids[i]);
                    }
                }
                match r.water {
                    WaterSettings::Coverage { fraction } => {
                        let total: f64 = w.surface.areas.iter().sum();
                        let max_area = w.surface.areas.iter().copied().fold(0., f64::max);
                        assert!((wet_area / total - fraction).abs() <= max_area / total + 1e-12);
                        let fitted = Water::generate(
                            &w.surface,
                            &w.terrain.elevation,
                            &volume(water.resolved_volume_cubic_meters),
                        )
                        .unwrap();
                        close(fitted.level_meters, water.level_meters);
                        assert_eq!(fitted.body_ids, water.body_ids);
                    }
                    WaterSettings::Volume {
                        volume_cubic_meters,
                    } => close(water.resolved_volume_cubic_meters, volume_cubic_meters),
                }
                let before = w.water.clone();
                let mut running = w;
                running.advance(1).unwrap();
                assert_eq!(before, running.water);
            }
        }
    }
}

#[test]
fn settings_are_strict_and_extreme_supported_radii_fit() {
    let mut r = recipe(1, 2);
    for invalid in [
        coverage(-0.1),
        coverage(1.1),
        coverage(f64::NAN),
        volume(-1.),
        volume(f64::INFINITY),
        volume(1e30),
    ] {
        r.water = invalid;
        assert!(World::generate(r.clone()).is_err());
    }
    for radius in [100_000., 20_000_000.] {
        r.radius_meters = radius;
        r.water = volume(4. * std::f64::consts::PI * radius * radius * 20_000.);
        let w = World::generate(r.clone()).unwrap();
        assert!(w.water.depth_meters.iter().all(|d| *d > 0.));
    }
    for invalid in [
        r#"{"mode":"coverage","fraction":0.5,"volumeCubicMeters":10}"#,
        r#"{"mode":"volume","fraction":0.5}"#,
        r#"{"mode":"other","fraction":0.5}"#,
    ] {
        assert!(serde_json::from_str::<WaterSettings>(invalid).is_err());
    }
}
