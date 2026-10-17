use planimulation_core::{
    drainage_landforms::{self as landforms, Edge, Settings},
    terrain_patch_geometry::{self, Settings as GridSettings},
};

fn settings() -> Settings {
    Settings {
        channel_head_area_square_meters: 5.,
        channel_head_slope: 0.25,
        channel_concavity: 0.,
        maximum_graph_slope: 0.5,
    }
}
fn chain() -> Vec<Edge> {
    vec![
        Edge {
            a: 0,
            b: 1,
            distance_meters: 2.,
        },
        Edge {
            a: 1,
            b: 2,
            distance_meters: 2.,
        },
        Edge {
            a: 2,
            b: 3,
            distance_meters: 4.,
        },
    ]
}

#[test]
fn independent_unequal_area_chain_separates_channel_and_roof_debits() {
    let bed = [0., 10., 20., 30.];
    let areas = [100., 1., 3., 4.];
    let fixed = [true, false, false, false];
    let r =
        landforms::construct(&bed, &areas, &fixed, &chain(), &[0, 0, 1, 2], settings()).unwrap();
    assert_eq!(
        r.structural_contributing_area_square_meters,
        [8., 8., 7., 4.]
    );
    assert_eq!(r.structural_channel, [true, true, true, false]);
    assert_eq!(r.channel_profile_constraint_meters, [0., 0.5, 1., 30.]);
    assert_eq!(r.elevation_meters, [0., 0.5, 1., 3.]);
    assert_eq!(r.limiting_constraint_node, [0, 1, 2, 2]);
    assert_eq!(r.channel_profile_detached_cubic_meters, 66.5);
    assert_eq!(r.roof_detached_cubic_meters, 108.);
    assert_eq!(r.detached_material_cubic_meters, 174.5);
    assert_eq!(r.relative_material_residual, 0.);
    assert_eq!(r.structural_terminal_area_relative_residual, 0.);
    assert_eq!(bed, [0., 10., 20., 30.]);
}

#[test]
fn concavity_uses_a_dimensionless_area_ratio_not_sampling_width() {
    let edges: Vec<_> = (0..3)
        .map(|i| Edge {
            a: i,
            b: i + 1,
            distance_meters: 4.,
        })
        .collect();
    let r = landforms::construct(
        &[1., 10., 20., 30.],
        &[100., 12., 3., 1.],
        &[true, false, false, false],
        &edges,
        &[0, 0, 1, 2],
        Settings {
            channel_head_area_square_meters: 1.,
            channel_head_slope: 0.5,
            channel_concavity: 0.5,
            maximum_graph_slope: 1.,
        },
    )
    .unwrap();
    assert_eq!(
        r.structural_contributing_area_square_meters,
        [16., 16., 4., 1.]
    );
    assert_eq!(r.elevation_meters, [1., 1.5, 2.5, 4.5]);
    assert_eq!(r.roof_detached_cubic_meters, 0.);
    assert_eq!(r.detached_material_cubic_meters, 180.);
}

