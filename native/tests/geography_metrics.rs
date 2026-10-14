use planimulation_core::{
    Surface,
    drainage::Drainage,
    geography_metrics::analyze,
    water::{Water, WaterSettings},
};

fn chain(areas: &[f64], distance: f64) -> Surface {
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
            s.neighbors.push(i as u32 - 1);
        }
        if i + 1 < n {
            s.neighbors.push(i as u32 + 1);
        }
        s.offsets.push(s.neighbors.len() as u32);
    }
    s.distances.resize(s.neighbors.len(), distance);
    s
}

fn water_at(s: &Surface, heights: &[f64], level: f64) -> Water {
    let volume: f64 = heights
        .iter()
        .zip(&s.areas)
        .map(|(&h, &a)| (level - h).max(0.) * a)
        .sum();
    Water::generate(
        s,
        heights,
        &WaterSettings::Volume {
            volume_cubic_meters: volume,
        },
    )
    .unwrap()
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs().max(1.) * 1e-12,
        "{actual} != {expected}"
    );
}

#[test]
fn exact_weighted_landmass_hypsometry_and_coast_graph_controls() {
    let s = chain(&[1., 2., 3., 4., 5.], 2.);
    let heights = [4., -2., 2., -2., 1.];
    let water = water_at(&s, &heights, 0.);
    let m = analyze(&s, &heights, &water).unwrap();
    assert_eq!(m.reference_area_square_meters, 15.);
    assert_eq!(m.dry_area_square_meters, 9.);
    close(m.land_coverage_fraction, 0.6);
    assert_eq!(m.coastline_adjacency_crossings, 4);
    assert_eq!(
        m.landmasses
            .iter()
            .map(|c| c.reference_area_square_meters)
            .collect::<Vec<_>>(),
        [5., 3., 1.]
    );
    close(m.largest_landmass_fraction_of_dry_land, 5. / 9.);
    let hypsometry = m.dry_hypsometry.unwrap();
    assert_eq!(
        hypsometry.quantile_height_above_water_meters,
        [1., 1., 1., 2., 4.]
    );
    close(hypsometry.mean_height_above_water_meters, 15. / 9.);
    assert_eq!(m.neighbor_scale.dry_dry_adjacency_pairs, 0);
    assert!(m.neighbor_scale.area_weighted_rms_bed_slope.is_none());
    assert_eq!(m.static_drainage.closed_dry_sink_count, 0);
    assert_eq!(m.static_drainage.terminal_catchments_with_dry_land, 2);
    assert_eq!(m.static_drainage.maximum_dry_receiver_hops, 1);
    assert_eq!(
        m.static_drainage
            .largest_terminal_catchment_area_square_meters,
        5.
    );
}

#[test]
fn neighbor_statistics_have_declared_physical_units_and_dry_area_weights() {
    let s = chain(&[1., 2., 1.], 2.);
    let h = [4., 2., 0.];
    let water = Water::generate(&s, &h, &WaterSettings::Coverage { fraction: 0. }).unwrap();
    let m = analyze(&s, &h, &water).unwrap();
    let scale = m.neighbor_scale;
    assert_eq!(scale.dry_dry_adjacency_pairs, 2);
    assert_eq!(scale.maximum_absolute_bed_slope, 1.);
    assert_eq!(scale.area_weighted_rms_bed_slope, Some(1.));
    assert_eq!(scale.area_weighted_rms_height_difference_meters, Some(2.));
    assert_eq!(scale.area_weighted_mean_neighbor_distance_meters, Some(2.));
    assert_eq!(scale.represented_dry_area_square_meters, 4.);
    assert_eq!(m.static_drainage.closed_dry_sink_count, 1);
    assert_eq!(m.static_drainage.closed_drainage_fraction_of_dry_land, 1.);
    assert_eq!(m.static_drainage.maximum_dry_receiver_hops, 2);
    assert_eq!(
        m.static_drainage
            .maximum_contributing_dry_area_square_meters,
        4.
    );
    let stretched = chain(&[1., 2., 1.], 4.);
    let stretched_metrics = analyze(&stretched, &h, &water).unwrap();
    assert_eq!(
        stretched_metrics.neighbor_scale.area_weighted_rms_bed_slope,
        Some(0.5)
    );
    assert_eq!(
        stretched_metrics
            .neighbor_scale
            .area_weighted_rms_height_difference_meters,
        Some(2.)
    );
}

