use planimulation_core::{
    Surface,
    drainage::Drainage,
    landscape_evolution,
    sedimentary_landscape::{Settings, prepare},
    water::{Water, WaterSettings},
};

fn chain(areas: &[f64]) -> Surface {
    let mut s = Surface {
        centers: vec![[1., 0., 0.]; areas.len()],
        faces: vec![],
        offsets: vec![0],
        neighbors: vec![],
        distances: vec![],
        areas: areas.to_vec(),
        boundary_offsets: vec![],
        boundaries: vec![],
    };
    for i in 0..areas.len() {
        if i > 0 {
            s.neighbors.push((i - 1) as u32);
        }
        if i + 1 < areas.len() {
            s.neighbors.push((i + 1) as u32);
        }
        s.offsets.push(s.neighbors.len() as u32);
    }
    s.distances.resize(s.neighbors.len(), 1.);
    s
}
fn water(s: &Surface, h: &[f64]) -> Water {
    Water::generate(
        s,
        h,
        &WaterSettings::Volume {
            volume_cubic_meters: h
                .iter()
                .zip(&s.areas)
                .map(|(&h, &a)| (-h).max(0.) * a)
                .sum(),
        },
    )
    .unwrap()
}
fn settings(steps: usize) -> Settings {
    Settings {
        erodibility_per_year: 1.,
        step_years: 1.,
        steps,
        maximum_aggradation_rate_meters_per_year: 10.,
    }
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= a.abs().max(b.abs()).max(1.) * 1e-10,
        "{a} != {b}"
    );
}

#[test]
fn delivered_erosion_fills_only_to_real_first_spill_and_retains_excess() {
    let s = chain(&[1.; 4]);
    let h = [-1., 5., 1., 10.];
    let w = water(&s, &h);
    let p = prepare(&s, &h, &w, &[0.; 4], settings(1)).unwrap();
    assert_eq!(p.elevation_meters, [-1., 2.5, 2.5, 5.5]);
    assert_eq!(p.sediment_thickness_meters, [0., 0., 1.5, 0.]);
    assert_eq!(p.pending_volume_cubic_meters, [0., 0., 3., 0.]);
    assert_eq!(p.gross_erosion_volume_cubic_meters, 7.);
    assert_eq!(p.main_ocean_export_volume_cubic_meters, 2.5);
    assert_eq!(p.gross_deposition_volume_cubic_meters, 1.5);
    assert_eq!(p.relative_ledger_residual, 0.);
    assert_eq!(p.history[0].dry_terminal_count_after_deposition, 0);
    assert_eq!(p.wet_body_exports[0].volume_cubic_meters, 2.5);
    assert_eq!(h, [-1., 5., 1., 10.]);
    // Real bed aggradation connected the old sink to the fixed outlet through
    // a flat physical plateau; no spill witness was substituted for routing.
    let before = Drainage::build(&s, &h, &w.body_ids);
    let after = Drainage::build(&s, &p.elevation_meters, &w.body_ids);
    assert_eq!(before.receivers[2], 2);
    assert_eq!(after.receivers[2], 1);
}

#[test]
fn later_steps_reroute_pending_stock_and_erode_previously_deposited_material() {
    let s = chain(&[1.; 4]);
    let h = [-1., 5., 1., 10.];
    let w = water(&s, &h);
    let p = prepare(&s, &h, &w, &[0.; 4], settings(2)).unwrap();
    assert!(p.remobilized_sediment_volume_cubic_meters > 0.);
    assert!(p.surviving_sediment_volume_cubic_meters < 1.5);
    assert_eq!(p.pending_volume_total_cubic_meters, 0.);
    close(
        p.gross_erosion_volume_cubic_meters,
        p.gross_deposition_volume_cubic_meters + p.main_ocean_export_volume_cubic_meters,
    );
    close(
        p.gross_deposition_volume_cubic_meters - p.remobilized_sediment_volume_cubic_meters,
        p.surviving_sediment_volume_cubic_meters,
    );
    assert!(p.history[1].main_ocean_export_volume_cubic_meters > 3.);
}

#[test]
fn no_erosion_supply_never_fills_pits_even_with_positive_uplift() {
    let s = chain(&[1.; 4]);
    let h = [-1., 5., 1., 10.];
    let w = water(&s, &h);
    let cfg = Settings {
        erodibility_per_year: 0.,
        ..settings(4)
    };
    let p = prepare(&s, &h, &w, &[0., 1., 0., 0.], cfg).unwrap();
    assert_eq!(p.elevation_meters, [-1., 9., 1., 10.]);
    assert_eq!(p.gross_deposition_volume_cubic_meters, 0.);
    assert_eq!(p.pending_volume_total_cubic_meters, 0.);
    assert_eq!(p.uplift_displacement_prism_cubic_meters, 4.);
}

