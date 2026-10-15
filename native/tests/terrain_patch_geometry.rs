use planimulation_core::terrain_patch_geometry::{self, Geometry, Settings, build};
use std::collections::BTreeSet;

fn settings() -> Settings {
    Settings {
        radius_meters: 6_371_000.,
        center_direction: [1., 0., 0.],
        orientation_radians: 0.,
        cells_x: 12,
        cells_y: 8,
        spacing_meters: 500.,
    }
}
fn relative(a: f64, b: f64, tolerance: f64) {
    assert!(
        (a - b).abs() / a.abs().max(b.abs()) <= tolerance,
        "{a} != {b}"
    );
}
fn total_area(g: &Geometry) -> f64 {
    g.reference_areas_square_meters.iter().sum()
}
fn angular_distance(a: [f64; 3], b: [f64; 3], radius: f64) -> f64 {
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let norm = cross.iter().map(|v| v * v).sum::<f64>().sqrt();
    norm.atan2(a.iter().zip(b).map(|(a, b)| a * b).sum()) * radius
}
fn unit_checks(g: &Geometry) {
    for p in g.vertex_directions.iter().chain(&g.cell_directions) {
        assert!(p.iter().all(|v| v.is_finite()));
        assert!((p.iter().map(|v| v * v).sum::<f64>() - 1.).abs() < 1e-12);
    }
    assert!(
        g.reference_areas_square_meters
            .iter()
            .all(|v| v.is_finite() && *v > 0.)
    );
    assert!(g.distances_meters.iter().all(|v| v.is_finite() && *v > 0.));
}

#[test]
fn geometry_repeats_owns_settings_and_perimeter_without_world_semantics() {
    let s = settings();
    let a = build(s).unwrap();
    let b = build(s).unwrap();
    assert_eq!(a.settings, s);
    let decoded: Settings = serde_json::from_value(serde_json::to_value(s).unwrap()).unwrap();
    assert_eq!(decoded, s);
    assert_eq!(build(decoded).unwrap().cell_directions, a.cell_directions);
    assert_eq!(s, settings());
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(a.model_version, terrain_patch_geometry::MODEL_VERSION);
    assert_eq!(a.vertex_directions.len(), 13 * 9);
    assert_eq!(a.cell_directions.len(), 12 * 8);
    assert_eq!(a.side_faces.len(), 8 * 11 + 12 * 7);
    assert_eq!(
        a.fixed_external_boundary.iter().filter(|&&v| v).count(),
        2 * (12 + 8) - 4
    );
    assert_eq!(a.offsets.len(), 12 * 8 + 1);
    unit_checks(&a);
}

#[test]
fn seam_and_pole_charts_have_finite_unit_geometry_without_longitude_wraps() {
    for p in [[-1., 0., 0.], [0., 1., 0.], [0., -1., 0.], [0., 0., 1.]] {
        let g = build(Settings {
            center_direction: p,
            ..settings()
        })
        .unwrap();
        unit_checks(&g);
        if p[0] == -1. {
            assert!(g.cell_directions.iter().any(|p| p[2] > 0.));
            assert!(g.cell_directions.iter().any(|p| p[2] < 0.));
        }
    }
}

#[test]
fn origin_rotation_preserves_spherical_cell_areas_and_metric_lengths() {
    let reference = build(settings()).unwrap();
    for center in [[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]] {
        for orientation in [0., std::f64::consts::FRAC_PI_4, 3.2] {
            let rotated = build(Settings {
                center_direction: center,
                orientation_radians: orientation,
                ..settings()
            })
            .unwrap();
            assert_eq!(reference.neighbors, rotated.neighbors);
            assert_eq!(
                reference.fixed_external_boundary,
                rotated.fixed_external_boundary
            );
            for (&a, &b) in reference
                .reference_areas_square_meters
                .iter()
                .zip(&rotated.reference_areas_square_meters)
            {
                relative(a, b, 1e-9);
            }
            for (&a, &b) in reference
                .distances_meters
                .iter()
                .zip(&rotated.distances_meters)
            {
                relative(a, b, 1e-9);
            }
            for (a, b) in reference.side_faces.iter().zip(&rotated.side_faces) {
                assert_eq!((a.a, a.b), (b.a, b.b));
                relative(
                    a.shared_face_length_meters,
                    b.shared_face_length_meters,
                    1e-9,
                );
            }
        }
    }
}

#[test]
fn tiny_chart_matches_flat_proxy_and_extreme_valid_geometry_stays_positive() {
    let g = build(settings()).unwrap();
    for &area in &g.reference_areas_square_meters {
        relative(area, 500. * 500., 1e-6);
    }
    for edge in &g.side_faces {
        relative(edge.shared_face_length_meters, 500., 1e-6);
        relative(edge.center_distance_meters, 500., 1e-6);
    }
    for (radius, spacing, center) in [
        (20_000_000., 10., [0., 1., 0.]),
        (100_000., 1000., [1., 0., 0.]),
    ] {
        let g = build(Settings {
            radius_meters: radius,
            spacing_meters: spacing,
            center_direction: center,
            cells_x: 4,
            cells_y: 4,
            ..settings()
        })
        .unwrap();
        unit_checks(&g);
    }
}

