use planimulation_core::{Recipe, Surface, World, drainage::Drainage, water::WaterSettings};

fn chain(n: usize) -> Surface {
    let mut s = Surface {
        centers: vec![[1., 0., 0.]; n],
        faces: vec![],
        offsets: vec![0],
        neighbors: vec![],
        distances: vec![],
        areas: vec![1.; n],
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    for i in 0..n {
        if i > 0 {
            s.neighbors.push(i as u32 - 1);
        }
        if i + 1 < n {
            s.neighbors.push(i as u32 + 1);
        }
        s.offsets.push(s.neighbors.len() as u32);
    }
    s.distances = vec![1.; s.neighbors.len()];
    s
}
fn check(s: &Surface, h: &[f64], wet: &[u32], d: &Drainage) {
    let n = h.len();
    let mut land = 0.;
    let mut received = 0.;
    for i in 0..n {
        if wet[i] == 0 {
            land += s.areas[i];
        }
        if d.receivers[i] == i as u32 {
            received += d.contributing_area[i];
        }
        let mut id = i;
        for step in 0..=n {
            assert!(step < n, "Cycle in drainage.");
            let r = d.receivers[id] as usize;
            if r == id {
                break;
            }
            assert!(h[r] <= h[id]);
            if h[r] == h[id] {
                assert!(d.flat_steps[r] < d.flat_steps[id]);
            }
            assert!(
                s.neighbors[s.offsets[id] as usize..s.offsets[id + 1] as usize]
                    .contains(&(r as u32))
            );
            id = r;
        }
        if wet[id] == 0 {
            assert_eq!(d.outlets[i], id as u32);
        } else {
            assert_eq!(wet[d.outlets[i] as usize], wet[id]);
        }
    }
    assert!((land - received).abs() <= land * 1e-12 + 1e-6);
}

#[test]
fn slopes_and_bowls_preserve_sinks_without_filling_the_bed() {
    let s = chain(5);
    let d = Drainage::build(&s, &[4., 3., 2., 1., 0.], &[0; 5]);
    assert_eq!(d.receivers, [1, 2, 3, 4, 4]);
    assert_eq!(d.outlets, [4; 5]);
    assert_eq!(d.contributing_area, [1., 2., 3., 4., 5.]);
    let heights = [4., 1., 0., 1., 4.];
    let d = Drainage::build(&s, &heights, &[0; 5]);
    assert_eq!(d.receivers, [1, 2, 2, 2, 3]);
    assert_eq!(d.contributing_area, [1., 2., 5., 2., 1.]);
    assert_eq!(heights, [4., 1., 0., 1., 4.]);
    check(&s, &heights, &[0; 5], &d);
}

#[test]
fn flats_route_to_exits_or_one_closed_sink_with_no_epsilon_heights() {
    let s = chain(5);
    let open = Drainage::build(&s, &[3., 2., 2., 2., 1.], &[0; 5]);
    assert_eq!(open.receivers, [1, 2, 3, 4, 4]);
    assert_eq!(open.flat_steps, [0, 2, 1, 0, 0]);
    let closed = Drainage::build(&s, &[3., 1., 1., 1., 3.], &[0; 5]);
    assert_eq!(closed.outlets, [1; 5]);
    assert_eq!(closed.receivers, [1, 1, 1, 2, 3]);
    let two = Drainage::build(&s, &[0., 1., 1., 1., 0.], &[0; 5]);
    assert_eq!(two.receivers, [0, 0, 1, 4, 4]);
    assert_eq!(two.outlets, [0, 0, 0, 4, 4]);
    let flat = Drainage::build(&s, &[7.; 5], &[0; 5]);
    assert_eq!(flat.outlets, [0; 5]);
    assert_eq!(flat.flat_steps, [0, 1, 2, 3, 4]);
    check(&s, &[7.; 5], &[0; 5], &flat);
}

#[test]
fn slope_uses_distance_and_area_uses_physical_weights() {
    let mut s = chain(3);
    s.distances = vec![100., 100., 1., 1.];
    s.areas = vec![2., 3., 5.];
    let d = Drainage::build(&s, &[0., 10., 5.], &[0; 3]);
    assert_eq!(d.receivers, [0, 2, 2]);
    assert_eq!(d.contributing_area, [2., 3., 8.]);
    s.distances = vec![2., 2., 1., 1.];
    let tie = Drainage::build(&s, &[0., 10., 5.], &[0; 3]);
    assert_eq!(tie.receivers[1], 0);
    let before = tie.clone();
    for a in &mut s.areas {
        *a *= 4.;
    }
    for d in &mut s.distances {
        *d *= 2.;
    }
    let scaled = Drainage::build(&s, &[0., 10., 5.], &[0; 3]);
    assert_eq!(scaled.receivers, before.receivers);
    assert_eq!(
        scaled.contributing_area,
        before
            .contributing_area
            .iter()
            .map(|a| a * 4.)
            .collect::<Vec<_>>()
    );
}

#[test]
fn existing_water_is_terminal_and_grouped_by_body_without_receiving_its_own_area() {
    let s = chain(5);
    let h = [3., 2., -1., -2., 4.];
    let wet = [0, 0, 1, 1, 0];
    let d = Drainage::build(&s, &h, &wet);
    assert_eq!(d.receivers, [1, 2, 2, 3, 3]);
    assert_eq!(d.outlets, [2; 5]);
    assert_eq!(d.contributing_area, [1., 2., 2., 1., 1.]);
    check(&s, &h, &wet, &d);
    let full = Drainage::build(&s, &h, &[1; 5]);
    assert_eq!(full.contributing_area, [0.; 5]);
    assert_eq!(full.outlets, [0; 5]);
}

fn recipe(seed: usize, level: u32, fraction: f64) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: "basins-1".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: format!("drainage-{seed}"),
        subdivision: level,
        radius_meters: 6371000.,
        plate_count: 12,
        max_plate_speed_cm_per_year: 8.,
        continental_fraction: 0.38,
        continental_scale: 1.,
        relief_scale: 1.,
        boundary_width_km: 300.,
        detail_amplitude_meters: 300.,
        water: WaterSettings::Coverage { fraction },
    }
}
#[test]
fn ensemble_is_acyclic_reproducible_and_accounts_for_every_dry_region() {
    for level in [0, 2, 4] {
        for seed in 0..20 {
            for fraction in [0., 0.71, 1.] {
                let r = recipe(seed, level, fraction);
                let mut w = World::generate(r.clone()).unwrap();
                assert_eq!(w.drainage, World::generate(r).unwrap().drainage);
                check(
                    &w.surface,
                    &w.terrain.elevation,
                    &w.water.body_ids,
                    &w.drainage,
                );
                let original = w.drainage.clone();
                let bed = w.terrain.clone();
                let water = w.water.clone();
                w.advance(1).unwrap();
                assert_eq!(w.drainage, original);
                assert_eq!(w.terrain, bed);
                assert_eq!(w.water, water);
            }
        }
    }
}

#[test]
fn finest_uniform_dry_sphere_routes_without_recursion_or_artificial_outlets() {
    let s = Surface::build(6, 6371000.);
    let n = s.areas.len();
    let d = Drainage::build(&s, &vec![0.; n], &vec![0; n]);
    assert!(d.outlets.iter().all(|&id| id == 0));
    assert_eq!(
        d.receivers
            .iter()
            .enumerate()
            .filter(|&(i, &r)| i == r as usize)
            .count(),
        1
    );
    for i in 1..n {
        assert_eq!(d.flat_steps[i], d.flat_steps[d.receivers[i] as usize] + 1);
    }
    let total: f64 = s.areas.iter().sum();
    assert!((d.contributing_area[0] / total - 1.).abs() < 1e-12);
}