#[test]
fn unequal_neighbor_relief_uses_region_area_not_unweighted_edge_counts() {
    let s = chain(&[1., 4., 1.], 1.);
    let h = [0., 1., 4.];
    let water = Water::generate(&s, &h, &WaterSettings::Coverage { fraction: 0. }).unwrap();
    let m = analyze(&s, &h, &water).unwrap();
    close(
        m.neighbor_scale.area_weighted_rms_bed_slope.unwrap(),
        5_f64.sqrt(),
    );
    close(
        m.dry_hypsometry.unwrap().mean_height_above_water_meters,
        8. / 6.,
    );
}

#[test]
fn exact_full_ocean_and_flat_full_land_and_isolated_region_controls() {
    let s = Surface::build(1, 6_371_000.);
    let h = vec![7.; s.areas.len()];
    for fraction in [0., 1.] {
        let water = Water::generate(&s, &h, &WaterSettings::Coverage { fraction }).unwrap();
        let m = analyze(&s, &h, &water).unwrap();
        assert_eq!(m.land_coverage_fraction, 1. - fraction);
        assert_eq!(m.coastline_adjacency_crossings, 0);
        if fraction == 1. {
            assert!(m.dry_hypsometry.is_none());
            assert!(m.landmasses.is_empty());
            assert_eq!(m.static_drainage.closed_dry_sink_count, 0);
            assert_eq!(m.static_drainage.closed_drainage_fraction_of_dry_land, 0.);
            assert_eq!(
                m.static_drainage
                    .maximum_contributing_dry_area_square_meters,
                0.
            );
            assert_eq!(m.static_drainage.maximum_dry_receiver_hops, 0);
        } else {
            assert_eq!(m.landmasses.len(), 1);
            assert_eq!(m.largest_landmass_fraction_of_dry_land, 1.);
            assert_eq!(m.static_drainage.closed_dry_sink_count, 1);
            assert_eq!(m.static_drainage.closed_drainage_fraction_of_dry_land, 1.);
            assert_eq!(
                m.dry_hypsometry.unwrap().quantile_height_above_water_meters,
                [0.; 5]
            );
        }
    }
    let single = chain(&[2.], 1.);
    let water = Water::generate(&single, &[0.], &WaterSettings::Coverage { fraction: 0. }).unwrap();
    let m = analyze(&single, &[0.], &water).unwrap();
    assert!(m.neighbor_scale.area_weighted_rms_bed_slope.is_none());
    assert_eq!(m.static_drainage.maximum_dry_receiver_hops, 0);
}

#[test]
fn datum_translation_determinism_and_input_immutability() {
    let s = chain(&[1., 2., 3., 4., 5.], 2.);
    let h = [4., -2., 2., -2., 1.];
    let water = water_at(&s, &h, 0.);
    let before = water.clone();
    let first = serde_json::to_value(analyze(&s, &h, &water).unwrap()).unwrap();
    let repeated = serde_json::to_value(analyze(&s, &h, &water).unwrap()).unwrap();
    let translated = h.map(|v| v + 1024.);
    let translated_water = water_at(&s, &translated, 1024.);
    assert_eq!(first, repeated);
    assert_eq!(
        first,
        serde_json::to_value(analyze(&s, &translated, &translated_water).unwrap()).unwrap()
    );
    assert_eq!(water, before);
    assert_eq!(h, [4., -2., 2., -2., 1.]);
    assert!(first.get("dryAreaSquareMeters").is_some());
    assert!(first.get("dry_area_square_meters").is_none());
}

