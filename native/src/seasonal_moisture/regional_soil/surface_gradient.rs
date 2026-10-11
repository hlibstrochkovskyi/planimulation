//! Full chordal tangent-gradient mobility candidate; no seasonal persistence.
//! Scalar edge slope is the mean of incident triangle gradient magnitudes.
use super::{surface_cotan, surface_flow::Layout};
use crate::{Surface, dot, norm};
use std::collections::BTreeMap;

pub(super) const MODEL_VERSION: &str = "regional-surface-full-gradient-candidate-1";

struct Triangle {
    ids: [usize; 3],
    dual: [[f64; 3]; 2],
}

fn dual_basis(points: [[f64; 3]; 3], radius: f64) -> Result<[[f64; 3]; 2], String> {
    if !radius.is_finite() || radius <= 0. || points.iter().flatten().any(|v| !v.is_finite()) {
        return Err("Invalid full-gradient geometry.".into());
    }
    let u = std::array::from_fn(|k| points[1][k] - points[0][k]);
    let v = std::array::from_fn(|k| points[2][k] - points[0][k]);
    let uu = dot(u, u);
    let uv = dot(u, v);
    let vv = dot(v, v);
    let determinant = uu * vv - uv * uv;
    if !determinant.is_finite() || determinant <= 0. {
        return Err("Degenerate full-gradient triangle.".into());
    }
    // Solve the two tangent constraints g.(R*u)=delta_1, g.(R*v)=delta_2.
    // Work in unit coordinates first to avoid radius^4 in the Gram determinant.
    let basis = [
        std::array::from_fn(|k| ((vv * u[k] - uv * v[k]) / determinant) / radius),
        std::array::from_fn(|k| ((uu * v[k] - uv * u[k]) / determinant) / radius),
    ];
    if basis.iter().flatten().any(|v| !v.is_finite()) {
        return Err("Unrepresentable full-gradient basis.".into());
    }
    Ok(basis)
}

impl Triangle {
    fn slope(&self, beds: &[f64], depth: &[f64]) -> Result<f64, String> {
        let [a, b, c] = self.ids;
        // Relative bed differences preserve thin water under a large common datum.
        let differences = [
            beds[b] - beds[a] + depth[b] - depth[a],
            beds[c] - beds[a] + depth[c] - depth[a],
        ];
        let gradient = std::array::from_fn(|k| {
            self.dual[0][k] * differences[0] + self.dual[1][k] * differences[1]
        });
        let slope = norm(gradient);
        if !slope.is_finite() {
            return Err("Unrepresentable full-gradient slope.".into());
        }
        Ok(slope)
    }
}

pub(super) struct Prepared {
    layout: Layout,
    triangles: Vec<Triangle>,
    incident: Vec<[usize; 2]>,
}

impl Prepared {
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn new(mesh: &Surface, layout: Layout, radius: f64) -> Result<Self, String> {
        let (faces, remainder) = mesh.faces.as_chunks::<3>();
        if !remainder.is_empty()
            || mesh.centers.len() != layout.areas.len()
            || layout.beds.len() != layout.areas.len()
            || layout.land.len() != layout.areas.len()
            || layout.areas != mesh.areas
            || layout.areas.iter().any(|a| !a.is_finite() || *a <= 0.)
            || layout.beds.iter().any(|b| !b.is_finite())
            || !layout.reference_level.is_finite()
            || faces.is_empty()
            || faces
                .iter()
                .flatten()
                .any(|&id| id as usize >= mesh.centers.len())
        {
            return Err("Full-gradient mesh/layout shape mismatch.".into());
        }
        let mut triangles = Vec::with_capacity(faces.len());
        let mut edges: BTreeMap<[usize; 2], Vec<usize>> = BTreeMap::new();
        for (t, face) in faces.iter().enumerate() {
            let ids = face.map(|id| id as usize);
            triangles.push(Triangle {
                ids,
                dual: dual_basis(ids.map(|id| mesh.centers[id]), radius)?,
            });
            for i in 0..3 {
                let a = ids[i];
                let b = ids[(i + 1) % 3];
                edges.entry([a.min(b), a.max(b)]).or_default().push(t);
            }
        }
        if edges.len() != layout.faces.len() {
            return Err("Full-gradient adjacency mismatch.".into());
        }
        let incident: Vec<_> = layout
            .faces
            .iter()
            .map(|face| match edges.get(&face.regions).map(Vec::as_slice) {
                Some(&[a, b]) if a != b => Ok([a, b]),
                _ => Err("Full-gradient edge requires two incident triangles.".into()),
            })
            .collect::<Result<_, String>>()?;
        // Own the immutable cotangent layout: a stencil cannot be paired later
        // with reordered edges or a different seasonal/world preparation.
        Ok(Self {
            layout: surface_cotan::prepare(mesh, layout)?,
            triangles,
            incident,
        })
    }

