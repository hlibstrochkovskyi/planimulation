use planimulation_core::{
    Surface,
    geodesic_envelope::{Field, Settings, Source},
};
fn settings() -> Settings {
    Settings {
        radius_meters: 6_371_000.,
        width_meters: 300_000.,
    }
}
fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = v.iter().map(|v| v * v).sum::<f64>().sqrt();
    v.map(|v| v / n)
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-12, "{a} != {b}");
}
#[test]
fn minor_arc_distance_has_known_interior_endpoints_poles_and_complement() {
    let s = Source::new([1., 0., 0.], [0., 1., 0.], [1., 0., 0.]).unwrap();
    close(s.distance_radians(unit([1., 1., 0.])).unwrap(), 0.);
    close(
        s.distance_radians([0., 0., 1.]).unwrap(),
        std::f64::consts::FRAC_PI_2,
    );
    close(
        s.distance_radians([-1., 0., 0.]).unwrap(),
        std::f64::consts::FRAC_PI_2,
    );
    close(
        s.distance_radians(unit([1., 1., 1.])).unwrap(),
        (1_f64 / 2_f64.sqrt()).atan(),
    );
    close(
        s.distance_radians(unit([1., -1., 0.])).unwrap(),
        std::f64::consts::FRAC_PI_4,
    );
    close(
        s.distance_radians(unit([-1., -1., 0.])).unwrap(),
        3. * std::f64::consts::FRAC_PI_4,
    );
}
#[test]
fn point_sources_match_analytic_spherical_decay_and_no_mesh_paths() {
    let f = Field::new(
        vec![Source::new([1., 0., 0.], [1., 0., 0.], [6000., 2500., 1500.]).unwrap()],
        settings(),
    )
    .unwrap();
    for angle in [0., 0.1, 1., std::f64::consts::PI] {
        let p = [angle.cos(), angle.sin(), 0.];
        let r = f.sample(p).unwrap();
        for (j, a) in [6000., 2500., 1500.].iter().enumerate() {
            assert!(
                (r.values_meters[j]
                    - a * (-angle * settings().radius_meters / settings().width_meters).exp())
                .abs()
                    < 1e-10
            );
        }
    }
}
#[test]
fn accelerated_queries_match_full_scan_for_many_spherical_sources() {
    let s = Surface::build(2, 1.);
    let sources: Vec<_> = s
        .centers
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            Source::new(
                p,
                unit([p[0] + 0.03, p[1] - 0.02, p[2] + 0.01]),
                [
                    (i % 17) as f64 * 200.,
                    (i % 13) as f64 * 100.,
                    (i % 7) as f64 * 100.,
                ],
            )
            .unwrap()
        })
        .collect();
    let f = Field::new(sources, settings()).unwrap();
    let query = Surface::build(3, 1.);
    let mut savings = 0;
    for &p in &query.centers {
        let a = f.sample(p).unwrap();
        let b = f.sample_brute_force(p).unwrap();
        assert_eq!(a.values_meters, b.values_meters);
        assert_eq!(a.winner_sources, b.winner_sources);
        assert!(a.evaluated_arcs <= f.sources().len());
        savings += f.sources().len() - a.evaluated_arcs;
    }
    assert!(savings > query.centers.len() * f.sources().len() / 2);
}
#[test]
fn stronger_farther_source_can_win_and_duplicate_ties_use_original_id() {
    let sources = vec![
        Source::new([1., 0., 0.], [1., 0., 0.], [1., 0., 0.]).unwrap(),
        Source::new(unit([1., 0.02, 0.]), unit([1., 0.02, 0.]), [6000., 0., 0.]).unwrap(),
    ];
    let f = Field::new(sources.clone(), settings()).unwrap();
    assert_eq!(f.sample([1., 0., 0.]).unwrap().winner_sources[0], Some(1));
    let f = Field::new(vec![sources[1].clone(); 20], settings()).unwrap();
    assert_eq!(f.sample([1., 0., 0.]).unwrap().winner_sources[0], Some(0));
}
#[test]
fn empty_stopped_fields_are_zero_and_ambiguous_or_invalid_inputs_refuse() {
    let f = Field::new(vec![], settings()).unwrap();
    assert_eq!(f.sample([1., 0., 0.]).unwrap().values_meters, [0.; 3]);
    assert!(f.sample([0.; 3]).is_err());
    assert!(Source::new([1., 0., 0.], [-1., 0., 0.], [1.; 3]).is_err());
    // Squaring this cross product underflows. Distinct unresolved endpoints
    // must still refuse rather than silently become a coincident point source.
    assert!(Source::new([1., 0., 0.], [1., 1e-170, 0.], [1.; 3]).is_err());
    assert!(Source::new([1., 0., 0.], [0., 1., 0.], [-1.; 3]).is_err());
    assert!(
        Field::new(
            vec![],
            Settings {
                width_meters: 0.,
                ..settings()
            }
        )
        .is_err()
    );
}

