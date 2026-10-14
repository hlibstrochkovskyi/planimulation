//! Synthetic dry prehistory with instantaneous downhill sediment delivery and
//! bounded leaf aggradation. Not a calibrated water/transport/settling model.
use crate::{
    Surface, drainage::Drainage, landscape_evolution, sediment_delivery, sediment_deposition,
    water::Water,
};
use serde::Serialize;

pub const MODEL_VERSION: &str = "sedimentary-landscape-preparation-1";

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub erodibility_per_year: f64,
    pub step_years: f64,
    pub steps: usize,
    pub maximum_aggradation_rate_meters_per_year: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub step: usize,
    pub gross_erosion_volume_cubic_meters: f64,
    pub gross_deposition_volume_cubic_meters: f64,
    pub debited_deposition_volume_cubic_meters: f64,
    pub relative_deposition_inventory_ledger_residual: f64,
    pub remobilized_sediment_volume_cubic_meters: f64,
    pub main_ocean_export_volume_cubic_meters: f64,
    pub other_initial_wet_export_volume_cubic_meters: f64,
    pub wet_body_exports: Vec<sediment_delivery::WetBodyDelivery>,
    pub pending_volume_total_cubic_meters: f64,
    pub changed_deposition_regions: usize,
    pub eligible_deposition_leaves: usize,
    pub dry_terminal_count_after_deposition: usize,
    pub changed_uplift_regions: usize,
    pub changed_incision_regions: usize,
    pub unresolved_positive_uplift_regions: usize,
    pub unresolved_positive_incision_regions: usize,
    pub unresolved_positive_deposition_leaves: usize,
    /// Diagnostic candidate only, not a material stock or accepted deposit.
    pub unresolved_deposition_candidate_volume_cubic_meters: f64,
    pub source_assembly_rounding_volume_cubic_meters: f64,
    pub unresolved_source_additions: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preparation {
    pub model_version: &'static str,
    pub incision_model_version: &'static str,
    pub delivery_model_version: &'static str,
    pub deposition_model_version: &'static str,
    pub settings: Settings,
    pub elevation_meters: Vec<f64>,
    /// Active surface sediment, removed before underlying bedrock at equal K.
    pub sediment_thickness_meters: Vec<f64>,
    /// Mobile retained solid-equivalent stock keyed by physical region, not basin.
    pub pending_volume_cubic_meters: Vec<f64>,
    pub accepted_uplift_displacement_meters: Vec<f64>,
    pub removed_erosion_thickness_meters: Vec<f64>,
    pub deposited_thickness_meters: Vec<f64>,
    pub uplift_displacement_prism_cubic_meters: f64,
    pub gross_erosion_volume_cubic_meters: f64,
    pub gross_deposition_volume_cubic_meters: f64,
    pub remobilized_sediment_volume_cubic_meters: f64,
    pub main_ocean_export_volume_cubic_meters: f64,
    pub other_initial_wet_export_volume_cubic_meters: f64,
    pub pending_volume_total_cubic_meters: f64,
    pub surviving_sediment_volume_cubic_meters: f64,
    pub wet_body_exports: Vec<sediment_delivery::WetBodyDelivery>,
    pub independent_net_bed_prism_change_cubic_meters: f64,
    pub relative_ledger_residual: f64,
    pub history: Vec<Step>,
}

#[derive(Default)]
struct Sum {
    value: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, v: f64) -> Result<(), String> {
        let adjusted = v - self.correction;
        let next = self.value + adjusted;
        self.correction = (next - self.value) - adjusted;
        self.value = next;
        if !v.is_finite() || !next.is_finite() || !self.correction.is_finite() {
            return Err("Sedimentary preparation prism overflowed.".into());
        }
        Ok(())
    }
}
fn sum(values: impl Iterator<Item = f64>) -> Result<f64, String> {
    let mut total = Sum::default();
    for v in values {
        total.add(v)?;
    }
    Ok(total.value)
}
fn add_history(total: &mut f64, v: f64) -> Result<(), String> {
    *total += v;
    if !total.is_finite() {
        return Err("Sedimentary thickness history overflowed.".into());
    }
    Ok(())
}

