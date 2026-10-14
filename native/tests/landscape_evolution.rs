use planimulation_core::{
    Surface,
    landscape_evolution::{Settings, prepare, structural_uplift_rates},
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
fn water_at(s: &Surface, h: &[f64]) -> Water {
    Water::generate(
        s,
        h,
        &WaterSettings::Volume {
            volume_cubic_meters: h
                .iter()
                .zip(&s.areas)
                .map(|(&h, &a)| a * (-h).max(0.))
                .sum(),
        },
    )
    .unwrap()
}
fn settings() -> Settings {
    Settings {
        erodibility_per_year: 1.,
        step_years: 1.,
        steps: 1,
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() <= b.abs().max(1.) * 1e-11, "{a} != {b}");
}

#[test]
fn coupled_single_link_has_known_source_implicit_solution_and_separate_ledgers() {
    let s = chain(&[1., 1.]);
    let h = [-1., 10.];
    let w = water_at(&s, &h);
    let p = prepare(&s, &h, &w, &[0., 1.], settings()).unwrap();
    assert_eq!(p.elevation_meters, [-1., 5.5]);
    assert_eq!(p.accepted_uplift_displacement_meters, [0., 1.]);
    assert_eq!(p.removed_erosion_thickness_meters, [0., 5.5]);
    assert_eq!(p.uplift_displacement_prism_cubic_meters, 1.);
    assert_eq!(p.exported_erosion_material_cubic_meters, 5.5);
    assert_eq!(p.independent_net_bed_prism_change_cubic_meters, -4.5);
    assert_eq!(p.relative_prism_ledger_residual, 0.);
    assert_eq!(h, [-1., 10.]);
}

#[test]
fn variable_uplift_rebuilds_routes_instead_of_reusing_former_dry_terminal() {
    let s = chain(&[1.; 3]);
    let h = [5., 1., 6.];
    let w = Water::generate(&s, &h, &WaterSettings::Coverage { fraction: 0. }).unwrap();
    let p = prepare(&s, &h, &w, &[0., 10., 0.], settings()).unwrap();
    assert_eq!(p.elevation_meters, [5., 8., 6.]);
    assert_eq!(p.uplift_displacement_prism_cubic_meters, 10.);
    assert_eq!(p.exported_erosion_material_cubic_meters, 3.);
    assert_eq!(p.independent_net_bed_prism_change_cubic_meters, 7.);
    // The original sink at region 1 becomes a ridge through actual displacement.
    // Regions 0 and 2 remain genuine dry terminals, not invented ocean outlets.
}

#[test]
fn fixed_outlet_ode_refinement_converges_with_continuous_forcing() {
    let s = chain(&[1.; 2]);
    let h = [-1., 10.];
    let w = water_at(&s, &h);
    let exact = 1. + 9. * (-1_f64).exp();
    let mut previous = f64::INFINITY;
    for steps in [1, 2, 4, 8, 16, 32, 64, 128] {
        let p = prepare(
            &s,
            &h,
            &w,
            &[0., 1.],
            Settings {
                steps,
                step_years: 1. / steps as f64,
                ..settings()
            },
        )
        .unwrap();
        let error = (p.elevation_meters[1] - exact).abs();
        assert!(error < previous);
        previous = error;
        close(p.uplift_displacement_prism_cubic_meters, 1.);
        close(
            p.elevation_meters[1] - 10.,
            1. - p.exported_erosion_material_cubic_meters,
        );
    }
    assert!(previous < 0.014);
}

#[test]
fn zero_processes_and_pure_uplift_preserve_their_exact_contracts() {
    let s = chain(&[1., 2., 4.]);
    let h = [-1., 3., 5.];
    let w = water_at(&s, &h);
    let zero = prepare(
        &s,
        &h,
        &w,
        &[0.; 3],
        Settings {
            erodibility_per_year: 0.,
            steps: 8,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(zero.elevation_meters, h);
    assert_eq!(zero.exported_erosion_material_cubic_meters, 0.);
    let raised = prepare(
        &s,
        &h,
        &w,
        &[0., 1., 2.],
        Settings {
            erodibility_per_year: 0.,
            steps: 4,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(raised.elevation_meters, [-1., 7., 13.]);
    assert_eq!(raised.uplift_displacement_prism_cubic_meters, 40.);
    assert_eq!(raised.independent_net_bed_prism_change_cubic_meters, 40.);
    assert_eq!(raised.exported_erosion_material_cubic_meters, 0.);
}

#[test]
fn subprecision_uplift_is_observed_but_never_booked_as_accepted_displacement() {
    let s = chain(&[1.; 2]);
    let h = [-1., 10.];
    let w = water_at(&s, &h);
    let p = prepare(
        &s,
        &h,
        &w,
        &[0., 1e-20],
        Settings {
            erodibility_per_year: 0.,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(p.elevation_meters, h);
    assert_eq!(p.uplift_displacement_prism_cubic_meters, 0.);
    assert_eq!(p.history[0].unresolved_positive_uplift_nodes, 1);
}

#[test]
fn structural_rates_have_declared_height_scale_and_do_not_move_wet_beds() {
    let s = chain(&[1.; 4]);
    let h = [-1., 1., 2., 3.];
    let w = water_at(&s, &h);
    let u = structural_uplift_rates(&[12000., 0., 3000., 6000.], &w, 1e-6, 1000.).unwrap();
    assert_eq!(u, [0., 0., 0.0005, 0.001]);
    assert_eq!(
        structural_uplift_rates(&[0.; 4], &w, 1e-6, 1000.).unwrap(),
        [0.; 4]
    );
    for (k, scale) in [
        (-1., 1000.),
        (1., f64::NAN),
        (f64::MAX, f64::MAX),
        (1e-300, 1e-300),
    ] {
        assert!(structural_uplift_rates(&[1.; 4], &w, k, scale).is_err());
    }
}

#[test]
fn invalid_workloads_rates_water_and_arithmetic_reject_without_mutating_inputs() {
    let s = chain(&[1.; 2]);
    let h = [-1., 10.];
    let w = water_at(&s, &h);
    for bad in [
        Settings {
            steps: 0,
            ..settings()
        },
        Settings {
            steps: 129,
            ..settings()
        },
        Settings {
            step_years: 0.,
            ..settings()
        },
        Settings {
            step_years: f64::MAX,
            steps: 2,
            ..settings()
        },
        Settings {
            erodibility_per_year: -1.,
            ..settings()
        },
    ] {
        assert!(prepare(&s, &h, &w, &[0., 1.], bad).is_err());
    }
    for rates in [
        vec![1., 1.],
        vec![0., -1.],
        vec![0., f64::NAN],
        vec![0., f64::MAX],
        vec![0.],
    ] {
        assert!(
            prepare(
                &s,
                &h,
                &w,
                &rates,
                Settings {
                    step_years: 2.,
                    ..settings()
                }
            )
            .is_err()
        );
    }
    let mut bad_w = w.clone();
    bad_w.depth_meters[0] = 0.;
    assert!(prepare(&s, &h, &bad_w, &[0., 1.], settings()).is_err());
    assert_eq!(h, [-1., 10.]);
    assert_eq!(w.depth_meters, [1., 0.]);
}

#[test]
fn generated_spheres_repeat_preserve_wet_beds_and_close_independent_prisms() {
    use planimulation_core::{Recipe, World};
    for seed in ["first-light", "4836187e", "geography-00", "geography-01"] {
        let r:Recipe=serde_json::from_value(serde_json::json!({"schemaVersion":1,"modelVersion":"multiscale-geography-2","randomVersion":"fnv1a-utf8-mulberry32-1","seed":seed,"subdivision":3,"radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,"detailAmplitudeMeters":300.,"terrainPreparationPasses":0,"water":{"mode":"coverage","fraction":0.71}})).unwrap();
        let w = World::generate(r).unwrap();
        let h = w.terrain.elevation.clone();
        let u = structural_uplift_rates(&w.terrain.convergence, &w.water, 1e-6, 1000.).unwrap();
        let settings = Settings {
            erodibility_per_year: 1e-6,
            step_years: 25000.,
            steps: 32,
        };
        let a = prepare(&w.surface, &h, &w.water, &u, settings).unwrap();
        let b = prepare(&w.surface, &h, &w.water, &u, settings).unwrap();
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::to_value(&b).unwrap()
        );
        let mut uplift = 0.;
        let mut eroded = 0.;
        let mut net = 0.;
        for (i, &initial_height) in h.iter().enumerate() {
            uplift += a.accepted_uplift_displacement_meters[i] * w.surface.areas[i];
            eroded += a.removed_erosion_thickness_meters[i] * w.surface.areas[i];
            net += (a.elevation_meters[i] - initial_height) * w.surface.areas[i];
            close(
                a.elevation_meters[i],
                initial_height + a.accepted_uplift_displacement_meters[i]
                    - a.removed_erosion_thickness_meters[i],
            );
            if w.water.body_ids[i] != 0 {
                assert_eq!(a.elevation_meters[i], initial_height);
            }
        }
        close(uplift, a.uplift_displacement_prism_cubic_meters);
        close(eroded, a.exported_erosion_material_cubic_meters);
        assert!((uplift - eroded - net).abs() / uplift.max(eroded) < 1e-11);
        assert_eq!(w.terrain.elevation, h);
    }
}

#[test]
fn no_uplift_matches_independent_positive_erosion_loop_exactly() {
    use planimulation_core::{drainage::Drainage, landscape};
    let s = chain(&[1., 2., 3., 4., 5., 6.]);
    let h = [-1., 8., 4., 4., 1., 2.];
    let w = water_at(&s, &h);
    let cfg = Settings {
        erodibility_per_year: 0.25,
        step_years: 0.5,
        steps: 8,
    };
    let p = prepare(&s, &h, &w, &[0.; 6], cfg).unwrap();
    let mut bed = h.to_vec();
    let mut volumes = Vec::new();
    for _ in 0..cfg.steps {
        let g = Drainage::build(&s, &bed, &w.body_ids);
        let links: Vec<_> = g
            .receivers
            .iter()
            .enumerate()
            .map(|(i, &r)| if i == r as usize { 0. } else { 1. })
            .collect();
        let inc = landscape::incise(
            &bed,
            &s.areas,
            &g.receivers,
            &links,
            landscape::Settings {
                erodibility_per_year: cfg.erodibility_per_year,
                step_years: cfg.step_years,
                base_level_meters: Some(w.level_meters),
            },
        )
        .unwrap();
        volumes.push(inc.removed_material_cubic_meters);
        bed = inc.elevation_meters;
    }
    assert_eq!(p.elevation_meters, bed);
    assert_eq!(p.uplift_displacement_prism_cubic_meters, 0.);
    close(
        p.exported_erosion_material_cubic_meters,
        volumes.iter().sum(),
    );
}

#[test]
fn fully_wet_sphere_stays_fixed_and_disconnected_surface_is_rejected() {
    let s = Surface::build(2, 1000.);
    let h: Vec<_> = (0..s.areas.len()).map(|i| (i % 7) as f64).collect();
    let w = Water::generate(&s, &h, &WaterSettings::Coverage { fraction: 1. }).unwrap();
    let rates = structural_uplift_rates(&vec![6000.; h.len()], &w, 1e-6, 2000.).unwrap();
    let p = prepare(
        &s,
        &h,
        &w,
        &rates,
        Settings {
            steps: 8,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(p.elevation_meters, h);
    assert_eq!(p.exported_erosion_material_cubic_meters, 0.);
    assert_eq!(p.uplift_displacement_prism_cubic_meters, 0.);
    let mut disconnected = chain(&[1.; 2]);
    disconnected.offsets = vec![0; 3];
    disconnected.neighbors.clear();
    disconnected.distances.clear();
    let bed = [-1., 10.];
    let w = water_at(&disconnected, &bed);
    assert!(
        prepare(&disconnected, &bed, &w, &[0., 1.], settings())
            .unwrap_err()
            .contains("connected")
    );
}

#[test]
fn region_step_budget_rejects_before_using_or_allocating_geometry() {
    let s = chain(&[1.; 2]);
    let w = water_at(&s, &[-1., 10.]);
    let bed = vec![1.; 500001];
    let rates = vec![0.; bed.len()];
    let error = prepare(
        &s,
        &bed,
        &w,
        &rates,
        Settings {
            steps: 128,
            ..settings()
        },
    )
    .unwrap_err();
    assert!(error.contains("workload"));
}

#[test]
fn weighted_multistep_source_erosion_balance_reconstructs_near_zero_net_change() {
    let s = chain(&[1., 4., 9.]);
    let h = [-1., 8., 10.];
    let w = water_at(&s, &h);
    let rates = [0., 13_f64.sqrt() * 8., 6.];
    let p = prepare(
        &s,
        &h,
        &w,
        &rates,
        Settings {
            step_years: 0.001,
            steps: 64,
            ..settings()
        },
    )
    .unwrap();
    close(p.elevation_meters[1], 8.);
    close(p.elevation_meters[2], 10.);
    assert!(p.uplift_displacement_prism_cubic_meters > 0.);
    close(
        p.uplift_displacement_prism_cubic_meters,
        p.exported_erosion_material_cubic_meters,
    );
    assert!(p.independent_net_bed_prism_change_cubic_meters.abs() < 1e-10);
    for (i, &old) in h.iter().enumerate() {
        close(
            p.elevation_meters[i],
            old + p.accepted_uplift_displacement_meters[i] - p.removed_erosion_thickness_meters[i],
        );
    }
}

#[test]
fn underflowed_positive_displacement_request_refuses_instead_of_becoming_zero() {
    let s = chain(&[1.; 2]);
    let h = [-1., 10.];
    let w = water_at(&s, &h);
    let error = prepare(
        &s,
        &h,
        &w,
        &[0., 1e-300],
        Settings {
            erodibility_per_year: 0.,
            step_years: 1e-300,
            steps: 1,
        },
    )
    .unwrap_err();
    assert!(error.contains("displacement"));
    assert_eq!(h, [-1., 10.]);
}