#[test]
fn continuous_arc_minimum_agrees_with_independent_dense_parameter_search() {
    let a = unit([1., 2., 3.]);
    let t = unit([-a[2], 0., a[0]]);
    let angular = |p: [f64; 3], q: [f64; 3]| {
        let cross = [
            p[1] * q[2] - p[2] * q[1],
            p[2] * q[0] - p[0] * q[2],
            p[0] * q[1] - p[1] * q[0],
        ];
        cross
            .iter()
            .map(|v| v * v)
            .sum::<f64>()
            .sqrt()
            .atan2(p.iter().zip(q).map(|(&p, q)| p * q).sum())
    };
    for length in [
        0.001,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI - 1e-8,
    ] {
        let b = unit(std::array::from_fn(|i| {
            a[i] * length.cos() + t[i] * length.sin()
        }));
        let source = Source::new(a, b, [1.; 3]).unwrap();
        for p in [
            unit([1., -3., 2.]),
            a.map(|v| -v),
            [0., 1., 0.],
            [0., 0., -1.],
        ] {
            let sampled = (0..=4096)
                .map(|k| {
                    let angle = length * k as f64 / 4096.;
                    angular(
                        p,
                        unit(std::array::from_fn(|i| {
                            a[i] * angle.cos() + t[i] * angle.sin()
                        })),
                    )
                })
                .fold(f64::INFINITY, f64::min);
            let exact = source.distance_radians(p).unwrap();
            assert!(exact <= sampled + 1e-12 && sampled - exact <= length / 4096. + 1e-12);
        }
    }
}

#[test]
fn broad_arcs_near_antipodal_limits_and_seam_crossings_keep_conservative_tree_bounds() {
    let s = Surface::build(2, 1.);
    let sources: Vec<_> = s
        .centers
        .iter()
        .take(64)
        .enumerate()
        .map(|(i, &p)| {
            let axis = if p[2].abs() > 0.9 {
                [0., 1., 0.]
            } else {
                [0., 0., 1.]
            };
            let t = unit([
                p[1] * axis[2] - p[2] * axis[1],
                p[2] * axis[0] - p[0] * axis[2],
                p[0] * axis[1] - p[1] * axis[0],
            ]);
            let length = std::f64::consts::PI - 1e-8;
            let end = unit(std::array::from_fn(|j| {
                p[j] * length.cos() + t[j] * length.sin()
            }));
            Source::new(p, end, [(i % 17 + 1) as f64 * 200., 100., 0.]).unwrap()
        })
        .collect();
    let field = Field::new(sources, settings()).unwrap();
    for &p in &s.centers {
        let fast = field.sample(p).unwrap();
        let slow = field.sample_brute_force(p).unwrap();
        assert_eq!(fast.values_meters, slow.values_meters);
        assert_eq!(fast.winner_sources, slow.winner_sources);
    }
}