#[test]
fn lower_roof_matches_independent_floyd_warshall_min_plus_oracle() {
    let n = 8;
    let mut state = 19_u64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((state >> 32) % 32) as f64 / 4. + 1.
    };
    let mut distances = vec![vec![f64::INFINITY; n]; n];
    let mut edges = Vec::new();
    for (i, row) in distances.iter_mut().enumerate() {
        row[i] = 0.;
    }
    for i in 0..n {
        for j in i + 1..n {
            let length = next();
            edges.push(Edge {
                a: i as u32,
                b: j as u32,
                distance_meters: length,
            });
        }
    }
    for edge in &edges {
        distances[edge.a as usize][edge.b as usize] = edge.distance_meters;
        distances[edge.b as usize][edge.a as usize] = edge.distance_meters;
    }
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                distances[i][j] = distances[i][j].min(distances[i][k] + distances[k][j]);
            }
        }
    }
    let settings = Settings {
        channel_head_area_square_meters: 100.,
        ..settings()
    };
    for _ in 0..8 {
        let bed: Vec<_> = (0..n).map(|_| next() * 4. - 10.).collect();
        let receivers: Vec<_> = (0..n as u32).collect();
        let result =
            landforms::construct(&bed, &[1.; 8], &[false; 8], &edges, &receivers, settings)
                .unwrap();
        for (i, row) in distances.iter().enumerate() {
            let expected = bed
                .iter()
                .zip(row)
                .map(|(h, distance)| h + 0.5 * distance)
                .fold(f64::INFINITY, f64::min);
            assert_eq!(result.elevation_meters[i], expected);
        }
        edges.reverse();
        let reversed =
            landforms::construct(&bed, &[1.; 8], &[false; 8], &edges, &receivers, settings)
                .unwrap();
        assert_eq!(result, reversed);
        let again = landforms::construct(
            &result.elevation_meters,
            &[1.; 8],
            &[false; 8],
            &edges,
            &receivers,
            settings,
        )
        .unwrap();
        assert_eq!(again.elevation_meters, result.elevation_meters);
        assert_eq!(again.detached_material_cubic_meters, 0.);
    }
}

#[test]
fn unchanged_affine_constant_and_disconnected_fields_are_exact() {
    let edges = vec![
        Edge {
            a: 0,
            b: 1,
            distance_meters: 2.,
        },
        Edge {
            a: 1,
            b: 2,
            distance_meters: 2.,
        },
    ];
    for bed in [[-1., 0., 1., 20.], [12.; 4]] {
        let r = landforms::construct(
            &bed,
            &[1.; 4],
            &[false; 4],
            &edges,
            &[0, 1, 2, 3],
            settings(),
        )
        .unwrap();
        assert_eq!(r.elevation_meters, bed);
        assert_eq!(r.detached_material_cubic_meters, 0.);
    }
}

#[test]
fn generic_dry_receivers_use_actual_slope_fixed_terminals_and_stable_ties() {
    let mut edges = vec![
        Edge {
            a: 0,
            b: 2,
            distance_meters: 2.,
        },
        Edge {
            a: 1,
            b: 2,
            distance_meters: 2.,
        },
        Edge {
            a: 2,
            b: 3,
            distance_meters: 1.,
        },
    ];
    let bed = [0., 0., 2., 3.];
    let fixed = [false, false, false, true];
    let receivers = landforms::dry_receivers(&bed, &[1.; 4], &fixed, &edges).unwrap();
    assert_eq!(receivers, [0, 1, 0, 3]);
    edges.reverse();
    assert_eq!(
        landforms::dry_receivers(&bed, &[1.; 4], &fixed, &edges).unwrap(),
        receivers
    );
    // A flat is a real terminal in this deliberately strict router.
    assert_eq!(
        landforms::dry_receivers(&[4.; 4], &[1.; 4], &fixed, &edges).unwrap(),
        [0, 1, 2, 3]
    );
    assert_eq!(bed, [0., 0., 2., 3.]);
    assert!(landforms::dry_receivers(&[f64::NAN; 4], &[1.; 4], &fixed, &edges).is_err());
    assert!(landforms::dry_receivers(&bed, &[1.; 3], &fixed, &edges).is_err());
}

#[test]
fn incompatible_fixed_boundary_refuses_without_changing_any_input() {
    let bed = [100., 0., 100., 20.];
    let areas = [1.; 4];
    let fixed = [true, false, false, false];
    let receivers = [0, 1, 2, 3];
    let error =
        landforms::construct(&bed, &areas, &fixed, &chain(), &receivers, settings()).unwrap_err();
    assert!(error.contains("incompatible"));
    assert_eq!(bed, [100., 0., 100., 20.]);
    assert_eq!(areas, [1.; 4]);
    assert_eq!(fixed, [true, false, false, false]);
    assert_eq!(receivers, [0, 1, 2, 3]);
}