#[test]
fn zero_aggradation_matches_frozen_process_but_keeps_dry_inventory() {
    let s = chain(&[1., 2., 3., 4.]);
    let h = [-1., 5., 1., 10.];
    let w = water(&s, &h);
    let cfg = Settings {
        maximum_aggradation_rate_meters_per_year: 0.,
        ..settings(8)
    };
    let p = prepare(&s, &h, &w, &[0., 0.1, 0., 0.2], cfg).unwrap();
    let old = landscape_evolution::prepare(
        &s,
        &h,
        &w,
        &[0., 0.1, 0., 0.2],
        landscape_evolution::Settings {
            erodibility_per_year: cfg.erodibility_per_year,
            step_years: cfg.step_years,
            steps: cfg.steps,
        },
    )
    .unwrap();
    assert_eq!(p.elevation_meters, old.elevation_meters);
    assert_eq!(
        p.removed_erosion_thickness_meters,
        old.removed_erosion_thickness_meters
    );
    assert_eq!(
        p.accepted_uplift_displacement_meters,
        old.accepted_uplift_displacement_meters
    );
    assert!(p.pending_volume_total_cubic_meters > 0.);
    assert_eq!(p.gross_deposition_volume_cubic_meters, 0.);
}

#[test]
fn bounded_rate_cannot_raise_terminal_faster_than_declared() {
    let s = chain(&[1.; 4]);
    let h = [-1., 5., 1., 10.];
    let w = water(&s, &h);
    let p = prepare(
        &s,
        &h,
        &w,
        &[0.; 4],
        Settings {
            maximum_aggradation_rate_meters_per_year: 0.25,
            ..settings(1)
        },
    )
    .unwrap();
    assert_eq!(p.elevation_meters[2], 1.25);
    assert_eq!(p.gross_deposition_volume_cubic_meters, 0.25);
    assert_eq!(p.pending_volume_total_cubic_meters, 4.25);
}

#[test]
fn closed_dry_root_retains_finite_stock_without_inventing_an_outlet() {
    let s = chain(&[1.; 3]);
    let h = [1., 5., 10.];
    let w = Water::generate(&s, &h, &WaterSettings::Coverage { fraction: 0. }).unwrap();
    let p = prepare(&s, &h, &w, &[0.; 3], settings(3)).unwrap();
    assert!(p.pending_volume_total_cubic_meters > 0.);
    assert_eq!(p.gross_deposition_volume_cubic_meters, 0.);
    assert_eq!(p.main_ocean_export_volume_cubic_meters, 0.);
    assert_eq!(p.other_initial_wet_export_volume_cubic_meters, 0.);
    assert_eq!(p.wet_body_exports.len(), 0);
    close(
        p.pending_volume_total_cubic_meters,
        p.gross_erosion_volume_cubic_meters,
    );
}

#[test]
fn wet_collectors_remain_separate_without_underwater_bed_change() {
    let s = chain(&[2., 1., 1., 1., 1.]);
    let h = [-1., 3., 6., 3., -1.];
    let w = water(&s, &h);
    let p = prepare(&s, &h, &w, &[0.; 5], settings(4)).unwrap();
    assert_eq!(p.elevation_meters[0], -1.);
    assert_eq!(p.elevation_meters[4], -1.);
    assert!(p.main_ocean_export_volume_cubic_meters > 0.);
    assert!(p.other_initial_wet_export_volume_cubic_meters > 0.);
    assert_eq!(p.wet_body_exports.len(), 2);
    close(
        p.wet_body_exports
            .iter()
            .map(|b| b.volume_cubic_meters)
            .sum(),
        p.gross_erosion_volume_cubic_meters,
    );
}

