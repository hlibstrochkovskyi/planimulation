use planimulation_core::{
    terrain_patch_facets::Router,
    terrain_patch_geometry::{self, Geometry, Settings},
};
fn grid(center: [f64; 3], orientation: f64, spacing: f64) -> Geometry {
    terrain_patch_geometry::build(Settings {
        radius_meters: 6371000.,
        center_direction: center,
        orientation_radians: orientation,
        cells_x: 9,
        cells_y: 9,
        spacing_meters: spacing,
    })
    .unwrap()
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
// Independent log-map height oracle about the central cell, whose local
// gradient there is exactly the supplied global tangent vector times slope.
fn tangent_plane(g: &Geometry, direction: [f64; 3], slope: f64) -> Vec<f64> {
    let center = g.cell_directions[40];
    g.cell_directions
        .iter()
        .map(|&p| {
            let cosine = dot(center, p);
            let projection: [f64; 3] = std::array::from_fn(|k| p[k] - center[k] * cosine);
            let sine = dot(projection, projection).sqrt();
            let height = if sine == 0. {
                0.
            } else {
                sine.atan2(cosine) * g.settings.radius_meters * dot(projection, direction) / sine
            };
            10000. - slope * height
        })
        .collect()
}
#[test]
fn affine_tangent_planes_recover_direction_and_slope_between_grid_axes() {
    for spacing in [1000., 500., 250.] {
        for orientation in [0., 0.137, std::f64::consts::FRAC_PI_4, 2.1] {
            for (center, east, north) in [
                ([1., 0., 0.], [0., 0., 1.], [0., 1., 0.]),
                ([-1., 0., 0.], [0., 0., -1.], [0., 1., 0.]),
                ([0., 1., 0.], [1., 0., 0.], [0., 0., 1.]),
                ([0., -1., 0.], [-1., 0., 0.], [0., 0., 1.]),
            ] {
                let g = grid(center, orientation, spacing);
                let router = Router::new(&g).unwrap();
                for degrees in [
                    0_f64, 1., 11., 22., 23., 44., 45., 67., 89., 112., 181., 269., 337.,
                ] {
                    let a = degrees.to_radians();
                    let direction = std::array::from_fn(|k| east[k] * a.cos() + north[k] * a.sin());
                    let h = tangent_plane(&g, direction, 0.1);
                    let r = router.route(&h).unwrap();
                    let d = &r.descents[40];
                    let actual = d.unit_tangent_direction;
                    assert!(
                        (actual[0] - a.cos()).hypot(actual[1] - a.sin()) < 1e-10,
                        "{degrees}deg, dx{spacing}, orientation{orientation}, dir{actual:?}"
                    );
                    assert!((d.slope - 0.1).abs() < 1e-11);
                    assert!(r.terminal_area_relative_residual < 1e-12);
                }
            }
        }
    }
}
#[test]
fn angular_area_weights_and_directional_derivatives_are_distinct_correct_quantities() {
    let g = grid([1., 0., 0.], 0., 500.);
    let a = 22_f64.to_radians();
    let h = tangent_plane(&g, [0., a.sin(), a.cos()], 0.1);
    let r = Router::new(&g).unwrap().route(&h).unwrap();
    let d = r.descents[40];
    assert_eq!(d.receivers, [41, 50]);
    assert!((d.area_weights[0] - (1. - 22. / 45.)).abs() < 1e-11);
    assert!((d.area_weights[1] - 22. / 45.).abs() < 1e-11);
    assert!((d.area_weights.iter().sum::<f64>() - 1.).abs() < 1e-14);
    let slope: f64 = d
        .branches()
        .map(|(j, _, alpha)| alpha * (h[40] - h[j]))
        .sum();
    assert!((slope - 0.1).abs() < 1e-11);
    let alpha_sum: f64 = d.derivative_coefficients_per_meter.iter().sum();
    assert!((d.derivative_coefficients_per_meter[1] / alpha_sum - d.area_weights[1]).abs() > 0.01);
}
#[test]
fn flat_real_boundary_exits_and_enclosed_flats_keep_their_physical_semantics() {
    let g = grid([1., 0., 0.], 0., 500.);
    let router = Router::new(&g).unwrap();
    let r = router.route(&vec![0.; 81]).unwrap();
    assert_eq!(r.closed_sink_count, 0);
    assert_eq!(r.closed_fraction_of_active_area, 0.);
    assert_eq!(r.flat_fallback_nodes, 49);
    assert_eq!(r.two_receiver_nodes, 0);
    let h: Vec<_> = g
        .fixed_external_boundary
        .iter()
        .map(|&b| if b { 100. } else { 0. })
        .collect();
    let r = router.route(&h).unwrap();
    assert_eq!(r.closed_sink_count, 1);
    assert!((r.closed_fraction_of_active_area - 1.).abs() < 1e-14);
    assert_eq!(r.flat_fallback_nodes, 48);
}
#[test]
fn generated_branches_are_adjacent_downhill_acyclic_and_area_reconciles_independently() {
    let g = grid([0., 1., 0.], 0.391, 500.);
    let h: Vec<_> = (0..81)
        .map(|i| ((i * 19 % 71) as f64).sin() * 300. + i as f64)
        .collect();
    let router = Router::new(&g).unwrap();
    let r = router.route(&h).unwrap();
    let again = router.route(&h).unwrap();
    assert_eq!(
        serde_json::to_vec(&r).unwrap(),
        serde_json::to_vec(&again).unwrap()
    );
    let mut rank = vec![0; 81];
    for (index, &id) in r.upstream_to_downstream.iter().enumerate() {
        rank[id as usize] = index;
    }
    let mut probabilities = vec![0.; 81];
    let mut terminal_area = 0.;
    for &i in r.upstream_to_downstream.iter().rev() {
        let i = i as usize;
        let d = &r.descents[i];
        let mut sum = 0.;
        for (j, w, alpha) in d.branches() {
            assert!(
                g.neighbors[g.offsets[i] as usize..g.offsets[i + 1] as usize].contains(&(j as u32))
            );
            assert!(h[j] < h[i] || d.flat_fallback && h[j] == h[i]);
            assert!(alpha > 0. && alpha.is_finite() && w > 0. && w <= 1.);
            assert!(rank[i] < rank[j]);
            sum += w;
            probabilities[i] += w * probabilities[j];
        }
        if sum == 0. {
            terminal_area += r.contributing_area_square_meters[i];
            probabilities[i] = f64::from(!g.fixed_external_boundary[i]);
        } else {
            assert!((sum - 1.).abs() < 1e-14);
        }
    }
    let active: f64 = g
        .reference_areas_square_meters
        .iter()
        .enumerate()
        .filter(|(i, _)| !g.fixed_external_boundary[*i])
        .map(|(_, a)| a)
        .sum();
    let closed: f64 = g
        .reference_areas_square_meters
        .iter()
        .enumerate()
        .filter(|(i, _)| !g.fixed_external_boundary[*i])
        .map(|(i, a)| a * probabilities[i])
        .sum();
    assert!((terminal_area - active).abs() / active < 1e-12);
    assert!((closed / active - r.closed_fraction_of_active_area).abs() < 1e-12);
    assert!(r.two_receiver_nodes > 0);
}

#[test]
fn one_donor_can_have_fractional_ancestry_to_a_real_sink_and_external_boundary() {
    let g = grid([1., 0., 0.], 0., 500.);
    let mut h = vec![1000.; 81];
    for (i, height) in [
        (40, 10.),
        (41, 1.),
        (50, 0.),
        (42, -100.),
        (59, -200.),
        (68, -300.),
        (77, -400.),
    ] {
        h[i] = height;
    }
    let r = Router::new(&g).unwrap().route(&h).unwrap();
    assert_eq!(r.descents[40].receivers, [41, 50]);
    assert_eq!(r.descents[41].receivers[0], 42);
    assert_eq!(r.descents[50].receivers[0], 59);
    assert_eq!(r.closed_sink_count, 1);
    let mut probability = vec![0.; 81];
    for &i in r.upstream_to_downstream.iter().rev() {
        let i = i as usize;
        probability[i] = if i == 42 {
            1.
        } else {
            r.descents[i]
                .branches()
                .map(|(j, weight, _)| weight * probability[j])
                .sum()
        };
    }
    assert!(probability[40] > 0. && probability[40] < 1.);
    assert!((probability[40] - r.descents[40].area_weights[0]).abs() < 1e-14);
}
#[test]
fn invalid_fields_and_geometry_refuse_and_owned_geometry_is_immutable() {
    let mut g = grid([1., 0., 0.], 0., 500.);
    let router = Router::new(&g).unwrap();
    let h = vec![0.; 81];
    let before = serde_json::to_vec(&router.route(&h).unwrap()).unwrap();
    g.cell_directions[0] = [f64::NAN; 3];
    assert!(Router::new(&g).is_err());
    assert_eq!(
        serde_json::to_vec(&router.route(&h).unwrap()).unwrap(),
        before
    );
    assert!(router.route(&h[..80]).is_err());
    let mut bad = h.clone();
    bad[40] = f64::NAN;
    assert!(router.route(&bad).is_err());
    assert_eq!(h, vec![0.; 81]);
}

#[test]
fn unsupported_work_and_extreme_arithmetic_refuse_without_hidden_geometry_growth() {
    let mut g = grid([1., 0., 0.], 0., 500.);
    g.settings.cells_x = u32::MAX;
    assert!(Router::new(&g).is_err());
    g = grid([1., 0., 0.], 0., 500.);
    g.vertex_directions.pop();
    assert!(Router::new(&g).is_err());
    let g = grid([1., 0., 0.], 0., 500.);
    let router = Router::new(&g).unwrap();
    let mut h = vec![f64::MAX; 81];
    h[40] = -f64::MAX;
    assert!(router.route(&h).is_err());
    assert_eq!(h[40], -f64::MAX);
    assert!(router.tangent_displacement(usize::MAX, 0).is_none());
    assert!(router.tangent_displacement(0, 1).is_none());
    assert!(router.tangent_displacement(40, 40).is_none());
}

#[test]
fn unconstrained_two_receiver_implicit_formula_can_reverse_a_valid_facet_branch() {
    let g = grid([1., 0., 0.], 0., 500.);
    let angle = 22_f64.to_radians();
    let h = tangent_plane(&g, [0., angle.sin(), angle.cos()], 0.1);
    let r = Router::new(&g).unwrap().route(&h).unwrap();
    let d = r.descents[40];
    assert!(d.area_weights.iter().all(|&w| w > 0.));
    let alpha_sum: f64 = d.derivative_coefficients_per_meter.iter().sum();
    let base: f64 = d
        .branches()
        .map(|(j, _, alpha)| alpha / alpha_sum * h[j])
        .sum();
    let high = d
        .branches()
        .map(|(j, _, _)| h[j])
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(h[40] > high);
    // Frozen outlet values and gamma100 are sufficient to expose the issue;
    // the unconstrained weighted-base formula is not an accepted eroder.
    let unconstrained = base + (h[40] - base) / 101.;
    assert!(unconstrained < high);
    assert!(unconstrained > base);
}

#[test]
fn planar_total_area_and_current_incision_pressure_retain_a_sampling_width_dependence() {
    use planimulation_core::facet_incision;
    let mut observations = Vec::new();
    for cells in [40u32, 80, 160] {
        let spacing = 20000. / f64::from(cells);
        let g = terrain_patch_geometry::build(Settings {
            radius_meters: 6371000.,
            center_direction: [1., 0., 0.],
            orientation_radians: 0.,
            cells_x: cells,
            cells_y: cells,
            spacing_meters: spacing,
        })
        .unwrap();
        let n = cells as usize;
        let h: Vec<_> = (0..n * n)
            .map(|i| 20000. - (i % n) as f64 * spacing - (0.5 - f64::from(cells) / 2.) * spacing)
            .collect();
        let flow = Router::new(&g).unwrap().route(&h).unwrap();
        assert_eq!(flow.closed_sink_count, 0);
        assert!(flow.terminal_area_relative_residual < 1e-12);
        let dt = 0.01;
        let incision = facet_incision::incise_power(
            &h,
            &g.reference_areas_square_meters,
            &flow.contributing_area_square_meters,
            &flow.descents,
            facet_incision::PowerSettings {
                erodibility_coefficient: 2e-4,
                area_exponent: 0.4,
                step_years: dt,
            },
        )
        .unwrap();
        let central = [
            (n / 2 - 1) * n + n / 2 - 1,
            (n / 2 - 1) * n + n / 2,
            (n / 2) * n + n / 2 - 1,
            (n / 2) * n + n / 2,
        ];
        // Interpolate the SAME chart point (0,0), not a cell center that moves
        // with resolution. Fixed collar nodes contribute no source area.
        let area = central
            .iter()
            .map(|&i| flow.contributing_area_square_meters[i])
            .sum::<f64>()
            / 4.;
        let rate = central
            .iter()
            .map(|&i| incision.incision.removed_thickness_meters[i] / dt)
            .sum::<f64>()
            / 4.;
        let expected_length = 10000. - spacing / 2.;
        assert!((area / spacing / expected_length - 1.).abs() < 1e-4);
        eprintln!(
            "planar width witness: spacing={spacing}m, area={area}m2, area/spacing={}m, accepted_rate={rate}m/year",
            area / spacing
        );
        observations.push((area, rate));
    }
    for pair in observations.windows(2) {
        assert!((0.49..0.52).contains(&(pair[1].0 / pair[0].0)));
        assert!((0.74..0.79).contains(&(pair[1].1 / pair[0].1)));
    }
    // This freezes a limitation of the current total-area model. The A/dx
    // diagnostic is a LENGTH; it must not silently replace physical area or
    // conserve the same units under the existing K/recipe/model identity.
}
