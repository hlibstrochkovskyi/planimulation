//! Read-only diagnostics of sampled physical geography, not realism scores.
//! All areas and distances are reference-sphere values. Coast crossings are
//! adjacency counts, not measured coastline length. Neighbor-scale statistics
//! change with resolution; they must not be compared as fixed-scale roughness.
use crate::{Surface, drainage::Drainage, water::Water};
use serde::Serialize;

pub const MODEL_VERSION: &str = "geography-metrics-1";
const MAX_NODES: usize = 1_000_000;
const MAX_DEGREE: usize = 32;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hypsometry {
    /// Inverse area-weighted empirical CDF, without interpolation between beds.
    pub quantile_probabilities: [f64; 5],
    pub quantile_height_above_water_meters: [f64; 5],
    pub minimum_height_above_water_meters: f64,
    pub maximum_height_above_water_meters: f64,
    pub mean_height_above_water_meters: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Landmass {
    /// Canonical minimum region ID; not an identity across resolution changes.
    pub first_region: u32,
    pub region_count: usize,
    pub reference_area_square_meters: f64,
    pub fraction_of_dry_land: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NeighborScale {
    pub dry_dry_adjacency_pairs: usize,
    pub maximum_absolute_bed_slope: f64,
    /// Each dry region averages its dry-neighbor squared slopes, then region
    /// means are weighted by physical area. Isolated dry cells are excluded.
    pub area_weighted_rms_bed_slope: Option<f64>,
    pub area_weighted_rms_height_difference_meters: Option<f64>,
    pub area_weighted_mean_neighbor_distance_meters: Option<f64>,
    pub represented_dry_area_square_meters: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticDrainage {
    pub closed_dry_sink_count: usize,
    pub terminal_catchments_with_dry_land: usize,
    pub closed_drainage_area_square_meters: f64,
    pub closed_drainage_fraction_of_dry_land: f64,
    pub largest_terminal_catchment_area_square_meters: f64,
    pub maximum_contributing_dry_area_square_meters: f64,
    /// Receiver graph hops, not channel length, travel time, or river order.
    pub maximum_dry_receiver_hops: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub model_version: &'static str,
    pub region_count: usize,
    pub reference_area_square_meters: f64,
    pub dry_area_square_meters: f64,
    pub land_coverage_fraction: f64,
    pub dry_hypsometry: Option<Hypsometry>,
    /// Sorted by descending physical area; ties prefer minimum region ID.
    pub landmasses: Vec<Landmass>,
    pub largest_landmass_fraction_of_dry_land: f64,
    pub coastline_adjacency_crossings: usize,
    pub neighbor_scale: NeighborScale,
    /// Existing bed-based single-receiver analysis, not active river dynamics.
    pub static_drainage: StaticDrainage,
}

/// Analyze validated immutable fields. Auxiliary storage is linear; hypsometry
/// sorting is O(N log N). This diagnostic bound does not extend desktop support.
pub fn analyze(surface: &Surface, heights: &[f64], water: &Water) -> Result<Metrics, String> {
    validate(surface, heights, water)?;
    let n = heights.len();
    let total_area = sum(surface.areas.iter().copied())?;
    let dry_area = sum((0..n)
        .filter(|&i| water.body_ids[i] == 0)
        .map(|i| surface.areas[i]))?;
    let mut dry_order: Vec<_> = (0..n).filter(|&i| water.body_ids[i] == 0).collect();
    dry_order.sort_unstable_by(|&a, &b| heights[a].total_cmp(&heights[b]).then(a.cmp(&b)));
    let dry_hypsometry = if dry_order.is_empty() {
        None
    } else {
        let probabilities = [0.05, 0.25, 0.5, 0.75, 0.95];
        let mut quantiles = [0.; 5];
        let mut area = 0.;
        let mut index = 0;
        let mean = sum(dry_order
            .iter()
            .map(|&i| (heights[i] - water.level_meters) * (surface.areas[i] / dry_area)))?;
        for &i in &dry_order {
            area += surface.areas[i];
            while index < probabilities.len()
                && (area >= dry_area * probabilities[index] || i == *dry_order.last().unwrap())
            {
                quantiles[index] = heights[i] - water.level_meters;
                index += 1;
            }
        }
        Some(Hypsometry {
            quantile_probabilities: probabilities,
            quantile_height_above_water_meters: quantiles,
            minimum_height_above_water_meters: heights[dry_order[0]] - water.level_meters,
            maximum_height_above_water_meters: heights[*dry_order.last().unwrap()]
                - water.level_meters,
            mean_height_above_water_meters: mean,
        })
    };

    let mut visited = vec![false; n];
    let mut queue = Vec::new();
    let mut landmasses = Vec::new();
    for start in 0..n {
        if visited[start] || water.body_ids[start] != 0 {
            continue;
        }
        queue.clear();
        queue.push(start);
        visited[start] = true;
        let mut cursor = 0;
        while cursor < queue.len() {
            let i = queue[cursor];
            cursor += 1;
            for k in surface.offsets[i]..surface.offsets[i + 1] {
                let j = surface.neighbors[k as usize] as usize;
                if !visited[j] && water.body_ids[j] == 0 {
                    visited[j] = true;
                    queue.push(j);
                }
            }
        }
        let area = sum(queue.iter().map(|&i| surface.areas[i]))?;
        landmasses.push(Landmass {
            first_region: start as u32,
            region_count: queue.len(),
            reference_area_square_meters: area,
            fraction_of_dry_land: area / dry_area,
        });
    }
    landmasses.sort_unstable_by(|a, b| {
        b.reference_area_square_meters
            .total_cmp(&a.reference_area_square_meters)
            .then(a.first_region.cmp(&b.first_region))
    });

    let mut crossings = 0;
    let mut pairs = 0;
    let mut max_slope: f64 = 0.;
    let mut local_statistics = Vec::new();
    for i in 0..n {
        // Bounded stack scratch avoids three heap allocations per region.
        let mut slopes = [0.; MAX_DEGREE];
        let mut differences = [0.; MAX_DEGREE];
        let mut distances = [0.; MAX_DEGREE];
        let mut count = 0;
        for k in surface.offsets[i]..surface.offsets[i + 1] {
            let k = k as usize;
            let j = surface.neighbors[k] as usize;
            if i < j && (water.body_ids[i] == 0) != (water.body_ids[j] == 0) {
                crossings += 1;
            }
            if water.body_ids[i] != 0 || water.body_ids[j] != 0 {
                continue;
            }
            if i < j {
                pairs += 1;
            }
            let difference = (heights[i] - heights[j]).abs();
            let slope = difference / surface.distances[k];
            max_slope = max_slope.max(slope);
            slopes[count] = slope * slope;
            differences[count] = difference * difference;
            distances[count] = surface.distances[k];
            count += 1;
        }
        if count > 0 {
            local_statistics.push((
                surface.areas[i],
                sum(slopes[..count].iter().copied())? / count as f64,
                sum(differences[..count].iter().copied())? / count as f64,
                sum(distances[..count].iter().copied())? / count as f64,
            ));
        }
    }
    let represented_area = sum(local_statistics.iter().map(|v| v.0))?;
    let averaged = if represented_area == 0. {
        None
    } else {
        Some([
            sum(local_statistics
                .iter()
                .map(|v| (v.0 / represented_area) * v.1))?
            .sqrt(),
            sum(local_statistics
                .iter()
                .map(|v| (v.0 / represented_area) * v.2))?
            .sqrt(),
            sum(local_statistics
                .iter()
                .map(|v| (v.0 / represented_area) * v.3))?,
        ])
    };
    let drainage = Drainage::build(surface, heights, &water.body_ids);
    let mut catchment_area = vec![0.; n];
    let mut closed_area_terms = Vec::new();
    let mut max_contributing: f64 = 0.;
    let mut closed_sinks = 0;
    for i in 0..n {
        max_contributing = max_contributing.max(drainage.contributing_area[i]);
        if water.body_ids[i] != 0 {
            continue;
        }
        let outlet = drainage.outlets[i] as usize;
        catchment_area[outlet] += surface.areas[i];
        if water.body_ids[outlet] == 0 {
            closed_area_terms.push(surface.areas[i]);
        }
        if drainage.receivers[i] == i as u32 {
            closed_sinks += 1;
        }
    }
    let closed_area = sum(closed_area_terms)?;
    // Topological order avoids recursive or quadratic downstream path walks.
    let mut incoming = vec![0_usize; n];
    for (i, &receiver) in drainage.receivers.iter().enumerate() {
        if receiver as usize != i {
            incoming[receiver as usize] += 1;
        }
    }
    queue.clear();
    queue.extend((0..n).filter(|&i| incoming[i] == 0));
    let mut cursor = 0;
    let mut hops = vec![0_usize; n];
    while cursor < queue.len() {
        let i = queue[cursor];
        cursor += 1;
        let receiver = drainage.receivers[i] as usize;
        if receiver != i {
            hops[receiver] = hops[receiver].max(hops[i] + 1);
            incoming[receiver] -= 1;
            if incoming[receiver] == 0 {
                queue.push(receiver);
            }
        }
    }
    if catchment_area
        .iter()
        .chain(&drainage.contributing_area)
        .any(|v| !v.is_finite())
    {
        return Err("Geographic drainage area overflowed.".into());
    }
    Ok(Metrics {
        model_version: MODEL_VERSION,
        region_count: n,
        reference_area_square_meters: total_area,
        dry_area_square_meters: dry_area,
        land_coverage_fraction: dry_area / total_area,
        dry_hypsometry,
        largest_landmass_fraction_of_dry_land: landmasses
            .first()
            .map_or(0., |v| v.fraction_of_dry_land),
        landmasses,
        coastline_adjacency_crossings: crossings,
        neighbor_scale: NeighborScale {
            dry_dry_adjacency_pairs: pairs,
            maximum_absolute_bed_slope: max_slope,
            area_weighted_rms_bed_slope: averaged.map(|v| v[0]),
            area_weighted_rms_height_difference_meters: averaged.map(|v| v[1]),
            area_weighted_mean_neighbor_distance_meters: averaged.map(|v| v[2]),
            represented_dry_area_square_meters: represented_area,
        },
        static_drainage: StaticDrainage {
            closed_dry_sink_count: closed_sinks,
            terminal_catchments_with_dry_land: catchment_area.iter().filter(|&&a| a > 0.).count(),
            closed_drainage_area_square_meters: closed_area,
            closed_drainage_fraction_of_dry_land: if dry_area == 0. {
                0.
            } else {
                closed_area / dry_area
            },
            largest_terminal_catchment_area_square_meters: catchment_area
                .iter()
                .copied()
                .fold(0., f64::max),
            maximum_contributing_dry_area_square_meters: max_contributing,
            maximum_dry_receiver_hops: hops.into_iter().max().unwrap_or(0),
        },
    })
}

fn sum(values: impl IntoIterator<Item = f64>) -> Result<f64, String> {
    let mut total = 0.;
    let mut correction = 0.;
    for value in values {
        if !value.is_finite() || value < 0. {
            return Err("Geographic statistic is nonfinite or negative.".into());
        }
        let adjusted = value - correction;
        let next = total + adjusted;
        correction = (next - total) - adjusted;
        total = next;
        if !total.is_finite() {
            return Err("Geographic statistic overflowed.".into());
        }
    }
    Ok(total)
}

fn validate(s: &Surface, h: &[f64], water: &Water) -> Result<(), String> {
    let n = h.len();
    if n == 0
        || n > MAX_NODES
        || s.areas.len() != n
        || s.offsets.len() != n + 1
        || water.depth_meters.len() != n
        || water.body_ids.len() != n
        || s.neighbors.len() != s.distances.len()
        || s.neighbors.len() > n * MAX_DEGREE
        || s.offsets[0] != 0
        || s.offsets[n] as usize != s.neighbors.len()
        || !water.level_meters.is_finite()
        || !water.resolved_volume_cubic_meters.is_finite()
        || water.resolved_volume_cubic_meters < 0.
    {
        return Err("Invalid geographic field shapes or water metadata.".into());
    }
    for i in 0..n {
        let start = s.offsets[i] as usize;
        let end = s.offsets[i + 1] as usize;
        if end < start || end > s.neighbors.len() || end - start > MAX_DEGREE {
            return Err("Invalid geographic adjacency offsets.".into());
        }
        if !h[i].is_finite()
            || !s.areas[i].is_finite()
            || s.areas[i] <= 0.
            || !water.depth_meters[i].is_finite()
            || water.depth_meters[i] < 0.
            || water.body_ids[i] as usize > n
        {
            return Err("Invalid geographic height, area or water field.".into());
        }
        let relative = h[i] - water.level_meters;
        let expected_depth = (-relative).max(0.);
        if !relative.is_finite()
            || (expected_depth - water.depth_meters[i]).abs() > 1e-8_f64.max(expected_depth * 1e-12)
            || (water.body_ids[i] == 0) != (expected_depth == 0.)
        {
            return Err("Geographic water classification disagrees with the physical bed.".into());
        }
        for k in start..end {
            let j = s.neighbors[k] as usize;
            if j >= n
                || j == i
                || !s.distances[k].is_finite()
                || s.distances[k] <= 0.
                || s.neighbors[start..k].contains(&(j as u32))
                || !(h[i] - h[j]).is_finite()
                || !((h[i] - h[j]) / s.distances[k]).is_finite()
            {
                return Err("Invalid geographic adjacency or physical distance.".into());
            }
        }
    }
    for i in 0..n {
        for k in s.offsets[i]..s.offsets[i + 1] {
            let j = s.neighbors[k as usize] as usize;
            let reverse =
                (s.offsets[j]..s.offsets[j + 1]).find(|&r| s.neighbors[r as usize] == i as u32);
            if reverse.is_none_or(|r| s.distances[r as usize] != s.distances[k as usize]) {
                return Err(
                    "Geographic adjacency must be symmetric with matching distances.".into(),
                );
            }
        }
    }
    // Do not let inconsistent body labels silently merge unrelated catchments.
    let mut visited = vec![false; n];
    let mut observed = vec![false; n + 1];
    let mut queue = Vec::new();
    for start in 0..n {
        let body = water.body_ids[start] as usize;
        if body == 0 || visited[start] {
            continue;
        }
        if observed[body] {
            return Err("A geographic water body label is disconnected.".into());
        }
        observed[body] = true;
        visited[start] = true;
        queue.clear();
        queue.push(start);
        let mut cursor = 0;
        while cursor < queue.len() {
            let i = queue[cursor];
            cursor += 1;
            for k in s.offsets[i]..s.offsets[i + 1] {
                let j = s.neighbors[k as usize] as usize;
                if water.body_ids[j] == 0 {
                    continue;
                }
                if water.body_ids[j] as usize != body {
                    return Err("Adjacent wet regions have different body labels.".into());
                }
                if !visited[j] {
                    visited[j] = true;
                    queue.push(j);
                }
            }
        }
    }
    if water.main_ocean_id as usize > n
        || (water.main_ocean_id == 0) != !observed.iter().any(|&v| v)
        || (water.main_ocean_id > 0 && !observed[water.main_ocean_id as usize])
    {
        return Err("Invalid geographic main-ocean label.".into());
    }
    let represented_volume = sum((0..n).map(|i| water.depth_meters[i] * s.areas[i]))?;
    if (represented_volume - water.resolved_volume_cubic_meters).abs()
        > 1e-9_f64.max(represented_volume * 1e-10)
    {
        return Err("Geographic water volume disagrees with its depths.".into());
    }
    Ok(())
}
