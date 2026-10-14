//! Bounded dry prehistory: imposed vertical displacement, rebuilt downhill
//! drainage and detachment-limited incision. No sink correction or deposition.
use crate::{Surface, drainage::Drainage, geography_metrics, landscape, water::Water};
use serde::Serialize;

pub const MODEL_VERSION: &str = "uplift-stream-power-preparation-1";
pub const CONVERGENCE_REFERENCE_METERS: f64 = 6000.;
const MAX_STEPS: usize = 128;
const MAX_REGION_STEPS: usize = 64_000_000;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub erodibility_per_year: f64,
    pub step_years: f64,
    pub steps: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub step: usize,
    pub uplift_displacement_prism_cubic_meters: f64,
    pub exported_erosion_material_cubic_meters: f64,
    pub changed_uplift_nodes: usize,
    pub changed_incision_nodes: usize,
    pub unresolved_positive_uplift_nodes: usize,
    pub unresolved_positive_incision_nodes: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preparation {
    pub model_version: &'static str,
    pub settings: Settings,
    pub elevation_meters: Vec<f64>,
    pub accepted_uplift_displacement_meters: Vec<f64>,
    pub removed_erosion_thickness_meters: Vec<f64>,
    /// Kinematic height-prism change, NOT newly created rock or density mass.
    pub uplift_displacement_prism_cubic_meters: f64,
    /// Separate off-grid reference-prism material reservoir; no deposition.
    pub exported_erosion_material_cubic_meters: f64,
    pub independent_net_bed_prism_change_cubic_meters: f64,
    pub relative_prism_ledger_residual: f64,
    pub history: Vec<Step>,
}

/// Prescribed active-belt proxy U = K * characteristic_height * convergence/6000.
/// This uses initial relief structure, not evolving plates or inferred rock mass.
/// Original wet terminals have zero displacement in this dry preparation only.
pub fn structural_uplift_rates(
    convergence_meters: &[f64],
    water: &Water,
    erodibility_per_year: f64,
    characteristic_height_meters: f64,
) -> Result<Vec<f64>, String> {
    if convergence_meters.len() != water.body_ids.len()
        || convergence_meters.is_empty()
        || convergence_meters.len() > landscape::MAX_NODES
        || !erodibility_per_year.is_finite()
        || erodibility_per_year < 0.
        || !characteristic_height_meters.is_finite()
        || characteristic_height_meters < 0.
        || convergence_meters.iter().any(|v| !v.is_finite() || *v < 0.)
    {
        return Err("Invalid structural uplift fields or parameters.".into());
    }
    let rate = erodibility_per_year * characteristic_height_meters;
    if !rate.is_finite()
        || erodibility_per_year > 0. && characteristic_height_meters > 0. && rate == 0.
    {
        return Err("Unrepresentable structural uplift scale.".into());
    }
    convergence_meters
        .iter()
        .enumerate()
        .map(|(i, &convergence)| {
            if water.body_ids[i] != 0 {
                return Ok(0.);
            }
            let value = rate * (convergence / CONVERGENCE_REFERENCE_METERS);
            if !value.is_finite() || rate > 0. && convergence > 0. && value == 0. {
                return Err("Unrepresentable structural uplift rate.".into());
            }
            Ok(value)
        })
        .collect()
}

fn accumulate(total: &mut f64, correction: &mut f64, value: f64) -> Result<(), String> {
    if !value.is_finite() {
        return Err("Nonfinite landscape prism contribution.".into());
    }
    let adjusted = value - *correction;
    let next = *total + adjusted;
    *correction = (next - *total) - adjusted;
    *total = next;
    if !total.is_finite() || !correction.is_finite() {
        return Err("Landscape prism accumulation overflowed.".into());
    }
    Ok(())
}

