use planimulation_core::{
    Recipe, Surface, World,
    drainage::Drainage,
    landscape,
    sediment_delivery::{Delivery, deliver, deliver_volumes},
    water::{Water, WaterSettings},
};

fn graph(areas: &[f64], edges: &[(usize, usize)]) -> Surface {
    let n = areas.len();
    let mut surface = Surface {
        centers: vec![[1., 0., 0.]; n],
        faces: vec![],
        offsets: vec![0],
        neighbors: vec![],
        distances: vec![],
        areas: areas.to_vec(),
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    for i in 0..n {
        let mut neighbors: Vec<_> = edges
            .iter()
            .filter_map(|&(a, b)| {
                if a == i {
                    Some(b as u32)
                } else if b == i {
                    Some(a as u32)
                } else {
                    None
                }
            })
            .collect();
        neighbors.sort_unstable();
        surface.neighbors.extend(neighbors);
        surface.offsets.push(surface.neighbors.len() as u32);
    }
    surface.distances.resize(surface.neighbors.len(), 1.);
    surface
}
fn dry(surface: &Surface, bed: &[f64]) -> Water {
    Water::generate(surface, bed, &WaterSettings::Coverage { fraction: 0. }).unwrap()
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= b.abs().max(1e-300) * 1e-11, "{a} != {b}");
}
fn independent_walk(surface: &Surface, receivers: &[u32], thickness: &[f64], result: &Delivery) {
    let n = receivers.len();
    let mut through = vec![0.; n];
    let mut terminals = vec![0.; n];
    let mut total = 0.;
    for (i, &thickness) in thickness.iter().enumerate() {
        let material = thickness * surface.areas[i];
        total += material;
        let mut cursor = i;
        for hop in 0..n {
            through[cursor] += material;
            if receivers[cursor] as usize == cursor {
                terminals[cursor] += material;
                break;
            }
            assert!(hop + 1 < n, "Independent walk found a cycle");
            cursor = receivers[cursor] as usize;
        }
    }
    close(total, result.source_volume_cubic_meters);
    for i in 0..n {
        close(through[i], result.through_volume_cubic_meters[i]);
        close(terminals[i], result.terminal_volume_cubic_meters[i]);
    }
}

#[test]
fn weighted_branches_deliver_actual_prisms_not_thickness_or_contributing_area() {
    let s = graph(&[2., 3., 5., 7., 11.], &[(0, 2), (1, 2), (2, 3), (3, 4)]);
    let bed = [8., 9., 5., 2., 0.];
    let water = dry(&s, &bed);
    let receivers = [2, 2, 3, 4, 4];
    let thickness = [1., 2., 3., 4., 0.];
    let p = deliver(&s, &bed, &water, &receivers, &thickness).unwrap();
    assert_eq!(p.through_volume_cubic_meters, [2., 6., 23., 51., 51.]);
    assert_eq!(p.terminal_volume_cubic_meters, [0., 0., 0., 0., 51.]);
    assert_eq!(p.source_volume_cubic_meters, 51.);
    assert_eq!(p.dry_terminal_volume_cubic_meters, 51.);
    assert_eq!(p.main_ocean_volume_cubic_meters, 0.);
    assert_eq!(p.other_initial_wet_body_volume_cubic_meters, 0.);
    assert!(p.wet_bodies.is_empty());
    independent_walk(&s, &receivers, &thickness, &p);
}

