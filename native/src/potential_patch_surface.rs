//! Minimax escape geometry, NOT an actual water inventory or bed correction.
//! Every perimeter cell retains its own prescribed bed height; no epsilon slope.
use crate::{terrain_patch_geometry::Geometry, terrain_patch_landscape};
use serde::Serialize;
use std::{cmp::Ordering, collections::BinaryHeap};

pub const MODEL_VERSION: &str = "potential-patch-surface-1";

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub model_version: &'static str,
    pub positive_depth_nodes: usize,
    pub positive_depth_area_square_meters: f64,
    pub maximum_potential_depth_meters: f64,
    /// Geometric reference prism only: never funded by rainfall or water stock.
    pub potential_storage_prism_cubic_meters: f64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Potential {
    pub surface_meters: Vec<f64>,
    pub depth_meters: Vec<f64>,
    /// Nonincreasing adjacent paths to the real perimeter, with deterministic
    /// heap ordering on equal levels. Not a measured channel network.
    pub escape_parents: Vec<u32>,
    pub summary: Summary,
}
#[derive(Clone, Copy)]
struct Entry {
    level: f64,
    node: usize,
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.level.total_cmp(&other.level) == Ordering::Equal && self.node == other.node
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .level
            .total_cmp(&self.level)
            .then_with(|| other.node.cmp(&self.node))
    }
}
#[derive(Default)]
struct Sum(f64, f64);
impl Sum {
    fn add(&mut self, value: f64) -> Result<(), String> {
        let adjusted = value - self.1;
        let next = self.0 + adjusted;
        self.1 = (next - self.0) - adjusted;
        self.0 = next;
        if !value.is_finite() || !self.0.is_finite() || !self.1.is_finite() {
            return Err("Potential patch geometry overflowed.".into());
        }
        Ok(())
    }
}

