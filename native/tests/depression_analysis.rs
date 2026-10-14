use planimulation_core::{
    Surface,
    depression_analysis::{EscapeAnalysis, analyze},
    water::{Water, WaterSettings},
};

fn chain(areas: &[f64]) -> Surface {
    let n = areas.len();
    let mut s = Surface {
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
        if i > 0 {
            s.neighbors.push((i - 1) as u32);
        }
        if i + 1 < n {
            s.neighbors.push((i + 1) as u32);
        }
        s.offsets.push(s.neighbors.len() as u32);
    }
    s.distances.resize(s.neighbors.len(), 1.);
    s
}

fn water_at(s: &Surface, h: &[f64], level: f64) -> Water {
    let volume = h
        .iter()
        .zip(&s.areas)
        .map(|(&h, &a)| a * (level - h).max(0.))
        .sum();
    Water::generate(
        s,
        h,
        &WaterSettings::Volume {
            volume_cubic_meters: volume,
        },
    )
    .unwrap()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= b.abs().max(1.) * 1e-11, "{a} != {b}");
}

fn audit_paths(s: &Surface, h: &[f64], w: &Water, a: &EscapeAnalysis, ocean: bool) {
    for start in 0..h.len() {
        let mut i = start;
        let mut maximum = h[i].max(w.level_meters);
        for step in 0..=h.len() {
            let j = a.witness_receivers[i] as usize;
            if j == i {
                assert!(w.body_ids[i] != 0);
                if ocean {
                    assert_eq!(w.body_ids[i], w.main_ocean_id);
                }
                assert_eq!(a.escape_levels_meters[start], maximum);
                break;
            }
            assert!(step < h.len(), "Cyclic minimax witness.");
            assert!(
                s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize].contains(&(j as u32))
            );
            assert!(a.escape_levels_meters[j] <= a.escape_levels_meters[i]);
            maximum = maximum.max(h[j].max(w.level_meters));
            i = j;
        }
    }
}

#[test]
fn nested_first_merge_is_not_escape_and_storage_is_not_double_counted() {
    let s = chain(&[1., 2., 3., 4., 5., 6., 7.]);
    let h = [-2., 8., 2., 5., 1., 6., 3.];
    let w = water_at(&s, &h, 0.);
    let result = analyze(&s, &h, &w).unwrap();
    let a = result.to_initial_water.as_ref().unwrap();
    assert_eq!(a.escape_levels_meters, [0., 8., 8., 8., 8., 8., 8.]);
    assert_eq!(a.components.len(), 1);
    let c = &a.components[0];
    assert_eq!(c.first_region, 2);
    assert_eq!(c.region_count, 5);
    assert_eq!(c.dry_area_square_meters, 25.);
    assert_eq!(c.escape_level_meters, 8.);
    assert_eq!(c.additional_geometric_volume_cubic_meters, 112.);
    assert_eq!(a.additional_geometric_volume_cubic_meters, 112.);
    assert_eq!(a.maximum_additional_depth_meters, 7.);
    close(a.positive_depth_fraction_of_dry_land, 25. / 27.);
    let min = result
        .dry_minimum_leaves
        .iter()
        .find(|l| l.minimum_region == 4)
        .unwrap();
    assert_eq!(min.first_merge_depth_meters, Some(4.));
    assert_eq!(min.first_merge_capacity_cubic_meters, Some(20.));
    assert_eq!(min.escape_depth_to_initial_water_meters, Some(7.));
    assert_eq!(min.static_dry_catchment_area_square_meters, 15.);
    audit_paths(&s, &h, &w, a, false);
}

