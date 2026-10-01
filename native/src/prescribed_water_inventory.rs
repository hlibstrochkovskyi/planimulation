//! Bounded, exact prescribed-runoff additions to generated basin stocks.
//! This does not route overflow, update drainage, or model elapsed time.
use crate::{
    Recipe, World,
    basins::Basins,
    drainage::Drainage,
    exact_initial_accounting::{ExactInitialAccounting, FRACTION_BITS, exact_units},
    initial_water_inventory::frontier,
};
use serde::{Deserialize, Serialize};

pub const INVENTORY_VERSION: &str = "prescribed-water-inventory-1";
const UNITS_PER_CUBIC_METER: i128 = 1_i128 << FRACTION_BITS;
const MAX_JSON_EXACT_STEP: u64 = (1_u64 << 53) - 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BasinStock {
    pub branch: usize,
    /// Decimal string preserves the full i128 value across JSON transports.
    pub volume_units: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub inventory_version: String,
    pub origin: ExactInitialAccounting,
    pub step: u64,
    pub accepted_input_units: String,
    pub stocks: Vec<BasinStock>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub input_units: i128,
    pub affected_branches: Vec<usize>,
}

pub struct PrescribedWaterInventory {
    origin: ExactInitialAccounting,
    initial_total_units: i128,
    step: u64,
    accepted_input_units: i128,
    stocks: Vec<i128>,
    capacities: Vec<Option<i128>>,
    slots_by_region: Vec<Option<usize>>,
    branches: Vec<usize>,
    drainage: Drainage,
    basins: Basins,
}

fn parse_units(value: &str) -> Result<i128, String> {
    let parsed = value
        .parse::<i128>()
        .map_err(|_| "Invalid prescribed-water units.")?;
    if parsed < 0 || parsed.to_string() != value {
        return Err("Invalid prescribed-water units.".into());
    }
    Ok(parsed)
}

impl PrescribedWaterInventory {
    pub fn from_world(world: &World) -> Result<Self, String> {
        let origin = ExactInitialAccounting::from_world(world)?;
        let (branches, owners) = frontier(&world.basins, world.water.level_meters);
        if origin.stocks.len() != branches.len()
            || origin
                .stocks
                .iter()
                .zip(&branches)
                .any(|(stock, &branch)| stock.branch != branch)
        {
            return Err("Exact initial stocks do not match the active basin frontier.".into());
        }
        let mut slots_by_branch = vec![None; world.basins.nodes().len()];
        for (slot, &branch) in branches.iter().enumerate() {
            slots_by_branch[branch] = Some(slot);
        }
        let slots_by_region: Vec<Option<usize>> = world
            .basins
            .region_nodes()
            .iter()
            .map(|&node| {
                owners[node]
                    .map(|owner| {
                        slots_by_branch[owner].ok_or_else(|| "Basin owner has no stock.".into())
                    })
                    .transpose()
            })
            .collect::<Result<_, String>>()?;

        // The authority is the sum of represented regional prisms, just as in
        // exact initial import, not the separately rounded f64 node capacity.
        let mut capacities = branches
            .iter()
            .map(|&branch| {
                world.basins.nodes()[branch]
                    .spill_level_meters
                    .map(|_| 0_i128)
            })
            .collect::<Vec<_>>();
        for (region, &slot) in slots_by_region.iter().enumerate() {
            let Some(slot) = slot else {
                continue;
            };
            let Some(spill) = world.basins.nodes()[branches[slot]].spill_level_meters else {
                continue;
            };
            let depth = (spill - world.terrain.elevation[region]).max(0.);
            let contribution = exact_units(world.surface.areas[region] * depth)?;
            capacities[slot] = Some(
                capacities[slot]
                    .unwrap()
                    .checked_add(contribution)
                    .ok_or("Prescribed-water capacity overflowed.")?,
            );
        }
        let stocks = origin
            .stocks
            .iter()
            .map(|stock| parse_units(&stock.volume_units))
            .collect::<Result<Vec<_>, _>>()?;
        let initial_total_units = parse_units(&origin.exact_total_units)?;
        let initial_stock_sum = stocks.iter().try_fold(0_i128, |sum, &volume| {
            sum.checked_add(volume)
                .ok_or("Prescribed-water initial stock sum overflowed.")
        })?;
        if initial_stock_sum != initial_total_units {
            return Err("Prescribed-water initial stocks do not balance.".into());
        }
        if stocks
            .iter()
            .zip(&capacities)
            .any(|(&stock, &capacity)| capacity.is_some_and(|limit| stock > limit))
        {
            return Err("Initial exact basin stock exceeds its represented capacity.".into());
        }
        Ok(Self {
            origin,
            initial_total_units,
            step: 0,
            accepted_input_units: 0,
            stocks,
            capacities,
            slots_by_region,
            branches,
            drainage: world.drainage.clone(),
            basins: world.basins.clone(),
        })
    }

