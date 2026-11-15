use planimulation_core::{
    irregular_patch,
    structural_patch_source::{self, Settings},
    terrain_patch_geometry,
};
use std::collections::VecDeque;

fn geometry() -> irregular_patch::Geometry {
    irregular_patch::build(
        "source-test",
        terrain_patch_geometry::Settings {
            radius_meters: 6371000.,
            center_direction: [1., 0., 0.],
            orientation_radians: 0.,
            cells_x: 8,
            cells_y: 8,
            spacing_meters: 250.,
        },
    )
    .unwrap()
}
fn settings() -> Settings {
    Settings {
        erodibility_per_year: 1e-3,
        maximum_profile_slope: 0.5,
    }
}
fn uplift(g: &irregular_patch::Geometry, amount: f64) -> Vec<f64> {
    g.fixed_external_boundary
        .iter()
        .map(|&fixed| if fixed { 0. } else { amount })
        .collect()
}

#[test]
fn minimax_levels_match_independent_threshold_connectivity_and_zero_forcing_is_not_a_clock() {
    let g = geometry();
    let n = g.chart_points_meters.len();
    let mut potential: Vec<_> = (0..n).map(|i| ((i * 17 + 3) % 9) as f64).collect();
    for (i, v) in potential.iter_mut().enumerate() {
        if g.fixed_external_boundary[i] {
            *v = 5.;
        }
    }
    potential[0] = 2.;
    let source = structural_patch_source::build(&g, &potential, &vec![0.; n], settings()).unwrap();
    let mut adjacent = vec![Vec::new(); n];
    for e in &g.edges {
        adjacent[e.a as usize].push(e.b as usize);
        adjacent[e.b as usize].push(e.a as usize);
    }
    let mut oracle = vec![f64::INFINITY; n];
    for level in 0..=8 {
        let mut seen = vec![false; n];
        let mut queue = VecDeque::new();
        for i in 0..n {
            if g.fixed_external_boundary[i] && potential[i] <= level as f64 {
                seen[i] = true;
                queue.push_back(i);
            }
        }
        while let Some(i) = queue.pop_front() {
            for &j in &adjacent[i] {
                if !seen[j] && potential[j] <= level as f64 {
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
        for i in 0..n {
            if !g.fixed_external_boundary[i] && seen[i] && !oracle[i].is_finite() {
                oracle[i] = level as f64;
            }
        }
    }
    for i in 0..n {
        if g.fixed_external_boundary[i] {
            oracle[i] = potential[i];
        }
    }
    assert_eq!(source.potential_levels_meters, oracle);
    assert_eq!(source.initial_bed_meters, oracle);
    assert_eq!(source.slope_capped_nodes, 0);
    let encoded = serde_json::to_value(&source).unwrap();
    assert!(encoded.get("years").is_none() && encoded.get("stepYears").is_none());
}

#[test]
fn source_area_and_uncapped_grade_have_independent_ancestry_and_link_formulas() {
    let g = geometry();
    let n = g.chart_points_meters.len();
    let potential = vec![0.; n];
    let u = uplift(&g, 1e-5);
    let source = structural_patch_source::build(&g, &potential, &u, settings()).unwrap();
    let mut collected = vec![0.; n];
    for start in 0..n {
        if g.fixed_external_boundary[start] {
            continue;
        }
        let mut i = start;
        for hops in 0..=n {
            assert!(hops < n);
            collected[i] += g.reference_areas_square_meters[start];
            let r = source.scaffold_receivers[i] as usize;
            if r == i {
                assert!(g.fixed_external_boundary[i]);
                break;
            }
            i = r;
        }
    }
    for i in 0..n {
        assert!(
            (collected[i] - source.scaffold_contributing_area_square_meters[i]).abs()
                / collected[i].max(1.)
                < 1e-14
        );
        let r = source.scaffold_receivers[i] as usize;
        if r == i {
            assert_eq!(source.initial_bed_meters[i], potential[i]);
            continue;
        }
        let edge = g
            .edges
            .iter()
            .find(|e| e.a as usize == i.min(r) && e.b as usize == i.max(r))
            .unwrap();
        let expected =
            u[i] / (settings().erodibility_per_year * collected[i].sqrt()) * edge.distance_meters;
        assert!(
            (source.initial_bed_meters[i] - source.initial_bed_meters[r] - expected).abs() < 1e-12
        );
    }
    assert_eq!(source.slope_capped_nodes, 0);
    assert!(source.terminal_area_relative_residual < 1e-12);
    let repeat = structural_patch_source::build(&g, &potential, &u, settings()).unwrap();
    assert_eq!(
        serde_json::to_value(&source).unwrap(),
        serde_json::to_value(&repeat).unwrap()
    );
}

#[test]
fn profile_cap_is_explicit_and_initial_height_is_not_an_erosion_debit() {
    let g = geometry();
    let n = g.chart_points_meters.len();
    let u = uplift(&g, 1.);
    let source = structural_patch_source::build(
        &g,
        &vec![0.; n],
        &u,
        Settings {
            erodibility_per_year: 1e-8,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(
        source.slope_capped_nodes,
        g.fixed_external_boundary.iter().filter(|&&f| !f).count()
    );
    for i in 0..n {
        let r = source.scaffold_receivers[i] as usize;
        if i == r {
            continue;
        }
        let edge = g
            .edges
            .iter()
            .find(|e| e.a as usize == i.min(r) && e.b as usize == i.max(r))
            .unwrap();
        assert!(
            (source.initial_bed_meters[i]
                - source.initial_bed_meters[r]
                - 0.5 * edge.distance_meters)
                .abs()
                < 1e-12
        );
    }
    assert!(
        serde_json::to_value(&source)
            .unwrap()
            .get("detachedMaterialCubicMeters")
            .is_none()
    );
}

#[test]
fn malformed_fields_and_unreachable_scaffold_refuse_immutably() {
    let g = geometry();
    let n = g.chart_points_meters.len();
    let p = vec![1.; n];
    let u = uplift(&g, 0.0005);
    for s in [
        Settings {
            erodibility_per_year: 0.,
            ..settings()
        },
        Settings {
            erodibility_per_year: f64::NAN,
            ..settings()
        },
        Settings {
            maximum_profile_slope: 0.,
            ..settings()
        },
        Settings {
            maximum_profile_slope: 3.,
            ..settings()
        },
    ] {
        assert!(structural_patch_source::build(&g, &p, &u, s).is_err());
    }
    let mut bad = u.clone();
    bad[0] = 1.;
    assert!(structural_patch_source::build(&g, &p, &bad, settings()).is_err());
    assert!(structural_patch_source::build(&g, &p[..n - 1], &u, settings()).is_err());
    let mut bad = p.clone();
    bad[n - 1] = f64::NAN;
    assert!(structural_patch_source::build(&g, &bad, &u, settings()).is_err());
    let mut disconnected = geometry();
    let interior = disconnected
        .fixed_external_boundary
        .iter()
        .position(|v| !*v)
        .unwrap() as u32;
    disconnected
        .edges
        .retain(|e| e.a != interior && e.b != interior);
    assert!(
        structural_patch_source::build(&disconnected, &p, &u, settings())
            .unwrap_err()
            .contains("unreachable")
    );
    assert_eq!(p, vec![1.; n]);
    assert_eq!(u, uplift(&g, 0.0005));
}
