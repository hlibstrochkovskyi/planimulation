use planimulation_core::{Random, Recipe, Surface, World, hash, wire};
use std::collections::{BTreeMap, BTreeSet};
fn recipe(level: u32, seed: &str) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: "terrain-1".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: seed.into(),
        subdivision: level,
        radius_meters: 6_371_000.,
        plate_count: 12,
        max_plate_speed_cm_per_year: 8.,
        continental_fraction: 0.38,
        continental_scale: 1.,
        relief_scale: 1.,
        boundary_width_km: 300.,
        detail_amplitude_meters: 300.,
    }
}
#[test]
fn random_reference() {
    assert_eq!(hash(b"foobar"), 0xbf9cf968);
    let mut r = Random(1);
    assert_eq!(
        (0..5).map(|_| r.next_u32()).collect::<Vec<_>>(),
        vec![2693262067, 11749833, 2265367787, 4213581821, 4159151403]
    );
}
#[test]
fn strict_recipes() {
    let r = recipe(2, "earth");
    r.validate().unwrap();
    let mut json = serde_json::to_value(&r).unwrap();
    json["extra"] = serde_json::json!(1);
    assert!(serde_json::from_value::<Recipe>(json).is_err());
    for mut r in [recipe(7, "x"), recipe(1, " "), recipe(1, &"x".repeat(129))] {
        assert!(r.validate().is_err());
        r.subdivision = 1;
        r.seed = "x".into();
        r.model_version = "surface-1".into();
        assert!(r.validate().is_err());
    }
}
#[test]
fn closed_connected_sphere_all_supported_resolutions() {
    for level in 0..=6 {
        let s = Surface::build(level, 1000.);
        let n = s.centers.len();
        assert_eq!(n, 10 * 4_usize.pow(level) + 2);
        let mut pentagons = 0;
        let mut edges = BTreeMap::new();
        for i in 0..n {
            let ns = &s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize];
            if ns.len() == 5 {
                pentagons += 1;
            } else {
                assert_eq!(ns.len(), 6);
            }
            assert_eq!(
                s.boundary_offsets[i + 1] - s.boundary_offsets[i],
                (ns.len() * 2) as u32
            );
            assert!(s.areas[i] > 0.);
            assert!((s.centers[i].iter().map(|v| v * v).sum::<f64>() - 1.).abs() < 1e-14);
            for &j in ns {
                let back = &s.neighbors
                    [s.offsets[j as usize] as usize..s.offsets[j as usize + 1] as usize];
                assert!(back.contains(&(i as u32)));
            }
        }
        for t in s.faces.as_chunks::<3>().0 {
            for j in 0..3 {
                let (a, b) = (t[j], t[(j + 1) % 3]);
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        assert_eq!(pentagons, 12);
        assert!(edges.values().all(|&v| v == 2));
        assert_eq!(n + s.faces.len() / 3, edges.len() + 2);
        let mut visited = BTreeSet::from([0]);
        let mut queue = vec![0];
        let mut index = 0;
        while index < queue.len() {
            let id = queue[index];
            index += 1;
            for &next in &s.neighbors[s.offsets[id] as usize..s.offsets[id + 1] as usize] {
                if visited.insert(next as usize) {
                    queue.push(next as usize);
                }
            }
        }
        assert_eq!(visited.len(), n);
        assert!(
            (s.areas.iter().sum::<f64>() / (4. * std::f64::consts::PI * 1e6) - 1.).abs() < 1e-10
        );
    }
}
#[test]
fn ensemble_reproduction_scaling_and_conservative_transport() {
    for seed in 0..20 {
        let r = recipe(2, &format!("ensemble-{seed}"));
        let mut a = World::generate(r.clone()).unwrap();
        let mut b = World::generate(r.clone()).unwrap();
        assert_eq!(wire::arrays(&a), wire::arrays(&b));
        let mut doubled = r;
        doubled.radius_meters *= 2.;
        let scaled = World::generate(doubled).unwrap();
        assert_eq!(a.field, scaled.field);
        for (small, big) in a.surface.areas.iter().zip(&scaled.surface.areas) {
            assert_eq!(small * 4., *big);
        }
        for (small, big) in a.surface.distances.iter().zip(&scaled.surface.distances) {
            assert_eq!(small * 2., *big);
        }
        let lo = a.field.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = a.field.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        a.advance(100).unwrap();
        for _ in 0..10 {
            b.advance(10).unwrap();
        }
        assert_eq!(a.field, b.field);
        assert_eq!(a.tick, 100);
        assert!(
            a.field
                .iter()
                .all(|v| v.is_finite() && *v >= lo && *v <= hi)
        );
        assert!((a.mass() / a.initial_mass - 1.).abs() < 1e-12);
        a.field.fill(0.3);
        let constant = a.field.clone();
        a.advance(100).unwrap();
        assert_eq!(a.field, constant);
        assert!(a.advance(0).is_err());
        assert!(a.advance(101).is_err());
    }
}
