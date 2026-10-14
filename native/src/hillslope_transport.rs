//! Explicit finite-volume linear hillslope transport of reference-area-equivalent
//! bed material. Fixed heights are external reservoirs, never inferred oceans.
//! Geometry and topology are caller-owned; this kernel does not certify a mesh.
use serde::Serialize;

pub const MODEL_VERSION: &str = "linear-hillslope-transfer-1";
pub const MAX_NODES: usize = 262_144;
/// Global edge-count budget divided by node count, not a per-node degree cap.
pub const MAX_EDGES_PER_NODE: usize = 4;
const LEDGER_TOLERANCE: f64 = 1e-11;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub a: u32,
    pub b: u32,
    pub shared_face_length_meters: f64,
    pub center_distance_meters: f64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub diffusivity_square_meters_per_year: f64,
    pub step_years: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transport {
    pub model_version: &'static str,
    pub elevation_meters: Vec<f64>,
    /// Requested signed a→b volume on each input edge. Fixed-fixed edges are
    /// zero. This is geometric throughput, not an independently accepted stock.
    pub requested_edge_volume_cubic_meters: Vec<f64>,
    /// Requested net input to each active cell; fixed entries are zero.
    pub requested_node_net_volume_cubic_meters: Vec<f64>,
    /// Representable height changes, measured from the actual returned bed.
    pub accepted_height_change_meters: Vec<f64>,
    pub accepted_node_prism_cubic_meters: Vec<f64>,
    pub accepted_net_bed_prism_cubic_meters: f64,
    /// Requested signed input from external fixed-height reservoirs. Positive
    /// means active bed gains material. Accepted bed must reconcile with it.
    pub requested_boundary_exchange_cubic_meters: f64,
    pub gross_requested_boundary_exchange_cubic_meters: f64,
    /// Sum of absolute requested edge volumes, counting each edge once.
    pub gross_requested_transfer_cubic_meters: f64,
    pub relative_exchange_ledger_residual: f64,
    /// Separate boundary-exchange scale prevents large internal throughput from
    /// concealing an unrepresentable small exchange with an external reservoir.
    pub relative_boundary_exchange_ledger_residual: f64,
    /// Sum |accepted_node_prism - requested_node_net|. Numerical observation,
    /// not material inventory; balanced rounding need not vanish globally.
    pub gross_absolute_node_prism_rounding_cubic_meters: f64,
    pub maximum_active_cfl: f64,
    pub changed_nodes: usize,
    pub unresolved_requested_change_nodes: usize,
}

#[derive(Clone, Copy, Default)]
struct Sum {
    high: f64,
    low: f64,
}
impl Sum {
    fn add(&mut self, value: f64) -> Result<(), String> {
        let next = self.high + value;
        let error = if self.high.abs() >= value.abs() {
            (self.high - next) + value
        } else {
            (value - next) + self.high
        };
        self.low += error;
        self.high = next;
        if !value.is_finite() || !self.high.is_finite() || !self.low.is_finite() {
            return Err("Hillslope volume/weight accumulation overflowed.".into());
        }
        Ok(())
    }
    fn value(self) -> Result<f64, String> {
        let result = self.high + self.low;
        if !result.is_finite() {
            return Err("Unrepresentable accumulated hillslope volume/weight.".into());
        }
        Ok(result)
    }
}

fn relative_residual(residual: f64, scale: f64) -> Result<f64, String> {
    let relative = if scale == 0. {
        if residual == 0. { 0. } else { f64::INFINITY }
    } else {
        residual.abs() / scale
    };
    if !relative.is_finite() || relative > LEDGER_TOLERANCE {
        return Err("Accepted hillslope bed does not reconcile with requested exchange.".into());
    }
    Ok(relative)
}

