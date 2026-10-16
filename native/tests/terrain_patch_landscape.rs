use planimulation_core::{
    terrain_patch_geometry::{self, Geometry},
    terrain_patch_landscape::{Settings, prepare, route},
};
fn grid(cells: u32, spacing: f64) -> Geometry {
    terrain_patch_geometry::build(terrain_patch_geometry::Settings {
        radius_meters: 6371000.,
        center_direction: [1., 0., 0.],
        orientation_radians: 0.,
        cells_x: cells,
        cells_y: cells,
        spacing_meters: spacing,
    })
    .unwrap()
}
fn settings() -> Settings {
    Settings {
        erodibility_per_year: 1e-6,
        diffusivity_square_meters_per_year: 1.,
        step_years: 25000.,
        steps: 4,
    }
}
fn rates(g: &Geometry, u: f64) -> Vec<f64> {
    g.fixed_external_boundary
        .iter()
        .map(|&fixed| if fixed { 0. } else { u })
        .collect()
}
#[test]
fn constant_zero_forcing_is_exact_and_equal_boundary_flats_have_real_exits() {
    let g = grid(8, 500.);
    let h = vec![17.; 64];
    let p = prepare(&g, &h, &rates(&g, 0.), settings()).unwrap();
    assert_eq!(p.elevation_meters, h);
    assert_eq!(p.detached_material_reservoir_cubic_meters, 0.);
    assert_eq!(p.accepted_uplift_prism_cubic_meters, 0.);
    assert_eq!(p.signed_hillslope_boundary_exchange_cubic_meters, 0.);
    assert_eq!(p.final_routing.closed_sink_count, 0);
    assert_eq!(p.final_routing.closed_fraction_of_active_area, 0.);
    for (i, &outlet) in p.final_routing.outlets.iter().enumerate() {
        assert!(g.fixed_external_boundary[outlet as usize]);
        if !g.fixed_external_boundary[i] {
            assert_ne!(p.final_routing.receivers[i], i as u32);
        }
    }
}
#[test]
fn enclosed_flat_is_not_connected_through_higher_boundary() {
    let g = grid(8, 500.);
    let h: Vec<_> = g
        .fixed_external_boundary
        .iter()
        .map(|&b| if b { 100. } else { 0. })
        .collect();
    let r = route(&g, &h).unwrap();
    assert_eq!(r.closed_sink_count, 1);
    assert_eq!(r.closed_fraction_of_active_area, 1.);
    let p = prepare(
        &g,
        &h,
        &rates(&g, 0.),
        Settings {
            diffusivity_square_meters_per_year: 0.,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(p.elevation_meters, h);
    assert_eq!(p.detached_material_reservoir_cubic_meters, 0.);
    assert_eq!(p.final_routing.closed_sink_count, 1);
}
#[test]
fn prescribed_uplift_only_keeps_boundary_and_accounts_actual_displacement() {
    let g = grid(8, 500.);
    let h = vec![0.; 64];
    let u = rates(&g, 0.001);
    let p = prepare(
        &g,
        &h,
        &u,
        Settings {
            erodibility_per_year: 0.,
            diffusivity_square_meters_per_year: 0.,
            step_years: 1000.,
            steps: 4,
        },
    )
    .unwrap();
    let expected: f64 = g
        .reference_areas_square_meters
        .iter()
        .enumerate()
        .filter(|(i, _)| !g.fixed_external_boundary[*i])
        .map(|(_, a)| a * 4.)
        .sum();
    assert!((p.accepted_uplift_prism_cubic_meters - expected).abs() / expected < 1e-14);
    assert_eq!(p.relative_prism_ledger_residual, 0.);
    for i in 0..64 {
        assert_eq!(
            p.elevation_meters[i],
            if g.fixed_external_boundary[i] { 0. } else { 4. }
        );
    }
    assert_eq!(p.detached_material_reservoir_cubic_meters, 0.);
    assert_eq!(u, rates(&g, 0.001));
}
#[test]
fn incision_and_hillslope_coupling_repeats_without_moving_fixed_heights() {
    let g = grid(12, 500.);
    let h: Vec<_> = (0..144)
        .map(|i| {
            if g.fixed_external_boundary[i] {
                0.
            } else {
                1. + (i % 7) as f64 / 8.
            }
        })
        .collect();
    let u = rates(&g, 0.0005);
    let a = prepare(&g, &h, &u, settings()).unwrap();
    let b = prepare(&g, &h, &u, settings()).unwrap();
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert!(a.detached_material_reservoir_cubic_meters > 0.);
    assert!(a.gross_requested_hillslope_transfer_cubic_meters > 0.);
    assert!(a.signed_hillslope_boundary_exchange_cubic_meters < 0.);
    assert!(a.relative_prism_ledger_residual < 1e-10);
    for (i, &initial) in h.iter().enumerate() {
        if g.fixed_external_boundary[i] {
            assert_eq!(a.elevation_meters[i], initial);
        }
        let reconstructed = initial + a.accepted_uplift_displacement_meters[i]
            - a.removed_incision_thickness_meters[i]
            + a.signed_hillslope_height_change_meters[i];
        assert!((a.elevation_meters[i] - reconstructed).abs() < 1e-12);
    }
}
#[test]
fn smaller_physical_spacing_resolves_more_stable_diffusion_substeps() {
    let coarse = grid(8, 1000.);
    let fine = grid(32, 250.);
    let a = prepare(&coarse, &vec![0.; 64], &rates(&coarse, 0.), settings()).unwrap();
    let b = prepare(&fine, &vec![0.; 1024], &rates(&fine, 0.), settings()).unwrap();
    assert_eq!(a.diffusion_substeps_per_step, 1);
    assert_eq!(b.diffusion_substeps_per_step, 3);
    // Curved areas make CFL marginally larger than the planar1.6, so the
    // 0.8 target correctly selects3, not an unstable rounded-down2.
}
#[test]
fn malformed_inputs_geometry_and_excessive_work_refuse_without_mutation() {
    let mut g = grid(8, 500.);
    let h = vec![0.; 64];
    let u = rates(&g, 0.);
    assert!(prepare(&g, &h[..63], &u, settings()).is_err());
    let mut bad = u.clone();
    bad[0] = 1.;
    assert!(prepare(&g, &h, &bad, settings()).is_err());
    assert!(
        prepare(
            &g,
            &h,
            &u,
            Settings {
                diffusivity_square_meters_per_year: 1e20,
                ..settings()
            }
        )
        .is_err()
    );
    assert!(
        prepare(
            &g,
            &h,
            &u,
            Settings {
                steps: 257,
                ..settings()
            }
        )
        .is_err()
    );
    g.neighbors[0] = u32::MAX;
    assert!(prepare(&g, &h, &u, settings()).is_err());
    assert_eq!(h, vec![0.; 64]);
    assert_eq!(u, vec![0.; 64]);
    assert_eq!(g.neighbors[0], u32::MAX);
}

#[test]
fn diffusion_driver_matches_independent_single_step_face_balance() {
    let g = grid(8, 500.);
    let mut h = vec![0.; 64];
    h[27] = 12.;
    h[28] = 4.;
    let dt = 100.;
    let diffusion = 0.25;
    let mut net = vec![0.; 64];
    for e in &g.side_faces {
        let a = e.a as usize;
        let b = e.b as usize;
        let q =
            dt * diffusion * e.shared_face_length_meters / e.center_distance_meters * (h[a] - h[b]);
        if !g.fixed_external_boundary[a] {
            net[a] -= q;
        }
        if !g.fixed_external_boundary[b] {
            net[b] += q;
        }
    }
    let p = prepare(
        &g,
        &h,
        &rates(&g, 0.),
        Settings {
            erodibility_per_year: 0.,
            diffusivity_square_meters_per_year: diffusion,
            step_years: dt,
            steps: 1,
        },
    )
    .unwrap();
    assert_eq!(p.diffusion_substeps_per_step, 1);
    for i in 0..64 {
        let expected = h[i] + net[i] / g.reference_areas_square_meters[i];
        assert!((p.elevation_meters[i] - expected).abs() < 1e-14);
    }
    assert_eq!(p.accepted_uplift_prism_cubic_meters, 0.);
    assert_eq!(p.detached_material_reservoir_cubic_meters, 0.);
}

#[test]
fn unrepresentable_uplift_is_observed_but_not_credited_as_material() {
    let g = grid(8, 500.);
    let h = vec![1e16; 64];
    let p = prepare(
        &g,
        &h,
        &rates(&g, 0.001),
        Settings {
            erodibility_per_year: 0.,
            diffusivity_square_meters_per_year: 0.,
            step_years: 1.,
            steps: 2,
        },
    )
    .unwrap();
    assert_eq!(p.elevation_meters, h);
    assert_eq!(p.accepted_uplift_prism_cubic_meters, 0.);
    assert_eq!(p.accepted_uplift_displacement_meters, vec![0.; 64]);
    assert!(p.history.iter().all(|s| s.unresolved_uplift_nodes == 36));
    assert_eq!(p.relative_prism_ledger_residual, 0.);
}

#[test]
fn aggregate_work_budget_refuses_valid_geometry_before_advancing() {
    let g = grid(512, 250.);
    let h = vec![0.; 512 * 512];
    let u = rates(&g, 0.001);
    let error = prepare(
        &g,
        &h,
        &u,
        Settings {
            erodibility_per_year: 0.,
            diffusivity_square_meters_per_year: 0.,
            step_years: 1.,
            steps: 256,
        },
    )
    .unwrap_err();
    assert_eq!(error, "Excessive patch preparation workload.");
    assert!(h.iter().all(|&v| v == 0.));
    assert_eq!(u, rates(&g, 0.001));
}

#[test]
fn eight_neighbor_plane_direction_error_does_not_disappear_with_refinement() {
    // A limitation witness for the pinned single-receiver algorithm, not an
    // acceptance test for a future direction-flexible routing formulation.
    let angle = 22_f64.to_radians();
    let mut observed = Vec::new();
    for spacing in [1000., 500., 250.] {
        let g = grid(16, spacing);
        let h: Vec<_> = (0..256)
            .map(|i| {
                let x = ((i % 16) as f64 - 7.5) * spacing;
                let y = ((i / 16) as f64 - 7.5) * spacing;
                10000. - 0.1 * (x * angle.cos() + y * angle.sin())
            })
            .collect();
        let r = route(&g, &h).unwrap();
        let i = 8 * 16 + 8;
        let j = r.receivers[i] as usize;
        let dx = (j % 16) as f64 - (i % 16) as f64;
        let dy = (j / 16) as f64 - (i / 16) as f64;
        let chosen = dy.atan2(dx);
        observed.push((chosen - angle).abs().to_degrees());
        assert_eq!(j, i + 1);
        assert_eq!(r.closed_sink_count, 0);
    }
    assert!(observed.iter().all(|&error| (error - 22.).abs() < 1e-12));
}

#[test]
fn facet_preparation_retains_noop_closed_basins_and_separate_model_metadata() {
    use planimulation_core::terrain_patch_landscape::{FACET_MODEL_VERSION, prepare_facets};
    let g = grid(8, 500.);
    let h = vec![17.; 64];
    let u = rates(&g, 0.);
    let p = prepare_facets(&g, &h, &u, settings()).unwrap();
    assert_eq!(p.elevation_meters, h);
    assert_eq!(p.model_version, FACET_MODEL_VERSION);
    assert_eq!(p.detached_material_reservoir_cubic_meters, 0.);
    assert_eq!(
        p.receiver_constraint.as_ref().unwrap().limited_node_updates,
        0
    );
    let old = prepare(&g, &h, &u, settings()).unwrap();
    let json = serde_json::to_value(&old).unwrap();
    assert!(json.get("receiverConstraint").is_none());
    assert!(
        json["history"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s.get("receiverConstraint").is_none())
    );
    let h: Vec<_> = g
        .fixed_external_boundary
        .iter()
        .map(|&b| if b { 100. } else { 0. })
        .collect();
    let p = prepare_facets(
        &g,
        &h,
        &u,
        Settings {
            diffusivity_square_meters_per_year: 0.,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(p.elevation_meters, h);
    assert_eq!(p.final_routing.closed_sink_count, 1);
    assert!((p.final_routing.closed_fraction_of_active_area - 1.).abs() < 1e-14);
}

#[test]
fn facet_coupling_repeats_actual_bed_ledger_and_records_hypothetical_constraint_separately() {
    use planimulation_core::terrain_patch_landscape::prepare_facets;
    let g = grid(12, 500.);
    let angle = 22_f64.to_radians();
    let h: Vec<_> = (0..144)
        .map(|i| 1000. - 50. * ((i % 12) as f64 * angle.cos() + (i / 12) as f64 * angle.sin()))
        .collect();
    let u = rates(&g, 0.0005);
    let s = Settings {
        erodibility_per_year: 1e-3,
        step_years: 1000.,
        steps: 4,
        ..settings()
    };
    let routing = planimulation_core::terrain_patch_facets::Router::new(&g)
        .unwrap()
        .route(&h)
        .unwrap();
    let stage = planimulation_core::facet_incision::incise(
        &h,
        &g.reference_areas_square_meters,
        &routing.contributing_area_square_meters,
        &routing.descents,
        planimulation_core::facet_incision::Settings {
            erodibility_per_year: s.erodibility_per_year,
            step_years: s.step_years,
        },
    )
    .unwrap();
    let mut detached = 0.;
    for (i, &height) in h.iter().enumerate() {
        assert!(stage.incision.elevation_meters[i] <= height);
        for (j, _, _) in routing.descents[i].branches() {
            assert!(stage.incision.elevation_meters[i] >= stage.incision.elevation_meters[j]);
        }
        detached +=
            (height - stage.incision.elevation_meters[i]) * g.reference_areas_square_meters[i];
    }
    assert!((detached - stage.incision.removed_material_cubic_meters).abs() / detached < 1e-12);
    let p = prepare_facets(&g, &h, &u, s).unwrap();
    let repeat = prepare_facets(&g, &h, &u, s).unwrap();
    assert_eq!(
        serde_json::to_vec(&p).unwrap(),
        serde_json::to_vec(&repeat).unwrap()
    );
    let c = p.receiver_constraint.as_ref().unwrap();
    assert!(c.limited_node_updates > 0);
    assert!(c.prevented_reference_prism_cubic_meters > 0.);
    let prevented: f64 = c
        .cumulative_prevented_incision_meters
        .iter()
        .zip(&g.reference_areas_square_meters)
        .map(|(h, a)| h * a)
        .sum();
    assert!((prevented - c.prevented_reference_prism_cubic_meters).abs() / prevented < 1e-12);
    assert_eq!(
        c.limited_node_updates,
        p.history
            .iter()
            .map(|s| s.receiver_constraint.as_ref().unwrap().limited_nodes)
            .sum::<usize>()
    );
    let mut reconstructed = vec![0.; h.len()];
    for step in &p.history {
        let c = step.receiver_constraint.as_ref().unwrap();
        assert_eq!(c.limited_nodes, c.limited_updates.len());
        for update in &c.limited_updates {
            assert!(update.prevented_incision_meters > 0. && update.accepted_incision_meters >= 0.);
            reconstructed[update.node as usize] += update.prevented_incision_meters;
        }
    }
    assert_eq!(reconstructed, c.cumulative_prevented_incision_meters);
    let mut net = 0.;
    for (i, &initial) in h.iter().enumerate() {
        if g.fixed_external_boundary[i] {
            assert_eq!(p.elevation_meters[i], initial);
        }
        let rebuilt = initial + p.accepted_uplift_displacement_meters[i]
            - p.removed_incision_thickness_meters[i]
            + p.signed_hillslope_height_change_meters[i];
        assert!((p.elevation_meters[i] - rebuilt).abs() < 1e-10);
        net += (p.elevation_meters[i] - initial) * g.reference_areas_square_meters[i];
    }
    let expected = p.accepted_uplift_prism_cubic_meters
        - p.detached_material_reservoir_cubic_meters
        + p.signed_hillslope_boundary_exchange_cubic_meters;
    assert!((net - expected).abs() / p.detached_material_reservoir_cubic_meters < 1e-12);
    assert_eq!(u, rates(&g, 0.0005));
}

#[test]
fn facet_work_limit_refuses_before_metric_construction_or_bed_advance() {
    use planimulation_core::terrain_patch_landscape::prepare_facets;
    let g = grid(512, 250.);
    let h = vec![0.; 512 * 512];
    let u = rates(&g, 0.001);
    let error = prepare_facets(
        &g,
        &h,
        &u,
        Settings {
            erodibility_per_year: 0.,
            diffusivity_square_meters_per_year: 0.,
            step_years: 1.,
            steps: 128,
        },
    )
    .unwrap_err();
    assert_eq!(error, "Excessive patch preparation workload.");
    assert!(h.iter().all(|&v| v == 0.));
}
