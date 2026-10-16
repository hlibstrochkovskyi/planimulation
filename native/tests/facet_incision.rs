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
