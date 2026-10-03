use planimulation_core::{
    Surface,
    moisture_transport::{Flow, Geometry, Settings, total_mass},
};
use std::f64::consts::PI;

const RADIUS: f64 = 1_000_000.;
const OMEGA: f64 = 1e-4;

fn rotation(point: [f64; 3]) -> [f64; 3] {
    [-OMEGA * RADIUS * point[2], 0., OMEGA * RADIUS * point[0]]
}

fn fixture(level: u32) -> (Surface, Geometry, Flow) {
    let surface = Surface::build(level, RADIUS);
    let geometry = Geometry::from_surface(&surface, RADIUS).unwrap();
    let flow = Flow::sample(&geometry, rotation).unwrap();
    (surface, geometry, flow)
}

fn blob(point: [f64; 3], longitude: f64) -> f64 {
    0.1 + (12. * (point[0] * longitude.cos() + point[2] * longitude.sin() - 1.)).exp()
}

fn initial(surface: &Surface) -> Vec<f64> {
    surface
        .centers
        .iter()
        .zip(&surface.areas)
        .map(|(&point, area)| blob(point, PI - 0.2) * area)
        .collect()
}

fn l1_error(surface: &Surface, stocks: &[f64], longitude: f64) -> f64 {
    stocks
        .iter()
        .zip(&surface.centers)
        .zip(&surface.areas)
        .map(|((&mass, &point), &area)| (mass / area - blob(point, longitude)).abs() * area)
        .sum::<f64>()
        / surface.areas.iter().sum::<f64>()
}

#[test]
fn boundaries_match_the_existing_barycentric_dual_at_all_supported_levels() {
    for level in 0..=6 {
        let surface = Surface::build(level, RADIUS);
        let geometry = Geometry::from_surface(&surface, RADIUS).unwrap();
        assert_eq!(geometry.boundaries().len(), surface.neighbors.len() / 2);
        let mut lengths = vec![0.; surface.centers.len()];
        for boundary in geometry.boundaries() {
            let [i, j] = boundary.regions;
            assert!(i < j);
            assert!(
                surface.neighbors[surface.offsets[i] as usize..surface.offsets[i + 1] as usize]
                    .contains(&(j as u32))
            );
            for segment in boundary.segments {
                assert!(segment.arc_length_meters > 0.);
                assert!(segment.quadrature_weight_meters <= segment.arc_length_meters);
                assert!(
                    segment
                        .midpoint
                        .iter()
                        .zip(segment.outward_normal)
                        .map(|(a, b)| a * b)
                        .sum::<f64>()
                        .abs()
                        < 1e-12
                );
                lengths[i] += segment.arc_length_meters;
                lengths[j] += segment.arc_length_meters;
            }
        }
        assert!(lengths.iter().all(|length| *length > 0.));
        assert!(
            (total_mass(geometry.areas_square_meters()) / (4. * PI * RADIUS.powi(2)) - 1.).abs()
                < 1e-12
        );
    }
}

#[test]
fn zero_wind_and_empty_stock_are_exact_fixed_points() {
    let (surface, geometry, rotating) = fixture(2);
    let still = Flow::sample(&geometry, |_| [0.; 3]).unwrap();
    let stock = initial(&surface);
    let step = still.advance(&stock, 86400., Settings::default()).unwrap();
    assert_eq!(step.stock_kilograms, stock);
    assert_eq!(step.budget.transported_kilograms, 0.);
    assert_eq!(step.budget.substeps, 1);
    let empty = vec![0.; stock.len()];
    assert_eq!(
        rotating
            .advance(&empty, 86400., Settings::default())
            .unwrap()
            .stock_kilograms,
        empty
    );
}

#[test]
fn rigid_rotation_preserves_uniform_column_water_without_divergence_correction() {
    for level in 0..=4 {
        let (surface, geometry, _) = fixture(level);
        let flow = Flow::sample(&geometry, |p| {
            let omega = [2e-5, -8e-5, 4e-5];
            [
                RADIUS * (omega[1] * p[2] - omega[2] * p[1]),
                RADIUS * (omega[2] * p[0] - omega[0] * p[2]),
                RADIUS * (omega[0] * p[1] - omega[1] * p[0]),
            ]
        })
        .unwrap();
        let stock: Vec<_> = surface.areas.iter().map(|area| 25. * area).collect();
        let step = flow.advance(&stock, 10000., Settings::default()).unwrap();
        for (&mass, &area) in step.stock_kilograms.iter().zip(&surface.areas) {
            assert!((mass / area - 25.).abs() < 1e-11);
        }
        assert!(step.budget.residual_kilograms.abs() / step.budget.initial_kilograms < 1e-13);
    }
}

