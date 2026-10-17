//! Generation-only channel profiles and lower graph-slope roofs. This is not
//! a time-integrated erosion/landslide law or a deposition model.
use crate::{terrain_patch_geometry::Geometry, terrain_patch_landscape};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BinaryHeap};

pub const MODEL_VERSION: &str = "drainage-landform-construction-1";
pub const MAX_NODES: usize = 262_144;
/// Total edge-count bound divided by node count, not a per-node degree cap.
pub const MAX_EDGES_PER_NODE: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Edge {
    pub a: u32,
    pub b: u32,
    pub distance_meters: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub channel_head_area_square_meters: f64,
    /// Dimensionless longitudinal grade at the head-area threshold.
    pub channel_head_slope: f64,
    pub channel_concavity: f64,
    /// Bound on neighboring samples, NOT a continuous triangle-gradient bound.
    pub maximum_graph_slope: f64,
}
impl Settings {
    pub fn validate(self) -> Result<(), String> {
        if !self.channel_head_area_square_meters.is_finite()
            || self.channel_head_area_square_meters <= 0.
            || !self.channel_head_slope.is_finite()
            || self.channel_head_slope <= 0.
            || self.channel_head_slope > self.maximum_graph_slope
            || !self.channel_concavity.is_finite()
            || !(0. ..=1.).contains(&self.channel_concavity)
            || !self.maximum_graph_slope.is_finite()
            || !(0. ..=2.).contains(&self.maximum_graph_slope)
            || self.maximum_graph_slope == 0.
        {
            return Err("Invalid drainage-landform construction settings.".into());
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Construction {
    pub model_version: &'static str,
    pub settings: Settings,
    pub elevation_meters: Vec<f64>,
    pub channel_profile_constraint_meters: Vec<f64>,
    pub structural_contributing_area_square_meters: Vec<f64>,
    pub structural_channel: Vec<bool>,
    /// Source upper constraint controlling each roof sample, for inspection.
    /// This is not the final physical drainage receiver.
    pub limiting_constraint_node: Vec<u32>,
    pub removed_thickness_meters: Vec<f64>,
    pub channel_profile_detached_cubic_meters: f64,
    pub roof_detached_cubic_meters: f64,
    /// Accepted removed reference prisms in a detached bookkeeping reservoir.
    /// Neither routed outlet export nor deposition/soil stock is represented.
    pub detached_material_cubic_meters: f64,
    pub relative_material_residual: f64,
    pub structural_terminal_area_relative_residual: f64,
    pub maximum_accepted_graph_slope: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    pub construction: Construction,
    /// Rebuilt on accepted heights, not the retained structural receiver graph.
    pub actual_dry_routing: terrain_patch_landscape::Routing,
}

#[derive(Default, Clone, Copy)]
struct Sum(f64, f64);
impl Sum {
    fn add(&mut self, value: f64) -> Result<(), String> {
        let next = self.0 + value;
        self.1 += if self.0.abs() >= value.abs() {
            (self.0 - next) + value
        } else {
            (value - next) + self.0
        };
        self.0 = next;
        if !value.is_finite() || !self.0.is_finite() || !self.1.is_finite() {
            return Err("Drainage-landform accumulation overflowed.".into());
        }
        Ok(())
    }
    fn value(self) -> Result<f64, String> {
        let value = self.0 + self.1;
        value
            .is_finite()
            .then_some(value)
            .ok_or_else(|| "Unrepresentable drainage-landform sum.".into())
    }
}

#[derive(Clone, Copy)]
struct Entry(f64, usize);
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0) == Ordering::Equal && self.1 == other.1
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
            .0
            .total_cmp(&self.0)
            .then_with(|| other.1.cmp(&self.1))
    }
}

fn relative(residual: f64, scale: f64) -> Result<f64, String> {
    let value = if scale == 0. {
        if residual == 0. { 0. } else { f64::INFINITY }
    } else {
        residual.abs() / scale
    };
    if !value.is_finite() || value > 1e-12 {
        return Err("Drainage-landform material or source area does not reconcile.".into());
    }
    Ok(value)
}

type Adjacency = Vec<Vec<(usize, f64)>>;
fn validated_adjacency(
    bed: &[f64],
    areas: &[f64],
    fixed: &[bool],
    edges: &[Edge],
) -> Result<Adjacency, String> {
    let n = bed.len();
    if !(1..=MAX_NODES).contains(&n)
        || areas.len() != n
        || fixed.len() != n
        || edges.len() > MAX_EDGES_PER_NODE * n
        || bed.iter().any(|v| !v.is_finite())
        || areas.iter().any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err("Invalid drainage-landform fields or workload.".into());
    }
    let mut adjacency = vec![Vec::<(usize, f64)>::new(); n];
    for e in edges {
        if e.a >= e.b
            || e.b as usize >= n
            || !e.distance_meters.is_finite()
            || e.distance_meters <= 0.
        {
            return Err("Invalid drainage-landform edge.".into());
        }
        adjacency[e.a as usize].push((e.b as usize, e.distance_meters));
        adjacency[e.b as usize].push((e.a as usize, e.distance_meters));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|v| v.0);
        if neighbors.windows(2).any(|v| v[0].0 == v[1].0) {
            return Err("Duplicate drainage-landform edge.".into());
        }
    }
    Ok(adjacency)
}