/// One synchronous explicit step. For edge a<b:
/// q[a→b] = dt*D*face_length/center_distance*(old_h[a]-old_h[b]).
/// The monotonicity condition is dt*D*sum(face/distance)/area <= 1 at
/// every active node. An unstable step refuses even for a constant field.
///
/// Paired requested transfers are distinct from floating accepted height changes.
/// Internal subprecision changes may remain unresolved if the actual global
/// prism balance passes. Nonzero external exchanges also pass their own scale
/// gate; they cannot be booked as successful lost-height changes. No clamping,
/// stock repair, sediment-layer integration or adaptive timestep is performed.
/// All inputs remain unchanged, including on failure.
pub fn transport(
    bed_meters: &[f64],
    reference_areas_square_meters: &[f64],
    fixed_boundary: &[bool],
    edges: &[Edge],
    settings: Settings,
) -> Result<Transport, String> {
    let n = bed_meters.len();
    if !(1..=MAX_NODES).contains(&n)
        || reference_areas_square_meters.len() != n
        || fixed_boundary.len() != n
        || edges.len() > MAX_EDGES_PER_NODE * n
        || bed_meters.iter().any(|v| !v.is_finite())
        || reference_areas_square_meters
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
        || !settings.diffusivity_square_meters_per_year.is_finite()
        || settings.diffusivity_square_meters_per_year < 0.
        || !settings.step_years.is_finite()
        || settings.step_years <= 0.
    {
        return Err("Invalid hillslope fields, settings or workload.".into());
    }
    let mut keys = Vec::with_capacity(edges.len());
    for edge in edges {
        if edge.a >= edge.b
            || edge.b as usize >= n
            || !edge.shared_face_length_meters.is_finite()
            || edge.shared_face_length_meters <= 0.
            || !edge.center_distance_meters.is_finite()
            || edge.center_distance_meters <= 0.
        {
            return Err("Invalid or noncanonical hillslope edge geometry.".into());
        }
        keys.push((u64::from(edge.a) << 32) | u64::from(edge.b));
    }
    keys.sort_unstable();
    if keys.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("Duplicate undirected hillslope edge.".into());
    }
    drop(keys);

    let mut result = Transport {
        model_version: MODEL_VERSION,
        elevation_meters: bed_meters.to_vec(),
        requested_edge_volume_cubic_meters: vec![0.; edges.len()],
        requested_node_net_volume_cubic_meters: vec![0.; n],
        accepted_height_change_meters: vec![0.; n],
        accepted_node_prism_cubic_meters: vec![0.; n],
        accepted_net_bed_prism_cubic_meters: 0.,
        requested_boundary_exchange_cubic_meters: 0.,
        gross_requested_boundary_exchange_cubic_meters: 0.,
        gross_requested_transfer_cubic_meters: 0.,
        relative_exchange_ledger_residual: 0.,
        relative_boundary_exchange_ledger_residual: 0.,
        gross_absolute_node_prism_rounding_cubic_meters: 0.,
        maximum_active_cfl: 0.,
        changed_nodes: 0,
        unresolved_requested_change_nodes: 0,
    };
    if settings.diffusivity_square_meters_per_year == 0. {
        return Ok(result);
    }
    let coefficient = settings.step_years * settings.diffusivity_square_meters_per_year;
    if !coefficient.is_finite() || coefficient <= 0. {
        return Err("Unrepresentable positive hillslope time coefficient.".into());
    }
    let mut weights = vec![Sum::default(); n];
    let mut minimum = bed_meters.to_vec();
    let mut maximum = bed_meters.to_vec();
    // Reuse the eventual edge-volume output as temporary transmissivity weights.
    for (k, edge) in edges.iter().enumerate() {
        let a = edge.a as usize;
        let b = edge.b as usize;
        if fixed_boundary[a] && fixed_boundary[b] {
            continue;
        }
        let ratio = edge.shared_face_length_meters / edge.center_distance_meters;
        let weight = coefficient * ratio;
        if !ratio.is_finite() || ratio <= 0. || !weight.is_finite() || weight <= 0. {
            return Err("Unrepresentable positive hillslope edge transmissivity.".into());
        }
        result.requested_edge_volume_cubic_meters[k] = weight;
        for (i, j) in [(a, b), (b, a)] {
            if !fixed_boundary[i] {
                weights[i].add(weight)?;
                minimum[i] = minimum[i].min(bed_meters[j]);
                maximum[i] = maximum[i].max(bed_meters[j]);
            }
        }
    }
    for i in 0..n {
        if fixed_boundary[i] {
            continue;
        }
        let weight = weights[i].value()?;
        let cfl = weight / reference_areas_square_meters[i];
        if !cfl.is_finite() || cfl > 1. || weight > 0. && cfl == 0. {
            return Err("Hillslope step violates its CFL or representability condition.".into());
        }
        result.maximum_active_cfl = result.maximum_active_cfl.max(cfl);
    }
    drop(weights);

    let mut net = vec![Sum::default(); n];
    let mut boundary = Sum::default();
    let mut boundary_gross = Sum::default();
    let mut gross = Sum::default();
    let mut has_external_transfer = vec![false; n];
    for (k, edge) in edges.iter().enumerate() {
        let a = edge.a as usize;
        let b = edge.b as usize;
        let weight = result.requested_edge_volume_cubic_meters[k];
        if weight == 0. {
            continue;
        }
        let difference = bed_meters[a] - bed_meters[b];
        let volume = weight * difference;
        if !difference.is_finite() || !volume.is_finite() || difference != 0. && volume == 0. {
            return Err("Unrepresentable requested hillslope transfer.".into());
        }
        result.requested_edge_volume_cubic_meters[k] = volume;
        gross.add(volume.abs())?;
        if !fixed_boundary[a] {
            net[a].add(-volume)?;
        }
        if !fixed_boundary[b] {
            net[b].add(volume)?;
        }
        if fixed_boundary[a] != fixed_boundary[b] {
            boundary.add(if fixed_boundary[a] { volume } else { -volume })?;
            boundary_gross.add(volume.abs())?;
            if volume != 0. {
                has_external_transfer[if fixed_boundary[a] { b } else { a }] = true;
            }
        }
    }
    let mut actual = Sum::default();
    let mut rounding = Sum::default();
    for i in 0..n {
        let requested = net[i].value()?;
        result.requested_node_net_volume_cubic_meters[i] = requested;
        if requested == 0. {
            continue;
        }
        let delta = requested / reference_areas_square_meters[i];
        let updated = bed_meters[i] + delta;
        if !delta.is_finite() || delta == 0. || !updated.is_finite() {
            return Err("Unrepresentable requested hillslope height update.".into());
        }
        if updated < minimum[i] || updated > maximum[i] {
            return Err("Hillslope update violates its local maximum principle.".into());
        }
        let change = updated - bed_meters[i];
        let prism = change * reference_areas_square_meters[i];
        if !change.is_finite() || !prism.is_finite() || change != 0. && prism == 0. {
            return Err("Unrepresentable accepted hillslope height prism.".into());
        }
        result.elevation_meters[i] = updated;
        result.accepted_height_change_meters[i] = change;
        result.accepted_node_prism_cubic_meters[i] = prism;
        rounding.add((prism - requested).abs())?;
        if change == 0. {
            if has_external_transfer[i] {
                return Err(
                    "External hillslope exchange has an unresolved active-bed update.".into(),
                );
            }
            result.unresolved_requested_change_nodes += 1;
        } else {
            result.changed_nodes += 1;
        }
        actual.add(prism)?;
    }
    result.accepted_net_bed_prism_cubic_meters = actual.value()?;
    result.gross_absolute_node_prism_rounding_cubic_meters = rounding.value()?;
    result.requested_boundary_exchange_cubic_meters = boundary.value()?;
    result.gross_requested_boundary_exchange_cubic_meters = boundary_gross.value()?;
    result.gross_requested_transfer_cubic_meters = gross.value()?;
    actual.add(-result.requested_boundary_exchange_cubic_meters)?;
    let residual = actual.value()?;
    result.relative_exchange_ledger_residual =
        relative_residual(residual, result.gross_requested_transfer_cubic_meters)?;
    if result.gross_requested_boundary_exchange_cubic_meters > 0. {
        result.relative_boundary_exchange_ledger_residual = relative_residual(
            residual,
            result.gross_requested_boundary_exchange_cubic_meters,
        )?;
    }
    Ok(result)
}
