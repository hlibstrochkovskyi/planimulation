//! Experimental chordal P1 stiffness with unchanged spherical column areas.
//! Edge conductances are weak-form exchanges, not measured dual-face widths.
use super::surface_flow::{COURANT, Layout};
use crate::{Surface, cross, dot, norm};
use std::collections::BTreeMap;

pub(super) const MODEL_VERSION: &str = "regional-surface-cotan-candidate-1";

fn triangle_weights(points: [[f64; 3]; 3]) -> Result<[f64; 3], String> {
    let mut weights = [0.; 3];
    for i in 0..3 {
        let u = std::array::from_fn(|k| points[(i + 1) % 3][k] - points[i][k]);
        let v = std::array::from_fn(|k| points[(i + 2) % 3][k] - points[i][k]);
        let denominator = norm(cross(u, v));
        if !denominator.is_finite() || denominator <= 0. {
            return Err("Degenerate cotangent triangle.".into());
        }
        weights[i] = 0.5 * dot(u, v) / denominator;
        if !weights[i].is_finite() {
            return Err("Unrepresentable cotangent triangle weight.".into());
        }
    }
    Ok(weights)
}

// Preparation is private to the verification candidate. No seasonal checkpoint
// can select these weights under an existing physics pin.
pub(super) fn prepare(mesh: &Surface, mut layout: Layout) -> Result<Layout, String> {
    let mut weights: BTreeMap<[usize; 2], (f64, usize)> = BTreeMap::new();
    for triangle in mesh.faces.as_chunks::<3>().0 {
        let ids = triangle.map(|id| id as usize);
        let cotans = triangle_weights(ids.map(|id| mesh.centers[id]))?;
        for i in 0..3 {
            let a = ids[(i + 1) % 3];
            let b = ids[(i + 2) % 3];
            let entry = weights.entry([a.min(b), a.max(b)]).or_default();
            entry.0 += cotans[i];
            entry.1 += 1;
        }
    }
    if weights.len() != layout.faces.len() {
        return Err("Cotangent candidate graph does not match physical adjacency.".into());
    }
    let mut sums = vec![0.; layout.areas.len()];
    for face in &mut layout.faces {
        let &(weight, count) = weights
            .get(&face.regions)
            .ok_or("Missing cotangent edge.")?;
        // Positivity is a required property, not an assumption about arbitrary
        // triangulations. Reject zero/negative weights; never clip or flip edges.
        if count != 2 || !weight.is_finite() || weight <= 0. {
            return Err(
                "Cotangent candidate requires two incident triangles and positive edge weights."
                    .into(),
            );
        }
        face.width_over_distance = weight;
        for &r in &face.regions {
            sums[r] += weight;
        }
    }
    let diffusivity = layout.settings.maximum_diffusivity_square_meters_per_second;
    layout.stable_seconds = if diffusivity == 0. {
        f64::MAX
    } else {
        layout
            .areas
            .iter()
            .zip(sums)
            .map(|(&a, s)| COURANT * a / (diffusivity * s))
            .fold(f64::MAX, f64::min)
    };
    if !layout.stable_seconds.is_finite() || layout.stable_seconds <= 0. {
        return Err("Unrepresentable cotangent stability bound.".into());
    }
    Ok(layout)
}

