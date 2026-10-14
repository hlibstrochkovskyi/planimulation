//! Inventory-limited, rate-bounded minimum-leaf aggradation; not DEM repair,
//! lake-water filling, delta morphology or a calibrated settling law.
use crate::{Surface, basins::Basins, geography_metrics, water::Water};
use serde::Serialize;

pub const MODEL_VERSION: &str = "minimum-leaf-aggradation-1";
const LEDGER_RELATIVE_TOLERANCE: f64 = 1e-11;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deposition {
    pub model_version: &'static str,
    pub elevation_meters: Vec<f64>,
    pub deposited_thickness_meters: Vec<f64>,
    pub pending_volume_cubic_meters: Vec<f64>,
    pub accepted_volume_cubic_meters: f64,
    /// Sum of actual representable donor-stock decrements, separately measured
    /// from accepted geometric prisms; never repaired to match geometry.
    pub debited_volume_cubic_meters: f64,
    /// Relative to deposited prisms, not a huge untouched pending inventory.
    pub relative_inventory_ledger_residual: f64,
    /// Positive geometric proposals whose donor debit could not be represented
    /// within the prism-relative tolerance. Their beds and stocks are unchanged.
    pub unresolved_positive_deposition_leaves: usize,
    /// Sum of skipped geometric proposals, NOT material transferred, lost,
    /// deposited or an extra inventory. The source remains in pending stock.
    pub unresolved_candidate_volume_cubic_meters: f64,
    pub eligible_leaf_count: usize,
    pub changed_regions: usize,
}

fn sum(values: impl Iterator<Item = f64>) -> Result<f64, String> {
    sum_checked(values.map(Ok))
}

fn sum_checked(values: impl Iterator<Item = Result<f64, String>>) -> Result<f64, String> {
    let mut total = 0.;
    let mut correction = 0.;
    for v in values {
        let v = v?;
        let adjusted = v - correction;
        let next = total + adjusted;
        correction = (next - total) - adjusted;
        total = next;
        if !v.is_finite() || v < 0. || !total.is_finite() || !correction.is_finite() {
            return Err("Deposition prism overflowed.".into());
        }
    }
    Ok(total)
}

