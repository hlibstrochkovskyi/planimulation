use planimulation_core::{
    Random, drainage_landforms, irregular_patch,
    terrain_patch_geometry::{self, Settings},
};
use std::collections::BTreeMap;

fn settings() -> Settings {
    Settings {
        radius_meters: 6371000.,
        center_direction: [1., 0., 0.],
        orientation_radians: 0.,
        cells_x: 24,
        cells_y: 18,
        spacing_meters: 250.,
    }
}

#[test]
fn mesh_repeats_exactly_and_has_an_independent_recorded_random_stream() {
    let a = irregular_patch::build("first-light", settings()).unwrap();
    let mut unrelated = Random::stream("first-light", "weather");
    for _ in 0..1000 {
        unrelated.next_u32();
    }
    let b = irregular_patch::build("first-light", settings()).unwrap();
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        serde_json::to_value(&b).unwrap()
    );
    assert_eq!(a.triangulator_version, "spade-2.15.1");
    assert_eq!(a.random_version, "fnv1a-utf8-mulberry32-1");
    assert!(a.random_stream.contains(irregular_patch::MODEL_VERSION));
    let other = irregular_patch::build("different-world", settings()).unwrap();
    assert_ne!(a.chart_points_meters, other.chart_points_meters);
    let boundary = a.fixed_external_boundary.iter().filter(|&&v| v).count();
    assert_eq!(
        boundary,
        other.fixed_external_boundary.iter().filter(|&&v| v).count()
    );
    assert_eq!(
        &a.chart_points_meters[..boundary],
        &other.chart_points_meters[..boundary]
    );
}

#[test]
fn independent_pairwise_separation_and_fixed_perimeter_controls() {
    let s = settings();
    let g = irregular_patch::build("separation", s).unwrap();
    let regular = terrain_patch_geometry::build(s).unwrap();
    let boundary = 2 * (s.cells_x + s.cells_y) as usize - 4;
    assert_eq!(
        g.fixed_external_boundary.iter().filter(|&&v| v).count(),
        boundary
    );
    let expected: Vec<_> = (0..s.cells_y)
        .flat_map(|row| (0..s.cells_x).map(move |col| (row, col)))
        .filter(|&(row, col)| row == 0 || col == 0 || row + 1 == s.cells_y || col + 1 == s.cells_x)
        .map(|(row, col)| {
            [
                (f64::from(col) + 0.5 - f64::from(s.cells_x) / 2.) * s.spacing_meters,
                (f64::from(row) + 0.5 - f64::from(s.cells_y) / 2.) * s.spacing_meters,
            ]
        })
        .collect();
    assert_eq!(&g.chart_points_meters[..boundary], expected);
    for (i, p) in g.point_directions[..boundary].iter().enumerate() {
        let j = regular
            .fixed_external_boundary
            .iter()
            .enumerate()
            .filter(|(_, v)| **v)
            .nth(i)
            .unwrap()
            .0;
        for (a, b) in p.iter().zip(regular.cell_directions[j]) {
            assert!((a - b).abs() < 1e-15);
        }
    }
    for i in boundary..g.chart_points_meters.len() {
        let p = g.chart_points_meters[i];
        assert!(p[0] >= expected[0][0] + 0.65 * s.spacing_meters);
        assert!(p[1] >= expected[0][1] + 0.65 * s.spacing_meters);
        for q in &g.chart_points_meters[boundary..i] {
            assert!((p[0] - q[0]).hypot(p[1] - q[1]) >= 0.8 * s.spacing_meters * (1. - 1e-13));
        }
    }
}