#[test]
fn cycles_uphill_fixed_nonadjacent_and_malformed_edges_refuse() {
    for receivers in [[1, 0, 2, 3], [0, 0, 3, 3], [0, 0, 0, 2]] {
        assert!(
            landforms::construct(
                &[0., 10., 20., 30.],
                &[1.; 4],
                &[false; 4],
                &chain(),
                &receivers,
                settings()
            )
            .is_err()
        );
    }
    // Isolate a genuinely equal-height cycle rather than an uphill refusal.
    assert!(
        landforms::construct(
            &[1.; 4],
            &[1.; 4],
            &[false; 4],
            &chain(),
            &[1, 0, 2, 3],
            settings()
        )
        .unwrap_err()
        .contains("Cyclic")
    );
    assert!(
        landforms::construct(
            &[0., 10., 20., 30.],
            &[1.; 4],
            &[false, true, false, false],
            &chain(),
            &[0, 0, 1, 2],
            settings()
        )
        .is_err()
    );
    for edges in [
        vec![Edge {
            a: 1,
            b: 0,
            distance_meters: 1.,
        }],
        vec![Edge {
            a: 0,
            b: 4,
            distance_meters: 1.,
        }],
        vec![Edge {
            a: 0,
            b: 1,
            distance_meters: 0.,
        }],
        vec![Edge {
            a: 0,
            b: 1,
            distance_meters: f64::NAN,
        }],
        vec![chain()[0], chain()[0]],
        vec![chain()[0]; 17],
    ] {
        assert!(
            landforms::construct(
                &[0.; 4],
                &[1.; 4],
                &[false; 4],
                &edges,
                &[0, 1, 2, 3],
                settings()
            )
            .is_err()
        );
    }
}

#[test]
fn invalid_settings_fields_overflow_and_subprecision_positive_offsets_refuse() {
    let base = settings();
    for bad in [
        Settings {
            channel_head_area_square_meters: 0.,
            ..base
        },
        Settings {
            channel_head_area_square_meters: f64::INFINITY,
            ..base
        },
        Settings {
            channel_head_slope: 0.,
            ..base
        },
        Settings {
            channel_head_slope: 1.,
            ..base
        },
        Settings {
            channel_concavity: -0.1,
            ..base
        },
        Settings {
            channel_concavity: f64::NAN,
            ..base
        },
        Settings {
            maximum_graph_slope: 0.,
            ..base
        },
        Settings {
            maximum_graph_slope: f64::INFINITY,
            ..base
        },
    ] {
        assert!(landforms::construct(&[0.], &[1.], &[false], &[], &[0], bad).is_err());
    }
    assert!(landforms::construct(&[], &[], &[], &[], &[], base).is_err());
    assert!(landforms::construct(&[0.], &[], &[false], &[], &[0], base).is_err());
    assert!(landforms::construct(&[f64::NAN], &[1.], &[false], &[], &[0], base).is_err());
    assert!(landforms::construct(&[0.], &[0.], &[false], &[], &[0], base).is_err());
    assert!(
        landforms::construct(
            &vec![0.; landforms::MAX_NODES + 1],
            &[],
            &[],
            &[],
            &[],
            base
        )
        .is_err()
    );
    let edge = [Edge {
        a: 0,
        b: 1,
        distance_meters: 1.,
    }];
    assert!(
        landforms::construct(&[0., 1.], &[f64::MAX; 2], &[false; 2], &edge, &[0, 0], base).is_err()
    );
    assert!(
        landforms::construct(
            &[1., 1.],
            &[1.; 2],
            &[false; 2],
            &[Edge {
                distance_meters: 1e-300,
                ..edge[0]
            }],
            &[0, 1],
            base
        )
        .unwrap_err()
        .contains("offset")
    );
    assert!(
        landforms::construct(
            &[0., f64::MAX],
            &[1., f64::MAX],
            &[true, false],
            &edge,
            &[0, 0],
            base
        )
        .is_err()
    );
}

