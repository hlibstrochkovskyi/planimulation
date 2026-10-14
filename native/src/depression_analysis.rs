//! Read-only depression geometry. Escape surfaces are NOT filled beds, new
//! water inventories or hydraulic routing. The sphere has no implicit edge.
use crate::{Surface, basins::Basins, drainage::Drainage, geography_metrics, water::Water};
use serde::Serialize;
use std::{cmp::Ordering, collections::BinaryHeap};

pub const ANALYSIS_VERSION: &str = "depression-geometry-1";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeafDepression {
    pub basin_node: usize,
    pub minimum_region: u32,
    pub minimum_bed_meters: f64,
    /// First sibling connection, not necessarily escape to water or the ocean.
    pub first_merge_depth_meters: Option<f64>,
    pub first_merge_capacity_cubic_meters: Option<f64>,
    /// Includes slopes draining to this minimum, not just its storage footprint.
    pub static_dry_catchment_area_square_meters: f64,
    pub escape_depth_to_initial_water_meters: Option<f64>,
    pub escape_depth_to_main_ocean_meters: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepthBand {
    /// Strictly greater than this depth and <= upper, with no epsilon cutoff.
    pub lower_exclusive_meters: f64,
    pub upper_inclusive_meters: Option<f64>,
    pub dry_region_count: usize,
    pub dry_area_square_meters: f64,
    pub additional_geometric_volume_cubic_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EscapeComponent {
    pub first_region: u32,
    pub region_count: usize,
    pub escape_level_meters: f64,
    pub dry_area_square_meters: f64,
    pub initial_wet_area_square_meters: f64,
    /// Sum of unique cell prisms ABOVE the existing initial surface.
    pub additional_geometric_volume_cubic_meters: f64,
    pub maximum_additional_depth_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EscapeAnalysis {
    pub outlet_region_count: usize,
    pub dry_area_square_meters: f64,
    pub positive_depth_dry_area_square_meters: f64,
    pub positive_depth_fraction_of_dry_land: f64,
    pub zero_depth_dry_region_count: usize,
    pub additional_geometric_volume_cubic_meters: f64,
    pub maximum_additional_depth_meters: f64,
    pub depth_bands: Vec<DepthBand>,
    /// Disjoint positive-depth components, sorted by volume then region ID.
    /// These are not every nested branch of the basin hierarchy.
    pub components: Vec<EscapeComponent>,
    #[serde(skip)]
    pub escape_levels_meters: Vec<f64>,
    /// Acyclic witness paths on the potential surface, NOT physical receivers.
    #[serde(skip)]
    pub witness_receivers: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analysis {
    pub analysis_version: &'static str,
    pub dry_minimum_leaves: Vec<LeafDepression>,
    /// None means there is no explicit wet target, not zero depression depth.
    pub to_initial_water: Option<EscapeAnalysis>,
    pub to_main_ocean: Option<EscapeAnalysis>,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    level: f64,
    region: usize,
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.level.total_cmp(&other.level) == Ordering::Equal && self.region == other.region
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
        // Reverse for a min queue. Region ID provides a deterministic flat tie.
        other
            .level
            .total_cmp(&self.level)
            .then(other.region.cmp(&self.region))
    }
}

/// Analyze an immutable, validated initial world with O(N+E) working storage
/// and O((N+E) log N) time. A minimax Dijkstra sweep is independently derived
/// from the Priority-Flood principle; no external implementation is copied.
pub fn analyze(s: &Surface, h: &[f64], water: &Water) -> Result<Analysis, String> {
    geography_metrics::validate(s, h, water)?;
    // Also verifies connectivity, even if there are no explicit wet outlets.
    let basins = Basins::build(s, h)?;
    let drainage = Drainage::build(s, h, &water.body_ids);
    let any = escape(s, h, water, |i| water.body_ids[i] != 0)?;
    let ocean = escape(s, h, water, |i| {
        water.main_ocean_id != 0 && water.body_ids[i] == water.main_ocean_id
    })?;
    let mut minima = vec![None; basins.nodes().len()];
    for (i, &node) in basins.region_nodes().iter().enumerate() {
        let branch = &basins.nodes()[node];
        if branch.children.is_empty() && h[i] == branch.birth_level_meters {
            minima[node] = Some(minima[node].map_or(i, |old: usize| old.min(i)));
        }
    }
    let mut catchment_areas = vec![0.; h.len()];
    for (i, &terminal) in drainage.outlets.iter().enumerate() {
        if water.body_ids[i] == 0 && water.body_ids[terminal as usize] == 0 {
            add(&mut catchment_areas[terminal as usize], s.areas[i])?;
        }
    }
    let mut leaves = Vec::new();
    for (node, minimum) in minima.into_iter().enumerate() {
        let Some(i) = minimum else { continue };
        if water.body_ids[i] != 0 {
            continue;
        }
        let branch = &basins.nodes()[node];
        let depth = branch.spill_level_meters.map(|level| level - h[i]);
        if depth.is_some_and(|v| !v.is_finite() || v < 0.) {
            return Err("Depression first-merge depth overflowed.".into());
        }
        leaves.push(LeafDepression {
            basin_node: node,
            minimum_region: i as u32,
            minimum_bed_meters: h[i],
            first_merge_depth_meters: depth,
            first_merge_capacity_cubic_meters: branch.capacity_cubic_meters,
            static_dry_catchment_area_square_meters: catchment_areas[i],
            escape_depth_to_initial_water_meters: any
                .as_ref()
                .map(|a| a.escape_levels_meters[i] - h[i]),
            escape_depth_to_main_ocean_meters: ocean
                .as_ref()
                .map(|a| a.escape_levels_meters[i] - h[i]),
        });
    }
    Ok(Analysis {
        analysis_version: ANALYSIS_VERSION,
        dry_minimum_leaves: leaves,
        to_initial_water: any,
        to_main_ocean: ocean,
    })
}

fn add(total: &mut f64, increment: f64) -> Result<(), String> {
    *total += increment;
    if !increment.is_finite() || increment < 0. || !total.is_finite() {
        return Err("Depression geometric statistic overflowed.".into());
    }
    Ok(())
}

fn escape(
    s: &Surface,
    h: &[f64],
    water: &Water,
    target: impl Fn(usize) -> bool,
) -> Result<Option<EscapeAnalysis>, String> {
    let n = h.len();
    let mut levels = vec![f64::INFINITY; n];
    let mut receivers: Vec<u32> = (0..n as u32).collect();
    let mut done = vec![false; n];
    let mut queue = BinaryHeap::new();
    let mut outlets = 0;
    for (i, level) in levels.iter_mut().enumerate() {
        if target(i) {
            *level = water.level_meters;
            queue.push(Entry {
                level: *level,
                region: i,
            });
            outlets += 1;
        }
    }
    if outlets == 0 {
        return Ok(None);
    }
    while let Some(Entry { level, region: i }) = queue.pop() {
        if done[i] {
            continue;
        }
        done[i] = true;
        for &j in &s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize] {
            let j = j as usize;
            // Existing inland water is not dry empty storage or an underwater
            // shortcut: all initial wet columns start at the prescribed level.
            let candidate = level.max(h[j].max(water.level_meters));
            if !done[j] && candidate < levels[j] {
                levels[j] = candidate;
                receivers[j] = i as u32;
                queue.push(Entry {
                    level: candidate,
                    region: j,
                });
            }
        }
    }
    if done.iter().any(|v| !v) {
        return Err("Depression escape targets cannot reach the whole surface.".into());
    }
    let mut depths = Vec::with_capacity(n);
    let mut dry_area = 0.;
    let mut positive_area = 0.;
    let mut zero_count = 0;
    let mut total_volume = 0.;
    let mut maximum_depth: f64 = 0.;
    let upper = [Some(1.), Some(10.), Some(50.), Some(100.), Some(500.), None];
    let lower = [0., 1., 10., 50., 100., 500.];
    let mut bands: Vec<_> = (0..upper.len())
        .map(|i| DepthBand {
            lower_exclusive_meters: lower[i],
            upper_inclusive_meters: upper[i],
            dry_region_count: 0,
            dry_area_square_meters: 0.,
            additional_geometric_volume_cubic_meters: 0.,
        })
        .collect();
    for i in 0..n {
        let depth = levels[i] - h[i].max(water.level_meters);
        let volume = depth * s.areas[i];
        add(&mut total_volume, volume)?;
        maximum_depth = maximum_depth.max(depth);
        depths.push(depth);
        if water.body_ids[i] == 0 {
            add(&mut dry_area, s.areas[i])?;
            if depth == 0. {
                zero_count += 1;
            } else {
                add(&mut positive_area, s.areas[i])?;
                let b = bands
                    .iter_mut()
                    .find(|b| b.upper_inclusive_meters.is_none_or(|u| depth <= u))
                    .unwrap();
                b.dry_region_count += 1;
                add(&mut b.dry_area_square_meters, s.areas[i])?;
                add(&mut b.additional_geometric_volume_cubic_meters, volume)?;
            }
        }
    }
    let mut seen = vec![false; n];
    let mut pending = Vec::new();
    let mut components = Vec::new();
    for start in 0..n {
        if seen[start] || depths[start] == 0. {
            continue;
        }
        seen[start] = true;
        pending.clear();
        pending.push(start);
        let mut component = EscapeComponent {
            first_region: start as u32,
            region_count: 0,
            escape_level_meters: levels[start],
            dry_area_square_meters: 0.,
            initial_wet_area_square_meters: 0.,
            additional_geometric_volume_cubic_meters: 0.,
            maximum_additional_depth_meters: 0.,
        };
        let mut cursor = 0;
        while cursor < pending.len() {
            let i = pending[cursor];
            cursor += 1;
            component.region_count += 1;
            let area = if water.body_ids[i] == 0 {
                &mut component.dry_area_square_meters
            } else {
                &mut component.initial_wet_area_square_meters
            };
            add(area, s.areas[i])?;
            add(
                &mut component.additional_geometric_volume_cubic_meters,
                depths[i] * s.areas[i],
            )?;
            component.maximum_additional_depth_meters =
                component.maximum_additional_depth_meters.max(depths[i]);
            for &j in &s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize] {
                let j = j as usize;
                if !seen[j] && depths[j] > 0. {
                    if levels[j] != component.escape_level_meters {
                        return Err(
                            "Adjacent positive-depth cells have inconsistent minimax levels."
                                .into(),
                        );
                    }
                    seen[j] = true;
                    pending.push(j);
                }
            }
        }
        components.push(component);
    }
    components.sort_by(|a, b| {
        b.additional_geometric_volume_cubic_meters
            .total_cmp(&a.additional_geometric_volume_cubic_meters)
            .then(a.first_region.cmp(&b.first_region))
    });
    Ok(Some(EscapeAnalysis {
        outlet_region_count: outlets,
        dry_area_square_meters: dry_area,
        positive_depth_dry_area_square_meters: positive_area,
        positive_depth_fraction_of_dry_land: if dry_area > 0. {
            positive_area / dry_area
        } else {
            0.
        },
        zero_depth_dry_region_count: zero_count,
        additional_geometric_volume_cubic_meters: total_volume,
        maximum_additional_depth_meters: maximum_depth,
        depth_bands: bands,
        components,
        escape_levels_meters: levels,
        witness_receivers: receivers,
    }))
}
