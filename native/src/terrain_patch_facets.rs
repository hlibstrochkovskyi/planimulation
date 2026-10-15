//! Bounded triangular-facet routing on an owned spherical patch.
//! Area diagnostics only: no water inventory, filled pits or changed bed.
use crate::{terrain_patch_geometry::Geometry, terrain_patch_landscape};
use serde::Serialize;

pub const MODEL_VERSION: &str = "spherical-patch-facet-routing-1";
type V2 = [f64; 2];
type V3 = [f64; 3];
fn dot(a: V3, b: V3) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn det(a: V2, b: V2) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn norm(a: V2) -> f64 {
    a[0].hypot(a[1])
}
fn product(a: V2, b: V2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn angle(a: V2, b: V2) -> f64 {
    det(a, b).abs().atan2(product(a, b))
}

#[derive(Clone, Copy, Default)]
struct Neighbor {
    id: u32,
    displacement: V2,
    distance: f64,
}

/// The tangent basis is deterministic (north-Y anchor, Z near a pole).
/// Neighbor vectors use great-circle lengths and initial great-circle bearings.
pub struct Router {
    geometry: Geometry,
    ring: Vec<[Neighbor; 8]>,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Descent {
    pub receivers: [u32; 2],
    /// Angular area partition; not incision coefficients or measured discharge.
    pub area_weights: [f64; 2],
    /// d = sum(alpha_j * tangent_displacement_j), units 1/m.
    pub derivative_coefficients_per_meter: [f64; 2],
    pub unit_tangent_direction: V2,
    pub slope: f64,
    pub flat_fallback: bool,
}
impl Descent {
    pub fn branches(&self) -> impl Iterator<Item = (usize, f64, f64)> + '_ {
        (0..2).filter(|&j| self.area_weights[j] > 0.).map(|j| {
            (
                self.receivers[j] as usize,
                self.area_weights[j],
                self.derivative_coefficients_per_meter[j],
            )
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Routing {
    pub model_version: &'static str,
    pub descents: Vec<Descent>,
    pub contributing_area_square_meters: Vec<f64>,
    pub upstream_to_downstream: Vec<u32>,
    /// Fractional ancestry can reach several terminals. No unique outlet ID.
    pub closed_fraction_of_active_area: f64,
    pub closed_sink_count: usize,
    pub two_receiver_nodes: usize,
    pub flat_fallback_nodes: usize,
    pub terminal_area_relative_residual: f64,
}

fn add(sum: &mut f64, correction: &mut f64, v: f64) -> Result<(), String> {
    let adjusted = v - *correction;
    let next = *sum + adjusted;
    *correction = (next - *sum) - adjusted;
    *sum = next;
    if !v.is_finite() || !next.is_finite() || !correction.is_finite() {
        return Err("Facet routing accumulation overflowed.".into());
    }
    Ok(())
}

impl Router {
    pub fn new(g: &Geometry) -> Result<Self, String> {
        // Reuse the bounded builder-contract validation. Coordinates below
        // receive their own unit/metric checks; no public array authentication.
        g.settings.validate()?;
        let nx = g.settings.cells_x as usize;
        let ny = g.settings.cells_y as usize;
        if g.cell_directions.len() != nx * ny || g.vertex_directions.len() != (nx + 1) * (ny + 1) {
            return Err("Invalid bounded facet geometry dimensions.".into());
        }
        terrain_patch_landscape::route(g, &vec![0.; nx * ny])?;
        let mut ring = vec![[Neighbor::default(); 8]; nx * ny];
        for (i, neighbors) in ring.iter_mut().enumerate() {
            let p = g.cell_directions[i];
            if p.iter().any(|v| !v.is_finite()) || (dot(p, p) - 1.).abs() > 1e-12 {
                return Err("Invalid facet patch direction.".into());
            }
            if g.fixed_external_boundary[i] {
                continue;
            }
            let anchor = if p[1].abs() > 0.9 {
                [0., 0., 1.]
            } else {
                [0., 1., 0.]
            };
            let e = cross(p, anchor);
            let length = dot(e, e).sqrt();
            let east = e.map(|v| v / length);
            let north = cross(east, p);
            for (k, j) in [
                i + 1,
                i + nx + 1,
                i + nx,
                i + nx - 1,
                i - 1,
                i - nx - 1,
                i - nx,
                i - nx + 1,
            ]
            .into_iter()
            .enumerate()
            {
                let q = g.cell_directions[j];
                if q.iter().any(|v| !v.is_finite()) || (dot(q, q) - 1.).abs() > 1e-12 {
                    return Err("Invalid facet neighbor direction.".into());
                }
                // Stable tangent projection, avoiding q-p*dot(p,q).
                let delta = std::array::from_fn(|a| q[a] - p[a]);
                let tangent = cross(p, cross(delta, p));
                let projected = [dot(tangent, east), dot(tangent, north)];
                let start = g.offsets[i] as usize;
                let position = g.neighbors[start..g.offsets[i + 1] as usize]
                    .binary_search(&(j as u32))
                    .unwrap()
                    + start;
                let distance = g.distances_meters[position];
                let scale = distance / norm(projected);
                let displacement = projected.map(|v| v * scale);
                if !scale.is_finite()
                    || displacement.iter().any(|v| !v.is_finite())
                    || norm(displacement) <= 0.
                {
                    return Err("Unrepresentable facet tangent geometry.".into());
                }
                neighbors[k] = Neighbor {
                    id: j as u32,
                    displacement,
                    distance,
                };
            }
            for k in 0..8 {
                let a = neighbors[k].displacement;
                let b = neighbors[(k + 1) % 8].displacement;
                if !det(a, b).is_finite() || det(a, b) == 0. || product(a, b) <= 0. {
                    return Err("Facet chart requires nondegenerate acute neighbor sectors.".into());
                }
            }
        }
        Ok(Self {
            geometry: g.clone(),
            ring,
        })
    }
    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    pub fn tangent_displacement(&self, cell: usize, neighbor: usize) -> Option<V2> {
        if self.geometry.fixed_external_boundary.get(cell).copied()? {
            return None;
        }
        self.ring
            .get(cell)?
            .iter()
            .find(|n| n.id as usize == neighbor)
            .map(|n| n.displacement)
    }

    pub fn route(&self, h: &[f64]) -> Result<Routing, String> {
        // The old plateau algorithm is used only where no positive facet or
        // edge descent exists. It never connects through higher physical bed.
        let baseline = terrain_patch_landscape::route(&self.geometry, h)?;
        let n = h.len();
        let mut descents = Vec::with_capacity(n);
        for i in 0..n {
            let mut best = Descent {
                receivers: [i as u32; 2],
                ..Descent::default()
            };
            if self.geometry.fixed_external_boundary[i] {
                descents.push(best);
                continue;
            }
            for &a in &self.ring[i] {
                let drop = h[i] - h[a.id as usize];
                let slope = drop / a.distance;
                if !slope.is_finite() {
                    return Err("Facet edge slope overflowed.".into());
                }
                if slope > best.slope {
                    best = Descent {
                        receivers: [a.id, i as u32],
                        area_weights: [1., 0.],
                        derivative_coefficients_per_meter: [1. / a.distance, 0.],
                        unit_tangent_direction: a.displacement.map(|v| v / a.distance),
                        slope,
                        flat_fallback: false,
                    };
                }
            }
            for k in 0..8 {
                let a = self.ring[i][k];
                let b = self.ring[i][(k + 1) % 8];
                let drop_a = h[i] - h[a.id as usize];
                let drop_b = h[i] - h[b.id as usize];
                if drop_a <= 0. || drop_b <= 0. {
                    continue;
                }
                let va = a.displacement;
                let vb = b.displacement;
                let determinant = det(va, vb);
                let gradient = [
                    (drop_a * vb[1] - drop_b * va[1]) / determinant,
                    (va[0] * drop_b - vb[0] * drop_a) / determinant,
                ];
                let slope = norm(gradient);
                if !slope.is_finite() {
                    return Err("Facet reconstructed slope overflowed.".into());
                }
                if slope <= best.slope {
                    continue;
                }
                let direction = gradient.map(|v| v / slope);
                let alpha = [
                    det(direction, vb) / determinant,
                    det(va, direction) / determinant,
                ];
                if alpha.iter().any(|v| !v.is_finite()) {
                    return Err("Facet derivative overflowed.".into());
                }
                if alpha[0] <= 0. || alpha[1] <= 0. {
                    continue;
                }
                let from_a = angle(va, direction);
                let from_b = angle(direction, vb);
                let span = from_a + from_b;
                let weights = [from_b / span, from_a / span];
                if weights
                    .iter()
                    .any(|w| !w.is_finite() || *w <= 0. || *w > 1.)
                {
                    return Err("Unrepresentable positive facet partition.".into());
                }
                best = Descent {
                    receivers: [a.id, b.id],
                    area_weights: weights,
                    derivative_coefficients_per_meter: alpha,
                    unit_tangent_direction: direction,
                    slope,
                    flat_fallback: false,
                };
            }
            if best.slope == 0. && baseline.receivers[i] != i as u32 {
                let j = baseline.receivers[i];
                let a = self.ring[i].iter().find(|v| v.id == j).unwrap();
                best = Descent {
                    receivers: [j, i as u32],
                    area_weights: [1., 0.],
                    derivative_coefficients_per_meter: [1. / a.distance, 0.],
                    unit_tangent_direction: a.displacement.map(|v| v / a.distance),
                    slope: 0.,
                    flat_fallback: true,
                };
            }
            descents.push(best);
        }
        let mut incoming = vec![0usize; n];
        for d in &descents {
            for (j, _, _) in d.branches() {
                incoming[j] += 1;
            }
        }
        let mut order: Vec<_> = (0..n)
            .filter(|&i| incoming[i] == 0)
            .map(|i| i as u32)
            .collect();
        let mut area: Vec<_> = self
            .geometry
            .reference_areas_square_meters
            .iter()
            .enumerate()
            .map(|(i, &a)| {
                if self.geometry.fixed_external_boundary[i] {
                    0.
                } else {
                    a
                }
            })
            .collect();
        let mut corrections = vec![0.; n];
        let mut cursor = 0;
        while cursor < order.len() {
            let i = order[cursor] as usize;
            cursor += 1;
            for (j, w, _) in descents[i].branches() {
                let contribution = area[i] * w;
                add(&mut area[j], &mut corrections[j], contribution)?;
                incoming[j] -= 1;
                if incoming[j] == 0 {
                    order.push(j as u32);
                }
            }
        }
        if order.len() != n {
            return Err("Facet drainage must be acyclic.".into());
        }
        let mut active = 0.;
        let mut active_c = 0.;
        let mut terminals = 0.;
        let mut terminal_c = 0.;
        let mut closed = 0.;
        let mut closed_c = 0.;
        let mut sinks = 0;
        for (i, d) in descents.iter().enumerate() {
            if !self.geometry.fixed_external_boundary[i] {
                add(
                    &mut active,
                    &mut active_c,
                    self.geometry.reference_areas_square_meters[i],
                )?;
            }
            if d.branches().next().is_none() {
                add(&mut terminals, &mut terminal_c, area[i])?;
                if !self.geometry.fixed_external_boundary[i] {
                    sinks += 1;
                    add(&mut closed, &mut closed_c, area[i])?;
                }
            }
        }
        let residual = (terminals - active).abs() / active;
        if !residual.is_finite() || residual > 1e-12 {
            return Err("Facet terminal-area reconciliation failed.".into());
        }
        Ok(Routing {
            model_version: MODEL_VERSION,
            two_receiver_nodes: descents.iter().filter(|d| d.area_weights[1] > 0.).count(),
            flat_fallback_nodes: descents.iter().filter(|d| d.flat_fallback).count(),
            descents,
            contributing_area_square_meters: area,
            upstream_to_downstream: order,
            closed_fraction_of_active_area: closed / active,
            closed_sink_count: sinks,
            terminal_area_relative_residual: residual,
        })
    }
}