#[cfg(test)]
mod tests {
    use super::super::{Mass, ResolutionBudget, surface};
    use super::*;
    use crate::moisture_transport::total_mass;
    fn layout(mesh: &Surface, cap: f64) -> Layout {
        prepare(
            mesh,
            Layout::from_fields(
                mesh,
                100_000.,
                vec![0.; mesh.areas.len()],
                vec![true; mesh.areas.len()],
                0.,
                super::super::surface_flow::Settings {
                    maximum_diffusivity_square_meters_per_second: cap,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn triangle_energy_matches_independent_affine_gradient_without_fitting() {
        for points in [
            [[0., 0., 0.], [2., 0., 0.], [0., 1., 0.]],
            [[0., 0., 0.], [2., 0., 0.], [0.4, 1.3, 0.]],
        ] {
            let weights = triangle_weights(points).unwrap();
            for gradient in [[1., 0., 0.], [0., 1., 0.], [2., -3., 0.]] {
                let values = points.map(|p| dot(p, gradient));
                let energy: f64 = (0..3)
                    .map(|i| weights[i] * (values[(i + 1) % 3] - values[(i + 2) % 3]).powi(2))
                    .sum();
                let area = 0.5 * norm(cross(points[1], points[2]));
                assert!((energy - area * dot(gradient, gradient)).abs() < 1e-13);
            }
        }
        assert!(triangle_weights([[0.; 3]; 3]).is_err());
    }
    #[test]
    fn every_supported_native_mesh_has_positive_weights_and_unchanged_physical_areas() {
        for level in 0..=6 {
            let mesh = Surface::build(level, 100_000.);
            let l = layout(&mesh, 10_000.);
            assert_eq!(l.areas, mesh.areas);
            assert!(l.faces.iter().all(|f| f.width_over_distance > 0.));
            assert!(l.stable_seconds > 0.);
        }
    }
    #[test]
    fn nonpositive_conductance_is_refused_not_clipped() {
        // Closed combinatorial sphere with an obtuse shared edge. This helper
        // tests the cotangent precondition independently of spherical preparation.
        let p = [[-1., 0., 0.], [1., 0., 0.], [0., 0.1, 0.], [0., -0.1, 0.]];
        let mut mesh = Surface::build(0, 100_000.);
        mesh.centers = p.to_vec();
        mesh.faces = vec![0, 1, 2, 1, 0, 3, 0, 2, 3, 1, 3, 2];
        let mut l = layout(&Surface::build(0, 100_000.), 10_000.);
        l.faces = [[0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3]]
            .map(|regions| super::super::surface_flow::Face {
                regions,
                width_over_distance: 1.,
                distance_meters: 1.,
            })
            .to_vec();
        assert!(
            prepare(&mesh, l)
                .err()
                .unwrap()
                .contains("positive edge weights")
        );
    }
    #[test]
    fn capped_flat_diffusion_dissipates_geometric_energy_without_losing_owned_mass() {
        let mesh = Surface::build(2, 100_000.);
        let l = layout(&mesh, 10_000.);
        let mut liquid: Vec<_> = mesh
            .centers
            .iter()
            .zip(&mesh.areas)
            .map(|(s, a)| Mass {
                high: 1000. * a * (100. + s[0]),
                low: 0.,
            })
            .collect();
        let initial = liquid.clone();
        let mut transfers = vec![Mass::default(); l.faces.len() * 2];
        let mut resolution = Some(ResolutionBudget::default());
        let by_region = vec![None; liquid.len()];
        let energy = |m: &[Mass]| {
            l.faces
                .iter()
                .map(|f| {
                    let [a, b] = f.regions;
                    f.width_over_distance
                        * (m[a].high / (1000. * l.areas[a]) - m[b].high / (1000. * l.areas[b]))
                            .powi(2)
                })
                .sum::<f64>()
        };
        let mut previous = energy(&liquid);
        for _ in 0..32 {
            surface::advance(
                &l,
                surface::Stocks {
                    by_region: &by_region,
                    liquid: &mut liquid,
                    bodies: &mut [],
                    transfers: &mut transfers,
                    resolution: &mut resolution,
                },
                0.5 * l.stable_seconds,
            )
            .unwrap();
            let now = energy(&liquid);
            assert!(now < previous);
            previous = now;
        }
        let terms: Vec<_> = liquid
            .iter()
            .zip(&initial)
            .flat_map(|(a, b)| [a.high, -b.high, a.low, -b.low])
            .collect();
        let sum = total_mass(&initial.iter().map(|m| m.high).collect::<Vec<_>>());
        assert!(total_mass(&terms).abs() / sum < 1e-12);
    }
    #[test]
    fn sill_blocking_finite_body_receipts_and_zero_cap_use_the_same_paired_stage() {
        let mesh = Surface::build(1, 100_000.);
        for (blocked, cap) in [(true, 10_000.), (false, 10_000.), (false, 0.)] {
            let mut l = layout(&mesh, cap);
            let [source, target] = l.faces[0].regions;
            l.beds.fill(100.);
            l.beds[source] = 0.;
            l.beds[target] = if blocked { 1. } else { 0. };
            l.land[target] = false;
            let mut liquid = vec![Mass::default(); mesh.areas.len()];
            liquid[source].high = 1000. * l.areas[source];
            let initial = liquid[source];
            let mut bodies = vec![Mass::default()];
            let mut by_region = vec![None; liquid.len()];
            by_region[target] = Some(0);
            let mut transfers = vec![Mass::default(); l.faces.len() * 2];
            let mut resolution = Some(ResolutionBudget::default());
            surface::advance(
                &l,
                surface::Stocks {
                    by_region: &by_region,
                    liquid: &mut liquid,
                    bodies: &mut bodies,
                    transfers: &mut transfers,
                    resolution: &mut resolution,
                },
                1.,
            )
            .unwrap();
            if blocked || cap == 0. {
                assert_eq!(liquid[source], initial);
                assert_eq!(bodies[0], Mass::default());
            } else {
                assert!(bodies[0].high > 0.);
                assert_eq!(bodies[0], transfers[0]);
                assert!(
                    total_mass(&[
                        liquid[source].high,
                        -initial.high,
                        liquid[source].low,
                        bodies[0].high,
                        bodies[0].low
                    ])
                    .abs()
                        < 1e-4
                );
            }
        }
    }
}
