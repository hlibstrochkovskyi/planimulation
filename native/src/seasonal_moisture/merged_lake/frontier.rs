//! One-level reversible ownership. Historical transfers are not water inventories.
use super::super::{Checkpoint as SeasonalCheckpoint, closed_lake, leaf_spill, reference_pool};
use super::{Checkpoint as ParentCheckpoint, Layout, depletion};
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock};
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "common-sill-lifecycle-1";

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub model_version: String,
    pub merge_counts: Vec<u64>,
    pub split_counts: Vec<u64>,
    pub pending_to_parent: leaf_spill::Components,
    pub capture_by_parent: leaf_spill::Components,
    pub delivery_to_parent: leaf_spill::Components,
    pub evaporation_from_parent: leaf_spill::Components,
}

fn state(cp: &SeasonalCheckpoint) -> &Checkpoint {
    cp.merged_lake_state
        .as_ref()
        .unwrap()
        .frontier
        .as_ref()
        .unwrap()
}
fn state_mut(cp: &mut SeasonalCheckpoint) -> &mut Checkpoint {
    cp.merged_lake_state
        .as_mut()
        .unwrap()
        .frontier
        .as_mut()
        .unwrap()
}

/// Exact two-product for a represented capacity and an exactly represented count.
/// Counts are bounded below 2^53 by the accepted simulation clock.
fn capacity_flow(cap: f64, count: u64) -> Result<CompensatedStock, String> {
    let count = count as f64;
    let high = cap * count;
    CompensatedStock::new(high, cap.mul_add(count, -high), f64::MAX)
}

pub(in super::super) fn checked_allocation(
    donor: CompensatedStock,
    requests: &[f64],
) -> Result<(CompensatedStock, Vec<f64>, f64), String> {
    let result = reference_pool::allocate(donor, requests)?;
    let mut checked = donor;
    for &grant in &result.1 {
        let before = checked;
        if checked.withdraw(grant) != grant
            || (grant > 0. && (checked.high, checked.low) == (before.high, before.low))
        {
            return Err("Common-sill evaporation is below donor pair resolution.".into());
        }
    }
    if (checked.high, checked.low) != (result.0.high, result.0.low) {
        return Err("Common-sill allocation differs from its debits.".into());
    }
    Ok(result)
}

impl Layout {
    pub fn frontier_checkpoint(&self) -> ParentCheckpoint {
        let n = self.by_region.len();
        ParentCheckpoint {
            model_version: super::FRONTIER_MODEL_VERSION.into(),
            parents: Vec::new(),
            frontier: Some(Checkpoint {
                model_version: MODEL_VERSION.into(),
                merge_counts: vec![0; self.groups.len()],
                split_counts: vec![0; self.groups.len()],
                pending_to_parent: leaf_spill::Components::zero(n),
                capture_by_parent: leaf_spill::Components::zero(n),
                delivery_to_parent: leaf_spill::Components::zero(n),
                evaporation_from_parent: leaf_spill::Components::zero(n),
            }),
        }
    }

    pub fn accounts(&self, cp: &SeasonalCheckpoint, r: usize) -> bool {
        self.owner(cp, r).is_some()
            || (self.split_enabled
                && self.by_region[r].is_some_and(|g| state(cp).merge_counts[g] > 0))
    }

    pub fn record_capture(
        &self,
        cp: &mut SeasonalCheckpoint,
        r: usize,
        high: f64,
        low: f64,
    ) -> Result<(), String> {
        if self.split_enabled && self.owner(cp, r).is_some() {
            let ledger = &mut state_mut(cp).capture_by_parent;
            ledger.credit(r, high)?;
            ledger.credit(r, low)?;
        }
        Ok(())
    }

    pub fn record_delivery(
        &self,
        cp: &mut SeasonalCheckpoint,
        r: usize,
        amount: f64,
    ) -> Result<(), String> {
        if self.split_enabled && self.terminal_active(cp, r) {
            state_mut(cp).delivery_to_parent.credit(r, amount)?;
        }
        Ok(())
    }