#[test]
fn inland_water_is_an_explicit_target_only_in_the_all_water_control() {
    let s = chain(&[10., 1., 1., 1., 1.]);
    let h = [-2., 5., -1., 2., 3.];
    let w = water_at(&s, &h, 0.);
    assert_eq!(w.main_ocean_id, w.body_ids[0]);
    assert_ne!(w.body_ids[0], w.body_ids[2]);
    let result = analyze(&s, &h, &w).unwrap();
    let any = result.to_initial_water.as_ref().unwrap();
    let ocean = result.to_main_ocean.as_ref().unwrap();
    assert_eq!(any.escape_levels_meters, [0., 5., 0., 2., 3.]);
    assert_eq!(any.additional_geometric_volume_cubic_meters, 0.);
    assert_eq!(ocean.escape_levels_meters, [0., 5., 5., 5., 5.]);
    assert_eq!(ocean.additional_geometric_volume_cubic_meters, 10.);
    assert_eq!(ocean.components[0].initial_wet_area_square_meters, 1.);
    assert_eq!(ocean.components[0].dry_area_square_meters, 2.);
    // Existing lake's -1..0 water is not charged again as empty capacity.
    audit_paths(&s, &h, &w, ocean, true);
}

#[test]
fn dry_sphere_has_no_invented_outlet_and_wet_sphere_has_no_new_storage() {
    let s = Surface::build(2, 1000.);
    let h = vec![0.; s.areas.len()];
    for fraction in [0., 1.] {
        let w = Water::generate(&s, &h, &WaterSettings::Coverage { fraction }).unwrap();
        let result = analyze(&s, &h, &w).unwrap();
        if fraction == 0. {
            assert!(result.to_initial_water.is_none());
            assert!(result.to_main_ocean.is_none());
            assert_eq!(result.dry_minimum_leaves.len(), 1);
            let l = &result.dry_minimum_leaves[0];
            assert_eq!(l.minimum_region, 0);
            assert!(l.first_merge_depth_meters.is_none());
            assert!(l.first_merge_capacity_cubic_meters.is_none());
            close(
                l.static_dry_catchment_area_square_meters,
                s.areas.iter().sum(),
            );
        } else {
            assert!(result.dry_minimum_leaves.is_empty());
            let a = result.to_initial_water.unwrap();
            assert_eq!(a.outlet_region_count, h.len());
            assert!(a.components.is_empty());
            assert_eq!(a.additional_geometric_volume_cubic_meters, 0.);
        }
    }
}

#[test]
fn flat_sill_routes_have_no_epsilon_and_reversed_neighbors_are_identical() {
    let mut s = chain(&[1.; 6]);
    let h = [-1., 3., 3., 1., 1., 3.];
    let w = water_at(&s, &h, 0.);
    let original = analyze(&s, &h, &w).unwrap();
    assert_eq!(original.dry_minimum_leaves.len(), 1);
    let a = original.to_initial_water.as_ref().unwrap();
    assert_eq!(a.components[0].region_count, 2);
    assert_eq!(a.additional_geometric_volume_cubic_meters, 4.);
    audit_paths(&s, &h, &w, a, false);
    for i in 0..h.len() {
        s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize].reverse();
    }
    assert_eq!(analyze(&s, &h, &w).unwrap(), original);
}