#[test]
fn generated_spheres_repeat_and_independently_close_all_three_ledgers() {
    use planimulation_core::{Recipe, World, landscape_evolution::structural_uplift_rates};
    for seed in ["first-light", "4836187e", "geography-00", "geography-01"] {
        let recipe:Recipe=serde_json::from_value(serde_json::json!({"schemaVersion":1,"modelVersion":"multiscale-geography-2","randomVersion":"fnv1a-utf8-mulberry32-1","seed":seed,"subdivision":3,"radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,"detailAmplitudeMeters":300.,"terrainPreparationPasses":0,"water":{"mode":"coverage","fraction":0.71}})).unwrap();
        let w = World::generate(recipe).unwrap();
        let u = structural_uplift_rates(&w.terrain.convergence, &w.water, 1e-6, 2000.).unwrap();
        let cfg = Settings {
            erodibility_per_year: 1e-6,
            step_years: 25000.,
            steps: 32,
            maximum_aggradation_rate_meters_per_year: 0.001,
        };
        let p = prepare(&w.surface, &w.terrain.elevation, &w.water, &u, cfg).unwrap();
        let q = prepare(&w.surface, &w.terrain.elevation, &w.water, &u, cfg).unwrap();
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            serde_json::to_value(&q).unwrap()
        );
        let prism = |thickness: &[f64]| {
            thickness
                .iter()
                .zip(&w.surface.areas)
                .map(|(&d, &a)| d * a)
                .sum::<f64>()
        };
        close(
            prism(&p.sediment_thickness_meters),
            p.surviving_sediment_volume_cubic_meters,
        );
        close(
            prism(&p.deposited_thickness_meters),
            p.gross_deposition_volume_cubic_meters,
        );
        close(
            prism(&p.removed_erosion_thickness_meters),
            p.gross_erosion_volume_cubic_meters,
        );
        close(
            p.pending_volume_cubic_meters.iter().sum(),
            p.pending_volume_total_cubic_meters,
        );
        for (i, &h) in p.elevation_meters.iter().enumerate() {
            close(
                h,
                w.terrain.elevation[i] + p.accepted_uplift_displacement_meters[i]
                    - p.removed_erosion_thickness_meters[i]
                    + p.deposited_thickness_meters[i],
            );
            if w.water.body_ids[i] != 0 {
                assert_eq!(h, w.terrain.elevation[i]);
            }
        }
        assert!(p.relative_ledger_residual < 1e-11);
    }
}

#[test]
fn settings_fields_and_budget_fail_without_mutation() {
    let s = chain(&[1.; 4]);
    let h = [-1., 5., 1., 10.];
    let w = water(&s, &h);
    for cfg in [
        Settings {
            steps: 0,
            ..settings(1)
        },
        Settings {
            steps: 129,
            ..settings(1)
        },
        Settings {
            maximum_aggradation_rate_meters_per_year: -1.,
            ..settings(1)
        },
        Settings {
            maximum_aggradation_rate_meters_per_year: f64::MAX,
            step_years: 2.,
            ..settings(1)
        },
        Settings {
            step_years: 0.,
            ..settings(1)
        },
        Settings {
            erodibility_per_year: -1.,
            ..settings(1)
        },
        Settings {
            maximum_aggradation_rate_meters_per_year: 1e-300,
            step_years: 1e-300,
            ..settings(1)
        },
    ] {
        assert!(prepare(&s, &h, &w, &[0.; 4], cfg).is_err());
    }
    assert!(prepare(&s, &h, &w, &[0.; 3], settings(1)).is_err());
    assert!(prepare(&s, &h, &w, &[1.; 4], settings(1)).is_err());
    assert!(
        prepare(&s, &vec![1.; 500001], &w, &[], settings(128))
            .unwrap_err()
            .contains("workload")
    );
    assert_eq!(h, [-1., 5., 1., 10.]);
}

#[test]
fn fully_wet_world_has_no_sediment_and_subprecision_uplift_is_reported() {
    let s = Surface::build(2, 1000.);
    let h = vec![0.; s.areas.len()];
    let w = Water::generate(&s, &h, &WaterSettings::Coverage { fraction: 1. }).unwrap();
    let p = prepare(&s, &h, &w, &vec![0.; h.len()], settings(4)).unwrap();
    assert_eq!(p.elevation_meters, h);
    assert_eq!(p.gross_erosion_volume_cubic_meters, 0.);
    assert_eq!(p.gross_deposition_volume_cubic_meters, 0.);
    assert_eq!(p.relative_ledger_residual, 0.);
    let s = chain(&[1.; 2]);
    let h = [-1., 10.];
    let w = water(&s, &h);
    let p = prepare(
        &s,
        &h,
        &w,
        &[0., 1e-20],
        Settings {
            erodibility_per_year: 0.,
            ..settings(1)
        },
    )
    .unwrap();
    assert_eq!(p.elevation_meters, h);
    assert_eq!(p.history[0].unresolved_positive_uplift_regions, 1);
    assert_eq!(p.uplift_displacement_prism_cubic_meters, 0.);
}