/// W_i = min_paths_to_perimeter max_bed_on_path. This pure bounded construction
/// is independent of climate, physical water stocks, and any rendering detail.
/// The caller supplies geometry from its pinned builder, not arbitrary graphs.
pub fn analyze(g: &Geometry, bed: &[f64]) -> Result<Potential, String> {
    terrain_patch_landscape::validate(g, bed)?;
    let n = bed.len();
    let mut levels = vec![f64::INFINITY; n];
    let mut parents: Vec<_> = (0..n as u32).collect();
    let mut settled = vec![false; n];
    let mut queue = BinaryHeap::new();
    for i in 0..n {
        if g.fixed_external_boundary[i] {
            levels[i] = bed[i];
            queue.push(Entry {
                level: bed[i],
                node: i,
            });
        }
    }
    // Independent minimax Dijkstra derivation. No external source is copied.
    while let Some(Entry { level, node: i }) = queue.pop() {
        if settled[i] {
            continue;
        }
        settled[i] = true;
        for &j in &g.neighbors[g.offsets[i] as usize..g.offsets[i + 1] as usize] {
            let j = j as usize;
            let candidate = level.max(bed[j]);
            if !settled[j] && candidate < levels[j] {
                levels[j] = candidate;
                parents[j] = i as u32;
                queue.push(Entry {
                    level: candidate,
                    node: j,
                });
            }
        }
    }
    if settled.iter().any(|v| !v) {
        return Err("Potential patch has unreachable cells.".into());
    }
    let mut depth = Vec::with_capacity(n);
    let mut area = Sum::default();
    let mut prism = Sum::default();
    let mut positive = 0;
    let mut maximum = 0_f64;
    for i in 0..n {
        let d = levels[i] - bed[i];
        if !d.is_finite() || d < 0. {
            return Err("Unrepresentable potential patch depth.".into());
        }
        if d > 0. {
            let volume = d * g.reference_areas_square_meters[i];
            if volume == 0. {
                return Err("Unrepresentable potential patch prism.".into());
            }
            positive += 1;
            area.add(g.reference_areas_square_meters[i])?;
            prism.add(volume)?;
            maximum = maximum.max(d);
        }
        depth.push(d);
    }
    Ok(Potential {
        surface_meters: levels,
        depth_meters: depth,
        escape_parents: parents,
        summary: Summary {
            model_version: MODEL_VERSION,
            positive_depth_nodes: positive,
            positive_depth_area_square_meters: area.0,
            maximum_potential_depth_meters: maximum,
            potential_storage_prism_cubic_meters: prism.0,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain_patch_geometry::{self, Settings};
    fn grid() -> Geometry {
        terrain_patch_geometry::build(Settings {
            radius_meters: 6371000.,
            center_direction: [1., 0., 0.],
            orientation_radians: 0.,
            cells_x: 7,
            cells_y: 7,
            spacing_meters: 250.,
        })
        .unwrap()
    }
    fn paths(g: &Geometry, bed: &[f64], p: &Potential) {
        for i in 0..bed.len() {
            assert!(p.surface_meters[i] >= bed[i]);
            if g.fixed_external_boundary[i] {
                assert_eq!(p.surface_meters[i], bed[i]);
            }
            let mut j = i;
            for hops in 0..=bed.len() {
                assert!(hops < bed.len());
                let k = p.escape_parents[j] as usize;
                if k == j {
                    assert!(g.fixed_external_boundary[j]);
                    break;
                }
                assert!(
                    g.neighbors[g.offsets[j] as usize..g.offsets[j + 1] as usize]
                        .contains(&(k as u32))
                );
                assert!(p.surface_meters[k] <= p.surface_meters[j]);
                j = k;
            }
        }
    }
    #[test]
    fn nested_bowls_use_real_sills_and_never_modify_bed() {
        let g = grid();
        let mut h = vec![9.; 49];
        h[3] = 4.;
        h[10] = 4.;
        h[17] = 2.;
        h[24] = -3.;
        h[25] = 7.;
        h[26] = 1.;
        let before = h.clone();
        let p = analyze(&g, &h).unwrap();
        assert_eq!(p.surface_meters[24], 4.);
        assert_eq!(p.depth_meters[24], 7.);
        assert_eq!(p.surface_meters[26], 7.);
        assert_eq!(p.summary.positive_depth_nodes, 3);
        let expected = 2. * g.reference_areas_square_meters[17]
            + 7. * g.reference_areas_square_meters[24]
            + 6. * g.reference_areas_square_meters[26];
        assert!((p.summary.potential_storage_prism_cubic_meters - expected).abs() < 1e-8);
        assert_eq!(h, before);
        paths(&g, &h, &p);
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            serde_json::to_value(analyze(&g, &h).unwrap()).unwrap()
        );
    }
    #[test]
    fn flat_and_downhill_planes_are_exact_noops_without_epsilon() {
        let g = grid();
        for h in [vec![123.; 49], (0..49).map(|i| i as f64).collect()] {
            let p = analyze(&g, &h).unwrap();
            assert_eq!(p.surface_meters, h);
            assert_eq!(p.summary.positive_depth_nodes, 0);
            assert_eq!(p.summary.potential_storage_prism_cubic_meters, 0.);
            paths(&g, &h, &p);
        }
    }
    #[test]
    fn independent_threshold_connectivity_oracle_proves_minimality() {
        let g = grid();
        // For each candidate level independently flood only cells below that
        // threshold; the first reachable threshold is the exact bottleneck.
        for seed in 0..16u64 {
            let mut state = seed + 1;
            let h: Vec<_> = (0..49)
                .map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    ((state >> 32) % 13) as f64 - 6.
                })
                .collect();
            let p = analyze(&g, &h).unwrap();
            let mut oracle = vec![f64::INFINITY; 49];
            for level in -6..=6 {
                let level = level as f64;
                let mut seen = [false; 49];
                let mut queue: Vec<_> = (0..49)
                    .filter(|&i| g.fixed_external_boundary[i] && h[i] <= level)
                    .collect();
                for &i in &queue {
                    seen[i] = true;
                }
                let mut cursor = 0;
                while cursor < queue.len() {
                    let i = queue[cursor];
                    cursor += 1;
                    for &j in &g.neighbors[g.offsets[i] as usize..g.offsets[i + 1] as usize] {
                        let j = j as usize;
                        if !seen[j] && h[j] <= level {
                            seen[j] = true;
                            queue.push(j);
                        }
                    }
                }
                for i in 0..49 {
                    if seen[i] && oracle[i].is_infinite() {
                        oracle[i] = level;
                    }
                }
            }
            assert_eq!(p.surface_meters, oracle);
            paths(&g, &h, &p);
        }
    }
    #[test]
    fn malformed_geometry_nonfinite_and_unrepresentable_depth_are_refused() {
        let g = grid();
        assert!(analyze(&g, &[0.; 48]).is_err());
        let mut h = vec![0.; 49];
        h[24] = f64::NAN;
        assert!(analyze(&g, &h).is_err());
        let mut h = vec![f64::MAX; 49];
        h[24] = -f64::MAX;
        let before: Vec<_> = h.iter().map(|v| v.to_bits()).collect();
        assert!(analyze(&g, &h).is_err());
        assert_eq!(before, h.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
        let mut bad = g.clone();
        bad.neighbors[0] = u32::MAX;
        assert!(analyze(&bad, &[0.; 49]).is_err());
        let mut bad = g.clone();
        bad.fixed_external_boundary[24] = true;
        assert!(analyze(&bad, &[0.; 49]).is_err());
        let mut bad = g.clone();
        bad.reference_areas_square_meters[24] = f64::MAX;
        let mut bowl = vec![10.; 49];
        bowl[24] = -5.;
        assert!(analyze(&bad, &bowl).is_err());
    }
    #[test]
    fn vertical_datum_shift_preserves_potential_depth_and_paths() {
        let g = grid();
        let mut bed = vec![10.; 49];
        bed[24] = -5.;
        let shifted: Vec<_> = bed.iter().map(|v| v + 1024.).collect();
        let a = analyze(&g, &bed).unwrap();
        let b = analyze(&g, &shifted).unwrap();
        assert_eq!(a.depth_meters, b.depth_meters);
        assert_eq!(a.escape_parents, b.escape_parents);
        assert_eq!(
            a.summary.potential_storage_prism_cubic_meters,
            b.summary.potential_storage_prism_cubic_meters
        );
        for (x, y) in a.surface_meters.iter().zip(b.surface_meters) {
            assert_eq!(x + 1024., y);
        }
    }
}