#[test]
fn minimax_matches_independent_all_pairs_floyd_warshall_on_spherical_graphs() {
    for level in [0, 1] {
        let s = Surface::build(level, 1000.);
        let n = s.areas.len();
        for seed in 0..8 {
            let h: Vec<_> = (0..n)
                .map(|i| ((i * 37 + seed * 19 + i * i * 3) % 29) as f64 - 12.)
                .collect();
            let w = water_at(&s, &h, 0.);
            let result = analyze(&s, &h, &w).unwrap();
            let effective: Vec<_> = h.iter().map(|&h| h.max(w.level_meters)).collect();
            let mut paths = vec![f64::INFINITY; n * n];
            for i in 0..n {
                paths[i * n + i] = effective[i];
                for &j in &s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize] {
                    paths[i * n + j as usize] = effective[i].max(effective[j as usize]);
                }
            }
            for k in 0..n {
                for i in 0..n {
                    for j in 0..n {
                        paths[i * n + j] =
                            paths[i * n + j].min(paths[i * n + k].max(paths[k * n + j]));
                    }
                }
            }
            for (a, ocean) in [
                (result.to_initial_water.as_ref().unwrap(), false),
                (result.to_main_ocean.as_ref().unwrap(), true),
            ] {
                for i in 0..n {
                    let expected = (0..n)
                        .filter(|&j| {
                            w.body_ids[j] != 0 && (!ocean || w.body_ids[j] == w.main_ocean_id)
                        })
                        .map(|j| paths[i * n + j])
                        .fold(f64::INFINITY, f64::min);
                    assert_eq!(a.escape_levels_meters[i], expected);
                }
                let direct: f64 = (0..n)
                    .map(|i| (a.escape_levels_meters[i] - effective[i]) * s.areas[i])
                    .sum();
                close(a.additional_geometric_volume_cubic_meters, direct);
                close(
                    a.components
                        .iter()
                        .map(|c| c.additional_geometric_volume_cubic_meters)
                        .sum(),
                    direct,
                );
                let band_area: f64 = a.depth_bands.iter().map(|b| b.dry_area_square_meters).sum();
                close(band_area, a.positive_depth_dry_area_square_meters);
                audit_paths(&s, &h, &w, a, ocean);
            }
        }
    }
}

#[test]
fn datum_shift_and_area_scaling_preserve_depths_and_scale_storage() {
    let s = chain(&[1., 2., 4., 8.]);
    let h = [-2., 7., 1., 3.];
    let w = water_at(&s, &h, 0.);
    let a = analyze(&s, &h, &w).unwrap();
    let mut shifted = h;
    for v in &mut shifted {
        *v += 1_000_000.;
    }
    let b = analyze(&s, &shifted, &water_at(&s, &shifted, 1_000_000.)).unwrap();
    assert_eq!(
        a.to_initial_water
            .as_ref()
            .unwrap()
            .additional_geometric_volume_cubic_meters,
        b.to_initial_water
            .as_ref()
            .unwrap()
            .additional_geometric_volume_cubic_meters
    );
    assert_eq!(
        a.dry_minimum_leaves[0].first_merge_depth_meters,
        b.dry_minimum_leaves[0].first_merge_depth_meters
    );
    let scaled = chain(&[3., 6., 12., 24.]);
    let c = analyze(&scaled, &h, &water_at(&scaled, &h, 0.)).unwrap();
    assert_eq!(
        a.to_initial_water.as_ref().unwrap().escape_levels_meters,
        c.to_initial_water.as_ref().unwrap().escape_levels_meters
    );
    close(
        c.to_initial_water
            .unwrap()
            .additional_geometric_volume_cubic_meters,
        3. * a
            .to_initial_water
            .unwrap()
            .additional_geometric_volume_cubic_meters,
    );
}

#[test]
fn represented_micro_depression_is_not_rounded_into_a_flat() {
    let s = chain(&[1.; 3]);
    let sill = f64::from_bits(1_f64.to_bits() + 1);
    let h = [-1., sill, 1.];
    let w = water_at(&s, &h, -0.);
    let a = analyze(&s, &h, &w).unwrap().to_initial_water.unwrap();
    assert_eq!(a.escape_levels_meters[2], sill);
    assert_eq!(a.components.len(), 1);
    assert_eq!(a.components[0].region_count, 1);
    assert_eq!(a.additional_geometric_volume_cubic_meters, sill - 1.);
    assert_eq!(a.depth_bands[0].dry_region_count, 1);
}

