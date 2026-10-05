//! Bounded unique-edge arrivals into an already active one-level parent.
//! Supply head and next-sill storage are ceilings, never fictitious outlets.
use super::super::{Checkpoint as SeasonalCheckpoint, leaf_spill};
use super::{Layout, frontier};
use crate::surface_water::CompensatedStock;

pub const PARENT_MODEL_VERSION: &str = "common-sill-receiver-3";
pub const LIFECYCLE_MODEL_VERSION: &str = "common-sill-lifecycle-2";

impl Layout {
    /// Caller retains the unique geographic edge and publishes its actual grants.
    /// This receiver neither reroutes input nor activates an inactive parent.
    pub fn receive_spill(
        &self,
        cp: &mut SeasonalCheckpoint,
        terminal: usize,
        supplying_head: f64,
        donor: &mut CompensatedStock,
    ) -> Result<Vec<f64>, String> {
        if !self.receiving_enabled {
            return Err(
                "Leaf spill into an active merged parent requires a receiving-frontier policy."
                    .into(),
            );
        }
        let g =
            self.by_terminal[terminal].ok_or("Parent spill receiver is not a child terminal.")?;
        let p = self
            .active(cp, g)
            .ok_or("Parent spill receiver is not active.")?;
        let group = &self.groups[g];
        let height = supplying_head - group.description.birth_level_meters;
        if !supplying_head.is_finite() || !height.is_finite() || height <= 0. {
            return Err("Parent spill receiver is backpressured at its birth sill.".into());
        }
        let bounded_height = match group.curve.capacity_cubic_meters() {
            Some(cap) => height.min(
                group
                    .curve
                    .level_for_volume(cap)?
                    .ok_or("Missing parent boundary level.")?,
            ),
            None => height,
        };
        let supplied_capacity = group.curve.volume_at_level(bounded_height)? * 1000.;
        let capacity = supplied_capacity.min(group.capacity());
        if !capacity.is_finite() || capacity <= 0. {
            return Err("Parent spill receiving-head capacity is unrepresentable.".into());
        }
        // Validate the actual pair, not a rounded displayed level.
        let parent = &cp.merged_lake_state.as_ref().unwrap().parents[p];
        let mut recipient = CompensatedStock::new(
            parent.surplus_high_kilograms,
            parent.surplus_low_kilograms,
            capacity,
        )
        .map_err(|_| "Parent spill receiver is above the supplying head.")?;
        let mut remaining = *donor;
        let grants = leaf_spill::fill(&mut remaining, &mut recipient, capacity)?;
        if remaining.high != 0. || remaining.low != 0. {
            return Err("Parent spill exceeds the supplying-head or next-sill capacity; backpressure/nested receiving remains unsupported.".into());
        }
        let ledger = frontier::state_mut(cp)
            .spill_to_parent
            .as_mut()
            .ok_or("Missing parent spill provenance.")?;
        for &grant in &grants {
            ledger.credit(terminal, grant)?;
        }
        cp.merged_lake_state.as_mut().unwrap().parents[p].set_surplus(recipient);
        *donor = remaining;
        Ok(grants)
    }
}
