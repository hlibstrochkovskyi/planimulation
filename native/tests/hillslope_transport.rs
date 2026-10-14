use planimulation_core::hillslope_transport::{self, Edge, Settings, transport};

fn edge(a: u32, b: u32) -> Edge {
    Edge {
        a,
        b,
        shared_face_length_meters: 1.,
        center_distance_meters: 1.,
    }
}
fn settings() -> Settings {
    Settings {
        diffusivity_square_meters_per_year: 1.,
        step_years: 0.25,
    }
}

#[test]
fn temporal_refinement_approaches_the_independent_closed_pair_exponential() {
    let exact = 0.5 + 0.5 * (-1_f64).exp();
    let mut previous = f64::INFINITY;
    for steps in [4, 8, 16, 32, 64] {
        let mut h = vec![1., 0.];
        for _ in 0..steps {
            h = transport(
                &h,
                &[1.; 2],
                &[false; 2],
                &[edge(0, 1)],
                Settings {
                    step_years: 0.5 / steps as f64,
                    ..settings()
                },
            )
            .unwrap()
            .elevation_meters;
        }
        let error = (h[0] - exact).abs();
        assert!(error < previous);
        assert!((h[0] + h[1] - 1.).abs() < 1e-14);
        previous = error;
    }
}

#[test]
fn dyadic_two_node_and_unequal_area_controls_exchange_equal_volumes() {
    for areas in [[2., 2.], [2., 4.]] {
        let h = [4., 0.];
        let t = transport(&h, &areas, &[false; 2], &[edge(0, 1)], settings()).unwrap();
        assert_eq!(t.model_version, hillslope_transport::MODEL_VERSION);
        assert_eq!(t.requested_edge_volume_cubic_meters, [1.]);
        assert_eq!(t.requested_node_net_volume_cubic_meters, [-1., 1.]);
        assert_eq!(t.accepted_node_prism_cubic_meters, [-1., 1.]);
        assert_eq!(t.elevation_meters, [3.5, 1. / areas[1]]);
        assert_eq!(t.accepted_net_bed_prism_cubic_meters, 0.);
        assert_eq!(t.requested_boundary_exchange_cubic_meters, 0.);
        assert_eq!(t.gross_requested_transfer_cubic_meters, 1.);
        assert_eq!(t.relative_exchange_ledger_residual, 0.);
        assert_eq!(h, [4., 0.]);
    }
    for length_scale in [1., 2., 4.] {
        let area_scale = length_scale * length_scale;
        let e = Edge {
            shared_face_length_meters: 3. * length_scale,
            center_distance_meters: 4. * length_scale,
            ..edge(0, 1)
        };
        let t = transport(
            &[8., 0.],
            &[2. * area_scale, 4. * area_scale],
            &[false; 2],
            &[e],
            Settings {
                diffusivity_square_meters_per_year: area_scale,
                ..settings()
            },
        )
        .unwrap();
        assert_eq!(t.elevation_meters, [7.25, 0.375]);
        assert_eq!(t.requested_edge_volume_cubic_meters, [1.5 * area_scale]);
        assert_eq!(t.accepted_net_bed_prism_cubic_meters, 0.);
    }
}

#[test]
fn fixed_height_reservoir_input_and_export_have_explicit_signs() {
    for fixed in [[false, true], [true, false]] {
        let t = transport(&[0., 4.], &[2.; 2], &fixed, &[edge(0, 1)], settings()).unwrap();
        let expected = if fixed[0] { -1. } else { 1. };
        assert_eq!(t.requested_boundary_exchange_cubic_meters, expected);
        assert_eq!(t.accepted_net_bed_prism_cubic_meters, expected);
        assert_eq!(t.gross_requested_boundary_exchange_cubic_meters, 1.);
        assert_eq!(t.relative_boundary_exchange_ledger_residual, 0.);
        for i in 0..2 {
            if fixed[i] {
                assert_eq!(t.elevation_meters[i], [0., 4.][i]);
                assert_eq!(t.accepted_height_change_meters[i], 0.);
                assert_eq!(t.requested_node_net_volume_cubic_meters[i], 0.);
            }
        }
    }
    let t = transport(&[0., 4.], &[1.; 2], &[true; 2], &[edge(0, 1)], settings()).unwrap();
    assert_eq!(t.elevation_meters, [0., 4.]);
    assert_eq!(t.requested_edge_volume_cubic_meters, [0.]);
    assert_eq!(t.gross_requested_transfer_cubic_meters, 0.);
}