#[test]
fn a_single_donor_transfers_only_to_neighbors_using_the_old_snapshot() {
    let (surface, _, flow) = fixture(2);
    let source = surface.centers.iter().position(|p| p[0] > 0.9).unwrap();
    let mut stock = vec![0.; surface.centers.len()];
    stock[source] = 1e9;
    let step = flow.advance(&stock, 1., Settings::default()).unwrap();
    assert_eq!(step.budget.substeps, 1);
    assert!(step.stock_kilograms[source] < stock[source]);
    assert!(step.stock_kilograms[source] > 0.);
    let neighbors =
        &surface.neighbors[surface.offsets[source] as usize..surface.offsets[source + 1] as usize];
    for (region, &mass) in step.stock_kilograms.iter().enumerate() {
        if region != source && !neighbors.contains(&(region as u32)) {
            assert_eq!(mass, 0.);
        }
    }
    assert!(step.budget.residual_kilograms.abs() < 1e-6);
}

#[test]
fn moving_blob_crosses_the_atlas_seam_and_error_decreases_with_resolution() {
    let mut errors = Vec::new();
    for level in 1..=4 {
        let (surface, _, flow) = fixture(level);
        let stock = initial(&surface);
        let step = flow.advance(&stock, 6000., Settings::default()).unwrap();
        assert!(
            step.stock_kilograms
                .iter()
                .all(|mass| *mass >= 0. && mass.is_finite())
        );
        assert!(step.budget.max_outgoing_fraction <= 0.8 * (1. + 1e-14));
        errors.push(l1_error(&surface, &step.stock_kilograms, PI + 0.4));
        if level == 4 {
            let moment = |axis: usize| {
                step.stock_kilograms
                    .iter()
                    .zip(&surface.centers)
                    .map(|(mass, point)| mass * point[axis])
                    .sum::<f64>()
            };
            // The center starts at z>0 and rotates through longitude +pi into z<0.
            assert!(moment(0) < 0. && moment(2) < 0.);
        }
    }
    for pair in errors.windows(2) {
        assert!(pair[1] < pair[0], "Spatial errors: {errors:?}");
    }
}

#[test]
fn smaller_time_steps_converge_at_fixed_mesh_and_keep_the_mass_budget() {
    let (surface, _, flow) = fixture(3);
    let stock = initial(&surface);
    let reference = flow
        .advance(
            &stock,
            6000.,
            Settings {
                max_outgoing_fraction: 0.025,
                ..Settings::default()
            },
        )
        .unwrap();
    let mut differences = Vec::new();
    for fraction in [0.8, 0.4, 0.2, 0.1] {
        let step = flow
            .advance(
                &stock,
                6000.,
                Settings {
                    max_outgoing_fraction: fraction,
                    ..Settings::default()
                },
            )
            .unwrap();
        differences.push(
            step.stock_kilograms
                .iter()
                .zip(&reference.stock_kilograms)
                .map(|(a, b)| (a - b).abs())
                .sum::<f64>()
                / total_mass(&stock),
        );
        assert!(step.budget.residual_kilograms.abs() / total_mass(&stock) < 1e-13);
    }
    for pair in differences.windows(2) {
        assert!(pair[1] < pair[0], "Temporal differences: {differences:?}");
    }
}

#[test]
fn stock_serialization_continues_the_same_prescribed_flow_exactly() {
    let (surface, _, flow) = fixture(2);
    let stock = initial(&surface);
    let first = flow.advance(&stock, 1000., Settings::default()).unwrap();
    let bytes = serde_json::to_vec(&first.stock_kilograms).unwrap();
    let restored: Vec<f64> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        flow.advance(&restored, 1000., Settings::default()).unwrap(),
        flow.advance(&first.stock_kilograms, 1000., Settings::default())
            .unwrap()
    );
    assert_eq!(
        flow.advance(&stock, 1000., Settings::default()).unwrap(),
        first
    );
}

#[test]
fn invalid_geometry_wind_stocks_and_excessive_work_reject_without_mutation() {
    let (mut surface, geometry, flow) = fixture(1);
    assert!(Geometry::from_surface(&surface, RADIUS * 2.).is_err());
    assert!(Flow::sample(&geometry, |_| [f64::NAN; 3]).is_err());
    assert!(Flow::sample(&geometry, |point| point).is_err());
    let stock = initial(&surface);
    let snapshot = stock.clone();
    assert!(
        flow.advance(
            &stock,
            86400.,
            Settings {
                max_substeps: 1,
                ..Settings::default()
            }
        )
        .is_err()
    );
    assert_eq!(stock, snapshot);
    for seconds in [0., -1., f64::INFINITY, f64::NAN] {
        assert!(flow.advance(&stock, seconds, Settings::default()).is_err());
    }
    for fraction in [0., -1., 1., f64::NAN] {
        assert!(
            flow.advance(
                &stock,
                1.,
                Settings {
                    max_outgoing_fraction: fraction,
                    ..Settings::default()
                }
            )
            .is_err()
        );
    }
    assert!(flow.advance(&[], 1., Settings::default()).is_err());
    let mut bad = stock;
    bad[0] = -1.;
    assert!(flow.advance(&bad, 1., Settings::default()).is_err());
    bad.fill(f64::MAX);
    assert!(flow.advance(&bad, 1., Settings::default()).is_err());
    surface.faces.pop();
    assert!(Geometry::from_surface(&surface, RADIUS).is_err());
}