#[test]
fn same_extent_refinement_retains_shared_vertices_and_additive_area() {
    let coarse = build(Settings {
        cells_x: 20,
        cells_y: 12,
        ..settings()
    })
    .unwrap();
    let fine = build(Settings {
        cells_x: 40,
        cells_y: 24,
        spacing_meters: 250.,
        ..settings()
    })
    .unwrap();
    for row in 0..=12 {
        for column in 0..=20 {
            assert_eq!(
                coarse.vertex_directions[row * 21 + column],
                fine.vertex_directions[(row * 2) * 41 + column * 2]
            );
        }
    }
    for row in 0..12 {
        for column in 0..20 {
            let children: f64 = [
                (row * 2) * 40 + column * 2,
                (row * 2) * 40 + column * 2 + 1,
                (row * 2 + 1) * 40 + column * 2,
                (row * 2 + 1) * 40 + column * 2 + 1,
            ]
            .iter()
            .map(|&i| fine.reference_areas_square_meters[i])
            .sum();
            relative(
                coarse.reference_areas_square_meters[row * 20 + column],
                children,
                1e-8,
            );
        }
    }
    relative(total_area(&coarse), total_area(&fine), 1e-8);
}

#[test]
fn routing_is_sorted_reciprocal_and_side_faces_use_shared_spherical_geometry() {
    let g = build(settings()).unwrap();
    let nx = g.settings.cells_x as usize;
    for i in 0..g.cell_directions.len() {
        let range = g.offsets[i] as usize..g.offsets[i + 1] as usize;
        let row = &g.neighbors[range.clone()];
        assert!(row.windows(2).all(|p| p[0] < p[1]));
        assert!(row.len() >= 3 && row.len() <= 8);
        for k in range {
            let j = g.neighbors[k] as usize;
            let start = g.offsets[j] as usize;
            let other = g.neighbors[start..g.offsets[j + 1] as usize]
                .binary_search(&(i as u32))
                .unwrap();
            assert_eq!(g.distances_meters[k], g.distances_meters[start + other]);
            relative(
                g.distances_meters[k],
                angular_distance(
                    g.cell_directions[i],
                    g.cell_directions[j],
                    g.settings.radius_meters,
                ),
                1e-8,
            );
        }
    }
    let mut unique = BTreeSet::new();
    for edge in &g.side_faces {
        assert!(edge.a < edge.b);
        assert!(unique.insert((edge.a, edge.b)));
        let a = edge.a as usize;
        let b = edge.b as usize;
        let start = g.offsets[a] as usize;
        let position = g.neighbors[start..g.offsets[a + 1] as usize]
            .binary_search(&edge.b)
            .unwrap();
        assert_eq!(
            edge.center_distance_meters,
            g.distances_meters[start + position]
        );
        let row = a / nx;
        let column = a % nx;
        let vertices = if b == a + 1 {
            assert_eq!(a / nx, b / nx);
            (
                row * (nx + 1) + column + 1,
                (row + 1) * (nx + 1) + column + 1,
            )
        } else {
            assert_eq!(b, a + nx);
            (
                (row + 1) * (nx + 1) + column,
                (row + 1) * (nx + 1) + column + 1,
            )
        };
        relative(
            edge.shared_face_length_meters,
            angular_distance(
                g.vertex_directions[vertices.0],
                g.vertex_directions[vertices.1],
                g.settings.radius_meters,
            ),
            1e-8,
        );
    }
}

#[test]
fn dimensions_and_chart_extent_refuse_before_allocation_and_max_budget_builds() {
    let original = settings();
    for invalid in 0..10 {
        let mut s = original;
        match invalid {
            0 => s.cells_x = 3,
            1 => s.cells_y = 513,
            2 => s.cells_x = u32::MAX,
            3 => s.center_direction = [0.; 3],
            4 => s.center_direction = [f64::NAN, 0., 0.],
            5 => s.orientation_radians = std::f64::consts::TAU,
            6 => s.orientation_radians = -0.1,
            7 => s.spacing_meters = 9.,
            8 => s.radius_meters = 99_999.,
            9 => {
                s.radius_meters = 100_000.;
                s.cells_x = 512;
                s.spacing_meters = 5000.;
            }
            _ => unreachable!(),
        }
        assert!(s.validate().is_err());
        assert!(build(s).is_err());
    }
    assert_eq!(original, settings());
    let max = build(Settings {
        cells_x: 512,
        cells_y: 512,
        spacing_meters: 250.,
        ..original
    })
    .unwrap();
    assert_eq!(max.cell_directions.len(), terrain_patch_geometry::MAX_CELLS);
    assert_eq!(max.vertex_directions.len(), 513 * 513);
    assert_eq!(max.neighbors.len(), 8 * 512 * 512 - 12 * 512 + 4);
    assert_eq!(max.side_faces.len(), 2 * 512 * 511);
    unit_checks(&max);
}