fn check_geometry(g: &irregular_patch::Geometry) {
    let n = g.chart_points_meters.len();
    let boundary = g.fixed_external_boundary.iter().filter(|&&v| v).count();
    assert_eq!(g.triangles.len(), 2 * n - 2 - boundary);
    assert_eq!(g.edges.len(), 3 * n - 3 - boundary);
    let mut edges = BTreeMap::<(u32, u32), usize>::new();
    let mut chart_area = 0.;
    let mut nodal = vec![false; n];
    for &t in &g.triangles {
        assert!(t[0] < t[1] && t[0] < t[2]);
        let [a, b, c] = t.map(|i| g.chart_points_meters[i as usize]);
        let determinant = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        assert!(determinant > 0.);
        chart_area += determinant / 2.;
        for i in t {
            nodal[i as usize] = true;
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    assert!(nodal.iter().all(|v| *v));
    assert!(g.triangles.windows(2).all(|t| t[0] < t[1]));
    let s = g.settings;
    let width = f64::from(s.cells_x - 1) * s.spacing_meters;
    let height = f64::from(s.cells_y - 1) * s.spacing_meters;
    assert!((chart_area - width * height).abs() / (width * height) < 1e-13);
    // Analytic integral of the gnomonic area Jacobian over a symmetric rectangle.
    let a = width / (2. * s.radius_meters);
    let b = height / (2. * s.radius_meters);
    let exact =
        4. * (a * b / (1. + a * a + b * b).sqrt()).atan() * s.radius_meters * s.radius_meters;
    let sum: f64 = g.reference_areas_square_meters.iter().sum();
    assert!((sum - exact).abs() / exact < 2e-11, "{sum} vs {exact}");
    assert!((sum - g.total_spherical_area_square_meters).abs() / sum < 1e-13);
    assert!(
        g.reference_areas_square_meters
            .iter()
            .all(|a| a.is_finite() && *a > 0.)
    );
    for (key, e) in edges.keys().zip(&g.edges) {
        assert_eq!(*key, (e.a, e.b));
        let p = g.point_directions[e.a as usize];
        let q = g.point_directions[e.b as usize];
        let chord = p
            .into_iter()
            .zip(q)
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f64>()
            .sqrt();
        let expected = 2. * (chord / 2.).asin() * s.radius_meters;
        assert!((e.distance_meters - expected).abs() / expected < 2e-11);
    }
    assert!(
        g.point_directions
            .iter()
            .all(|p| (p.iter().map(|v| v * v).sum::<f64>() - 1.).abs() < 1e-12)
    );
}

#[test]
fn spherical_area_distance_incidence_and_tiny_domains_have_independent_oracles() {
    for s in [
        settings(),
        Settings {
            cells_x: 4,
            cells_y: 4,
            spacing_meters: 10.,
            ..settings()
        },
        Settings {
            cells_x: 12,
            cells_y: 8,
            radius_meters: 100000.,
            spacing_meters: 400.,
            ..settings()
        },
    ] {
        check_geometry(&irregular_patch::build("geometry", s).unwrap());
    }
}

#[test]
fn physical_rescaling_and_orientation_do_not_reroll_chart_sampling() {
    let s = settings();
    let a = irregular_patch::build("scale", s).unwrap();
    let b = irregular_patch::build(
        "scale",
        Settings {
            radius_meters: s.radius_meters * 2.,
            spacing_meters: s.spacing_meters * 2.,
            ..s
        },
    )
    .unwrap();
    assert_eq!(a.triangles, b.triangles);
    assert_eq!(a.point_directions, b.point_directions);
    for (p, q) in a.chart_points_meters.iter().zip(&b.chart_points_meters) {
        assert_eq!(p.map(|v| v * 2.), *q);
    }
    for (x, y) in a
        .reference_areas_square_meters
        .iter()
        .zip(&b.reference_areas_square_meters)
    {
        assert_eq!(x * 4., *y);
    }
    for (x, y) in a.edges.iter().zip(&b.edges) {
        assert_eq!(x.distance_meters * 2., y.distance_meters);
    }
    let rotated = irregular_patch::build(
        "scale",
        Settings {
            orientation_radians: 1.234,
            ..s
        },
    )
    .unwrap();
    assert_eq!(a.chart_points_meters, rotated.chart_points_meters);
    assert_eq!(a.triangles, rotated.triangles);
    for (x, y) in a
        .reference_areas_square_meters
        .iter()
        .zip(&rotated.reference_areas_square_meters)
    {
        assert!((x - y).abs() / x < 2e-11);
    }
    check_geometry(&rotated);
}

#[test]
fn seam_and_pole_frames_keep_finite_physical_geometry() {
    for center in [[-1., 0., 0.], [0., 1., 0.], [0., -1., 0.]] {
        let g = irregular_patch::build(
            "pole",
            Settings {
                center_direction: center,
                cells_x: 12,
                cells_y: 10,
                ..settings()
            },
        )
        .unwrap();
        check_geometry(&g);
    }
}

#[test]
fn regular_source_bilinear_interpolant_has_an_independent_tensor_polynomial_oracle() {
    let s = settings();
    let dx = s.spacing_meters;
    let polynomial = |x: f64, y: f64| 10. + 0.01 * x + 0.02 * y + 0.000001 * x * y;
    let field: Vec<_> = (0..s.cells_y)
        .flat_map(|row| {
            (0..s.cells_x).map(move |column| {
                polynomial(
                    (f64::from(column) + 0.5 - f64::from(s.cells_x) / 2.) * dx,
                    (f64::from(row) + 0.5 - f64::from(s.cells_y) / 2.) * dx,
                )
            })
        })
        .collect();
    let mesh = irregular_patch::build("interpolation", s).unwrap();
    let sampled =
        irregular_patch::sample_regular_source(s, &field, &mesh.chart_points_meters).unwrap();
    for (&got, &[x, y]) in sampled.iter().zip(&mesh.chart_points_meters) {
        assert!((got - polynomial(x, y)).abs() < 1e-12);
    }
    for constant in [0., 12., 123.456, 1e7] {
        assert_eq!(
            irregular_patch::sample_regular_source(
                s,
                &vec![constant; field.len()],
                &mesh.chart_points_meters
            )
            .unwrap(),
            vec![constant; sampled.len()]
        );
    }
    for q in [[1e6, 0.], [0., f64::NAN], [0., 1e6]] {
        assert!(irregular_patch::sample_regular_source(s, &field, &[q]).is_err());
    }
    assert!(
        irregular_patch::sample_regular_source(s, &field[..field.len() - 1], &[[0., 0.]]).is_err()
    );
}

#[test]
fn malformed_settings_and_oversized_work_refuse_before_sampling() {
    for s in [
        Settings {
            cells_x: 257,
            ..settings()
        },
        Settings {
            cells_y: 0,
            ..settings()
        },
        Settings {
            radius_meters: 1.,
            ..settings()
        },
        Settings {
            orientation_radians: f64::NAN,
            ..settings()
        },
        Settings {
            cells_x: 256,
            spacing_meters: 5000.,
            radius_meters: 100000.,
            ..settings()
        },
        Settings {
            center_direction: [0.; 3],
            ..settings()
        },
    ] {
        assert!(irregular_patch::build("refusal", s).is_err());
    }
    for seed in ["", "  "] {
        assert!(irregular_patch::build(seed, settings()).is_err());
    }
    assert!(irregular_patch::build(&"a".repeat(129), settings()).is_err());
    let mut bed = vec![0.; (settings().cells_x * settings().cells_y) as usize];
    bed[0] = f64::INFINITY;
    assert!(irregular_patch::sample_regular_source(settings(), &bed, &[[0., 0.]]).is_err());
}

#[test]
fn largest_nominal_patch_is_bounded_and_has_positive_spherical_geometry() {
    let g = irregular_patch::build(
        "maximum-workload",
        Settings {
            cells_x: 256,
            cells_y: 256,
            spacing_meters: 78.125,
            ..settings()
        },
    )
    .unwrap();
    assert!(g.chart_points_meters.len() > 40962);
    assert!(g.chart_points_meters.len() <= irregular_patch::MAX_POINTS);
    check_geometry(&g);
}

#[test]
fn generated_mesh_drives_real_channel_construction_and_actual_downhill_drainage() {
    let g = irregular_patch::build(
        "construction",
        Settings {
            cells_x: 32,
            cells_y: 32,
            ..settings()
        },
    )
    .unwrap();
    let half = 31. * g.settings.spacing_meters / 2.;
    let bed: Vec<_> = g
        .chart_points_meters
        .iter()
        .zip(&g.fixed_external_boundary)
        .map(|(p, &fixed)| {
            if fixed {
                0.
            } else {
                500. * (1. - (p[0] / half).powi(2)) * (1. - (p[1] / half).powi(2))
            }
        })
        .collect();
    let receivers = drainage_landforms::dry_receivers(
        &bed,
        &g.reference_areas_square_meters,
        &g.fixed_external_boundary,
        &g.edges,
    )
    .unwrap();
    let constructed = drainage_landforms::construct(
        &bed,
        &g.reference_areas_square_meters,
        &g.fixed_external_boundary,
        &g.edges,
        &receivers,
        drainage_landforms::Settings {
            channel_head_area_square_meters: 250000.,
            channel_head_slope: 0.1,
            channel_concavity: 0.5,
            maximum_graph_slope: std::f64::consts::FRAC_PI_6.tan(),
        },
    )
    .unwrap();
    let h = &constructed.elevation_meters;
    assert!(
        h.iter()
            .enumerate()
            .all(|(i, &h)| h <= bed[i] && (!g.fixed_external_boundary[i] || h == bed[i]))
    );
    assert!(constructed.detached_material_cubic_meters > 0.);
    let actual = drainage_landforms::dry_receivers(
        h,
        &g.reference_areas_square_meters,
        &g.fixed_external_boundary,
        &g.edges,
    )
    .unwrap();
    assert!(
        actual
            .iter()
            .enumerate()
            .all(|(i, &r)| r as usize == i || h[r as usize] < h[i])
    );
    let mut sorted: Vec<_> = (0..h.len()).collect();
    sorted.sort_by(|&i, &j| h[j].total_cmp(&h[i]));
    let mut collected: Vec<_> = g
        .reference_areas_square_meters
        .iter()
        .zip(&g.fixed_external_boundary)
        .map(|(&a, &fixed)| if fixed { 0. } else { a })
        .collect();
    let source: f64 = collected.iter().sum();
    for i in sorted {
        if actual[i] as usize != i {
            let amount = collected[i];
            collected[actual[i] as usize] += amount;
        }
    }
    let terminal: f64 = collected
        .iter()
        .enumerate()
        .filter(|(i, _)| actual[*i] as usize == *i)
        .map(|(_, a)| a)
        .sum();
    assert!((source - terminal).abs() / source < 1e-12);
}