    pub fn restore(checkpoint: Checkpoint) -> Result<Self, String> {
        if checkpoint.inventory_version != INVENTORY_VERSION {
            return Err("Unsupported prescribed-water inventory version.".into());
        }
        if checkpoint.step > MAX_JSON_EXACT_STEP {
            return Err("Prescribed-water step exceeds exact JSON integer range.".into());
        }
        let world = World::generate(checkpoint.origin.origin_recipe.clone())?;
        let mut state = Self::from_world(&world)?;
        if checkpoint.origin != state.origin || checkpoint.stocks.len() != state.stocks.len() {
            return Err("Prescribed-water checkpoint origin does not match its world.".into());
        }
        let accepted = parse_units(&checkpoint.accepted_input_units)?;
        let mut total = 0_i128;
        for (slot, saved) in checkpoint.stocks.iter().enumerate() {
            if saved.branch != state.branches[slot] {
                return Err("Prescribed-water checkpoint frontier changed.".into());
            }
            let volume = parse_units(&saved.volume_units)?;
            if volume < state.stocks[slot]
                || state.capacities[slot]
                    .is_some_and(|capacity| volume >= capacity && volume > state.stocks[slot])
            {
                return Err(
                    "Prescribed-water checkpoint stock is outside its supported interval.".into(),
                );
            }
            total = total
                .checked_add(volume - state.stocks[slot])
                .ok_or("Prescribed-water checkpoint ledger overflowed.")?;
            state.stocks[slot] = volume;
        }
        if total != accepted || (checkpoint.step == 0 && accepted != 0) {
            return Err("Prescribed-water checkpoint input ledger does not balance.".into());
        }
        state.check_balance(accepted, &state.stocks)?;
        state.step = checkpoint.step;
        state.accepted_input_units = accepted;
        Ok(state)
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            inventory_version: INVENTORY_VERSION.into(),
            origin: self.origin.clone(),
            step: self.step,
            accepted_input_units: self.accepted_input_units.to_string(),
            stocks: self
                .branches
                .iter()
                .zip(&self.stocks)
                .map(|(&branch, &units)| BasinStock {
                    branch,
                    volume_units: units.to_string(),
                })
                .collect(),
        }
    }

    /// Input volumes use the exact initial-accounting unit of 2^-56 m³.
    pub fn apply_runoff(&mut self, prescribed_units: &[i128]) -> Result<Step, String> {
        let routed = self.drainage.route_runoff_units(prescribed_units)?;
        let mut next = self.stocks.clone();
        let mut affected = vec![false; next.len()];
        for (region, &units) in routed.terminal_units.iter().enumerate() {
            if units == 0 {
                continue;
            }
            let slot = self.slots_by_region[region]
                .ok_or("Runoff terminal has no initial active basin owner.")?;
            next[slot] = next[slot]
                .checked_add(units)
                .ok_or("Prescribed-water stock overflowed.")?;
            affected[slot] = true;
        }
        for (slot, &volume) in next.iter().enumerate() {
            if affected[slot] && self.capacities[slot].is_some_and(|capacity| volume >= capacity) {
                return Err("Prescribed runoff reaches an unsupported basin spill limit.".into());
            }
        }
        let accepted = self
            .accepted_input_units
            .checked_add(routed.total_input_units)
            .ok_or("Prescribed-water input ledger overflowed.")?;
        self.check_balance(accepted, &next)?;
        let step = self
            .step
            .checked_add(1)
            .ok_or("Prescribed-water step counter overflowed.")?;
        if step > MAX_JSON_EXACT_STEP {
            return Err("Prescribed-water step exceeds exact JSON integer range.".into());
        }
        self.stocks = next;
        self.accepted_input_units = accepted;
        self.step = step;
        Ok(Step {
            input_units: routed.total_input_units,
            affected_branches: self
                .branches
                .iter()
                .enumerate()
                .filter_map(|(slot, &branch)| affected[slot].then_some(branch))
                .collect(),
        })
    }

    /// Remaining exact capacity before this initial branch's spill threshold.
    /// None denotes the closed planet's root, which has no external spill.
    pub fn remaining_before_spill_units(&self, branch: usize) -> Result<Option<i128>, String> {
        let slot = self
            .branches
            .binary_search(&branch)
            .map_err(|_| "Branch has no active prescribed-water stock.")?;
        Ok(self.capacities[slot].map(|capacity| capacity - self.stocks[slot]))
    }

    fn check_balance(&self, accepted: i128, stocks: &[i128]) -> Result<(), String> {
        let expected = self
            .initial_total_units
            .checked_add(accepted)
            .ok_or("Prescribed-water total ledger overflowed.")?;
        let actual = stocks.iter().try_fold(0_i128, |sum, &volume| {
            sum.checked_add(volume)
                .ok_or("Prescribed-water stock sum overflowed.")
        })?;
        if actual != expected {
            return Err("Prescribed-water total ledger does not balance.".into());
        }
        Ok(())
    }

    /// Approximate level for inspection only. Exact integer stock remains the
    /// physical accounting authority, including sub-display-resolution input.
    pub fn level_meters(&self, branch: usize) -> Result<Option<f64>, String> {
        let slot = self
            .branches
            .binary_search(&branch)
            .map_err(|_| "Branch has no active prescribed-water stock.")?;
        let units = self.stocks[slot];
        if units == 0 {
            return Ok(None);
        }
        let target = units as f64 / UNITS_PER_CUBIC_METER as f64;
        if !target.is_finite() {
            return Err("Prescribed-water level target is not finite.".into());
        }
        let node = &self.basins.nodes()[branch];
        let mut low = node.birth_level_meters;
        let mut high = if let Some(spill) = node.spill_level_meters {
            spill
        } else {
            low + target / node.support_area_square_meters + 1.
        };
        if !high.is_finite() || self.basins.volume_at_level(branch, high)? < target {
            return Err("Prescribed-water level cannot be resolved.".into());
        }
        if self.basins.volume_at_level(branch, low)? >= target {
            return Ok(Some(low));
        }
        for _ in 0..80 {
            let midpoint = low + (high - low) * 0.5;
            if midpoint == low || midpoint == high {
                break;
            }
            if self.basins.volume_at_level(branch, midpoint)? < target {
                low = midpoint;
            } else {
                high = midpoint;
            }
        }
        Ok(Some(low))
    }

    pub fn origin_recipe(&self) -> &Recipe {
        &self.origin.origin_recipe
    }
}