    pub fn edge_slopes(&self, depth: &[f64]) -> Result<Vec<f64>, String> {
        if depth.len() != self.layout.beds.len() || depth.iter().any(|d| !d.is_finite() || *d < 0.)
        {
            return Err("Invalid full-gradient depths.".into());
        }
        let slopes: Vec<_> = self
            .triangles
            .iter()
            .map(|t| t.slope(&self.layout.beds, depth))
            .collect::<Result<_, _>>()?;
        // Average magnitudes, not vectors: opposed gradients must not cancel.
        Ok(self
            .incident
            .iter()
            .map(|&[a, b]| 0.5 * slopes[a] + 0.5 * slopes[b])
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Mass, ResolutionBudget, surface, surface_verification};
    use super::*;

    fn layout(mesh: &Surface, cap: f64) -> Layout {
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
        .unwrap()
    }

    #[test]
    fn capped_and_nonlinear_dry_arrivals_are_positive_frozen_and_audited() {
        let mesh = Surface::build(1, 100_000.);
        for cap in [1., 1e8] {
            let prepared = Prepared::new(&mesh, layout(&mesh, cap), 100_000.).unwrap();
            let l = prepared.layout();
            let neighbors: Vec<_> = l
                .faces
                .iter()
                .filter_map(|f| {
                    if f.regions[0] == 0 {
                        Some(f.regions[1])
                    } else if f.regions[1] == 0 {
                        Some(f.regions[0])
                    } else {
                        None
                    }
                })
                .collect();
            let second_ring: Vec<_> = (1..l.areas.len())
                .filter(|r| {
                    !neighbors.contains(r)
                        && l.faces.iter().any(|f| {
                            f.regions.contains(r) && f.regions.iter().any(|n| neighbors.contains(n))
                        })
                })
                .collect();
            assert!(!second_ring.is_empty());
            let mut liquid = vec![Mass::default(); l.areas.len()];
            liquid[0].high = 1000. * l.areas[0];
            let initial = liquid.clone();
            let depth: Vec<_> = liquid
                .iter()
                .zip(&l.areas)
                .map(|(m, a)| m.high / (1000. * a))
                .collect();
            for (f, slope) in l.faces.iter().zip(prepared.edge_slopes(&depth).unwrap()) {
                if f.regions.contains(&0) {
                    let mobility = 1. / (l.settings.roughness * slope.sqrt());
                    assert_eq!(mobility > cap, cap == 1.);
                }
            }
            let mut transfers = vec![Mass::default(); 2 * l.faces.len()];
            let mut resolution = Some(ResolutionBudget::default());
            let by_region = vec![None; liquid.len()];
            for step in 0..2 {
                surface::advance_full_gradient(
                    &prepared,
                    surface::Stocks {
                        by_region: &by_region,
                        liquid: &mut liquid,
                        bodies: &mut [],
                        transfers: &mut transfers,
                        resolution: &mut resolution,
                    },
                    0.25 * l.stable_seconds,
                )
                .unwrap();
                assert!(neighbors.iter().all(|&r| liquid[r].high > 0.));
                if step == 0 {
                    assert!(second_ring.iter().all(|&r| liquid[r] == Mass::default()));
                } else {
                    assert!(second_ring.iter().any(|&r| liquid[r].high > 0.));
                }
                surface_verification::audit(l, &initial, &liquid, &transfers).unwrap();
                assert!(liquid.iter().all(|m| m.stock(f64::MAX).is_ok()));
            }
        }
    }

    #[test]
    fn crest_blocking_body_receipts_zero_cap_and_work_bound_stay_explicit() {
        let mesh = Surface::build(1, 100_000.);
        for (blocked, cap) in [(true, 10_000.), (false, 10_000.), (false, 0.)] {
            let mut l = layout(&mesh, cap);
            let [source, target] = l.faces[0].regions;
            l.beds.fill(100.);
            l.beds[source] = 0.;
            l.beds[target] = if blocked { 1. } else { 0. };
            l.land[target] = false;
            let prepared = Prepared::new(&mesh, l, 100_000.).unwrap();
            let l = prepared.layout();
            let mut liquid = vec![Mass::default(); l.areas.len()];
            liquid[source].high = 1000. * l.areas[source];
            let initial = liquid.clone();
            let mut bodies = vec![Mass::default()];
            let mut by_region = vec![None; liquid.len()];
            by_region[target] = Some(0);
            let mut transfers = vec![Mass::default(); 2 * l.faces.len()];
            let mut resolution = Some(ResolutionBudget::default());
            surface::advance_full_gradient(
                &prepared,
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
                assert_eq!(liquid, initial);
                assert_eq!(bodies[0], Mass::default());
            } else {
                assert!(bodies[0].high > 0.);
                assert_eq!(bodies[0], transfers[0]);
                let residual = crate::moisture_transport::total_mass(&[
                    liquid[source].high,
                    -initial[source].high,
                    liquid[source].low,
                    bodies[0].high,
                    bodies[0].low,
                ]);
                assert!(residual.abs() < 1e-4);
            }
            let before = liquid.clone();
            let history = transfers.clone();
            assert!(
                surface::advance_full_gradient(
                    &prepared,
                    surface::Stocks {
                        by_region: &by_region,
                        liquid: &mut liquid,
                        bodies: &mut bodies,
                        transfers: &mut transfers,
                        resolution: &mut resolution
                    },
                    f64::INFINITY
                )
                .is_err()
            );
            assert_eq!(before, liquid);
            assert_eq!(history, transfers);
        }
    }

    #[test]
    fn affine_gradients_rotation_radius_and_datum_are_independently_exact() {
        for points in [
            [[0., 0., 0.], [2., 0., 0.], [0.4, 1.3, 0.]],
            [[0., 0., 0.], [0., 2., 0.], [0., 0.4, 1.3]],
        ] {
            let expected = if points[1][0] == 2. {
                [2., -3., 0.]
            } else {
                [0., 2., -3.]
            };
            let depth = points.map(|p| 10. + dot(p, expected));
            for radius in [1., 2., 100_000.] {
                let t = Triangle {
                    ids: [0, 1, 2],
                    dual: dual_basis(points, radius).unwrap(),
                };
                let slope = t.slope(&[0.; 3], &depth).unwrap();
                assert!((slope * radius / 13_f64.sqrt() - 1.).abs() < 1e-14);
                assert_eq!(slope, t.slope(&[1e15; 3], &depth).unwrap());
                assert_eq!(t.slope(&[0.; 3], &[10.; 3]).unwrap(), 0.);
            }
        }
        assert!(dual_basis([[0.; 3]; 3], 1.).is_err());
        assert!(dual_basis([[f64::NAN; 3]; 3], 1.).is_err());
        assert!(dual_basis([[0.; 3]; 3], 0.).is_err());
    }

    #[test]
    fn sloped_bed_lake_at_rest_and_common_datum_have_exactly_zero_requests() {
        let mesh = Surface::build(1, 100_000.);
        for datum in [0., 1e15] {
            let mut l = layout(&mesh, 1e6);
            for (r, bed) in l.beds.iter_mut().enumerate() {
                *bed = datum + if r % 2 == 0 { 0. } else { 0.5 };
            }
            let prepared = Prepared::new(&mesh, l, 100_000.).unwrap();
            let l = prepared.layout();
            let mut liquid: Vec<_> = l
                .areas
                .iter()
                .enumerate()
                .map(|(r, a)| Mass {
                    high: 1000. * a * if r % 2 == 0 { 1. } else { 0.5 },
                    low: 0.,
                })
                .collect();
            let initial = liquid.clone();
            let mut transfers = vec![Mass::default(); 2 * l.faces.len()];
            let mut resolution = Some(ResolutionBudget::default());
            surface::advance_full_gradient(
                &prepared,
                surface::Stocks {
                    by_region: &vec![None; liquid.len()],
                    liquid: &mut liquid,
                    bodies: &mut [],
                    transfers: &mut transfers,
                    resolution: &mut resolution,
                },
                l.stable_seconds,
            )
            .unwrap();
            assert_eq!(liquid, initial);
            assert!(transfers.iter().all(|m| *m == Mass::default()));
            assert_eq!(resolution, Some(ResolutionBudget::default()));
        }
    }

    #[test]
    fn opposed_triangle_gradients_do_not_cancel_at_the_shared_edge() {
        let points = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., -1., 0.]];
        let triangles = [[0, 1, 2], [1, 0, 3]].map(|ids| Triangle {
            ids,
            dual: dual_basis(ids.map(|id| points[id]), 1.).unwrap(),
        });
        let slopes = triangles.map(|t| t.slope(&[0.; 4], &[0., 0., 1., 1.]).unwrap());
        assert_eq!(slopes, [1., 1.]);
        assert_eq!(0.5 * slopes[0] + 0.5 * slopes[1], 1.);
    }

    #[test]
    fn supported_meshes_and_invalid_inputs_have_explicit_preconditions() {
        for level in 0..=6 {
            let mesh = Surface::build(level, 100_000.);
            let layout = Layout::from_fields(
                &mesh,
                100_000.,
                vec![0.; mesh.areas.len()],
                vec![true; mesh.areas.len()],
                0.,
                Default::default(),
            )
            .unwrap();
            let prepared = Prepared::new(&mesh, layout.clone(), 100_000.).unwrap();
            assert_eq!(prepared.layout.areas, layout.areas);
            assert!(
                prepared
                    .edge_slopes(&vec![1.; mesh.areas.len()])
                    .unwrap()
                    .iter()
                    .all(|&s| s == 0.)
            );
            assert!(prepared.edge_slopes(&[]).is_err());
            assert!(
                prepared
                    .edge_slopes(&vec![f64::NAN; mesh.areas.len()])
                    .is_err()
            );
            assert!(prepared.edge_slopes(&vec![-1.; mesh.areas.len()]).is_err());
            if level == 0 {
                let mut broken = Surface::build(0, 100_000.);
                broken.faces[0] = u32::MAX;
                assert!(Prepared::new(&broken, layout.clone(), 100_000.).is_err());
                broken = Surface::build(0, 100_000.);
                broken.faces.push(0);
                assert!(Prepared::new(&broken, layout.clone(), 100_000.).is_err());
                let mut bad_layout = layout.clone();
                bad_layout.faces.swap(0, 1);
                // Reordering is prepared explicitly, never a later stencil swap.
                assert!(Prepared::new(&mesh, bad_layout, 100_000.).is_ok());
                assert!(Prepared::new(&mesh, layout.clone(), 0.).is_err());
                let mut bad_layout = layout.clone();
                bad_layout.faces.pop();
                assert!(Prepared::new(&mesh, bad_layout, 100_000.).is_err());
            }
        }
    }
}