    pub(super) fn record_merge(&self, cp: &mut SeasonalCheckpoint, g: usize) -> Result<(), String> {
        if !self.split_enabled {
            return Ok(());
        }
        for &r in &self.groups[g].description.child_terminals {
            let pending = cp
                .leaf_spill_state
                .as_ref()
                .unwrap()
                .pending_input
                .stock(r)?;
            state_mut(cp).pending_to_parent.credit(r, pending.high)?;
            state_mut(cp).pending_to_parent.credit(r, pending.low)?;
        }
        let counts = &mut state_mut(cp).merge_counts;
        counts[g] = counts[g]
            .checked_add(1)
            .ok_or("Common-sill merge count overflow.")?;
        Ok(())
    }

    /// Additional terms for a leaf's historical identity, including inactive parents.
    pub fn leaf_terms(
        &self,
        cp: &SeasonalCheckpoint,
        lake: &closed_lake::Lake,
    ) -> Result<Vec<f64>, String> {
        if !self.split_enabled {
            return Ok(Vec::new());
        }
        let r = lake.terminal_region();
        let Some(g) = self.by_terminal[r] else {
            return Ok(Vec::new());
        };
        let history = state(cp);
        let cap = lake.capacity_cubic_meters().unwrap() * 1000.;
        let merged = capacity_flow(cap, history.merge_counts[g])?;
        let split = capacity_flow(cap, history.split_counts[g])?;
        let pending = history.pending_to_parent.stock(r)?;
        let delivered = history.delivery_to_parent.stock(r)?;
        let mut terms = vec![
            merged.high,
            merged.low,
            -split.high,
            -split.low,
            pending.high,
            pending.low,
            delivered.high,
            delivered.low,
        ];
        if self.terminal_active(cp, r) {
            let queue = cp
                .leaf_spill_state
                .as_ref()
                .unwrap()
                .pending_input
                .stock(r)?;
            terms.extend([-queue.high, -queue.low]);
        }
        for &i in lake.regions() {
            let capture = history.capture_by_parent.stock(i)?;
            let evaporation = history.evaporation_from_parent.stock(i)?;
            terms.extend([
                capture.high,
                capture.low,
                -evaporation.high,
                -evaporation.low,
            ]);
        }
        Ok(terms)
    }

    /// Consume the common layer before returning complete birth contents to children.
    /// Remaining *potential* demand is independently limited by each child inventory.
    pub fn evaporate_frontier(
        &self,
        cp: &mut SeasonalCheckpoint,
        demand: &[f64],
        leaves: &closed_lake::Layout,
    ) -> Result<(Vec<(usize, f64)>, f64), String> {
        let mut packets = Vec::new();
        let mut maximum: f64 = 0.;
        let mut leaf_by_terminal = vec![None; self.by_terminal.len()];
        for lake in leaves.lakes() {
            leaf_by_terminal[lake.terminal_region()] = Some(lake);
        }
        for (g, group) in self.groups.iter().enumerate() {
            let Some(p) = self.active(cp, g) else {
                continue;
            };
            let regions = &group.description.regions;
            let requests: Vec<_> = regions.iter().map(|&r| demand[r]).collect();
            let donor =
                cp.merged_lake_state.as_ref().unwrap().parents[p].surplus(group.capacity())?;
            let split = depletion::covers(depletion::sum(&requests)?, donor);
            let grants = if split {
                depletion::full(donor, &requests)?
            } else {
                let (after, grants, residual) = checked_allocation(donor, &requests)?;
                cp.merged_lake_state.as_mut().unwrap().parents[p].set_surplus(after);
                maximum = maximum.max(residual.abs());
                grants
                    .into_iter()
                    .enumerate()
                    .filter(|(_, grant)| *grant > 0.)
                    .collect()
            };
            let mut remaining: Vec<_> = requests
                .iter()
                .map(|&r| CompensatedStock::new(r, 0., f64::MAX))
                .collect::<Result<_, _>>()?;
            for (i, grant) in grants {
                if remaining[i].withdraw(grant) != grant {
                    return Err("Common-layer grant exceeds remaining local demand.".into());
                }
                let r = regions[i];
                state_mut(cp).evaporation_from_parent.credit(r, grant)?;
                packets.push((r, grant));
            }
            if !split {
                continue;
            }
            // The depletion helper has debited the complete high/low pair. Never snap a tail.
            cp.merged_lake_state.as_mut().unwrap().parents.remove(p);
            for (&r, &cap) in group
                .description
                .child_terminals
                .iter()
                .zip(&group.description.child_capacities_kilograms)
            {
                cp.terminal_water_kilograms[r] = cap;
                cp.terminal_low_kilograms.as_mut().unwrap()[r] = 0.;
            }
            let counts = &mut state_mut(cp).split_counts;
            counts[g] = counts[g]
                .checked_add(1)
                .ok_or("Common-sill split count overflow.")?;
            for &terminal in &group.description.child_terminals {
                let lake = leaf_by_terminal[terminal]
                    .ok_or("Common-sill child is absent from the leaf geometry.")?;
                let r = lake.terminal_region();
                let requests: Vec<_> = lake
                    .regions()
                    .iter()
                    .map(|i| {
                        let local = regions.binary_search(i).unwrap();
                        remaining[local].available()
                    })
                    .collect();
                let donor = CompensatedStock::new(cp.terminal_water_kilograms[r], 0., f64::MAX)?;
                let (after, grants, residual) = checked_allocation(donor, &requests)?;
                cp.terminal_water_kilograms[r] = after.high;
                cp.terminal_low_kilograms.as_mut().unwrap()[r] = after.low;
                maximum = maximum.max(residual.abs());
                packets.extend(
                    lake.regions()
                        .iter()
                        .copied()
                        .zip(grants)
                        .filter(|(_, grant)| *grant > 0.),
                );
            }
        }
        Ok((packets, maximum))
    }