/// Strict adjacent steepest descent on the actual supplied bed. Ties use the
/// smallest receiver ID. Fixed nodes and all flats/minima remain terminals;
/// this deliberately does not install an epsilon slope or a lake connection.
pub fn dry_receivers(
    bed: &[f64],
    areas: &[f64],
    fixed: &[bool],
    edges: &[Edge],
) -> Result<Vec<u32>, String> {
    let adjacency = validated_adjacency(bed, areas, fixed, edges)?;
    let mut receivers: Vec<_> = (0..bed.len() as u32).collect();
    for i in 0..bed.len() {
        if fixed[i] {
            continue;
        }
        let mut steepest = 0.;
        for &(j, length) in &adjacency[i] {
            let slope = (bed[i] - bed[j]) / length;
            if !slope.is_finite() {
                return Err("Unrepresentable actual dry graph slope.".into());
            }
            if slope > steepest || slope > 0. && slope == steepest && j < receivers[i] as usize {
                steepest = slope;
                receivers[i] = j as u32;
            }
        }
    }
    Ok(receivers)
}

/// The caller supplies physical reference areas, unique canonical undirected
/// edges and an adjacent, nonascending, acyclic structural receiver graph.
/// Fixed nodes must be terminals. This validates those numerical properties,
/// not the caller's geometric/bed provenance. No input is mutated on failure.
///
/// Selected channels obey S=S_head*(C/C_head)^(-concavity), integrated from
/// existing terminal heights upstream. Constraints only lower the input bed.
/// A multi-source shortest-path envelope then bounds all graph-edge slopes.
/// It may reroute actual drainage; callers MUST rebuild it on accepted heights.
pub fn construct(
    bed: &[f64],
    areas: &[f64],
    fixed: &[bool],
    edges: &[Edge],
    receivers: &[u32],
    settings: Settings,
) -> Result<Construction, String> {
    settings.validate()?;
    if receivers.len() != bed.len() {
        return Err("Invalid structural receiver field shape.".into());
    }
    let adjacency = validated_adjacency(bed, areas, fixed, edges)?;
    let n = bed.len();
    let mut incoming = vec![0_u32; n];
    let mut receiver_lengths = vec![0.; n];
    let mut collected = vec![Sum::default(); n];
    let mut active_area = Sum::default();
    for i in 0..n {
        let r = receivers[i] as usize;
        if r >= n || fixed[i] && r != i || r != i && bed[r] > bed[i] {
            return Err("Invalid structural drainage-landform receiver.".into());
        }
        if !fixed[i] {
            collected[i].add(areas[i])?;
            active_area.add(areas[i])?;
        }
        if r != i {
            let k = adjacency[i]
                .binary_search_by_key(&r, |v| v.0)
                .map_err(|_| "Nonadjacent structural drainage-landform receiver.")?;
            receiver_lengths[i] = adjacency[i][k].1;
            incoming[r] += 1;
        }
    }
    let mut order: Vec<_> = (0..n).filter(|&i| incoming[i] == 0).collect();
    let mut cursor = 0;
    while cursor < order.len() {
        let i = order[cursor];
        cursor += 1;
        let r = receivers[i] as usize;
        if r != i {
            let c = collected[i].value()?;
            collected[r].add(c)?;
            incoming[r] -= 1;
            if incoming[r] == 0 {
                order.push(r);
            }
        }
    }
    if order.len() != n {
        return Err("Cyclic structural drainage-landform graph.".into());
    }
    let contributing: Vec<_> = collected
        .iter()
        .map(|s| s.value())
        .collect::<Result<_, _>>()?;
    let mut terminal_area = Sum::default();
    for i in 0..n {
        if receivers[i] as usize == i {
            terminal_area.add(contributing[i])?;
        }
    }
    let active_area = active_area.value()?;
    let area_residual = relative(terminal_area.value()? - active_area, active_area)?;
    let channel: Vec<_> = contributing
        .iter()
        .map(|&a| a >= settings.channel_head_area_square_meters)
        .collect();
    let mut profile = bed.to_vec();
    for &i in order.iter().rev() {
        let r = receivers[i] as usize;
        if r == i || !channel[i] {
            continue;
        }
        if !channel[r] {
            return Err("Structural channel threshold is not downstream closed.".into());
        }
        let ratio = contributing[i] / settings.channel_head_area_square_meters;
        let slope = settings.channel_head_slope * ratio.powf(-settings.channel_concavity);
        let offset = slope * receiver_lengths[i];
        let candidate = profile[r] + offset;
        if !ratio.is_finite()
            || !slope.is_finite()
            || !offset.is_finite()
            || offset <= 0.
            || !candidate.is_finite()
            || candidate <= profile[r]
        {
            return Err("Unrepresentable positive channel-profile offset.".into());
        }
        if candidate < profile[i] {
            profile[i] = candidate;
        }
    }
    let mut height = profile.clone();
    let mut source: Vec<_> = (0..n as u32).collect();
    let mut queue: BinaryHeap<_> = profile
        .iter()
        .enumerate()
        .map(|(i, &h)| Entry(h, i))
        .collect();
    while let Some(Entry(value, i)) = queue.pop() {
        if value != height[i] {
            continue;
        }
        for &(j, length) in &adjacency[i] {
            let offset = settings.maximum_graph_slope * length;
            let candidate = value + offset;
            if !offset.is_finite() || offset <= 0. || !candidate.is_finite() || candidate <= value {
                return Err("Unrepresentable positive roof offset.".into());
            }
            if candidate < height[j] {
                height[j] = candidate;
                source[j] = source[i];
                queue.push(Entry(candidate, j));
            }
        }
    }
    if (0..n).any(|i| fixed[i] && height[i].to_bits() != bed[i].to_bits()) {
        return Err("Drainage-landform construction is incompatible with fixed heights.".into());
    }
    let mut maximum_slope = 0_f64;
    for e in edges {
        let slope = (height[e.a as usize] - height[e.b as usize]).abs() / e.distance_meters;
        if !slope.is_finite() || slope > settings.maximum_graph_slope * (1. + 1e-12) {
            return Err("Accepted roof exceeds its graph-edge slope bound.".into());
        }
        maximum_slope = maximum_slope.max(slope);
    }
    let mut channel_detached = Sum::default();
    let mut roof_detached = Sum::default();
    let mut total_detached = Sum::default();
    let mut removed = Vec::with_capacity(n);
    for i in 0..n {
        let a = bed[i] - profile[i];
        let b = profile[i] - height[i];
        let c = bed[i] - height[i];
        if a < 0. || b < 0. || c < 0. || !c.is_finite() {
            return Err("Construction raised bed or produced unrepresentable removal.".into());
        }
        for amount in [a, b, c] {
            if amount > 0. && amount * areas[i] == 0. {
                return Err("Unrepresentable positive construction material prism.".into());
            }
        }
        channel_detached.add(a * areas[i])?;
        roof_detached.add(b * areas[i])?;
        total_detached.add(c * areas[i])?;
        removed.push(c);
    }
    let channel_detached = channel_detached.value()?;
    let roof_detached = roof_detached.value()?;
    let detached = total_detached.value()?;
    let residual = relative((detached - channel_detached) - roof_detached, detached)?;
    Ok(Construction {
        model_version: MODEL_VERSION,
        settings,
        elevation_meters: height,
        channel_profile_constraint_meters: profile,
        structural_contributing_area_square_meters: contributing,
        structural_channel: channel,
        limiting_constraint_node: source,
        removed_thickness_meters: removed,
        channel_profile_detached_cubic_meters: channel_detached,
        roof_detached_cubic_meters: roof_detached,
        detached_material_cubic_meters: detached,
        relative_material_residual: residual,
        structural_terminal_area_relative_residual: area_residual,
        maximum_accepted_graph_slope: maximum_slope,
    })
}

/// Physical patch adapter. Both source and output receivers come from the
/// pinned actual-bed router; the camera and display resolution play no role.
pub fn construct_patch(g: &Geometry, bed: &[f64], settings: Settings) -> Result<Patch, String> {
    let original = terrain_patch_landscape::route(g, bed)?;
    let mut edges = Vec::new();
    for i in 0..bed.len() {
        for k in g.offsets[i] as usize..g.offsets[i + 1] as usize {
            let j = g.neighbors[k];
            if (i as u32) < j {
                edges.push(Edge {
                    a: i as u32,
                    b: j,
                    distance_meters: g.distances_meters[k],
                });
            }
        }
    }
    let construction = construct(
        bed,
        &g.reference_areas_square_meters,
        &g.fixed_external_boundary,
        &edges,
        &original.receivers,
        settings,
    )?;
    let actual_dry_routing = terrain_patch_landscape::route(g, &construction.elevation_meters)?;
    Ok(Patch {
        construction,
        actual_dry_routing,
    })
}
