use planimulation_core::{Recipe, Surface, World, crust::*};

fn recipe() -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: "drainage-1".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: "crust-test".into(),
        subdivision: 3,
        radius_meters: 6_371_000.,
        plate_count: 12,
        max_plate_speed_cm_per_year: 8.,
        continental_fraction: 0.38,
        continental_scale: 1.,
        relief_scale: 1.,
        boundary_width_km: 300.,
        detail_amplitude_meters: 300.,
        water: planimulation_core::water::WaterSettings::Coverage { fraction: 0.71 },
    }
}

#[test]
fn area_fitting_uses_physical_weights_and_keeps_ties_together() {
    let values = [0.3, 0.2, 0.1];
    let areas = [1., 8., 1.];
    let threshold = fit_threshold(&values, &areas, 0.4);
    assert_eq!(values.iter().filter(|&&v| v > threshold).count(), 1);
    assert!(fit_threshold(&values, &areas, 0.) > 0.3);
    assert!(fit_threshold(&values, &areas, 1.) < 0.1);
    assert!(continentality(0.3, threshold) > 0.5);
    // A plateau cannot be split without inventing a spatial discontinuity.
    assert!(fit_threshold(&[0.2; 3], &[1.; 3], 0.5) > 0.2);
    assert!(fit_threshold(&[0.2; 3], &[1.; 3], 0.6) < 0.2);
    assert_eq!(continentality(0., 0.), 0.5);
    assert_eq!(continentality(-1., 0.), 0.);
    assert_eq!(continentality(1., 0.), 1.);
}

#[test]
fn ensemble_is_bounded_reproducible_and_area_targets_are_monotone() {
    for level in [0, 2, 4] {
        let s = Surface::build(level, 6_371_000.);
        let total: f64 = s.areas.iter().sum();
        let tolerance = s.areas.iter().copied().fold(0., f64::max) / total + 1e-12;
        for seed in 0..20 {
            for scale in [0.5, 1., 2.] {
                let name = format!("crust-ensemble-{seed}");
                let mut previous = vec![0.; s.centers.len()];
                for target in [0., 0.1, 0.38, 0.7, 1.] {
                    let c = Crust::build(&s, &name, target, scale);
                    assert_eq!(c, Crust::build(&s, &name, target, scale));
                    let mut covered = 0.;
                    for (id, &v) in c.continentality.iter().enumerate() {
                        assert!(v.is_finite() && (0.0..=1.).contains(&v));
                        assert!(v >= previous[id]);
                        assert!(c.potential[id].abs() <= 1.);
                        assert_eq!(v, continentality(c.potential[id], c.threshold));
                        assert_eq!(c.thickness_meters[id], 7000. + 28000. * v);
                        assert_eq!(c.density_kg_per_cubic_meter[id], 3000. - 200. * v);
                        if v > 0.5 {
                            covered += s.areas[id];
                        }
                    }
                    assert!(
                        (covered / total - target).abs() <= tolerance,
                        "seed={seed} level={level} scale={scale} target={target}"
                    );
                    if target == 0. || target == 1. {
                        assert!(c.continentality.iter().all(|&v| v == target));
                    }
                    previous = c.continentality;
                }
            }
        }
    }
}

#[test]
fn spherical_field_is_continuous_and_shared_directions_survive_refinement() {
    let field = Potential::new("seam", 0.5);
    assert!((field.sample([-1., 0., 1e-10]) - field.sample([-1., 0., -1e-10])).abs() < 1e-8);
    for y in [-1., 1.] {
        assert!((field.sample([1e-10, y, 0.]) - field.sample([-1e-10, y, 0.])).abs() < 1e-8);
    }
    let coarse = Surface::build(2, 1000.);
    let fine = Surface::build(3, 1000.);
    let a = Crust::build(&coarse, "refinement", 0.38, 1.);
    let b = Crust::build(&fine, "refinement", 0.38, 1.);
    assert_eq!(a.potential, b.potential[..a.potential.len()]);
    assert_ne!(
        a.potential,
        Crust::build(&coarse, "different", 0.38, 1.).potential
    );
    assert_ne!(
        a.potential,
        Crust::build(&coarse, "refinement", 0.38, 2.).potential
    );
}

#[test]
fn crust_parameters_are_independent_of_plates_radius_and_diagnostic_time() {
    let r = recipe();
    let mut w = World::generate(r.clone()).unwrap();
    for (count, speed, radius) in [(2, 0., r.radius_meters), (24, 20., r.radius_meters * 2.)] {
        let mut changed = r.clone();
        changed.plate_count = count;
        changed.max_plate_speed_cm_per_year = speed;
        changed.radius_meters = radius;
        assert_eq!(w.crust, World::generate(changed).unwrap().crust);
    }
    let mut changed = r.clone();
    changed.continental_fraction = 0.7;
    changed.continental_scale = 2.;
    let changed = World::generate(changed).unwrap();
    assert_eq!(w.tectonics, changed.tectonics);
    assert_eq!(w.field, changed.field);
    let mut mixed_plate = false;
    for plate in 0..r.plate_count {
        let values: Vec<_> = w
            .tectonics
            .owners
            .iter()
            .enumerate()
            .filter(|(_, p)| **p == plate)
            .map(|(id, _)| w.crust.continentality[id])
            .collect();
        mixed_plate |= values.iter().any(|&v| v < 0.5) && values.iter().any(|&v| v > 0.5);
    }
    assert!(mixed_plate, "Crust must not simply label entire plates.");
    let original = w.crust.clone();
    w.advance(100).unwrap();
    assert_eq!(original, w.crust);
    for (fraction, scale) in [
        (-0.1, 1.),
        (1.1, 1.),
        (f64::NAN, 1.),
        (0.38, 0.4),
        (0.38, 2.1),
        (0.38, f64::INFINITY),
    ] {
        let mut bad = r.clone();
        bad.continental_fraction = fraction;
        bad.continental_scale = scale;
        assert!(World::generate(bad).is_err());
    }
}