#[test]
fn main_ocean_and_inland_wet_terminals_are_separate_and_do_not_route_underwater() {
    let s = graph(
        &[4., 6., 1., 1., 1., 2., 3.],
        &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6)],
    );
    let bed = [-2., -1., 2., 5., 2., -1., -2.];
    let water = Water::generate(
        &s,
        &bed,
        &WaterSettings::Volume {
            volume_cubic_meters: 22.,
        },
    )
    .unwrap();
    assert_eq!(water.level_meters, 0.);
    assert_eq!(water.body_ids, [1, 1, 0, 0, 0, 2, 2]);
    assert_eq!(water.main_ocean_id, 1);
    let receivers = [0, 1, 1, 2, 5, 5, 6];
    let thickness = [0., 0., 2., 3., 7., 0., 0.];
    let p = deliver(&s, &bed, &water, &receivers, &thickness).unwrap();
    assert_eq!(p.source_volume_cubic_meters, 12.);
    assert_eq!(p.main_ocean_volume_cubic_meters, 5.);
    assert_eq!(p.other_initial_wet_body_volume_cubic_meters, 7.);
    assert_eq!(p.dry_terminal_volume_cubic_meters, 0.);
    assert_eq!(p.wet_bodies[0].volume_cubic_meters, 5.);
    assert_eq!(p.wet_bodies[1].volume_cubic_meters, 7.);
    assert_eq!(p.terminal_volume_cubic_meters, [0., 5., 0., 0., 0., 7., 0.]);
    independent_walk(&s, &receivers, &thickness, &p);
}

#[test]
fn multiple_dry_sinks_and_equal_height_dag_retain_supply_without_bed_change() {
    let s = graph(&[1., 2., 4., 8.], &[(0, 1), (1, 2), (2, 3)]);
    let bed = [0.; 4];
    let water = dry(&s, &bed);
    let receivers = [0, 0, 3, 3];
    let thickness = [1., 2., 3., 4.];
    let p = deliver(&s, &bed, &water, &receivers, &thickness).unwrap();
    assert_eq!(p.terminal_volume_cubic_meters, [5., 0., 0., 44.]);
    assert_eq!(p.dry_terminal_volume_cubic_meters, 49.);
    assert_eq!(bed, [0.; 4]);
    independent_walk(&s, &receivers, &thickness, &p);
}

#[test]
fn zero_source_and_full_water_preserve_zero_delivery_with_present_body_entry() {
    let s = graph(&[1.; 3], &[(0, 1), (1, 2)]);
    let bed = [0.; 3];
    let water = Water::generate(&s, &bed, &WaterSettings::Coverage { fraction: 1. }).unwrap();
    let p = deliver(&s, &bed, &water, &[0, 1, 2], &[0.; 3]).unwrap();
    assert_eq!(p.through_volume_cubic_meters, [0.; 3]);
    assert_eq!(p.terminal_volume_cubic_meters, [0.; 3]);
    assert_eq!(p.source_volume_cubic_meters, 0.);
    assert_eq!(p.relative_volume_ledger_residual, 0.);
    assert_eq!(p.wet_bodies.len(), 1);
    assert_eq!(p.wet_bodies[0].volume_cubic_meters, 0.);
}

#[test]
fn cycles_uphill_nonadjacent_and_out_of_range_receivers_refuse_even_zero_source() {
    let s = graph(&[1.; 3], &[(0, 1), (1, 2)]);
    let flat = [0.; 3];
    let water = dry(&s, &flat);
    for receivers in [[1, 0, 2], [2, 1, 2], [3, 1, 2]] {
        assert!(deliver(&s, &flat, &water, &receivers, &[0.; 3]).is_err());
    }
    let bed = [0., 1., 2.];
    assert!(deliver(&s, &bed, &dry(&s, &bed), &[1, 1, 2], &[0.; 3]).is_err());
}

#[test]
fn wet_motion_sources_bad_water_and_disconnected_surfaces_refuse() {
    let s = graph(&[1.; 3], &[(0, 1), (1, 2)]);
    let bed = [-2., -1., 2.];
    let water = Water::generate(
        &s,
        &bed,
        &WaterSettings::Volume {
            volume_cubic_meters: 3.,
        },
    )
    .unwrap();
    assert!(deliver(&s, &bed, &water, &[0, 0, 1], &[0.; 3]).is_err());
    assert!(deliver(&s, &bed, &water, &[0, 1, 1], &[1., 0., 0.]).is_err());
    let mut bad_water = water.clone();
    bad_water.depth_meters[0] += 1.;
    assert!(deliver(&s, &bed, &bad_water, &[0, 1, 1], &[0.; 3]).is_err());
    let disconnected = graph(&[1.; 3], &[(0, 1)]);
    let flat = [0.; 3];
    assert!(
        deliver(
            &disconnected,
            &flat,
            &dry(&disconnected, &flat),
            &[0, 0, 2],
            &[0.; 3]
        )
        .is_err()
    );
}