/// Zero initial sediment and pending stock. Original wet beds are fixed external
/// collectors, separately designated main ocean and other initial water bodies.
/// All nonterminal transport is instantaneous; excess retained at a leaf's first
/// spill reroutes only when the next actual bed DAG permits it. No forced outlet.
pub fn prepare(
    surface: &Surface,
    initial_bed: &[f64],
    initial_water: &Water,
    uplift_rates: &[f64],
    settings: Settings,
) -> Result<Preparation, String> {
    let n = initial_bed.len();
    let rise = settings.maximum_aggradation_rate_meters_per_year * settings.step_years;
    if !(1..=crate::landscape::MAX_NODES).contains(&n)
        || !(1..=128).contains(&settings.steps)
        || n.checked_mul(settings.steps).is_none_or(|v| v > 64_000_000)
        || !settings
            .maximum_aggradation_rate_meters_per_year
            .is_finite()
        || settings.maximum_aggradation_rate_meters_per_year < 0.
        || !rise.is_finite()
        || rise < 0.
        || settings.maximum_aggradation_rate_meters_per_year > 0. && rise == 0.
        || !(settings.step_years * settings.steps as f64).is_finite()
    {
        return Err("Invalid or excessive sedimentary preparation workload/rate.".into());
    }
    let one_step = landscape_evolution::Settings {
        erodibility_per_year: settings.erodibility_per_year,
        step_years: settings.step_years,
        steps: 1,
    };
    // Reuse the frozen validated uplift/incision kernel, not a changed old pin.
    // The first call validates fields, connectivity, wet source rates and dt/K
    // before this owner allocates full histories.
    let mut process =
        landscape_evolution::prepare(surface, initial_bed, initial_water, uplift_rates, one_step)?;
    let mut result = Preparation {
        model_version: MODEL_VERSION,
        incision_model_version: landscape_evolution::MODEL_VERSION,
        delivery_model_version: sediment_delivery::MODEL_VERSION,
        deposition_model_version: sediment_deposition::MODEL_VERSION,
        settings,
        elevation_meters: initial_bed.to_vec(),
        sediment_thickness_meters: vec![0.; n],
        pending_volume_cubic_meters: vec![0.; n],
        accepted_uplift_displacement_meters: vec![0.; n],
        removed_erosion_thickness_meters: vec![0.; n],
        deposited_thickness_meters: vec![0.; n],
        uplift_displacement_prism_cubic_meters: 0.,
        gross_erosion_volume_cubic_meters: 0.,
        gross_deposition_volume_cubic_meters: 0.,
        remobilized_sediment_volume_cubic_meters: 0.,
        main_ocean_export_volume_cubic_meters: 0.,
        other_initial_wet_export_volume_cubic_meters: 0.,
        wet_body_exports: Vec::new(),
        pending_volume_total_cubic_meters: 0.,
        surviving_sediment_volume_cubic_meters: 0.,
        independent_net_bed_prism_change_cubic_meters: 0.,
        relative_ledger_residual: 0.,
        history: Vec::with_capacity(settings.steps),
    };
    let (mut uplift, mut erosion, mut deposition, mut remobilized, mut ocean, mut inland) = (
        Sum::default(),
        Sum::default(),
        Sum::default(),
        Sum::default(),
        Sum::default(),
        Sum::default(),
    );
    for step in 0..settings.steps {
        if step != 0 {
            process = landscape_evolution::prepare(
                surface,
                &result.elevation_meters,
                initial_water,
                uplift_rates,
                one_step,
            )?;
        }
        // Reconstruct precisely the same pre-incision raised bed used by the
        // frozen kernel. Incision may lower it, so do not route on its output.
        let raised: Vec<_> = result
            .elevation_meters
            .iter()
            .zip(uplift_rates)
            .map(|(&h, &u)| {
                if u == 0. {
                    h
                } else {
                    h + u * settings.step_years
                }
            })
            .collect();
        let graph = Drainage::build(surface, &raised, &initial_water.body_ids);
        let mut source = Vec::with_capacity(n);
        let mut reworked = Sum::default();
        let mut assembly_rounding = Sum::default();
        let mut unresolved_additions = 0;
        for (i, &removed) in process.removed_erosion_thickness_meters.iter().enumerate() {
            let old_sediment = result.sediment_thickness_meters[i];
            let retained = old_sediment - old_sediment.min(removed);
            let actual = old_sediment - retained;
            result.sediment_thickness_meters[i] = retained;
            reworked.add(actual * surface.areas[i])?;
            let new_volume = removed * surface.areas[i];
            let supplied = new_volume + result.pending_volume_cubic_meters[i];
            if !supplied.is_finite() || removed > 0. && new_volume == 0. {
                return Err("Unrepresentable sedimentary supply prism.".into());
            }
            let pending = result.pending_volume_cubic_meters[i];
            if new_volume > 0. && supplied == pending || pending > 0. && supplied == new_volume {
                unresolved_additions += 1;
            }
            // TwoSum remainder observes plain f64 assembly without stock repair.
            let recovered_pending = supplied - new_volume;
            let remainder =
                (new_volume - (supplied - recovered_pending)) + (pending - recovered_pending);
            assembly_rounding.add(remainder.abs())?;
            source.push(supplied);
            add_history(
                &mut result.accepted_uplift_displacement_meters[i],
                process.accepted_uplift_displacement_meters[i],
            )?;
            add_history(&mut result.removed_erosion_thickness_meters[i], removed)?;
        }
        let delivered = sediment_delivery::deliver_volumes(
            surface,
            &raised,
            initial_water,
            &graph.receivers,
            &source,
        )?;
        if step == 0 {
            result.wet_body_exports = delivered.wet_bodies.clone();
        } else {
            for (total, increment) in result
                .wet_body_exports
                .iter_mut()
                .zip(&delivered.wet_bodies)
            {
                if total.body_id != increment.body_id {
                    return Err("Initial wet collector identities changed.".into());
                }
                add_history(
                    &mut total.volume_cubic_meters,
                    increment.volume_cubic_meters,
                )?;
            }
        }
        let dry_pending: Vec<_> = delivered
            .terminal_volume_cubic_meters
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                if initial_water.body_ids[i] == 0 {
                    v
                } else {
                    0.
                }
            })
            .collect();
        let deposited = sediment_deposition::deposit(
            surface,
            &process.elevation_meters,
            initial_water,
            &dry_pending,
            rise,
        )?;
        for (i, &d) in deposited.deposited_thickness_meters.iter().enumerate() {
            add_history(&mut result.sediment_thickness_meters[i], d)?;
            add_history(&mut result.deposited_thickness_meters[i], d)?;
        }
        uplift.add(process.uplift_displacement_prism_cubic_meters)?;
        erosion.add(process.exported_erosion_material_cubic_meters)?;
        deposition.add(deposited.accepted_volume_cubic_meters)?;
        remobilized.add(reworked.value)?;
        ocean.add(delivered.main_ocean_volume_cubic_meters)?;
        inland.add(delivered.other_initial_wet_body_volume_cubic_meters)?;
        result.elevation_meters = deposited.elevation_meters;
        result.pending_volume_cubic_meters = deposited.pending_volume_cubic_meters;
        let final_graph =
            Drainage::build(surface, &result.elevation_meters, &initial_water.body_ids);
        result.history.push(Step {
            step: step + 1,
            gross_erosion_volume_cubic_meters: process.exported_erosion_material_cubic_meters,
            gross_deposition_volume_cubic_meters: deposited.accepted_volume_cubic_meters,
            debited_deposition_volume_cubic_meters: deposited.debited_volume_cubic_meters,
            relative_deposition_inventory_ledger_residual: deposited
                .relative_inventory_ledger_residual,
            remobilized_sediment_volume_cubic_meters: reworked.value,
            main_ocean_export_volume_cubic_meters: delivered.main_ocean_volume_cubic_meters,
            other_initial_wet_export_volume_cubic_meters: delivered
                .other_initial_wet_body_volume_cubic_meters,
            wet_body_exports: delivered.wet_bodies,
            pending_volume_total_cubic_meters: sum(result
                .pending_volume_cubic_meters
                .iter()
                .copied())?,
            changed_deposition_regions: deposited.changed_regions,
            eligible_deposition_leaves: deposited.eligible_leaf_count,
            changed_uplift_regions: process.history[0].changed_uplift_nodes,
            changed_incision_regions: process.history[0].changed_incision_nodes,
            unresolved_positive_uplift_regions: process.history[0].unresolved_positive_uplift_nodes,
            unresolved_positive_incision_regions: process.history[0]
                .unresolved_positive_incision_nodes,
            unresolved_positive_deposition_leaves: deposited.unresolved_positive_deposition_leaves,
            unresolved_deposition_candidate_volume_cubic_meters: deposited
                .unresolved_candidate_volume_cubic_meters,
            source_assembly_rounding_volume_cubic_meters: assembly_rounding.value,
            unresolved_source_additions: unresolved_additions,
            dry_terminal_count_after_deposition: final_graph
                .receivers
                .iter()
                .enumerate()
                .filter(|&(i, &r)| r as usize == i && initial_water.body_ids[i] == 0)
                .count(),
        });
    }
    result.uplift_displacement_prism_cubic_meters = uplift.value;
    result.gross_erosion_volume_cubic_meters = erosion.value;
    result.gross_deposition_volume_cubic_meters = deposition.value;
    result.remobilized_sediment_volume_cubic_meters = remobilized.value;
    result.main_ocean_export_volume_cubic_meters = ocean.value;
    result.other_initial_wet_export_volume_cubic_meters = inland.value;
    result.pending_volume_total_cubic_meters =
        sum(result.pending_volume_cubic_meters.iter().copied())?;
    result.surviving_sediment_volume_cubic_meters = sum(result
        .sediment_thickness_meters
        .iter()
        .zip(&surface.areas)
        .map(|(&d, &a)| d * a))?;
    result.independent_net_bed_prism_change_cubic_meters = sum(result
        .elevation_meters
        .iter()
        .zip(initial_bed)
        .zip(&surface.areas)
        .map(|((&h, &old), &a)| (h - old) * a))?;
    // Independent ledgers, scaled by their own exchanges. Large displacement
    // cannot hide an error in a small sediment stock. Never repair either one.
    let material_scale = erosion
        .value
        .max(deposition.value)
        .max(ocean.value)
        .max(inland.value)
        .max(result.pending_volume_total_cubic_meters);
    let bed_scale = uplift.value.max(erosion.value).max(deposition.value);
    let sediment_scale = deposition
        .value
        .max(remobilized.value)
        .max(result.surviving_sediment_volume_cubic_meters);
    let collectors = sum(result
        .wet_body_exports
        .iter()
        .map(|b| b.volume_cubic_meters))?;
    let residuals = [
        (
            erosion.value
                - deposition.value
                - ocean.value
                - inland.value
                - result.pending_volume_total_cubic_meters,
            material_scale,
        ),
        (
            uplift.value - erosion.value + deposition.value
                - result.independent_net_bed_prism_change_cubic_meters,
            bed_scale,
        ),
        (
            deposition.value - remobilized.value - result.surviving_sediment_volume_cubic_meters,
            sediment_scale,
        ),
        (
            collectors - ocean.value - inland.value,
            ocean.value.max(inland.value),
        ),
    ];
    result.relative_ledger_residual = residuals
        .iter()
        .map(|&(v, scale)| v.abs() / if scale == 0. { 1. } else { scale })
        .fold(0., f64::max);
    if !result.relative_ledger_residual.is_finite() || result.relative_ledger_residual > 1e-11 {
        return Err("Sedimentary preparation material/bed/survival ledger failed.".into());
    }
    Ok(result)
}