#[test]
fn depth_bands_use_exact_inclusive_upper_boundaries() {
    let s = chain(&[1.; 8]);
    let h = [-1., 1000., 999., 990., 950., 900., 500., 499.5];
    let w = water_at(&s, &h, 0.);
    let a = analyze(&s, &h, &w).unwrap().to_initial_water.unwrap();
    assert_eq!(
        a.depth_bands
            .iter()
            .map(|b| b.dry_region_count)
            .collect::<Vec<_>>(),
        [1; 6]
    );
    assert_eq!(
        a.depth_bands
            .iter()
            .map(|b| b.additional_geometric_volume_cubic_meters)
            .collect::<Vec<_>>(),
        [1., 10., 50., 100., 500., 500.5]
    );
}

#[test]
fn quantized_sphere_leaf_catalog_matches_independent_dry_terminals() {
    use planimulation_core::drainage::Drainage;
    for seed in 0..16 {
        let s = Surface::build(2, 1000.);
        let h: Vec<_> = (0..s.areas.len())
            .map(|i| ((i * 17 + i * i + seed * 31) % 9) as f64 - 3.)
            .collect();
        for fraction in [0., 0.4, 1.] {
            let w = Water::generate(&s, &h, &WaterSettings::Coverage { fraction }).unwrap();
            let graph = Drainage::build(&s, &h, &w.body_ids);
            let a = analyze(&s, &h, &w).unwrap();
            let mut minima: Vec<_> = a
                .dry_minimum_leaves
                .iter()
                .map(|l| l.minimum_region)
                .collect();
            minima.sort_unstable();
            let terminals: Vec<_> = (0..h.len())
                .filter(|&i| graph.receivers[i] as usize == i && w.body_ids[i] == 0)
                .map(|i| i as u32)
                .collect();
            assert_eq!(minima, terminals);
            let closed_area: f64 = (0..h.len())
                .filter(|&i| w.body_ids[i] == 0 && w.body_ids[graph.outlets[i] as usize] == 0)
                .map(|i| s.areas[i])
                .sum();
            close(
                a.dry_minimum_leaves
                    .iter()
                    .map(|l| l.static_dry_catchment_area_square_meters)
                    .sum(),
                closed_area,
            );
        }
    }
}

#[test]
fn rejects_corrupt_fields_disconnected_graphs_and_geometric_overflow() {
    let s = chain(&[1.; 4]);
    let h = [-1., 5., 1., 2.];
    let w = water_at(&s, &h, 0.);
    for case in 0..7 {
        let mut bad = chain(&[1.; 4]);
        let mut bed = h;
        let mut water = w.clone();
        match case {
            0 => bed[2] = f64::NAN,
            1 => bad.areas[2] = 0.,
            2 => bad.offsets[2] = u32::MAX,
            3 => bad.neighbors[0] = 99,
            4 => water.main_ocean_id = 0,
            5 => water.body_ids[2] = 1,
            _ => bad.distances[0] = 2.,
        }
        assert!(analyze(&bad, &bed, &water).is_err());
    }
    let mut disconnected = chain(&[1.; 4]);
    disconnected.offsets = vec![0; 5];
    disconnected.neighbors.clear();
    disconnected.distances.clear();
    assert!(analyze(&disconnected, &h, &w).is_err());
    let large = chain(&[1., 1., 1e308]);
    let h = [-1., 10., 0.];
    let w = water_at(&large, &h, 0.);
    assert!(analyze(&large, &h, &w).is_err());
}

#[test]
fn large_iterative_plateau_is_immutable_and_exactly_repeatable() {
    let s = Surface::build(6, 1000.);
    let mut h = vec![1.; s.areas.len()];
    h[0] = -1.;
    let w = water_at(&s, &h, 0.);
    let old_h = h.clone();
    let old_w = w.clone();
    let a = analyze(&s, &h, &w).unwrap();
    let b = analyze(&s, &h, &w).unwrap();
    assert_eq!(a, b);
    assert_eq!(h, old_h);
    assert_eq!(w.body_ids, old_w.body_ids);
    assert_eq!(w.depth_meters, old_w.depth_meters);
    let escape = a.to_initial_water.unwrap();
    assert!(escape.components.is_empty());
    audit_paths(&s, &h, &w, &escape, false);
}