#[test]
fn zero_diffusivity_and_stable_constant_fields_are_exact_noops_after_validation() {
    for h in [[1., 1.], [0., 4.]] {
        let settings = if h[0] == h[1] {
            settings()
        } else {
            Settings {
                diffusivity_square_meters_per_year: 0.,
                ..settings()
            }
        };
        let t = transport(&h, &[1.; 2], &[false; 2], &[edge(0, 1)], settings).unwrap();
        assert_eq!(t.elevation_meters, h);
        assert_eq!(t.changed_nodes, 0);
        assert_eq!(t.unresolved_requested_change_nodes, 0);
        assert_eq!(t.requested_edge_volume_cubic_meters, [0.]);
        assert_eq!(t.gross_requested_transfer_cubic_meters, 0.);
    }
    assert!(
        transport(
            &[0., 4.],
            &[1.; 2],
            &[false; 2],
            &[edge(0, 1); 2],
            Settings {
                diffusivity_square_meters_per_year: 0.,
                ..settings()
            }
        )
        .is_err()
    );
}

#[test]
fn cfl_at_one_is_stable_and_larger_steps_refuse_even_constant_beds() {
    let stable = Settings {
        step_years: 1.,
        ..settings()
    };
    let t = transport(&[0., 4.], &[1.; 2], &[false; 2], &[edge(0, 1)], stable).unwrap();
    assert_eq!(t.elevation_meters, [4., 0.]);
    assert_eq!(t.maximum_active_cfl, 1.);
    for h in [[0., 4.], [4., 4.]] {
        assert!(
            transport(
                &h,
                &[1.; 2],
                &[false; 2],
                &[edge(0, 1)],
                Settings {
                    step_years: 1.0000000000000002,
                    ..stable
                }
            )
            .is_err()
        );
    }
    assert!(
        transport(
            &[0., 4., 0.],
            &[1.; 3],
            &[false; 3],
            &[edge(0, 1), edge(1, 2)],
            Settings {
                step_years: 0.75,
                ..settings()
            }
        )
        .is_err()
    );
}

#[test]
fn closed_chain_is_conservative_deterministic_and_within_local_extrema() {
    let h = [0., 4., 0.];
    let edges = [edge(0, 1), edge(1, 2)];
    let a = transport(&h, &[1.; 3], &[false; 3], &edges, settings()).unwrap();
    let b = transport(&h, &[1.; 3], &[false; 3], &edges, settings()).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.elevation_meters, [1., 2., 1.]);
    assert_eq!(a.accepted_node_prism_cubic_meters, [1., -2., 1.]);
    assert_eq!(a.accepted_net_bed_prism_cubic_meters, 0.);
    assert_eq!(a.gross_requested_transfer_cubic_meters, 2.);
    assert_eq!(h, [0., 4., 0.]);
    assert_eq!(edges[0].a, 0);
}

#[test]
fn subprecision_internal_noops_are_observed_but_unbalanced_or_external_losses_refuse() {
    let h = [1e20, 1e20 + 16384.];
    let t = transport(&h, &[1.; 2], &[false; 2], &[edge(0, 1)], settings()).unwrap();
    assert_eq!(t.elevation_meters, h);
    assert_eq!(t.unresolved_requested_change_nodes, 2);
    assert_eq!(t.requested_node_net_volume_cubic_meters, [4096., -4096.]);
    assert_eq!(t.accepted_node_prism_cubic_meters, [0., 0.]);
    assert_eq!(t.gross_absolute_node_prism_rounding_cubic_meters, 8192.);
    assert!(transport(&h, &[1.; 2], &[false, true], &[edge(0, 1)], settings()).is_err());
    // Lost boundary requests with opposite signs must not authenticate each
    // other merely because the aggregate requested reservoir exchange is zero.
    assert!(
        transport(
            &[h[0], h[1], h[1], h[0]],
            &[1.; 4],
            &[false, true, false, true],
            &[edge(0, 1), edge(2, 3)],
            settings()
        )
        .is_err()
    );
    assert!(
        transport(
            &h,
            &[1.; 2],
            &[false; 2],
            &[edge(0, 1)],
            Settings {
                step_years: 0.5,
                ..settings()
            }
        )
        .is_err()
    );
    let tiny_boundary = Edge {
        shared_face_length_meters: 1e-15,
        ..edge(1, 2)
    };
    // Internal gross throughput must not supply the tolerance scale for this
    // much smaller, wholly unrepresentable external-reservoir export.
    assert!(
        transport(
            &[h[0], h[1], h[0]],
            &[1.; 3],
            &[false, false, true],
            &[edge(0, 1), tiny_boundary],
            Settings {
                step_years: 0.0625,
                ..settings()
            }
        )
        .is_err()
    );
}

