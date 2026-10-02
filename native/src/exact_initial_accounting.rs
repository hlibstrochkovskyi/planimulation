//! Opt-in, read-only exact accounting of represented initial-water contributions.
//! This is not a dynamic spill solver or a full simulation checkpoint.
use crate::{
    Recipe, World,
    initial_water_inventory::{InitialWaterInventory, frontier},
};
use serde::{Deserialize, Serialize};

pub const ACCOUNTING_VERSION: &str = "exact-initial-accounting-1";
pub const FRACTION_BITS: u32 = 56;
const UNIT_DENOMINATOR: i128 = 1_i128 << FRACTION_BITS;
const FLOAT_SIGNIFICAND_DENOMINATOR: i128 = 1_i128 << 53;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExactInitialStock {
    pub branch: usize,
    /// Decimal strings avoid JSON's binary64 number range and preserve i128.
    pub volume_units: String,
    pub legacy_stock_units: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExactInitialAccounting {
    pub accounting_version: String,
    pub unit_fraction_bits: u32,
    pub origin_recipe: Recipe,
    pub source_total_units: String,
    pub exact_total_units: String,
    /// Exact regional sum minus the independently rounded legacy total.
    pub reconciliation_units: String,
    pub stocks: Vec<ExactInitialStock>,
}

fn canonical_units(value: &str, signed: bool) -> Result<i128, String> {
    let parsed = value
        .parse::<i128>()
        .map_err(|_| "Invalid exact water units.")?;
    if parsed.to_string() != value || (!signed && parsed < 0) {
        return Err("Invalid exact water units.".into());
    }
    Ok(parsed)
}

/// Accept only nonnegative finite binary64 values exactly divisible by 2^-56 m³.
pub fn exact_units(volume_cubic_meters: f64) -> Result<i128, String> {
    if !volume_cubic_meters.is_finite() || volume_cubic_meters.is_sign_negative() {
        return Err("Invalid exact initial-water volume.".into());
    }
    if volume_cubic_meters == 0. {
        return Ok(0);
    }
    let bits = volume_cubic_meters.to_bits();
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if exponent_bits == 0 {
        (fraction, -1022)
    } else {
        ((1_u64 << 52) | fraction, exponent_bits - 1023)
    };
    let shift = exponent - 52 + FRACTION_BITS as i32;
    if shift >= 0 {
        let shift = shift as u32;
        if shift >= 127 || (significand as u128) > ((i128::MAX as u128) >> shift) {
            return Err("Exact initial-water units exceed i128.".into());
        }
        Ok((significand as i128) << shift)
    } else {
        let right = (-shift) as u32;
        if right >= 64 || significand & ((1_u64 << right) - 1) != 0 {
            return Err("Initial-water volume is below the exact unit.".into());
        }
        Ok((significand >> right) as i128)
    }
}

/// For n positive represented contributions, round-to-nearest sequential sum
/// error is at most gamma_(n-1) * exact_sum. The supported n has n*u < 1/2,
/// so 2*n*u*exact_sum is a conservative integer-unit upper bound.
fn summation_bound_units(exact_sum: i128, contributions: usize) -> Result<i128, String> {
    let factor = i128::try_from(contributions)
        .ok()
        .and_then(|count| count.checked_mul(2))
        .ok_or("Initial-water summation bound overflowed.")?;
    if factor >= FLOAT_SIGNIFICAND_DENOMINATOR {
        return Err("Too many initial-water contributions for the summation bound.".into());
    }
    let whole = (exact_sum / FLOAT_SIGNIFICAND_DENOMINATOR)
        .checked_mul(factor)
        .ok_or("Initial-water summation bound overflowed.")?;
    let remainder = (exact_sum % FLOAT_SIGNIFICAND_DENOMINATOR)
        .checked_mul(factor)
        .and_then(|part| part.checked_add(FLOAT_SIGNIFICAND_DENOMINATOR - 1))
        .ok_or("Initial-water summation bound overflowed.")?
        / FLOAT_SIGNIFICAND_DENOMINATOR;
    whole
        .checked_add(remainder)
        .ok_or_else(|| "Initial-water summation bound overflowed.".into())
}

impl ExactInitialAccounting {
    pub fn from_world(world: &World) -> Result<Self, String> {
        world.recipe.validate()?;
        let inventory = InitialWaterInventory::from_world(world)?;
        let (branches, owners) = frontier(&world.basins, world.water.level_meters);
        if branches.len() != inventory.stocks.len() {
            return Err("Initial-water frontier changed during exact import.".into());
        }
        let mut slots = vec![None; world.basins.nodes().len()];
        for (slot, &branch) in branches.iter().enumerate() {
            slots[branch] = Some(slot);
        }
        let mut stocks = vec![0_i128; branches.len()];
        let mut counts = vec![0_usize; branches.len()];
        let mut total = 0_i128;
        let mut total_count = 0_usize;
        for (region, &depth) in world.water.depth_meters.iter().enumerate() {
            if depth == 0. {
                continue;
            }
            let contribution = world.surface.areas[region] * depth;
            if !contribution.is_finite() || contribution <= 0. {
                return Err("Wet region has no representable initial-water contribution.".into());
            }
            let units = exact_units(contribution)?;
            let branch = owners[world.basins.region_nodes()[region]]
                .ok_or("Wet region has no exact initial-water owner.")?;
            let slot = slots[branch].ok_or("Wet region has no active exact-water stock.")?;
            stocks[slot] = stocks[slot]
                .checked_add(units)
                .ok_or("Exact initial-water stock overflowed.")?;
            counts[slot] += 1;
            total = total
                .checked_add(units)
                .ok_or("Exact initial-water total overflowed.")?;
            total_count += 1;
        }
        let mut exact_stocks = Vec::with_capacity(stocks.len());
        for (slot, legacy) in inventory.stocks.iter().enumerate() {
            let legacy_units = exact_units(legacy.volume_cubic_meters)?;
            let difference = stocks[slot]
                .checked_sub(legacy_units)
                .ok_or("Exact initial-water stock difference overflowed.")?;
            if difference.abs() > summation_bound_units(stocks[slot], counts[slot])? {
                return Err("Legacy initial-water stock exceeds its summation bound.".into());
            }
            exact_stocks.push(ExactInitialStock {
                branch: legacy.branch,
                volume_units: stocks[slot].to_string(),
                legacy_stock_units: legacy_units.to_string(),
            });
        }
        let source_total = exact_units(inventory.initial_volume_cubic_meters)?;
        let reconciliation = total
            .checked_sub(source_total)
            .ok_or("Exact initial-water reconciliation overflowed.")?;
        if reconciliation.abs() > summation_bound_units(total, total_count)? {
            return Err("Legacy initial-water total exceeds its summation bound.".into());
        }
        Ok(Self {
            accounting_version: ACCOUNTING_VERSION.into(),
            unit_fraction_bits: FRACTION_BITS,
            origin_recipe: world.recipe.clone(),
            source_total_units: source_total.to_string(),
            exact_total_units: total.to_string(),
            reconciliation_units: reconciliation.to_string(),
            stocks: exact_stocks,
        })
    }

    /// Regenerate pinned geometry and audit the entire imported ledger before use.
    pub fn restore(self) -> Result<Self, String> {
        if self.accounting_version != ACCOUNTING_VERSION || self.unit_fraction_bits != FRACTION_BITS
        {
            return Err("Unsupported exact initial-water accounting version.".into());
        }
        self.origin_recipe.validate()?;
        let source = canonical_units(&self.source_total_units, false)?;
        let total = canonical_units(&self.exact_total_units, false)?;
        let reconciliation = canonical_units(&self.reconciliation_units, true)?;
        if total.checked_sub(source) != Some(reconciliation) || self.stocks.len() > 40_962 {
            return Err("Invalid exact initial-water ledger.".into());
        }
        let mut stock_total = 0_i128;
        let mut last_branch = None;
        for stock in &self.stocks {
            if last_branch.is_some_and(|last| stock.branch <= last) {
                return Err("Invalid exact initial-water stock order.".into());
            }
            last_branch = Some(stock.branch);
            stock_total = stock_total
                .checked_add(canonical_units(&stock.volume_units, false)?)
                .ok_or("Exact initial-water stock total overflowed.")?;
            canonical_units(&stock.legacy_stock_units, false)?;
        }
        if stock_total != total {
            return Err("Exact initial-water stocks do not balance.".into());
        }
        let generated = World::generate(self.origin_recipe.clone())?;
        if self != Self::from_world(&generated)? {
            return Err("Exact initial-water ledger does not match its generating recipe.".into());
        }
        Ok(self)
    }

    pub fn exact_total_cubic_meters(&self) -> Result<f64, String> {
        Ok(canonical_units(&self.exact_total_units, false)? as f64 / UNIT_DENOMINATOR as f64)
    }
}
