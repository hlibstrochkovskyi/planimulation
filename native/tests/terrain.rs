use planimulation_core::{Recipe, Surface, World, terrain::*};

fn recipe(seed: usize, level: u32) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: "water-1".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: format!("terrain-{seed}"),
        subdivision: level,
        radius_meters: 6371000.,
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
fn controlled_boundary_and_crust_responses() {
    assert_eq!(crust_baseline(7000., 3000.), -4500.);
    assert!((crust_baseline(35000., 2800.) - 166.6666666667).abs() < 1e-8);
    assert_eq!(boundary_amplitudes(0., 0.5), [0.; 3]);
    assert_eq!(boundary_amplitudes(-0.04, 1.), [3000., 0., 0.]);
    assert_eq!(boundary_amplitudes(0.04, 0.), [0., 1250., 0.]);
    assert_eq!(boundary_amplitudes(0.04, 1.), [0., 0., 750.]);
    assert!(boundary_amplitudes(-0.08, 1.)[0] > boundary_amplitudes(-0.04, 1.)[0]);
}
#[test]
fn graph_envelope_follows_shortest_paths_and_does_not_sum_sources() {
    let s = Surface {
        centers: vec![[1., 0., 0.]; 3],
        faces: vec![],
        offsets: vec![0, 1, 3, 4],
        neighbors: vec![1, 0, 2, 1],
        distances: vec![1000.; 4],
        areas: vec![1.; 3],
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    let a = spread(&s, &[100., 0., 0.], 1000.);
    for (i, &value) in a.iter().enumerate() {
        assert!((value - 100. * (-(i as f64)).exp()).abs() < 1e-12);
    }
    let b = spread(&s, &[100., 0., 100.], 1000.);
    assert_eq!(
        a[1], b[1],
        "Two equal sources do not double the center height."
    );
    let broad = spread(&s, &[100., 0., 0.], 2000.);
    assert!(broad[2] > a[2]);
    assert_eq!(spread(&s, &[0.; 3], 1000.), vec![0.; 3]);
}
#[test]
fn elevation_ensemble_reconstructs_and_is_reproducible() {
    for level in [0, 2, 4] {
        for seed in 0..20 {
            let r = recipe(seed, level);
            let w = World::generate(r.clone()).unwrap();
            assert_eq!(w.terrain, World::generate(r).unwrap().terrain);
            let t = &w.terrain;
            for i in 0..t.elevation.len() {
                assert_eq!(
                    t.elevation[i],
                    t.baseline[i] + t.convergence[i] + t.divergence[i] + t.detail[i]
                );
                assert!(t.elevation[i].is_finite() && (-8500.0..=18167.).contains(&t.elevation[i]));
                assert!((0.0..=6000.).contains(&t.convergence[i]));
                assert!((-1500.0..=2500.).contains(&t.divergence[i]));
                assert!(t.detail[i].abs() <= 300.);
            }
        }
    }
}
#[test]
fn parameter_changes_and_diagnostic_time_preserve_independent_fields() {
    let r = recipe(42, 3);
    let w = World::generate(r.clone()).unwrap();
    let mut flat = r.clone();
    flat.relief_scale = 0.;
    flat.detail_amplitude_meters = 0.;
    let flat = World::generate(flat).unwrap();
    assert_eq!(flat.terrain.elevation, w.terrain.baseline);
    assert_eq!(flat.tectonics, w.tectonics);
    assert_eq!(flat.crust, w.crust);
    assert_eq!(flat.field, w.field);
    let mut half = r.clone();
    half.relief_scale = 0.5;
    half.detail_amplitude_meters *= 0.5;
    let half = World::generate(half).unwrap();
    for i in 0..w.terrain.elevation.len() {
        assert_eq!(half.terrain.convergence[i], w.terrain.convergence[i] * 0.5);
        assert_eq!(half.terrain.divergence[i], w.terrain.divergence[i] * 0.5);
        assert_eq!(half.terrain.detail[i], w.terrain.detail[i] * 0.5);
    }
    let mut stopped = r.clone();
    stopped.max_plate_speed_cm_per_year = 0.;
    let stopped = World::generate(stopped).unwrap();
    assert!(
        stopped
            .terrain
            .convergence
            .iter()
            .chain(&stopped.terrain.divergence)
            .all(|&v| v == 0.)
    );
    assert_eq!(stopped.terrain.detail, w.terrain.detail);
    let mut wider = r.clone();
    wider.boundary_width_km *= 2.;
    let wider = World::generate(wider).unwrap();
    assert!(
        wider
            .terrain
            .convergence
            .iter()
            .zip(&w.terrain.convergence)
            .all(|(a, b)| a >= b)
    );
    let mut scaled = r.clone();
    scaled.radius_meters *= 2.;
    scaled.boundary_width_km *= 2.;
    let scaled = World::generate(scaled).unwrap();
    assert_eq!(
        scaled.terrain, w.terrain,
        "Scaling radius and physical width together preserves the field."
    );
    let before = w.terrain.clone();
    let mut running = w;
    running.advance(100).unwrap();
    assert_eq!(before, running.terrain);
    for (relief, width, detail) in [
        (-1., 300., 300.),
        (3., 300., 300.),
        (1., 0., 300.),
        (1., 1001., 300.),
        (1., 300., -1.),
        (1., 300., 1001.),
        (f64::NAN, 300., 300.),
    ] {
        let mut invalid = r.clone();
        invalid.relief_scale = relief;
        invalid.boundary_width_km = width;
        invalid.detail_amplitude_meters = detail;
        assert!(World::generate(invalid).is_err());
    }
}
#[test]
fn detail_is_continuous_at_seam_and_poles() {
    let field = Detail::new("seam");
    for p in [[-1., 0., 0.], [0., 1., 0.], [0., -1., 0.]] {
        let mut a = p;
        let mut b = p;
        a[2] += 1e-10;
        b[2] -= 1e-10;
        assert!((field.sample(a) - field.sample(b)).abs() < 1e-8);
    }
}