#[test]
fn malformed_graphs_fields_and_water_are_refused_without_panics() {
    let h = [2., 1., 0.];
    let s = chain(&[1.; 3], 1.);
    let water = Water::generate(&s, &h, &WaterSettings::Coverage { fraction: 0. }).unwrap();
    assert!(analyze(&s, &h[..2], &water).is_err());
    assert!(analyze(&s, &[f64::NAN, 1., 0.], &water).is_err());
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        let mut invalid = chain(&[1.; 3], 1.);
        invalid.areas[1] = value;
        assert!(analyze(&invalid, &h, &water).is_err());
        let mut invalid = chain(&[1.; 3], 1.);
        invalid.distances[1] = value;
        assert!(analyze(&invalid, &h, &water).is_err());
    }
    let mut invalid = chain(&[1.; 3], 1.);
    invalid.offsets[1] = 10;
    assert!(analyze(&invalid, &h, &water).is_err());
    let mut invalid = chain(&[1.; 3], 1.);
    invalid.neighbors[0] = 3;
    assert!(analyze(&invalid, &h, &water).is_err());
    let mut invalid = chain(&[1.; 3], 1.);
    invalid.neighbors[0] = 0;
    assert!(analyze(&invalid, &h, &water).is_err());
    let mut invalid = chain(&[1.; 3], 1.);
    invalid.distances[0] = 2.;
    assert!(analyze(&invalid, &h, &water).is_err());
    let mut invalid = water.clone();
    invalid.body_ids[1] = u32::MAX;
    assert!(analyze(&s, &h, &invalid).is_err());
    let mut invalid = water.clone();
    invalid.depth_meters[1] = 1.;
    assert!(analyze(&s, &h, &invalid).is_err());
    let mut invalid = water.clone();
    invalid.resolved_volume_cubic_meters = 1.;
    assert!(analyze(&s, &h, &invalid).is_err());
    let mut invalid = chain(&[f64::MAX; 3], 1.);
    assert!(analyze(&invalid, &h, &water).is_err());
    invalid.areas = vec![1.; 3];
    assert!(analyze(&invalid, &[f64::MAX, 1., -f64::MAX], &water).is_err());
}

#[test]
fn inconsistent_connected_and_disconnected_water_labels_are_refused() {
    let s = chain(&[1.; 5], 1.);
    let h = [-1., -1., 2., -1., -1.];
    let water = water_at(&s, &h, 0.);
    let mut invalid = water.clone();
    invalid.body_ids[1] = 2;
    assert!(analyze(&s, &h, &invalid).is_err());
    let mut invalid = water.clone();
    invalid.body_ids[3] = 1;
    invalid.body_ids[4] = 1;
    assert!(analyze(&s, &h, &invalid).is_err());
    let mut invalid = water.clone();
    invalid.main_ocean_id = 0;
    assert!(analyze(&s, &h, &invalid).is_err());
    assert!(analyze(&s, &h, &water).is_ok());
}

#[test]
fn spherical_receiver_walks_independently_reconstruct_drainage_aggregates() {
    let s = Surface::build(2, 6_371_000.);
    let heights: Vec<_> = s
        .centers
        .iter()
        .map(|p| 1200. * p[2] + 700. * (9. * p[0]).sin() * (7. * p[1]).cos())
        .collect();
    let water = Water::generate(&s, &heights, &WaterSettings::Coverage { fraction: 0.71 }).unwrap();
    let metrics = analyze(&s, &heights, &water).unwrap();
    let drainage = Drainage::build(&s, &heights, &water.body_ids);
    let n = heights.len();
    let mut closed_area = 0.;
    let mut catchments = vec![0.; n];
    let mut max_hops = 0;
    for i in 0..n {
        if water.body_ids[i] != 0 {
            continue;
        }
        let mut current = i;
        let mut hops = 0;
        while drainage.receivers[current] as usize != current {
            current = drainage.receivers[current] as usize;
            hops += 1;
            assert!(hops < n);
        }
        max_hops = max_hops.max(hops);
        let canonical = if water.body_ids[current] == 0 {
            closed_area += s.areas[i];
            current
        } else {
            water
                .body_ids
                .iter()
                .position(|&b| b == water.body_ids[current])
                .unwrap()
        };
        catchments[canonical] += s.areas[i];
    }
    close(
        metrics.static_drainage.closed_drainage_area_square_meters,
        closed_area,
    );
    assert_eq!(metrics.static_drainage.maximum_dry_receiver_hops, max_hops);
    assert_eq!(
        metrics.static_drainage.terminal_catchments_with_dry_land,
        catchments.iter().filter(|&&a| a > 0.).count()
    );
    close(
        metrics
            .static_drainage
            .largest_terminal_catchment_area_square_meters,
        catchments.into_iter().fold(0., f64::max),
    );
    let hypsometry = metrics.dry_hypsometry.unwrap();
    for (p, q) in hypsometry
        .quantile_probabilities
        .into_iter()
        .zip(hypsometry.quantile_height_above_water_meters)
    {
        let mut below = 0.;
        let mut through = 0.;
        for (i, &h) in heights.iter().enumerate() {
            if water.body_ids[i] != 0 {
                continue;
            }
            if h - water.level_meters < q {
                below += s.areas[i];
            }
            if h - water.level_meters <= q {
                through += s.areas[i];
            }
        }
        assert!(below <= p * metrics.dry_area_square_meters);
        assert!(through >= p * metrics.dry_area_square_meters);
    }
}