/// Pending stock is attached to physical region IDs. Only pending on current
/// minimum plateaus can deposit; stock elsewhere stays pending for rerouting.
/// Non-root leaves stop at first sibling connection. No artificial global drain.
/// Geometry and actual donor-stock decrements must agree within 1e-11 of each
/// leaf's deposited prisms. If its stock debit cannot represent that geometry,
/// the whole leaf stays unchanged and is counted as unresolved. The measured
/// rounding discrepancy for accepted leaves is returned, never repaired. This
/// instantaneous primitive does not preserve hidden compensated pending stocks.
pub fn deposit(
    s: &Surface,
    h: &[f64],
    water: &Water,
    pending: &[f64],
    maximum_rise_meters: f64,
) -> Result<Deposition, String> {
    if !(1..=crate::landscape::MAX_NODES).contains(&h.len())
        || pending.len() != h.len()
        || !maximum_rise_meters.is_finite()
        || maximum_rise_meters < 0.
        || pending.iter().any(|v| !v.is_finite() || *v < 0.)
    {
        return Err("Invalid leaf deposition fields or rate bound.".into());
    }
    geography_metrics::validate(s, h, water)?;
    if pending
        .iter()
        .enumerate()
        .any(|(i, &v)| water.body_ids[i] != 0 && v != 0.)
    {
        return Err("Initial wet collectors cannot hold dry deposition inventory.".into());
    }
    let basins = Basins::build(s, h)?;
    let mut footprints = vec![Vec::new(); basins.nodes().len()];
    for (i, &node) in basins.region_nodes().iter().enumerate() {
        if basins.nodes()[node].children.is_empty() {
            footprints[node].push(i);
        }
    }
    let initial_stock = sum(pending.iter().copied())?;
    let mut result = Deposition {
        model_version: MODEL_VERSION,
        elevation_meters: h.to_vec(),
        deposited_thickness_meters: vec![0.; h.len()],
        pending_volume_cubic_meters: pending.to_vec(),
        accepted_volume_cubic_meters: 0.,
        debited_volume_cubic_meters: 0.,
        relative_inventory_ledger_residual: 0.,
        unresolved_positive_deposition_leaves: 0,
        unresolved_candidate_volume_cubic_meters: 0.,
        eligible_leaf_count: 0,
        changed_regions: 0,
    };
    let mut accepted_prisms = Vec::new();
    let mut debited_prisms = Vec::new();
    let mut unresolved_prisms = Vec::new();
    for (node, cells) in footprints.iter().enumerate() {
        let branch = &basins.nodes()[node];
        let Some(spill) = branch.spill_level_meters else {
            continue;
        };
        if cells.iter().any(|&i| water.body_ids[i] != 0) {
            continue;
        }
        let donors: Vec<_> = cells
            .iter()
            .copied()
            .filter(|&i| h[i] == branch.birth_level_meters && pending[i] > 0.)
            .collect();
        if donors.is_empty() || maximum_rise_meters == 0. {
            continue;
        }
        let inventory = sum(donors.iter().map(|&i| pending[i]))?;
        let mut upper = (branch.birth_level_meters + maximum_rise_meters).min(spill);
        if !upper.is_finite() {
            return Err("Deposition rate/level overflowed.".into());
        }
        // Rounded addition must not increase the declared thickness allowance.
        if upper - branch.birth_level_meters > maximum_rise_meters {
            upper = upper.next_down();
        }
        let volume_at = |level: f64| {
            sum_checked(cells.iter().map(|&i| {
                let thickness = (level - h[i]).max(0.);
                let volume = thickness * s.areas[i];
                if thickness > 0. && volume == 0. {
                    Err("Positive deposition prism underflowed.".to_string())
                } else {
                    Ok(volume)
                }
            }))
        };
        let mut low = branch.birth_level_meters;
        let mut high = upper;
        if volume_at(high)? <= inventory {
            low = high;
        } else {
            // Keep a feasible lower bound. No epsilon height or stock repair.
            for _ in 0..80 {
                let mid = low + (high - low) * 0.5;
                if mid <= low || mid >= high {
                    break;
                }
                if volume_at(mid)? <= inventory {
                    low = mid;
                } else {
                    high = mid;
                }
            }
        }
        let accepted = volume_at(low)?;
        if accepted == 0. {
            continue;
        }
        let mut remaining = accepted;
        let mut donor_debits = Vec::new();
        let mut candidate_stocks = Vec::new();
        let mut unresolved = false;
        for &i in &donors {
            if remaining <= 0. {
                break;
            }
            let debit = remaining.min(result.pending_volume_cubic_meters[i]);
            let new = result.pending_volume_cubic_meters[i] - debit;
            let actual = result.pending_volume_cubic_meters[i] - new;
            if !new.is_finite() || new < 0. || !actual.is_finite() || actual < 0. {
                return Err("Invalid representable deposition inventory debit.".into());
            }
            if actual > remaining && (actual - remaining) / accepted > LEDGER_RELATIVE_TOLERANCE {
                unresolved = true;
                break;
            }
            candidate_stocks.push((i, new));
            donor_debits.push(actual);
            remaining -= actual;
        }
        let actual_debit = sum(donor_debits.into_iter())?;
        if unresolved
            || actual_debit == 0.
            || remaining.abs() / accepted > LEDGER_RELATIVE_TOLERANCE
            || (actual_debit - accepted).abs() / accepted > LEDGER_RELATIVE_TOLERANCE
        {
            result.unresolved_positive_deposition_leaves += 1;
            unresolved_prisms.push(accepted);
            continue;
        }
        // Commit atomically only after all required stock debits are feasible.
        for (i, new) in candidate_stocks {
            result.pending_volume_cubic_meters[i] = new;
        }
        for &i in cells {
            let raised = h[i].max(low);
            let delta = raised - h[i];
            if delta > 0. {
                result.changed_regions += 1;
            }
            result.elevation_meters[i] = raised;
            result.deposited_thickness_meters[i] = delta;
        }
        accepted_prisms.push(accepted);
        debited_prisms.push(actual_debit);
        result.eligible_leaf_count += 1;
    }
    result.accepted_volume_cubic_meters = sum(accepted_prisms.into_iter())?;
    result.debited_volume_cubic_meters = sum(debited_prisms.into_iter())?;
    result.unresolved_candidate_volume_cubic_meters = sum(unresolved_prisms.into_iter())?;
    let actual = sum(result
        .deposited_thickness_meters
        .iter()
        .zip(&s.areas)
        .map(|(&d, &a)| d * a))?;
    let remaining = sum(result.pending_volume_cubic_meters.iter().copied())?;
    result.relative_inventory_ledger_residual = if actual == 0. {
        0.
    } else {
        (actual - result.debited_volume_cubic_meters).abs() / actual
    };
    if (actual - result.accepted_volume_cubic_meters).abs() > actual * LEDGER_RELATIVE_TOLERANCE
        || result.relative_inventory_ledger_residual > LEDGER_RELATIVE_TOLERANCE
        || (initial_stock - remaining - actual).abs()
            > initial_stock.max(actual).max(1.) * LEDGER_RELATIVE_TOLERANCE
    {
        return Err("Deposition geometry/inventory ledger failed.".into());
    }
    Ok(result)
}
