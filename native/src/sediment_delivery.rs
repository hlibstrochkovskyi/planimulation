//! Instantaneous routing of actually removed reference-area-equivalent material.
//! No water flux, deposition, new outlet, density mass, or bed change is inferred.
use crate::{Surface, geography_metrics, water::Water};
use serde::Serialize;

pub const MODEL_VERSION: &str = "sediment-delivery-1";

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WetBodyDelivery {
    pub body_id: u32,
    pub volume_cubic_meters: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Delivery {
    pub model_version: &'static str,
    /// Local source plus upstream supply. Summing this array double counts
    /// material that visits several regions; it is not a global inventory.
    pub through_volume_cubic_meters: Vec<f64>,
    /// Nonzero only at self-receivers. Dry supply is retained, not deposited.
    pub terminal_volume_cubic_meters: Vec<f64>,
    /// All supplied prisms: newly eroded material or explicitly owned pending
    /// dry inventory. This total alone does not identify new erosion.
    pub source_volume_cubic_meters: f64,
    pub dry_terminal_volume_cubic_meters: f64,
    pub main_ocean_volume_cubic_meters: f64,
    /// Initial wet bodies other than the designated main ocean, not marine
    /// export. Their common label does not imply underwater transport.
    pub other_initial_wet_body_volume_cubic_meters: f64,
    /// Ascending body IDs, including existing bodies receiving zero material.
    pub wet_bodies: Vec<WetBodyDelivery>,
    pub relative_volume_ledger_residual: f64,
}

#[derive(Clone, Copy, Default)]
struct Sum {
    high: f64,
    low: f64,
}
impl Sum {
    fn add(&mut self, value: f64) -> Result<(), String> {
        if !value.is_finite() || value < 0. {
            return Err("Invalid sediment volume contribution.".into());
        }
        let next = self.high + value;
        let error = if self.high >= value {
            (self.high - next) + value
        } else {
            (value - next) + self.high
        };
        self.low += error;
        self.high = next;
        if !self.high.is_finite() || !self.low.is_finite() {
            return Err("Sediment accumulation overflowed.".into());
        }
        Ok(())
    }
    fn value(self) -> Result<f64, String> {
        let value = self.high + self.low;
        if !value.is_finite() || value < 0. {
            return Err("Unrepresentable accumulated sediment volume.".into());
        }
        Ok(value)
    }
}

/// Route supplied accepted incision thickness on its pre-incision physical DAG.
/// The routing bed and initial water must agree. Wet beds cannot supply material
/// in this dry-preparation kernel. Positive sources may originate at dry self
/// receivers; they remain retained supply and do not raise that terminal's bed.
/// This kernel cannot prove supplied thickness was produced by an incision law;
/// the owning preparation must pass its actual accepted removal, not a request.
pub fn deliver(
    surface: &Surface,
    routing_bed_meters: &[f64],
    initial_water: &Water,
    receivers: &[u32],
    removed_thickness_meters: &[f64],
) -> Result<Delivery, String> {
    let n = routing_bed_meters.len();
    if !(1..=crate::landscape::MAX_NODES).contains(&n)
        || surface.areas.len() != n
        || removed_thickness_meters.len() != n
        || removed_thickness_meters
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
    {
        return Err("Invalid sediment source or receiver fields.".into());
    }
    let source_volumes = removed_thickness_meters
        .iter()
        .zip(&surface.areas)
        .map(|(&thickness, &area)| {
            let volume = thickness * area;
            if !area.is_finite()
                || area <= 0.
                || !volume.is_finite()
                || thickness > 0. && volume == 0.
            {
                return Err("Unrepresentable accepted sediment source prism.".into());
            }
            Ok(volume)
        })
        .collect::<Result<Vec<_>, String>>()?;
    deliver_volumes(
        surface,
        routing_bed_meters,
        initial_water,
        receivers,
        &source_volumes,
    )
}

/// Route explicit finite material inventory without a volume/thickness round
/// trip. Sources can combine accepted erosion and pending dry material, but the
/// caller must separately debit that owned inventory; this pure function does
/// not mutate it. Wet collectors remain terminal and cannot supply material.
pub fn deliver_volumes(
    surface: &Surface,
    routing_bed_meters: &[f64],
    initial_water: &Water,
    receivers: &[u32],
    source_volume_cubic_meters: &[f64],
) -> Result<Delivery, String> {
    let n = routing_bed_meters.len();
    if !(1..=crate::landscape::MAX_NODES).contains(&n)
        || receivers.len() != n
        || source_volume_cubic_meters.len() != n
        || source_volume_cubic_meters
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
    {
        return Err("Invalid sediment source or receiver fields.".into());
    }
    // Shared validator supplies the one-million-region / degree-32 bounds,
    // reciprocal distances, finite fields and initial wet-body consistency.
    geography_metrics::validate(surface, routing_bed_meters, initial_water)?;
    let mut seen = vec![false; n];
    let mut connected = vec![0_usize];
    seen[0] = true;
    let mut cursor = 0;
    while cursor < connected.len() {
        let i = connected[cursor];
        cursor += 1;
        for &j in &surface.neighbors[surface.offsets[i] as usize..surface.offsets[i + 1] as usize] {
            let j = j as usize;
            if !seen[j] {
                seen[j] = true;
                connected.push(j);
            }
        }
    }
    if connected.len() != n {
        return Err("Sediment delivery requires a connected surface.".into());
    }
    drop(seen);
    drop(connected);

    let mut incoming = vec![0_u32; n];
    let mut supply = vec![Sum::default(); n];
    let mut source = Sum::default();
    for i in 0..n {
        let r = receivers[i] as usize;
        if r >= n {
            return Err("Invalid sediment receiver.".into());
        }
        if initial_water.body_ids[i] != 0 && (r != i || source_volume_cubic_meters[i] != 0.) {
            return Err(
                "Initial wet terminals must be fixed and cannot supply dry incision.".into(),
            );
        }
        if r != i {
            if routing_bed_meters[r] > routing_bed_meters[i]
                || !surface.neighbors[surface.offsets[i] as usize..surface.offsets[i + 1] as usize]
                    .contains(&receivers[i])
            {
                return Err(
                    "Sediment receiver must be adjacent and no higher than its donor.".into(),
                );
            }
            incoming[r] += 1;
        }
        let volume = source_volume_cubic_meters[i];
        supply[i].add(volume)?;
        source.add(volume)?;
    }
    let mut order: Vec<_> = (0..n).filter(|&i| incoming[i] == 0).collect();
    let mut through_volume_cubic_meters = vec![0.; n];
    let mut terminal_volume_cubic_meters = vec![0.; n];
    let mut dry = Sum::default();
    let mut main_ocean = Sum::default();
    let mut other_wet = Sum::default();
    let mut terminals = Sum::default();
    let mut bodies = vec![Sum::default(); n + 1];
    cursor = 0;
    while cursor < order.len() {
        let i = order[cursor];
        cursor += 1;
        let volume = supply[i].value()?;
        through_volume_cubic_meters[i] = volume;
        let r = receivers[i] as usize;
        if r != i {
            supply[r].add(volume)?;
            incoming[r] -= 1;
            if incoming[r] == 0 {
                order.push(r);
            }
            continue;
        }
        terminal_volume_cubic_meters[i] = volume;
        terminals.add(volume)?;
        let body = initial_water.body_ids[i];
        if body == 0 {
            dry.add(volume)?;
        } else {
            bodies[body as usize].add(volume)?;
            if body == initial_water.main_ocean_id {
                main_ocean.add(volume)?;
            } else {
                other_wet.add(volume)?;
            }
        }
    }
    if order.len() != n {
        return Err("Sediment receiver graph contains a cycle.".into());
    }
    let source_volume_cubic_meters = source.value()?;
    let dry_terminal_volume_cubic_meters = dry.value()?;
    let main_ocean_volume_cubic_meters = main_ocean.value()?;
    let other_initial_wet_body_volume_cubic_meters = other_wet.value()?;
    let mut partition = Sum::default();
    for volume in [
        dry_terminal_volume_cubic_meters,
        main_ocean_volume_cubic_meters,
        other_initial_wet_body_volume_cubic_meters,
    ] {
        partition.add(volume)?;
    }
    let scale = if source_volume_cubic_meters == 0. {
        1.
    } else {
        source_volume_cubic_meters
    };
    let relative_volume_ledger_residual = (terminals.value()? - source_volume_cubic_meters)
        .abs()
        .max((partition.value()? - source_volume_cubic_meters).abs())
        / scale;
    if !relative_volume_ledger_residual.is_finite() || relative_volume_ledger_residual > 1e-11 {
        return Err("Sediment source/terminal volume ledger failed.".into());
    }
    let mut observed = vec![false; n + 1];
    for &body in &initial_water.body_ids {
        observed[body as usize] = true;
    }
    let wet_bodies = (1..=n)
        .filter(|&body| observed[body])
        .map(|body| {
            Ok(WetBodyDelivery {
                body_id: body as u32,
                volume_cubic_meters: bodies[body].value()?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Delivery {
        model_version: MODEL_VERSION,
        through_volume_cubic_meters,
        terminal_volume_cubic_meters,
        source_volume_cubic_meters,
        dry_terminal_volume_cubic_meters,
        main_ocean_volume_cubic_meters,
        other_initial_wet_body_volume_cubic_meters,
        wet_bodies,
        relative_volume_ledger_residual,
    })
}