#[test]
fn area_and_distance_unit_rescaling_keeps_dimensionless_construction() {
    let a = landforms::construct(
        &[0., 10., 20., 30.],
        &[100., 1., 3., 4.],
        &[true, false, false, false],
        &chain(),
        &[0, 0, 1, 2],
        settings(),
    )
    .unwrap();
    let edges: Vec<_> = chain()
        .iter()
        .map(|e| Edge {
            distance_meters: e.distance_meters * 2.,
            ..*e
        })
        .collect();
    let b = landforms::construct(
        &[0., 20., 40., 60.],
        &[400., 4., 12., 16.],
        &[true, false, false, false],
        &edges,
        &[0, 0, 1, 2],
        Settings {
            channel_head_area_square_meters: 20.,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(
        b.elevation_meters,
        a.elevation_meters
            .iter()
            .map(|v| v * 2.)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        b.detached_material_cubic_meters,
        a.detached_material_cubic_meters * 8.
    );
    assert_eq!(b.structural_channel, a.structural_channel);
    let shifted = landforms::construct(
        &[1024., 1034., 1044., 1054.],
        &[100., 1., 3., 4.],
        &[true, false, false, false],
        &chain(),
        &[0, 0, 1, 2],
        settings(),
    )
    .unwrap();
    assert_eq!(
        shifted.elevation_meters,
        a.elevation_meters
            .iter()
            .map(|v| v + 1024.)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        shifted.detached_material_cubic_meters,
        a.detached_material_cubic_meters
    );
}

#[test]
fn patch_adapter_rebuilds_actual_drainage_and_does_not_invent_basin_outlets() {
    let g = terrain_patch_geometry::build(GridSettings {
        radius_meters: 6371000.,
        center_direction: [1., 0., 0.],
        orientation_radians: 0.,
        cells_x: 9,
        cells_y: 9,
        spacing_meters: 250.,
    })
    .unwrap();
    let bed: Vec<_> = (0_usize..81)
        .map(|i| {
            if g.fixed_external_boundary[i] {
                100.
            } else {
                ((i % 9).abs_diff(4) + (i / 9).abs_diff(4)) as f64
            }
        })
        .collect();
    let result = landforms::construct_patch(
        &g,
        &bed,
        Settings {
            channel_head_area_square_meters: 1e12,
            maximum_graph_slope: 1.,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(result.construction.elevation_meters, bed);
    assert!(result.actual_dry_routing.closed_fraction_of_active_area > 0.99);
    assert_eq!(result.actual_dry_routing.receivers[40], 40);
    let again = landforms::construct_patch(
        &g,
        &bed,
        Settings {
            channel_head_area_square_meters: 1e12,
            maximum_graph_slope: 1.,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::to_value(&again).unwrap()
    );
}

#[test]
fn long_chain_is_iterative_and_uses_actual_accepted_reference_prisms() {
    let n = 20000_usize;
    let edges: Vec<_> = (1..n)
        .map(|i| Edge {
            a: i as u32 - 1,
            b: i as u32,
            distance_meters: 1.,
        })
        .collect();
    let bed: Vec<_> = (0..n).map(|i| i as f64 * 2.).collect();
    let receiver: Vec<_> = (0..n).map(|i| i.saturating_sub(1) as u32).collect();
    let r = landforms::construct(
        &bed,
        &vec![1.; n],
        &vec![false; n],
        &edges,
        &receiver,
        Settings {
            channel_head_area_square_meters: 100000.,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(r.elevation_meters[n - 1], (n - 1) as f64 * 0.5);
    assert!(r.elevation_meters.windows(2).all(|w| w[1] - w[0] == 0.5));
    let independent: f64 = bed
        .iter()
        .zip(&r.elevation_meters)
        .map(|(a, b)| a - b)
        .sum();
    assert_eq!(r.detached_material_cubic_meters, independent);
}