#[test]
fn invalid_shapes_nonfinite_sources_product_underflow_and_overflow_refuse() {
    let s = graph(&[1.; 2], &[(0, 1)]);
    let bed = [1., 0.];
    let water = dry(&s, &bed);
    for thickness in [
        vec![0.],
        vec![-1., 0.],
        vec![f64::NAN, 0.],
        vec![f64::INFINITY, 0.],
    ] {
        assert!(deliver(&s, &bed, &water, &[1, 1], &thickness).is_err());
    }
    assert!(deliver(&s, &bed, &water, &[1], &[0.; 2]).is_err());
    for area in [1e-300, f64::MAX] {
        let altered = graph(&[area, 1.], &[(0, 1)]);
        let thickness = if area < 1. { 1e-300 } else { 2. };
        assert!(
            deliver(
                &altered,
                &bed,
                &dry(&altered, &bed),
                &[1, 1],
                &[thickness, 0.]
            )
            .is_err()
        );
    }
    assert!(deliver(&s, &bed, &water, &[1, 1], &[f64::MAX; 2]).is_err());
    assert_eq!(bed, [1., 0.]);
    assert_eq!(water, dry(&s, &bed));
}

#[test]
fn compensated_branch_sum_retains_small_sources_and_exact_repeats() {
    let s = graph(&[1.; 5], &[(0, 4), (1, 4), (2, 4), (3, 4)]);
    let bed = [1., 1., 1., 1., 0.];
    let water = dry(&s, &bed);
    let thickness = [1e16, 1., 1., 1., 0.];
    let p = deliver(&s, &bed, &water, &[4, 4, 4, 4, 4], &thickness).unwrap();
    assert_eq!(p.source_volume_cubic_meters, 1e16 + 4.);
    assert_eq!(p.dry_terminal_volume_cubic_meters, 1e16 + 4.);
    assert_eq!(
        p,
        deliver(&s, &bed, &water, &[4, 4, 4, 4, 4], &thickness).unwrap()
    );
}

#[test]
fn explicit_pending_volumes_follow_changed_downhill_links_without_thickness_round_trip() {
    let s = graph(&[3., 7., 11.], &[(0, 1), (1, 2)]);
    let former_bed = [3., 1., 4.];
    let water = dry(&s, &former_bed);
    let inventory = [0., 0.1, 0.];
    let retained = deliver_volumes(&s, &former_bed, &water, &[1, 1, 1], &inventory).unwrap();
    assert_eq!(retained.terminal_volume_cubic_meters, inventory);
    let new_bed = [3., 5., 4.];
    let rerouted = deliver_volumes(&s, &new_bed, &water, &[0, 0, 2], &inventory).unwrap();
    assert_eq!(rerouted.source_volume_cubic_meters, 0.1);
    assert_eq!(rerouted.terminal_volume_cubic_meters, [0.1, 0., 0.]);
    assert_eq!(inventory, [0., 0.1, 0.]);
    // Volume mode must not invent a source when the inventory is exactly zero.
    assert_eq!(
        deliver_volumes(&s, &new_bed, &water, &[0, 0, 2], &[0.; 3])
            .unwrap()
            .source_volume_cubic_meters,
        0.
    );
    assert!(deliver_volumes(&s, &new_bed, &water, &[0, 0, 2], &[f64::NAN; 3]).is_err());
    assert!(deliver_volumes(&s, &new_bed, &water, &[0, 0, 2], &[0.; 2]).is_err());
}

