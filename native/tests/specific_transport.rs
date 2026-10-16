use planimulation_core::{
    facet_incision, specific_transport as transport,
    terrain_patch_facets::{Descent, Router},
    terrain_patch_geometry::{self, Geometry},
};

fn edge(to: u32, alpha: f64) -> Descent {
    Descent {
        receivers: [to, to],
        area_weights: [1., 0.],
        derivative_coefficients_per_meter: [alpha, 0.],
        slope: 1.,
        unit_tangent_direction: [1., 0.],
        flat_fallback: false,
    }
}
fn terminal(i: u32) -> Descent {
    Descent {
        receivers: [i; 2],
        ..Descent::default()
    }
}
fn grid(n: u32, dx: f64) -> Geometry {
    terrain_patch_geometry::build(terrain_patch_geometry::Settings {
        radius_meters: 6371000.,
        center_direction: [1., 0., 0.],
        orientation_radians: 0.,
        cells_x: n,
        cells_y: n,
        spacing_meters: dx,
    })
    .unwrap()
}
#[test]
fn unequal_chain_and_split_have_independent_area_width_and_flux_budgets() {
    let d = [
        edge(1, 0.5),
        Descent {
            receivers: [2, 3],
            area_weights: [0.5, 0.5],
            derivative_coefficients_per_meter: [0.25, 0.75],
            ..edge(2, 1.)
        },
        terminal(2),
        terminal(3),
    ];
    let before = serde_json::to_vec(&d).unwrap();
    let r = transport::accumulate(&[4., 8., 2., 6.], &[false, false, false, true], &d).unwrap();
    assert_eq!(r.contributing_area_square_meters, [4., 12., 5., 9.]);
    assert_eq!(r.outgoing_width_meters, [Some(2.), Some(8.), None, None]);
    assert_eq!(r.specific_area_meters, [Some(2.), Some(1.5), None, None]);
    assert_eq!(
        r.edge_flux_square_meters,
        [[4., 0.], [3., 9.], [0.; 2], [0.; 2]]
    );
    assert_eq!(r.closed_sink_count, 1);
    assert_eq!(r.closed_fraction_of_active_area, 5. / 14.);
    assert_eq!(r.terminal_area_relative_residual, 0.);
    assert_eq!(serde_json::to_vec(&d).unwrap(), before);
    assert_eq!(
        serde_json::to_vec(&r).unwrap(),
        serde_json::to_vec(
            &transport::accumulate(&[4., 8., 2., 6.], &[false, false, false, true], &d).unwrap()
        )
        .unwrap()
    );
}
#[test]
fn axisymmetric_annular_control_recovers_the_radial_specific_area_and_width() {
    // Independent finite-volume annuli, not a claim about arbitrary 2D facets.
    // Inward flow from a circular divide R: a(r)=(R²-r²)/(2r).
    // Each discrete ring includes its own source to its inner face.
    let (dr, rings) = (10., 20usize);
    let mut areas = vec![std::f64::consts::PI * (dr / 2_f64).powi(2)];
    let mut ds = vec![terminal(0)];
    for i in 1..=rings {
        let r = i as f64 * dr;
        areas.push(std::f64::consts::PI * ((r + dr / 2.).powi(2) - (r - dr / 2.).powi(2)));
        ds.push(edge((i - 1) as u32, 1. / dr));
    }
    let r = transport::accumulate(&areas, &vec![false; rings + 1], &ds).unwrap();
    let outer = (rings as f64 + 0.5) * dr;
    for i in 1..=rings {
        let radius = i as f64 * dr;
        let expected = (outer.powi(2) - (radius - dr / 2.).powi(2)) / (2. * radius);
        assert!((r.specific_area_meters[i].unwrap() - expected).abs() < 1e-12);
        assert!(
            (r.outgoing_width_meters[i].unwrap() - 2. * std::f64::consts::PI * radius).abs()
                < 1e-12
        );
    }
    assert!(
        (r.contributing_area_square_meters[0] - std::f64::consts::PI * outer.powi(2)).abs() < 1e-10
    );
    assert_eq!(r.specific_area_meters[0], None);
}
#[test]
fn fixed_collected_area_still_has_a_width_singularity_in_a_concentrated_channel() {
    // A channel collecting the SAME 1km² into one strip is the opposite
    // limit to parallel sheet flow. This candidate does not resolve its width.
    let mut previous = None;
    for dx in [500_f64, 250., 125.] {
        let r = transport::accumulate(
            &[1_000_000. - dx * dx, dx * dx, dx * dx],
            &[false, false, true],
            &[edge(1, 1. / dx), edge(2, 1. / dx), terminal(2)],
        )
        .unwrap();
        assert_eq!(r.contributing_area_square_meters[1], 1_000_000.);
        let a = r.specific_area_meters[1].unwrap();
        assert_eq!(a, 1_000_000. / dx);
        if let Some(old) = previous {
            assert_eq!(a, 2. * old);
        }
        previous = Some(a);
    }
}
#[test]
fn derivative_shares_preserve_the_directional_first_moment() {
    let g = grid(9, 500.);
    let router = Router::new(&g).unwrap();
    for degrees in [1_f64, 22., 44., 67., 112., 181., 269., 337.] {
        let a = degrees.to_radians();
        let h: Vec<_> = g
            .cell_directions
            .iter()
            .map(|p| 10000. - 6371000. * (a.cos() * p[2] + a.sin() * p[1]))
            .collect();
        let old = router.route(&h).unwrap();
        let r = transport::route(&router, &h).unwrap();
        let i = 40;
        let d = r.descents[i];
        let alpha: f64 = d.derivative_coefficients_per_meter.iter().sum();
        let mut moment = [0.; 2];
        for (j, w, _) in d.branches() {
            let v = router.tangent_displacement(i, j).unwrap();
            for k in 0..2 {
                moment[k] += w * v[k];
            }
        }
        for (m, direction) in moment.into_iter().zip(d.unit_tangent_direction) {
            assert!((m * alpha - direction).abs() < 2e-15);
        }
        assert_eq!(
            d.derivative_coefficients_per_meter,
            old.descents[i].derivative_coefficients_per_meter
        );
        assert!(
            (r.outgoing_width_meters[i].unwrap() - g.reference_areas_square_meters[i] * alpha)
                .abs()
                < 1e-12
        );
    }
}
#[test]
fn parallel_plane_specific_pressure_and_actual_incision_do_not_shrink_with_strip_width() {
    let mut observed = Vec::new();
    for n in [40, 80, 160] {
        let dx = 20000. / f64::from(n);
        let g = grid(n, dx);
        let h: Vec<_> = (0..(n * n) as usize)
            .map(|i| 20000. - ((i % n as usize) as f64 + 0.5 - f64::from(n) / 2.) * dx)
            .collect();
        let r = transport::route(&Router::new(&g).unwrap(), &h).unwrap();
        let p = facet_incision::incise_specific(
            &h,
            &g.reference_areas_square_meters,
            &r.contributing_area_square_meters,
            &r.specific_area_meters,
            &r.descents,
            facet_incision::SpecificSettings {
                erodibility_coefficient: 2e-4,
                specific_area_exponent: 0.4,
                step_years: 0.01,
            },
        )
        .unwrap();
        let mid = n as usize / 2;
        let ids = [
            (mid - 1) * n as usize + mid - 1,
            (mid - 1) * n as usize + mid,
            mid * n as usize + mid - 1,
            mid * n as usize + mid,
        ];
        let a = ids
            .iter()
            .map(|&i| r.specific_area_meters[i].unwrap())
            .sum::<f64>()
            / 4.;
        let rate = ids
            .iter()
            .map(|&i| p.incision.removed_thickness_meters[i] / 0.01)
            .sum::<f64>()
            / 4.;
        println!("dx={dx}, specific={a}, incision_m_per_year={rate}");
        assert!((a - (10000. - dx / 2.)).abs() < 0.05);
        assert_eq!(
            p.incision.contributing_area_square_meters,
            r.contributing_area_square_meters
        );
        observed.push(rate);
    }
    assert!(observed[2] / observed[0] > 1. && observed[2] / observed[0] < 1.01);
}
#[test]
fn malformed_cyclic_overflow_and_fixed_donor_graphs_refuse_without_mutation() {
    let good = [edge(1, 1.), terminal(1)];
    let mut cases = vec![[edge(1, 1.), edge(0, 1.)]];
    for change in 0..8 {
        let mut d = good;
        match change {
            0 => d[0].receivers[0] = 2,
            1 => d[0].receivers[0] = 0,
            2 => d[0].area_weights[0] = 0.5,
            3 => d[0].derivative_coefficients_per_meter[1] = 1.,
            4 => d[0].derivative_coefficients_per_meter[0] = f64::NAN,
            5 => d[0].slope = f64::INFINITY,
            6 => {
                d[0].area_weights = [0.5, 0.5];
                d[0].derivative_coefficients_per_meter = [1.; 2];
            }
            _ => d[0].unit_tangent_direction[0] = f64::NAN,
        }
        cases.push(d);
    }
    for d in cases {
        let before = format!("{d:?}");
        assert!(transport::accumulate(&[1.; 2], &[false, true], &d).is_err());
        assert_eq!(format!("{d:?}"), before);
    }
    for areas in [[0., 1.], [f64::NAN, 1.], [f64::MAX, f64::MAX]] {
        assert!(transport::accumulate(&areas, &[false, false], &good).is_err());
    }
    assert!(transport::accumulate(&[1.; 2], &[true, true], &good).is_err());
    assert!(transport::accumulate(&[1.; 2], &[false], &good).is_err());
    assert!(transport::accumulate(&[], &[], &[]).is_err());
}
#[test]
fn specific_incision_known_link_units_and_terminal_pressure_refusals() {
    let d = [edge(1, 0.5), terminal(1)];
    let s = facet_incision::SpecificSettings {
        erodibility_coefficient: 2.,
        specific_area_exponent: 0.4,
        step_years: 3.,
    };
    let p = facet_incision::incise_specific(
        &[100., 0.],
        &[1.; 2],
        &[25.; 2],
        &[Some(32.), None],
        &d,
        s,
    )
    .unwrap();
    // 32^.4=4, gamma=3*2*4*.5=12; h=100/13.
    assert!((p.incision.elevation_meters[0] - 100. / 13.).abs() < 1e-13);
    assert_eq!(p.model_version, facet_incision::SPECIFIC_MODEL_VERSION);
    for pressure in [
        vec![],
        vec![None, None],
        vec![Some(32.), Some(1.)],
        vec![Some(f64::NAN), None],
        vec![Some(0.), None],
    ] {
        assert!(
            facet_incision::incise_specific(&[100., 0.], &[1.; 2], &[25.; 2], &pressure, &d, s)
                .is_err()
        );
    }
    let scaled_d = [edge(1, 0.5 / 10.), terminal(1)];
    let q = facet_incision::incise_specific(
        &[1000., 0.],
        &[100.; 2],
        &[2500.; 2],
        &[Some(320.), None],
        &scaled_d,
        facet_incision::SpecificSettings {
            erodibility_coefficient: s.erodibility_coefficient * 10_f64.powf(0.6),
            ..s
        },
    )
    .unwrap();
    assert!((q.incision.elevation_meters[0] / 10. - p.incision.elevation_meters[0]).abs() < 1e-13);
}
#[test]
fn specific_preparation_has_distinct_serialized_units_and_preflights_aggregate_work() {
    use planimulation_core::terrain_patch_landscape::{SpecificSettings, prepare_specific};
    let s = SpecificSettings {
        erodibility_coefficient: 500_f64.powf(-1.4),
        specific_area_exponent: 0.4,
        diffusivity_square_meters_per_year: 0.,
        step_years: 1.,
        steps: 256,
    };
    let json = serde_json::to_value(s).unwrap();
    assert_eq!(json["specificAreaExponent"], 0.4);
    assert!(json.get("areaExponent").is_none());
    let g = grid(256, 500.);
    let h = vec![0.; 256 * 256];
    let error = prepare_specific(&g, &h, &h, s).unwrap_err();
    assert!(error.contains("work"), "{error}");
    assert!(h.iter().all(|&x| x == 0.));
    let g = grid(8, 500.);
    for (k, m, dt) in [
        (f64::NAN, 0.4, 1.),
        (-1., 0.4, 1.),
        (1., -0.1, 1.),
        (1., 1.1, 1.),
        (1., 0.4, 0.),
    ] {
        assert!(
            prepare_specific(
                &g,
                &[0.; 64],
                &[0.; 64],
                SpecificSettings {
                    erodibility_coefficient: k,
                    specific_area_exponent: m,
                    step_years: dt,
                    steps: 1,
                    ..s
                }
            )
            .is_err()
        );
    }
}
#[test]
fn coupled_specific_bed_is_repeatable_accounted_and_does_not_fill_real_sinks() {
    use planimulation_core::terrain_patch_landscape::{SpecificSettings, prepare_specific};
    let g = grid(16, 500.);
    let h: Vec<_> = (0..256)
        .map(|i| {
            if g.fixed_external_boundary[i] {
                0.
            } else {
                1. + (i as f64).sin()
            }
        })
        .collect();
    let u: Vec<_> = g
        .fixed_external_boundary
        .iter()
        .map(|b| if *b { 0. } else { 0.0005 })
        .collect();
    let s = SpecificSettings {
        erodibility_coefficient: 500_f64.powf(-1.4),
        specific_area_exponent: 0.4,
        diffusivity_square_meters_per_year: 1.,
        step_years: 37500.,
        steps: 4,
    };
    let p = prepare_specific(&g, &h, &u, s).unwrap();
    assert_eq!(
        serde_json::to_vec(&p).unwrap(),
        serde_json::to_vec(&prepare_specific(&g, &h, &u, s).unwrap()).unwrap()
    );
    assert!(p.relative_prism_ledger_residual < 1e-12);
    for (i, &initial) in h.iter().enumerate() {
        assert!(
            (p.elevation_meters[i]
                - (initial + p.accepted_uplift_displacement_meters[i]
                    - p.removed_incision_thickness_meters[i]
                    + p.signed_hillslope_height_change_meters[i]))
                .abs()
                < 1e-10
        );
        if g.fixed_external_boundary[i] {
            assert_eq!(p.elevation_meters[i], h[i]);
        }
    }
    let enclosed: Vec<_> = g
        .fixed_external_boundary
        .iter()
        .map(|b| if *b { 100. } else { 0. })
        .collect();
    let p = prepare_specific(
        &g,
        &enclosed,
        &[0.; 256],
        SpecificSettings {
            diffusivity_square_meters_per_year: 0.,
            ..s
        },
    )
    .unwrap();
    assert_eq!(p.elevation_meters, enclosed);
    assert!((p.final_routing.closed_fraction_of_active_area - 1.).abs() < 1e-14);
}