#[test]
fn malformed_geometry_shapes_workloads_and_unrepresentable_products_refuse_immutably() {
    let h = [0., 4.];
    let original_edges = [edge(0, 1)];
    for invalid in [
        edge(1, 0),
        edge(0, 0),
        edge(0, 2),
        Edge {
            shared_face_length_meters: 0.,
            ..edge(0, 1)
        },
        Edge {
            center_distance_meters: f64::NAN,
            ..edge(0, 1)
        },
    ] {
        assert!(transport(&h, &[1.; 2], &[false; 2], &[invalid], settings()).is_err());
    }
    for bad in [f64::NAN, -1., f64::INFINITY] {
        assert!(
            transport(
                &h,
                &[1.; 2],
                &[false; 2],
                &original_edges,
                Settings {
                    diffusivity_square_meters_per_year: bad,
                    ..settings()
                }
            )
            .is_err()
        );
    }
    assert!(transport(&h, &[1.], &[false; 2], &original_edges, settings()).is_err());
    assert!(transport(&h, &[1.; 2], &[false], &original_edges, settings()).is_err());
    assert!(transport(&h, &[0., 1.], &[false; 2], &original_edges, settings()).is_err());
    assert!(transport(&h, &[1.; 2], &[false; 2], &[edge(0, 1); 9], settings()).is_err());
    let large = vec![0.; hillslope_transport::MAX_NODES + 1];
    assert!(transport(&large, &large, &[], &[], settings()).is_err());
    assert!(
        transport(
            &[0., 1e-300],
            &[1.; 2],
            &[false; 2],
            &original_edges,
            Settings {
                diffusivity_square_meters_per_year: 1e-300,
                step_years: 1.,
            }
        )
        .is_err()
    );
    assert_eq!(h, [0., 4.]);
    assert_eq!(original_edges[0].shared_face_length_meters, 1.);
}

#[test]
fn square_grid_manufactured_planar_and_quadratic_interior_changes_are_known() {
    let width = 9;
    let dx = 0.5;
    let dt = 0.015625;
    let d = 0.25;
    let mut edges = Vec::new();
    for y in 0..width {
        for x in 0..width {
            let i = y * width + x;
            if x + 1 < width {
                edges.push(Edge {
                    shared_face_length_meters: dx,
                    center_distance_meters: dx,
                    ..edge(i as u32, (i + 1) as u32)
                });
            }
            if y + 1 < width {
                edges.push(Edge {
                    shared_face_length_meters: dx,
                    center_distance_meters: dx,
                    ..edge(i as u32, (i + width) as u32)
                });
            }
        }
    }
    for quadratic in [false, true] {
        let h: Vec<_> = (0..width * width)
            .map(|i| {
                let x = (i % width) as f64 * dx;
                let y = (i / width) as f64 * dx;
                if quadratic { x * x + y * y } else { x + 2. * y }
            })
            .collect();
        let t = transport(
            &h,
            &vec![dx * dx; h.len()],
            &vec![false; h.len()],
            &edges,
            Settings {
                diffusivity_square_meters_per_year: d,
                step_years: dt,
            },
        )
        .unwrap();
        for y in 1..width - 1 {
            for x in 1..width - 1 {
                let i = y * width + x;
                assert_eq!(
                    t.accepted_height_change_meters[i],
                    if quadratic { 4. * d * dt } else { 0. }
                );
            }
        }
        assert_eq!(t.accepted_net_bed_prism_cubic_meters, 0.);
    }
}

#[test]
fn sine_heat_operator_has_second_order_spatial_refinement() {
    let mut previous = f64::INFINITY;
    for subdivisions in [16, 32, 64] {
        let n = subdivisions + 1;
        let dx = 1. / subdivisions as f64;
        let dt = dx * dx / 8.;
        let mut h: Vec<_> = (0..n)
            .map(|i| (std::f64::consts::PI * i as f64 * dx).sin())
            .collect();
        h[0] = 0.;
        h[n - 1] = 0.;
        let mut fixed = vec![false; n];
        fixed[0] = true;
        fixed[n - 1] = true;
        let edges: Vec<_> = (0..n - 1)
            .map(|i| Edge {
                center_distance_meters: dx,
                ..edge(i as u32, (i + 1) as u32)
            })
            .collect();
        let t = transport(
            &h,
            &vec![dx; n],
            &fixed,
            &edges,
            Settings {
                diffusivity_square_meters_per_year: 1.,
                step_years: dt,
            },
        )
        .unwrap();
        let error = (1..n - 1)
            .map(|i| {
                (t.accepted_height_change_meters[i] / dt + std::f64::consts::PI.powi(2) * h[i])
                    .powi(2)
            })
            .sum::<f64>()
            .sqrt();
        // The unnormalized norm adds sqrt(2) samples at each refinement;
        // second-order pointwise convergence therefore gives a ratio ~2.83.
        assert!(error < previous * 0.4);
        previous = error;
    }
}