#[test]
fn generated_sphere_accepted_incision_has_independent_paths_and_no_input_mutation() {
    for seed in ["first-light", "sediment-a", "sediment-b"] {
        let recipe: Recipe = serde_json::from_value(serde_json::json!({
            "schemaVersion":1, "modelVersion":"multiscale-geography-2",
            "randomVersion":"fnv1a-utf8-mulberry32-1", "seed":seed,
            "subdivision":2, "radiusMeters":6371000., "plateCount":12,
            "maxPlateSpeedCmPerYear":8., "continentalFraction":0.4,
            "continentalScale":1., "reliefScale":1., "boundaryWidthKm":300.,
            "detailAmplitudeMeters":180., "terrainPreparationPasses":0,
            "water":{"mode":"coverage", "fraction":0.71}
        }))
        .unwrap();
        let world = World::generate(recipe).unwrap();
        let bed = &world.terrain.elevation;
        let drainage = Drainage::build(&world.surface, bed, &world.water.body_ids);
        let links: Vec<_> = drainage
            .receivers
            .iter()
            .enumerate()
            .map(|(i, &r)| {
                if r as usize == i {
                    return 0.;
                }
                let edge = (world.surface.offsets[i] as usize
                    ..world.surface.offsets[i + 1] as usize)
                    .find(|&k| world.surface.neighbors[k] == r)
                    .unwrap();
                world.surface.distances[edge]
            })
            .collect();
        let incision = landscape::incise(
            bed,
            &world.surface.areas,
            &drainage.receivers,
            &links,
            landscape::Settings {
                erodibility_per_year: 1e-6,
                step_years: 25000.,
                base_level_meters: Some(world.water.level_meters),
            },
        )
        .unwrap();
        let before = (bed.clone(), world.water.clone(), drainage.receivers.clone());
        let p = deliver(
            &world.surface,
            bed,
            &world.water,
            &drainage.receivers,
            &incision.removed_thickness_meters,
        )
        .unwrap();
        independent_walk(
            &world.surface,
            &drainage.receivers,
            &incision.removed_thickness_meters,
            &p,
        );
        close(
            p.source_volume_cubic_meters,
            incision.removed_material_cubic_meters,
        );
        assert_eq!(
            p,
            deliver(
                &world.surface,
                bed,
                &world.water,
                &drainage.receivers,
                &incision.removed_thickness_meters
            )
            .unwrap()
        );
        assert_eq!(
            before,
            (bed.clone(), world.water.clone(), drainage.receivers.clone())
        );
    }
}

#[test]
fn long_flat_dag_routes_iteratively_with_linear_storage() {
    let n = 100_000;
    let mut surface = Surface {
        centers: vec![],
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
            surface.neighbors.push((i - 1) as u32);
        }
        if i + 1 < n {
            surface.neighbors.push((i + 1) as u32);
        }
        surface.offsets.push(surface.neighbors.len() as u32);
    }
    surface.distances.resize(surface.neighbors.len(), 1.);
    let bed = vec![0.; n];
    let water = dry(&surface, &bed);
    let receivers: Vec<_> = (0..n).map(|i| (i + 1).min(n - 1) as u32).collect();
    let p = deliver_volumes(&surface, &bed, &water, &receivers, &vec![1.; n]).unwrap();
    assert_eq!(p.source_volume_cubic_meters, n as f64);
    assert_eq!(p.dry_terminal_volume_cubic_meters, n as f64);
    assert_eq!(p.terminal_volume_cubic_meters[n - 1], n as f64);
    assert!(
        p.through_volume_cubic_meters
            .iter()
            .enumerate()
            .all(|(i, &v)| v == (i + 1) as f64)
    );
}

#[test]
fn singleton_subnormal_stock_is_retained_and_oversized_surface_refuses() {
    let surface = graph(&[1.], &[]);
    let bed = [0.];
    let water = dry(&surface, &bed);
    let tiny = f64::from_bits(1);
    let p = deliver_volumes(&surface, &bed, &water, &[0], &[tiny]).unwrap();
    assert_eq!(p.source_volume_cubic_meters, tiny);
    assert_eq!(p.dry_terminal_volume_cubic_meters, tiny);
    assert_eq!(p.relative_volume_ledger_residual, 0.);
    let excessive = vec![0.; 1_000_001];
    assert!(
        deliver_volumes(
            &surface,
            &excessive,
            &water,
            &vec![0; excessive.len()],
            &excessive
        )
        .is_err()
    );
}
