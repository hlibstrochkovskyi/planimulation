use planimulation_core::{
    facet_incision::{self, Settings, incise},
    terrain_patch_facets::Descent,
};
fn terminal(i: u32) -> Descent {
    Descent {
        receivers: [i; 2],
        ..Descent::default()
    }
}
fn edge(to: u32, unused: u32, alpha: f64) -> Descent {
    Descent {
        receivers: [to, unused],
        area_weights: [1., 0.],
        derivative_coefficients_per_meter: [alpha, 0.],
        unit_tangent_direction: [1., 0.],
        slope: 1.,
        flat_fallback: false,
    }
}
fn pair() -> Vec<Descent> {
    let a = 1. / 5_f64.sqrt();
    vec![
        Descent {
            receivers: [1, 2],
            area_weights: [0.5, 0.5],
            derivative_coefficients_per_meter: [a, a],
            unit_tangent_direction: [2. * a, a],
            slope: 1.25_f64.sqrt(),
            flat_fallback: false,
        },
        terminal(1),
        terminal(2),
    ]
}
fn settings() -> Settings {
    Settings {
        erodibility_per_year: 1.,
        step_years: 1.,
    }
}
#[test]
fn single_receiver_reduces_to_known_link_and_downstream_first_chain() {
    let r = incise(
        &[10., 0.],
        &[1.; 2],
        &[1., 0.],
        &[edge(1, 0, 1.), terminal(1)],
        settings(),
    )
    .unwrap();
    assert_eq!(r.incision.elevation_meters, [5., 0.]);
    assert_eq!(r.incision.removed_material_cubic_meters, 5.);
    assert_eq!(r.receiver_constraint.limited_nodes, 0);
    let r = incise(
        &[9., 6., 0.],
        &[4., 5., 1.],
        &[4., 9., 0.],
        &[edge(1, 0, 1.), edge(2, 1, 1.), terminal(2)],
        settings(),
    )
    .unwrap();
    assert_eq!(r.incision.elevation_meters, [4., 1.5, 0.]);
    assert_eq!(r.incision.removed_material_cubic_meters, 42.5);
}
#[test]
fn two_receiver_formula_has_independent_unlimited_solution() {
    let h = [10., 9., 8.5];
    let r = incise(
        &h,
        &[2., 1., 1.],
        &[1., 0., 0.],
        &pair(),
        Settings {
            erodibility_per_year: 5_f64.sqrt() / 2.,
            ..settings()
        },
    )
    .unwrap();
    assert!((r.incision.elevation_meters[0] - 9.375).abs() < 1e-14);
    assert!((r.incision.removed_material_cubic_meters - 1.25).abs() < 1e-14);
    assert_eq!(r.receiver_constraint.limited_nodes, 0);
    assert_eq!(h, [10., 9., 8.5]);
}
#[test]
fn explicit_receiver_bound_records_prevented_not_detached_material() {
    let r = incise(
        &[10., 9., 8.5],
        &[2., 1., 1.],
        &[1., 0., 0.],
        &pair(),
        Settings {
            erodibility_per_year: 4. * 5_f64.sqrt(),
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(r.incision.elevation_meters, [9., 9., 8.5]);
    assert_eq!(r.incision.removed_material_cubic_meters, 2.);
    assert_eq!(r.receiver_constraint.limited_nodes, 1);
    assert!((r.receiver_constraint.prevented_incision_meters[0] - 1. / 9.).abs() < 1e-14);
    assert!((r.receiver_constraint.prevented_reference_prism_cubic_meters - 2. / 9.).abs() < 1e-14);
    assert_eq!(r.incision.unresolved_positive_incision_nodes, 0);
    assert_eq!(r.model_version, facet_incision::MODEL_VERSION);
}
#[test]
fn time_refinement_matches_a_frozen_two_outlet_exponential_before_constraint() {
    let exact = 8.75 + 1.25 * (-0.25_f64).exp();
    let mut previous = f64::INFINITY;
    for steps in [1, 2, 4, 8, 16, 32] {
        let mut h = vec![10., 9., 8.5];
        for _ in 0..steps {
            let r = incise(
                &h,
                &[1.; 3],
                &[1., 0., 0.],
                &pair(),
                Settings {
                    erodibility_per_year: 5_f64.sqrt() / 2.,
                    step_years: 0.25 / steps as f64,
                },
            )
            .unwrap();
            assert_eq!(r.receiver_constraint.limited_nodes, 0);
            h = r.incision.elevation_meters;
        }
        let error = (h[0] - exact).abs();
        assert!(error < previous);
        previous = error;
    }
}
#[test]
fn zero_forcing_terminals_and_subprecision_loss_keep_their_own_semantics() {
    let h = [10., 9., 8.5];
    let r = incise(
        &h,
        &[1.; 3],
        &[1., 0., 0.],
        &pair(),
        Settings {
            erodibility_per_year: 0.,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(r.incision.elevation_meters, h);
    assert_eq!(r.receiver_constraint.limited_nodes, 0);
    let r = incise(
        &[1e9, 0.],
        &[1.; 2],
        &[1., 0.],
        &[edge(1, 0, 1.), terminal(1)],
        Settings {
            erodibility_per_year: 1e-30,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(r.incision.elevation_meters, [1e9, 0.]);
    assert_eq!(r.incision.removed_material_cubic_meters, 0.);
    assert_eq!(r.incision.unresolved_positive_incision_nodes, 1);
    assert_eq!(
        r.receiver_constraint.prevented_reference_prism_cubic_meters,
        0.
    );
}
#[test]
fn malformed_cycles_partitions_overflows_and_uphill_graphs_refuse_immutably() {
    let h = [10., 9., 8.5];
    let original = pair();
    for k in 0..8 {
        let mut d = original.clone();
        match k {
            0 => d[0].receivers[0] = 99,
            1 => d[0].receivers[1] = 1,
            2 => d[0].area_weights = [0.5, 0.2],
            3 => d[0].derivative_coefficients_per_meter[0] = f64::NAN,
            4 => d[1] = edge(0, 1, 1.),
            5 => d[0].area_weights = [0., 1.],
            6 => d[0].derivative_coefficients_per_meter = [f64::MAX; 2],
            7 => d[0].unit_tangent_direction[0] = f64::INFINITY,
            _ => unreachable!(),
        }
        assert!(
            incise(
                &h,
                &[1.; 3],
                &[1., 0., 0.],
                &d,
                Settings {
                    erodibility_per_year: 0.,
                    ..settings()
                }
            )
            .is_err()
        );
    }
    assert!(
        incise(
            &[1., 1.],
            &[1.; 2],
            &[1.; 2],
            &[edge(1, 0, 1.), edge(0, 1, 1.)],
            settings()
        )
        .is_err()
    );
    assert!(incise(&h, &[1.; 2], &[1., 0., 0.], &original, settings()).is_err());
    assert!(
        incise(
            &h,
            &[1.; 3],
            &[1., 0., 0.],
            &original,
            Settings {
                erodibility_per_year: f64::MAX,
                step_years: 2.
            }
        )
        .is_err()
    );
    assert_eq!(h, [10., 9., 8.5]);
}

#[test]
fn power_law_matches_independent_integer_power_links_and_zero_exponent() {
    use facet_incision::{PowerSettings, incise_power};
    let law = PowerSettings {
        erodibility_coefficient: 1.,
        area_exponent: 0.4,
        step_years: 1.,
    };
    // 32^(2/5) = 4, hence 20/(1+4) = 4 with a fixed zero outlet.
    let r = incise_power(
        &[20., 0.],
        &[2., 1.],
        &[32., 0.],
        &[edge(1, 0, 1.), terminal(1)],
        law,
    )
    .unwrap();
    assert!((r.incision.elevation_meters[0] - 4.).abs() < 1e-14);
    assert!((r.incision.removed_material_cubic_meters - 32.).abs() < 1e-14);
    assert_eq!(r.model_version, facet_incision::POWER_MODEL_VERSION);
    for a in [1e-12, 1., 1e12] {
        let r = incise_power(
            &[20., 0.],
            &[2., 1.],
            &[a, 0.],
            &[edge(1, 0, 1.), terminal(1)],
            PowerSettings {
                area_exponent: 0.,
                ..law
            },
        )
        .unwrap();
        assert_eq!(r.incision.elevation_meters, [10., 0.]);
    }
}
#[test]
fn square_root_power_path_preserves_old_fields_and_constraint_exactly() {
    use facet_incision::{PowerSettings, incise_power};
    for k in [0., 0.01, 1., 100.] {
        let old = incise(
            &[10., 9., 8.5],
            &[2., 1., 1.],
            &[32., 0., 0.],
            &pair(),
            Settings {
                erodibility_per_year: k,
                step_years: 1.,
            },
        )
        .unwrap();
        let new = incise_power(
            &[10., 9., 8.5],
            &[2., 1., 1.],
            &[32., 0., 0.],
            &pair(),
            PowerSettings {
                erodibility_coefficient: k,
                area_exponent: 0.5,
                step_years: 1.,
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(old.incision).unwrap(),
            serde_json::to_value(new.incision).unwrap()
        );
        assert_eq!(
            serde_json::to_value(old.receiver_constraint).unwrap(),
            serde_json::to_value(new.receiver_constraint).unwrap()
        );
    }
}
#[test]
fn coefficient_units_preserve_similarity_under_horizontal_and_vertical_scaling() {
    use facet_incision::{PowerSettings, incise_power};
    let law = PowerSettings {
        erodibility_coefficient: 0.1,
        area_exponent: 0.4,
        step_years: 1.,
    };
    let r = incise_power(&[10., 9., 8.5], &[2., 1., 1.], &[32., 0., 0.], &pair(), law).unwrap();
    // Horizontal length x32 => area x1024 and derivatives /32. K x2 because
    // 32^(1-2m)=2. Height x4 => accepted reference prisms x4096.
    let mut d = pair();
    for link in &mut d {
        for a in &mut link.derivative_coefficients_per_meter {
            *a /= 32.;
        }
    }
    let scaled = incise_power(
        &[40., 36., 34.],
        &[2048., 1024., 1024.],
        &[32768., 0., 0.],
        &d,
        PowerSettings {
            erodibility_coefficient: 0.2,
            ..law
        },
    )
    .unwrap();
    for (a, b) in r
        .incision
        .elevation_meters
        .iter()
        .zip(&scaled.incision.elevation_meters)
    {
        assert!((4. * a - b).abs() < 1e-13);
    }
    assert!(
        (scaled.incision.removed_material_cubic_meters
            / (4096. * r.incision.removed_material_cubic_meters)
            - 1.)
            .abs()
            < 1e-13
    );
}
#[test]
fn power_law_time_refinement_converges_to_independent_fixed_graph_ode() {
    use facet_incision::{PowerSettings, incise_power};
    // A=32, m=.4 => A^m=4. Sum(alpha)=2/sqrt(5); K=sqrt(5)/8
    // gives unit decay rate. This solution stays above the high outlet.
    let exact = 8.75 + 1.25 * (-0.25_f64).exp();
    let mut previous = f64::INFINITY;
    for steps in [1, 2, 4, 8, 16, 32] {
        let mut h = vec![10., 9., 8.5];
        for _ in 0..steps {
            let r = incise_power(
                &h,
                &[1.; 3],
                &[32., 0., 0.],
                &pair(),
                PowerSettings {
                    erodibility_coefficient: 5_f64.sqrt() / 8.,
                    area_exponent: 0.4,
                    step_years: 0.25 / steps as f64,
                },
            )
            .unwrap();
            assert_eq!(r.receiver_constraint.limited_nodes, 0);
            h = r.incision.elevation_meters;
        }
        let error = (h[0] - exact).abs();
        assert!(error < previous);
        previous = error;
    }
}
#[test]
fn power_law_validation_and_receiver_cap_are_not_bypassed_by_exponent_changes() {
    use facet_incision::{PowerSettings, incise_power};
    let law = PowerSettings {
        erodibility_coefficient: 0.,
        area_exponent: 0.4,
        step_years: 1.,
    };
    for m in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
        assert!(
            incise_power(
                &[10., 9., 8.5],
                &[1.; 3],
                &[32., 0., 0.],
                &pair(),
                PowerSettings {
                    area_exponent: m,
                    ..law
                }
            )
            .is_err()
        );
    }
    let mut cycle = pair();
    cycle[1] = edge(0, 1, 1.);
    assert!(incise_power(&[10., 10., 8.5], &[1.; 3], &[32., 1., 0.], &cycle, law).is_err());
    let r = incise_power(
        &[10., 9., 8.5],
        &[2., 1., 1.],
        &[32., 0., 0.],
        &pair(),
        PowerSettings {
            erodibility_coefficient: 100.,
            ..law
        },
    )
    .unwrap();
    assert_eq!(r.incision.elevation_meters, [9., 9., 8.5]);
    assert_eq!(r.incision.removed_material_cubic_meters, 2.);
    assert!(r.receiver_constraint.prevented_reference_prism_cubic_meters > 0.);
    assert!(
        incise_power(
            &[10., 0.],
            &[1.; 2],
            &[f64::MAX, 0.],
            &[edge(1, 0, 1.), terminal(1)],
            PowerSettings {
                erodibility_coefficient: f64::MAX,
                area_exponent: 1.,
                ..law
            }
        )
        .is_err()
    );
}
