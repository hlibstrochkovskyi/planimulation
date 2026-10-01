//! Exact prescribed-runoff stocks with bounded, unambiguous sill transfers.
//! This does not update drainage or model elapsed time.
use crate::{
    Recipe, World,
    basins::Basins,
    drainage::Drainage,
    exact_initial_accounting::{ExactInitialAccounting, FRACTION_BITS, exact_units},
    initial_water_inventory::frontier,
    spill_connections::SpillConnections,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const INVENTORY_VERSION: &str = "prescribed-water-inventory-2";
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

/// Approximate render fields derived from exact stocks; never an accounting source.
pub struct WaterDisplay {
    pub depth_meters: Vec<f64>,
    pub surface_levels_meters: Vec<f64>,
    pub body_ids: Vec<u32>,
    pub main_ocean_id: u32,
}

pub struct PrescribedWaterInventory {
    origin: ExactInitialAccounting,
    initial_total_units: i128,
    step: u64,
    accepted_input_units: i128,
    stocks: BTreeMap<usize, i128>,
    capacities: Vec<Option<i128>>,
    initial_branches: Vec<usize>,
    heights: Vec<f64>,
    areas: Vec<f64>,
    enter: Vec<usize>,
    leave: Vec<usize>,
    drainage: Drainage,
    basins: Basins,
    connections: SpillConnections,
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
        let initial_stocks = origin
            .stocks
            .iter()
            .map(|stock| parse_units(&stock.volume_units))
            .collect::<Result<Vec<_>, _>>()?;
        let initial_total_units = parse_units(&origin.exact_total_units)?;
        let initial_stock_sum = initial_stocks.iter().try_fold(0_i128, |sum, &volume| {
            sum.checked_add(volume)
                .ok_or("Prescribed-water initial stock sum overflowed.")
        })?;
        if initial_stock_sum != initial_total_units {
            return Err("Prescribed-water initial stocks do not balance.".into());
        }
        if initial_stocks
            .iter()
            .zip(&capacities)
            .any(|(&stock, &capacity)| capacity.is_some_and(|limit| stock > limit))
        {
            return Err("Initial exact basin stock exceeds its represented capacity.".into());
        }
        let mut enter = vec![0; world.basins.nodes().len()];
        let mut leave = enter.clone();
        let mut stack = vec![(world.basins.root(), false)];
        let mut clock = 0;
        while let Some((branch, closing)) = stack.pop() {
            if closing {
                leave[branch] = clock;
                continue;
            }
            enter[branch] = clock;
            clock += 1;
            stack.push((branch, true));
            for &child in world.basins.nodes()[branch].children.iter().rev() {
                stack.push((child, false));
            }
        }
        let connections = SpillConnections::build(&world.surface, &world.terrain.elevation)?;
        if connections.basins() != &world.basins {
            return Err("Spill passages do not match the generated basin hierarchy.".into());
        }
        let mut capacity_by_branch = vec![None; world.basins.nodes().len()];
        for (slot, &branch) in branches.iter().enumerate() {
            capacity_by_branch[branch] = capacities[slot];
        }
        Ok(Self {
            origin,
            initial_total_units,
            step: 0,
            accepted_input_units: 0,
            stocks: branches.iter().copied().zip(initial_stocks).collect(),
            capacities: capacity_by_branch,
            initial_branches: branches,
            heights: world.terrain.elevation.clone(),
            areas: world.surface.areas.clone(),
            enter,
            leave,
            drainage: world.drainage.clone(),
            basins: world.basins.clone(),
            connections,
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
        if checkpoint.origin != state.origin || checkpoint.stocks.is_empty() {
            return Err("Prescribed-water checkpoint origin does not match its world.".into());
        }
        let accepted = parse_units(&checkpoint.accepted_input_units)?;
        let mut saved_stocks = BTreeMap::new();
        let mut previous = None;
        for saved in &checkpoint.stocks {
            if saved.branch >= state.basins.nodes().len()
                || previous.is_some_and(|branch| saved.branch <= branch)
            {
                return Err("Prescribed-water checkpoint frontier is not canonical.".into());
            }
            previous = Some(saved.branch);
            let volume = parse_units(&saved.volume_units)?;
            saved_stocks.insert(saved.branch, volume);
        }
        for (&branch, &stock) in &saved_stocks {
            if saved_stocks
                .keys()
                .any(|&other| other != branch && state.is_descendant(other, branch))
            {
                return Err("Prescribed-water checkpoint has overlapping branches.".into());
            }
            let initial = state
                .origin
                .stocks
                .iter()
                .find(|entry| entry.branch == branch)
                .map(|entry| parse_units(&entry.volume_units))
                .transpose()?;
            let minimum = match initial {
                Some(initial) => initial,
                None => state.birth_capacity_units(branch)?,
            };
            if stock < minimum
                || state
                    .capacity_units(branch)?
                    .is_some_and(|capacity| stock > capacity)
            {
                return Err("Prescribed-water checkpoint stock exceeds its basin bounds.".into());
            }
        }
        for &initial in &state.initial_branches {
            let mut current = Some(initial);
            while let Some(branch) = current {
                if saved_stocks.contains_key(&branch) {
                    break;
                }
                current = state.basins.nodes()[branch].parent;
            }
            if current.is_none() {
                return Err(
                    "Prescribed-water checkpoint does not cover the initial frontier.".into(),
                );
            }
        }
        if saved_stocks.keys().any(|&branch| {
            !state
                .initial_branches
                .iter()
                .any(|&initial| state.is_descendant(branch, initial))
        }) {
            return Err("Prescribed-water checkpoint contains an alien branch.".into());
        }
        if checkpoint.step == 0 && (accepted != 0 || saved_stocks != state.stocks) {
            return Err("Prescribed-water checkpoint input ledger does not balance.".into());
        }
        state.check_balance(accepted, &saved_stocks)?;
        state.stocks = saved_stocks;
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
                .stocks
                .iter()
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
        let mut capacities = self.capacities.clone();
        let mut direct = BTreeMap::<usize, i128>::new();
        for (region, &units) in routed.terminal_units.iter().enumerate() {
            if units == 0 {
                continue;
            }
            let branch = self
                .owner_of_region(region, &next)
                .ok_or("Runoff terminal has no active basin owner.")?;
            let entry = direct.entry(branch).or_default();
            *entry = entry
                .checked_add(units)
                .ok_or("Prescribed-water direct input overflowed.")?;
        }
        let mut spilling = Vec::new();
        let mut affected = BTreeSet::new();
        for (&branch, &units) in &direct {
            let previous = next[&branch];
            let volume = previous
                .checked_add(units)
                .ok_or("Prescribed-water stock overflowed.")?;
            next.insert(branch, volume);
            affected.insert(branch);
            if self
                .cached_capacity_units(branch, &mut capacities)?
                .is_some_and(|capacity| volume >= capacity)
            {
                spilling.push(branch);
            }
        }
        if spilling.len() > 1 {
            return Err("Simultaneous spill sources need an allocation policy.".into());
        }
        if let Some(branch) = spilling.pop() {
            self.resolve_spill(branch, &mut next, &mut capacities, &mut affected)?;
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
        self.capacities = capacities;
        self.accepted_input_units = accepted;
        self.step = step;
        Ok(Step {
            input_units: routed.total_input_units,
            affected_branches: affected.into_iter().collect(),
        })
    }

    /// Remaining exact capacity before this initial branch's spill threshold.
    /// None denotes the closed planet's root, which has no external spill.
    pub fn remaining_before_spill_units(&self, branch: usize) -> Result<Option<i128>, String> {
        let stock = self
            .stocks
            .get(&branch)
            .ok_or("Branch has no active prescribed-water stock.")?;
        Ok(self
            .capacity_units(branch)?
            .map(|capacity| capacity - stock))
    }

    fn is_descendant(&self, ancestor: usize, descendant: usize) -> bool {
        self.enter[ancestor] <= self.enter[descendant]
            && self.enter[descendant] < self.leave[ancestor]
    }

    fn owner_of_region(&self, region: usize, stocks: &BTreeMap<usize, i128>) -> Option<usize> {
        let mut current = Some(*self.basins.region_nodes().get(region)?);
        while let Some(branch) = current {
            if stocks.contains_key(&branch) {
                return Some(branch);
            }
            current = self.basins.nodes()[branch].parent;
        }
        None
    }

    fn capacity_units(&self, branch: usize) -> Result<Option<i128>, String> {
        let spill = match self.basins.nodes()[branch].spill_level_meters {
            Some(spill) => spill,
            None => return Ok(None),
        };
        if let Some(cached) = self.capacities[branch] {
            return Ok(Some(cached));
        }
        let mut total = 0_i128;
        for (region, &node) in self.basins.region_nodes().iter().enumerate() {
            if self.is_descendant(branch, node) {
                let depth = (spill - self.heights[region]).max(0.);
                total = total
                    .checked_add(exact_units(self.areas[region] * depth)?)
                    .ok_or("Prescribed-water capacity overflowed.")?;
            }
        }
        Ok(Some(total))
    }

    fn cached_capacity_units(
        &self,
        branch: usize,
        capacities: &mut [Option<i128>],
    ) -> Result<Option<i128>, String> {
        if capacities[branch].is_none() {
            capacities[branch] = self.capacity_units(branch)?;
        }
        Ok(capacities[branch])
    }

    fn birth_capacity_units(&self, branch: usize) -> Result<i128, String> {
        let children = &self.basins.nodes()[branch].children;
        if children.is_empty() {
            return Err("A leaf cannot be restored as a newly merged basin.".into());
        }
        children.iter().try_fold(0_i128, |total, &child| {
            total
                .checked_add(
                    self.capacity_units(child)?
                        .ok_or("Merged child has no spill capacity.")?,
                )
                .ok_or_else(|| "Prescribed-water merge capacity overflowed.".into())
        })
    }

    fn resolve_spill(
        &self,
        mut source: usize,
        stocks: &mut BTreeMap<usize, i128>,
        capacities: &mut [Option<i128>],
        affected: &mut BTreeSet<usize>,
    ) -> Result<(), String> {
        for _ in 0..self.basins.nodes().len() * 2 {
            let Some(limit) = self.cached_capacity_units(source, capacities)? else {
                return Ok(());
            };
            let volume = stocks[&source];
            if volume < limit {
                return Ok(());
            }
            let excess = volume - limit;
            stocks.insert(source, limit);
            let parent = self.basins.nodes()[source]
                .parent
                .ok_or("A closed root cannot spill.")?;
            let children = &self.basins.nodes()[parent].children;
            let mut all_full = true;
            for &child in children {
                let Some(&stock) = stocks.get(&child) else {
                    all_full = false;
                    break;
                };
                if Some(stock) != self.cached_capacity_units(child, capacities)? {
                    all_full = false;
                    break;
                }
            }
            if all_full {
                let mut merged = excess;
                for &child in children {
                    merged = merged
                        .checked_add(stocks.remove(&child).unwrap())
                        .ok_or("Prescribed-water merged stock overflowed.")?;
                }
                stocks.insert(parent, merged);
                affected.insert(parent);
                source = parent;
                continue;
            }
            if excess == 0 {
                return Ok(());
            }
            let mut receivers = BTreeSet::new();
            for candidate in self.connections.receivers(source)? {
                for plateau in candidate.plateaus {
                    for contact in &self.connections.plateaus()[plateau].contacts {
                        if contact.child_branch != candidate.branch {
                            continue;
                        }
                        let receiver = self
                            .owner_of_region(contact.edge[1] as usize, stocks)
                            .ok_or("Spill contact has no active receiving basin.")?;
                        if receiver != source
                            && self
                                .cached_capacity_units(receiver, capacities)?
                                .is_none_or(|capacity| stocks[&receiver] < capacity)
                        {
                            receivers.insert(receiver);
                        }
                    }
                }
            }
            if receivers.len() != 1 {
                return Err(if receivers.is_empty() {
                    "Spill has no open geographic receiver."
                } else {
                    "Spill has multiple geographic receivers."
                }
                .into());
            }
            let receiver = *receivers.first().unwrap();
            stocks.insert(
                receiver,
                stocks[&receiver]
                    .checked_add(excess)
                    .ok_or("Prescribed-water receiving stock overflowed.")?,
            );
            affected.insert(receiver);
            source = receiver;
        }
        Err("Prescribed-water spill exceeded its finite transition bound.".into())
    }

    fn check_balance(&self, accepted: i128, stocks: &BTreeMap<usize, i128>) -> Result<(), String> {
        let expected = self
            .initial_total_units
            .checked_add(accepted)
            .ok_or("Prescribed-water total ledger overflowed.")?;
        let actual = stocks.values().try_fold(0_i128, |sum, &volume| {
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
        let units = *self
            .stocks
            .get(&branch)
            .ok_or("Branch has no active prescribed-water stock.")?;
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

    pub fn runoff_branch(&self, region: usize) -> Result<usize, String> {
        let terminal = *self
            .drainage
            .outlets
            .get(region)
            .ok_or("Runoff source is outside the generated world.")?
            as usize;
        self.owner_of_region(terminal, &self.stocks)
            .ok_or_else(|| "Runoff terminal has no active basin owner.".into())
    }

    /// The display can lose sub-resolution water, but its input stock cannot.
    pub fn display(&self, world: &World) -> Result<WaterDisplay, String> {
        if world.recipe != self.origin.origin_recipe
            || world.basins != self.basins
            || world.terrain.elevation != self.heights
            || world.surface.areas != self.areas
        {
            return Err("Water display requires its original generated world.".into());
        }
        let mut levels = BTreeMap::new();
        for &branch in self.stocks.keys() {
            levels.insert(branch, self.level_meters(branch)?);
        }
        let count = self.heights.len();
        let mut depth_meters = vec![0.; count];
        let mut surface_levels_meters = self.heights.clone();
        for region in 0..count {
            if let Some(branch) = self.owner_of_region(region, &self.stocks)
                && let Some(level) = levels[&branch]
            {
                let depth = (level - self.heights[region]).max(0.);
                if !depth.is_finite() {
                    return Err("Water display depth is not finite.".into());
                }
                depth_meters[region] = depth;
                if depth > 0. {
                    surface_levels_meters[region] = level;
                }
            }
        }
        let mut body_ids = vec![0_u32; count];
        let mut largest_area = 0.;
        let mut main_ocean_id = 0;
        let mut next_id = 0_u32;
        let mut queue = Vec::new();
        for start in 0..count {
            if depth_meters[start] == 0. || body_ids[start] != 0 {
                continue;
            }
            next_id = next_id
                .checked_add(1)
                .ok_or("Too many displayed water bodies.")?;
            queue.clear();
            queue.push(start);
            body_ids[start] = next_id;
            let mut cursor = 0;
            let mut area = 0.;
            while cursor < queue.len() {
                let region = queue[cursor];
                cursor += 1;
                area += self.areas[region];
                for index in world.surface.offsets[region] as usize
                    ..world.surface.offsets[region + 1] as usize
                {
                    let neighbor = world.surface.neighbors[index] as usize;
                    if depth_meters[neighbor] > 0. && body_ids[neighbor] == 0 {
                        body_ids[neighbor] = next_id;
                        queue.push(neighbor);
                    }
                }
            }
            if area > largest_area {
                largest_area = area;
                main_ocean_id = next_id;
            }
        }
        Ok(WaterDisplay {
            depth_meters,
            surface_levels_meters,
            body_ids,
            main_ocean_id,
        })
    }
}