/// Own all returned arrays; failures never mutate caller fields. Wet beds and
/// their original base level stay fixed. Dry sinks can uplift, but are never
/// given an artificial outlet or incised while self-receiving.
pub fn prepare(
    surface: &Surface,
    initial_bed: &[f64],
    initial_water: &Water,
    uplift_rates_meters_per_year: &[f64],
    settings: Settings,
) -> Result<Preparation, String> {
    let n = initial_bed.len();
    if !(1..=MAX_STEPS).contains(&settings.steps)
        || n.checked_mul(settings.steps)
            .is_none_or(|v| v > MAX_REGION_STEPS)
        || uplift_rates_meters_per_year.len() != n
        || !settings.erodibility_per_year.is_finite()
        || settings.erodibility_per_year < 0.
        || !settings.step_years.is_finite()
        || settings.step_years <= 0.
        || !(settings.step_years * settings.steps as f64).is_finite()
        || uplift_rates_meters_per_year
            .iter()
            .any(|r| !r.is_finite() || *r < 0.)
    {
        return Err("Invalid or excessive landscape preparation workload/settings.".into());
    }
    geography_metrics::validate(surface, initial_bed, initial_water)?;
    let mut visited = vec![false; n];
    let mut connected = vec![0_usize];
    visited[0] = true;
    let mut cursor = 0;
    while cursor < connected.len() {
        let i = connected[cursor];
        cursor += 1;
        for &j in &surface.neighbors[surface.offsets[i] as usize..surface.offsets[i + 1] as usize] {
            let j = j as usize;
            if !visited[j] {
                visited[j] = true;
                connected.push(j);
            }
        }
    }
    if connected.len() != n {
        return Err("Landscape preparation requires a connected surface.".into());
    }
    drop(visited);
    drop(connected);
    if uplift_rates_meters_per_year
        .iter()
        .enumerate()
        .any(|(i, &r)| initial_water.body_ids[i] != 0 && r != 0.)
    {
        return Err("Fixed wet terminals cannot uplift in this dry preparation.".into());
    }
    let mut result = Preparation {
        model_version: MODEL_VERSION,
        settings,
        elevation_meters: initial_bed.to_vec(),
        accepted_uplift_displacement_meters: vec![0.; n],
        removed_erosion_thickness_meters: vec![0.; n],
        uplift_displacement_prism_cubic_meters: 0.,
        exported_erosion_material_cubic_meters: 0.,
        independent_net_bed_prism_change_cubic_meters: 0.,
        relative_prism_ledger_residual: 0.,
        history: Vec::with_capacity(settings.steps),
    };
    let mut uplift_correction = 0.;
    let mut erosion_correction = 0.;
    for step in 0..settings.steps {
        let mut record = Step {
            step: step + 1,
            uplift_displacement_prism_cubic_meters: 0.,
            exported_erosion_material_cubic_meters: 0.,
            changed_uplift_nodes: 0,
            changed_incision_nodes: 0,
            unresolved_positive_uplift_nodes: 0,
            unresolved_positive_incision_nodes: 0,
        };
        let mut step_correction = 0.;
        for (i, &rate) in uplift_rates_meters_per_year.iter().enumerate() {
            if rate == 0. {
                continue;
            }
            let requested = rate * settings.step_years;
            let old = result.elevation_meters[i];
            let raised = old + requested;
            if !requested.is_finite() || requested <= 0. || !raised.is_finite() || raised < old {
                return Err("Unrepresentable prescribed landscape displacement.".into());
            }
            let accepted = raised - old;
            if accepted == 0. {
                record.unresolved_positive_uplift_nodes += 1;
            } else {
                record.changed_uplift_nodes += 1;
            }
            let volume = accepted * surface.areas[i];
            if accepted > 0. && volume == 0. {
                return Err("Unrepresentable displacement prism.".into());
            }
            accumulate(
                &mut record.uplift_displacement_prism_cubic_meters,
                &mut step_correction,
                volume,
            )?;
            result.accepted_uplift_displacement_meters[i] += accepted;
            if !result.accepted_uplift_displacement_meters[i].is_finite() {
                return Err("Displacement history overflowed.".into());
            }
            result.elevation_meters[i] = raised;
        }
        // Rebuild AFTER uplift: variable rates can reverse an old receiver edge.
        let graph = Drainage::build(surface, &result.elevation_meters, &initial_water.body_ids);
        let mut links = vec![0.; n];
        for (i, &receiver) in graph.receivers.iter().enumerate() {
            if receiver as usize == i {
                continue;
            }
            let edge = (surface.offsets[i] as usize..surface.offsets[i + 1] as usize)
                .find(|&k| surface.neighbors[k] == receiver)
                .ok_or("Nonadjacent landscape receiver.")?;
            links[i] = surface.distances[edge];
        }
        let incision = landscape::incise(
            &result.elevation_meters,
            &surface.areas,
            &graph.receivers,
            &links,
            landscape::Settings {
                erodibility_per_year: settings.erodibility_per_year,
                step_years: settings.step_years,
                base_level_meters: Some(initial_water.level_meters),
            },
        )?;
        record.exported_erosion_material_cubic_meters = incision.removed_material_cubic_meters;
        record.changed_incision_nodes = incision.changed_nodes;
        record.unresolved_positive_incision_nodes = incision.unresolved_positive_incision_nodes;
        for (sum, &increment) in result
            .removed_erosion_thickness_meters
            .iter_mut()
            .zip(&incision.removed_thickness_meters)
        {
            *sum += increment;
            if !sum.is_finite() {
                return Err("Erosion history overflowed.".into());
            }
        }
        accumulate(
            &mut result.uplift_displacement_prism_cubic_meters,
            &mut uplift_correction,
            record.uplift_displacement_prism_cubic_meters,
        )?;
        accumulate(
            &mut result.exported_erosion_material_cubic_meters,
            &mut erosion_correction,
            record.exported_erosion_material_cubic_meters,
        )?;
        result.elevation_meters = incision.elevation_meters;
        result.history.push(record);
    }
    let mut net_correction = 0.;
    for (i, &old) in initial_bed.iter().enumerate() {
        accumulate(
            &mut result.independent_net_bed_prism_change_cubic_meters,
            &mut net_correction,
            (result.elevation_meters[i] - old) * surface.areas[i],
        )?;
    }
    let gross = result
        .uplift_displacement_prism_cubic_meters
        .max(result.exported_erosion_material_cubic_meters)
        .max(1.);
    result.relative_prism_ledger_residual = ((result.uplift_displacement_prism_cubic_meters
        - result.exported_erosion_material_cubic_meters)
        - result.independent_net_bed_prism_change_cubic_meters)
        .abs()
        / gross;
    if !result.relative_prism_ledger_residual.is_finite()
        || result.relative_prism_ledger_residual > 1e-11
    {
        return Err("Landscape displacement/erosion prism ledger failed.".into());
    }
    Ok(result)
}