    pub(super) fn validate_frontier(
        &self,
        cp: &SeasonalCheckpoint,
    ) -> Result<(f64, usize), String> {
        let parent_state = cp
            .merged_lake_state
            .as_ref()
            .ok_or("Missing common-sill state.")?;
        let history = parent_state
            .frontier
            .as_ref()
            .ok_or("Missing common-sill lifecycle accounting.")?;
        let n = self.by_region.len();
        if parent_state.model_version != super::FRONTIER_MODEL_VERSION
            || history.model_version != MODEL_VERSION
            || history.merge_counts.len() != self.groups.len()
            || history.split_counts.len() != self.groups.len()
            || parent_state.parents.len() > self.groups.len()
            || parent_state
                .parents
                .windows(2)
                .any(|w| w[0].basin_node >= w[1].basin_node)
        {
            return Err("Invalid common-sill lifecycle version, frontier, or shape.".into());
        }
        let ledgers = [
            &history.pending_to_parent,
            &history.capture_by_parent,
            &history.delivery_to_parent,
            &history.evaporation_from_parent,
        ];
        for (kind, ledger) in ledgers.iter().enumerate() {
            if ledger.high_kilograms.len() != n || ledger.low_kilograms.len() != n {
                return Err("Invalid common-sill lifecycle ledger shape.".into());
            }
            for r in 0..n {
                let value = ledger.stock(r)?;
                let group = if kind == 0 || kind == 2 {
                    self.by_terminal[r]
                } else {
                    self.by_region[r]
                };
                if (group.is_none_or(|g| history.merge_counts[g] == 0) || cp.elapsed_seconds == 0)
                    && value.high != 0.
                {
                    return Err("Common-sill lifecycle flow has no historical owner.".into());
                }
                let actual = match kind {
                    1 => Some(CompensatedStock::new(
                        cp.cumulative_lake_capture_kilograms.as_ref().unwrap()[r],
                        cp.cumulative_lake_capture_low_kilograms.as_ref().unwrap()[r],
                        f64::MAX,
                    )?),
                    2 => Some(CompensatedStock::new(
                        cp.cumulative_runoff_transfers[r].terminal_delivery,
                        0.,
                        f64::MAX,
                    )?),
                    3 => Some(CompensatedStock::new(
                        cp.cumulative_runoff_transfers[r].terminal_evaporation,
                        0.,
                        f64::MAX,
                    )?),
                    _ => None,
                };
                if let Some(actual) = actual {
                    let difference =
                        total_mass(&[value.high - actual.high, value.low - actual.low]);
                    if difference > 1e-12 * actual.high.max(1.) {
                        return Err(
                            "Common-sill lifecycle flow exceeds the physical gross flow.".into(),
                        );
                    }
                }
            }
        }
        for parent in &parent_state.parents {
            self.groups
                .binary_search_by_key(&parent.basin_node, |g| g.description.basin_node)
                .map_err(|_| "Common-sill frontier contains an ineligible parent.")?;
        }
        let mut maximum = (0., 0);
        for (g, group) in self.groups.iter().enumerate() {
            let active = self.active(cp, g);
            let merged = history.merge_counts[g];
            let split = history.split_counts[g];
            if split > merged
                || merged - split != u64::from(active.is_some())
                || merged > cp.elapsed_seconds.saturating_mul(2)
                || merged >= (1_u64 << 53)
            {
                return Err(
                    "Common-sill lifecycle counts do not match the owned frontier or clock.".into(),
                );
            }
            let mut terms = Vec::new();
            if let Some(p) = active {
                let parent = &parent_state.parents[p];
                if (parent.birth_high_kilograms, parent.birth_low_kilograms)
                    != (group.birth.high, group.birth.low)
                    || parent.surplus(group.capacity())?.high == 0.
                {
                    return Err(
                        "Common-sill parent has invalid birth contents or an empty common layer."
                            .into(),
                    );
                }
                terms.extend(parent.components());
            }
            for (&r, &cap) in group
                .description
                .child_terminals
                .iter()
                .zip(&group.description.child_capacities_kilograms)
            {
                if active.is_some() {
                    if cp.terminal_water_kilograms[r] != 0.
                        || cp.terminal_low_kilograms.as_ref().unwrap()[r] != 0.
                    {
                        return Err("Common-sill child duplicates its parent's ownership.".into());
                    }
                    let queue = cp
                        .leaf_spill_state
                        .as_ref()
                        .unwrap()
                        .pending_input
                        .stock(r)?;
                    terms.extend([queue.high, queue.low]);
                }
                let inflow = capacity_flow(cap, merged)?;
                let outflow = capacity_flow(cap, split)?;
                let pending = history.pending_to_parent.stock(r)?;
                let delivery = history.delivery_to_parent.stock(r)?;
                terms.extend([
                    -inflow.high,
                    -inflow.low,
                    outflow.high,
                    outflow.low,
                    -pending.high,
                    -pending.low,
                    -delivery.high,
                    -delivery.low,
                ]);
            }
            for &r in &group.description.regions {
                let capture = history.capture_by_parent.stock(r)?;
                let evaporation = history.evaporation_from_parent.stock(r)?;
                terms.extend([
                    -capture.high,
                    -capture.low,
                    evaporation.high,
                    evaporation.low,
                ]);
            }
            let scale = total_mass(&terms.iter().map(|v| v.abs()).collect::<Vec<_>>()).max(1.);
            let residual = total_mass(&terms);
            if !scale.is_finite() || !residual.is_finite() {
                return Err("Nonfinite common-sill lifecycle identity.".into());
            }
            if residual.abs() / scale > maximum.0 {
                maximum = (residual.abs() / scale, group.description.child_terminals[0]);
            }
        }
        Ok(maximum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn topology_transfer_counts_match_an_independent_integer_product() {
        for cap in [2_f64.powi(60) - 128., 2_f64.powi(60) + 256.] {
            for count in [0, 1, 3, 10_003, 630_720_000] {
                let flow = capacity_flow(cap, count).unwrap();
                assert_eq!(flow.high as i128 as f64, flow.high);
                assert_eq!(flow.low as i128 as f64, flow.low);
                assert_eq!(
                    flow.high as i128 + flow.low as i128,
                    cap as i128 * count as i128
                );
            }
        }
    }
    #[test]
    fn positive_unrepresented_debits_are_not_accepted_as_evaporation() {
        let donor = CompensatedStock::new(1e20, 1e-300, f64::MAX).unwrap();
        assert!(
            checked_allocation(donor, &[1e-320])
                .unwrap_err()
                .contains("resolution")
        );
    }
}
