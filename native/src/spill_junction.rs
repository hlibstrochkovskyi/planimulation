//! One closed shared-sill junction, not general multiway/nested hydrology.
use crate::nested_reservoir::{Geometry, Stock};
use crate::reservoir::{Boundary, Reservoir, add, checkpoint_number, tolerance};
use crate::spill_connections::SpillConnections;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const EXPERIMENT_VERSION: &str = "spill-junction-1";
pub const POLICY_VERSION: &str = "receiver-weighted-capped-1";

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Weight {
    pub branch: usize,
    /// Prescribed dimensionless receiver preference, not inferred conductance.
    #[serde(deserialize_with = "checkpoint_number")]
    pub weight: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Setup {
    pub geometry: Geometry,
    pub policy_version: String,
    /// Exactly one positive weight per leaf, in analysis child order.
    pub weights: Vec<Weight>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum Storage {
    Separate(Vec<Stock>),
    Merged(Stock),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inventory {
    pub storage: Storage,
    #[serde(deserialize_with = "checkpoint_number")]
    pub input_cubic_meters: f64,
    pub pulse_count: u64,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub experiment_version: String,
    pub setup: Setup,
    pub inventory: Inventory,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub phase: &'static str,
    pub storage: Storage,
    /// One per separate leaf, or one root level when merged; null when dry.
    pub levels_meters: Vec<Option<f64>>,
    pub total_stored_cubic_meters: f64,
    pub storage_residual_cubic_meters: f64,
    pub budget_residual_cubic_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pulse {
    pub index: u64,
    pub inputs: Vec<Stock>,
    pub local_retained: Vec<Stock>,
    pub spill_supplied: Vec<Stock>,
    pub spill_received: Vec<Stock>,
    pub input_to_common_storage_cubic_meters: f64,
    pub snapshot: Snapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpillJunction {
    checkpoint: Checkpoint,
    connections: SpillConnections,
    plateau: usize,
    children: Vec<usize>,
    curves: Vec<Reservoir>,
    common: Reservoir,
    capacities: Vec<f64>,
    connection_volume: f64,
    sill: f64,
}

fn sum(values: impl IntoIterator<Item = f64>) -> Result<f64, String> {
    values.into_iter().try_fold(0., add)
}

/// Weighted water filling in volume space. Each pass saturates a receiver or
/// completes allocation; at most n passes, no convergence tolerance loop.
fn allocate(deficits: &[f64], weights: &[Weight], budget: f64) -> Result<Vec<f64>, String> {
    let mut result = vec![0.; deficits.len()];
    if budget == 0. {
        return Ok(result);
    }
    if budget >= sum(deficits.iter().copied())? {
        return Ok(deficits.to_vec());
    }
    let mut active: Vec<_> = (0..deficits.len()).filter(|&i| deficits[i] > 0.).collect();
    let mut remaining = budget;
    for _ in 0..deficits.len() {
        if active.is_empty() {
            return Err("Allocation has a remainder but no receiving capacity.".into());
        }
        let max_weight = active.iter().map(|&i| weights[i].weight).fold(0., f64::max);
        let weight_sum = sum(active.iter().map(|&i| weights[i].weight / max_weight))?;
        let share = |i: usize| remaining * (weights[i].weight / max_weight) / weight_sum;
        let capped: Vec<_> = active
            .iter()
            .copied()
            .filter(|&i| deficits[i] <= share(i))
            .collect();
        if capped.is_empty() {
            // The final (largest branch ID) receiver takes only the bounded
            // arithmetic remainder; it is not a priority recipient.
            let mut rest = remaining;
            for (position, &i) in active.iter().enumerate() {
                let expected = share(i);
                let grant = if position + 1 == active.len() {
                    rest
                } else {
                    expected
                };
                if !grant.is_finite()
                    || grant <= 0.
                    || grant > deficits[i]
                    || grant > rest
                    || (grant - expected).abs() > tolerance(remaining)
                {
                    return Err("Junction allocation is below numeric precision.".into());
                }
                result[i] = grant;
                rest -= grant;
            }
            return Ok(result);
        }
        let consumed = sum(capped.iter().map(|&i| deficits[i]))?;
        if consumed <= 0. || consumed > remaining || remaining - consumed >= remaining {
            return Err("Junction capped allocation cannot make numeric progress.".into());
        }
        for i in capped {
            result[i] = deficits[i];
        }
        remaining -= consumed;
        active.retain(|&i| result[i] == 0.);
        if remaining == 0. {
            return Ok(result);
        }
    }
    Err("Junction allocation exceeded its finite receiver bound.".into())
}

impl SpillJunction {
    pub fn new(setup: Setup) -> Result<Self, String> {
        let mut result = Self::prepare(Checkpoint {
            experiment_version: EXPERIMENT_VERSION.into(),
            setup,
            inventory: Inventory {
                storage: Storage::Separate(vec![]),
                input_cubic_meters: 0.,
                pulse_count: 0,
            },
        })?;
        result.checkpoint.inventory.storage =
            Storage::Separate(result.stocks(&vec![0.; result.children.len()]));
        result.snapshot()?;
        Ok(result)
    }
    pub fn restore(checkpoint: Checkpoint) -> Result<Self, String> {
        let result = Self::prepare(checkpoint)?;
        result.snapshot()?;
        Ok(result)
    }
    fn prepare(checkpoint: Checkpoint) -> Result<Self, String> {
        if checkpoint.experiment_version != EXPERIMENT_VERSION
            || checkpoint.setup.policy_version != POLICY_VERSION
        {
            return Err("Unsupported junction experiment or allocation policy version.".into());
        }
        let setup = &checkpoint.setup;
        let surface = setup.geometry.surface()?;
        let heights: Vec<_> = setup
            .geometry
            .columns
            .iter()
            .map(|c| c.bed_meters)
            .collect();
        let connections = SpillConnections::build(&surface, &heights)?;
        let b = connections.basins();
        let root = b.root();
        let children = b.nodes()[root].children.clone();
        if children.len() < 2 || children.iter().any(|&c| !b.nodes()[c].children.is_empty()) {
            return Err("Junction requires one root merge of at least two leaf reservoirs.".into());
        }
        // All children must touch ONE connected sill; a same-height chain with
        // an intervening lower bowl is deliberately not a shared junction.
        let connectors: Vec<_> = connections
            .plateaus()
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.contacts
                    .iter()
                    .map(|c| c.child_branch)
                    .collect::<BTreeSet<_>>()
                    .len()
                    > 1
            })
            .collect();
        if connectors.len() != 1 {
            return Err("Separate or alternative sill plateaus are not a shared junction.".into());
        }
        let plateau = connectors[0].0;
        let contacts: Vec<_> = connectors[0]
            .1
            .contacts
            .iter()
            .map(|c| c.child_branch)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if contacts != children {
            return Err("Every leaf must directly contact the shared sill.".into());
        }
        if setup.weights.len() != children.len()
            || setup
                .weights
                .iter()
                .zip(&children)
                .any(|(w, &id)| w.branch != id || !w.weight.is_finite() || w.weight <= 0.)
        {
            return Err(
                "Specify one finite positive receiver weight per ordered leaf branch.".into(),
            );
        }
        let max_weight = setup.weights.iter().map(|w| w.weight).fold(0., f64::max);
        if setup.weights.iter().any(|w| w.weight / max_weight == 0.) {
            return Err("Receiver weight ratio is below numeric precision.".into());
        }
        let sill = b.nodes()[root].birth_level_meters;
        let mut curves = Vec::new();
        let mut capacities = Vec::new();
        for &child in &children {
            let columns = setup
                .geometry
                .columns
                .iter()
                .enumerate()
                .filter(|(r, _)| b.region_nodes()[*r] == child)
                .map(|(_, c)| c.clone())
                .collect();
            let curve = Reservoir::new(
                columns,
                Boundary::ExternalCollector {
                    spill_level_meters: sill,
                },
                0.,
            )?;
            capacities.push(curve.capacity_cubic_meters().unwrap());
            curves.push(curve);
        }
        let connection_volume = sum(capacities.iter().copied())?;
        let common = Reservoir::new(setup.geometry.columns.clone(), Boundary::Closed, 0.)?;
        if (common.volume_at_level(sill)? - connection_volume).abs() > tolerance(connection_volume)
        {
            return Err("Junction connection storage is inconsistent.".into());
        }
        Ok(Self {
            checkpoint,
            connections,
            plateau,
            children,
            curves,
            common,
            capacities,
            connection_volume,
            sill,
        })
    }

    pub fn checkpoint(&self) -> Checkpoint {
        self.checkpoint.clone()
    }
    pub fn connections(&self) -> &SpillConnections {
        &self.connections
    }
    pub fn plateau(&self) -> usize {
        self.plateau
    }
    pub fn capacities(&self) -> Vec<Stock> {
        self.stocks(&self.capacities)
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.audit(&self.checkpoint.inventory)
    }
    fn stocks(&self, values: &[f64]) -> Vec<Stock> {
        self.children
            .iter()
            .zip(values)
            .map(|(&branch, &volume_cubic_meters)| Stock {
                branch,
                volume_cubic_meters,
            })
            .collect()
    }
    fn valid_stocks(&self, values: &[Stock]) -> bool {
        values.len() == self.children.len()
            && values.iter().zip(&self.children).all(|(v, &id)| {
                v.branch == id && v.volume_cubic_meters.is_finite() && v.volume_cubic_meters >= 0.
            })
    }
    fn audit(&self, inventory: &Inventory) -> Result<Snapshot, String> {
        if !inventory.input_cubic_meters.is_finite()
            || inventory.input_cubic_meters < 0.
            || (inventory.pulse_count == 0 && inventory.input_cubic_meters != 0.)
        {
            return Err("Invalid junction input ledger.".into());
        }
        let (phase, levels, total, resolved) = match &inventory.storage {
            Storage::Separate(stocks) => {
                if !self.valid_stocks(stocks)
                    || stocks
                        .iter()
                        .zip(&self.capacities)
                        .any(|(v, &cap)| v.volume_cubic_meters > cap)
                    || stocks
                        .iter()
                        .zip(&self.capacities)
                        .all(|(v, &cap)| v.volume_cubic_meters == cap)
                    || (inventory.pulse_count == 0
                        && stocks.iter().any(|v| v.volume_cubic_meters != 0.))
                {
                    return Err("Invalid separate junction inventories.".into());
                }
                let levels: Vec<_> = stocks
                    .iter()
                    .zip(&self.curves)
                    .map(|(v, c)| c.level_for_volume(v.volume_cubic_meters))
                    .collect::<Result<_, _>>()?;
                if levels
                    .iter()
                    .zip(stocks)
                    .zip(&self.capacities)
                    .any(|((h, v), &cap)| {
                        v.volume_cubic_meters < cap && h.is_some_and(|h| h >= self.sill)
                    })
                {
                    return Err("Separate level is below threshold precision.".into());
                }
                let resolved = sum(levels
                    .iter()
                    .zip(&self.curves)
                    .map(|(h, c)| h.map_or(Ok(0.), |h| c.volume_at_level(h)))
                    .collect::<Result<Vec<_>, _>>()?)?;
                (
                    "separate",
                    levels,
                    sum(stocks.iter().map(|v| v.volume_cubic_meters))?,
                    resolved,
                )
            }
            Storage::Merged(stock) => {
                let v = stock.volume_cubic_meters;
                if stock.branch != self.connections.basins().root()
                    || !v.is_finite()
                    || v < self.connection_volume
                    || inventory.pulse_count == 0
                {
                    return Err(
                        "Merged junction requires all leaves filled to the shared sill.".into(),
                    );
                }
                let at_sill = v == self.connection_volume;
                let level = if at_sill {
                    self.sill
                } else {
                    self.common
                        .level_for_volume(v)?
                        .ok_or("Missing common level.")?
                };
                if !at_sill && level <= self.sill {
                    return Err("Common level is below threshold precision.".into());
                }
                (
                    if at_sill { "atSill" } else { "merged" },
                    vec![Some(level)],
                    v,
                    self.common.volume_at_level(level)?,
                )
            }
        };
        let residual = inventory.input_cubic_meters - total;
        if residual.abs() > tolerance(inventory.input_cubic_meters)
            || (resolved - total).abs() > 1e-9_f64.max(total * 1e-10)
        {
            return Err("Junction storage or inventory budget does not balance.".into());
        }
        Ok(Snapshot {
            phase,
            storage: inventory.storage.clone(),
            levels_meters: levels,
            total_stored_cubic_meters: total,
            storage_residual_cubic_meters: resolved - total,
            budget_residual_cubic_meters: residual,
        })
    }

    /// A simultaneous batch in canonical leaf order. No per-source flow
    /// attribution is invented for the pooled, weighted overflow.
    pub fn add_input(&mut self, inputs: &[Stock]) -> Result<Pulse, String> {
        if !self.valid_stocks(inputs) {
            return Err("Input must contain every leaf once in canonical order.".into());
        }
        let input = sum(inputs.iter().map(|v| v.volume_cubic_meters))?;
        let old = &self.checkpoint.inventory;
        let mut next = old.clone();
        next.input_cubic_meters = add(old.input_cubic_meters, input)?;
        next.pulse_count = old
            .pulse_count
            .checked_add(1)
            .ok_or("Junction pulse counter overflowed.")?;
        let n = self.children.len();
        let mut local = vec![0.; n];
        let mut supplied = vec![0.; n];
        let mut received = vec![0.; n];
        let common_input;
        next.storage = match &old.storage {
            Storage::Merged(stock) => {
                common_input = input;
                Storage::Merged(Stock {
                    branch: stock.branch,
                    volume_cubic_meters: add(stock.volume_cubic_meters, input)?,
                })
            }
            Storage::Separate(stocks) => {
                let mut values: Vec<_> = stocks.iter().map(|v| v.volume_cubic_meters).collect();
                for i in 0..n {
                    local[i] = inputs[i]
                        .volume_cubic_meters
                        .min(self.capacities[i] - values[i]);
                    values[i] = add(values[i], local[i])?;
                    supplied[i] = inputs[i].volume_cubic_meters - local[i];
                    if values[i] > self.capacities[i]
                        || (supplied[i] > 0. && values[i] != self.capacities[i])
                    {
                        return Err("Only an exactly saturated junction source can spill.".into());
                    }
                }
                let excess = sum(supplied.iter().copied())?;
                let deficits: Vec<_> = self
                    .capacities
                    .iter()
                    .zip(&values)
                    .map(|(cap, v)| cap - v)
                    .collect();
                let deficit = sum(deficits.iter().copied())?;
                received = allocate(&deficits, &self.checkpoint.setup.weights, excess)?;
                common_input = if excess >= deficit {
                    excess - deficit
                } else {
                    0.
                };
                if (sum(received.iter().copied())? + common_input - excess).abs()
                    > tolerance(excess)
                {
                    return Err("Junction spill pool does not balance.".into());
                }
                for i in 0..n {
                    values[i] = add(values[i], received[i])?;
                }
                if values == self.capacities {
                    Storage::Merged(Stock {
                        branch: self.connections.basins().root(),
                        volume_cubic_meters: add(self.connection_volume, common_input)?,
                    })
                } else {
                    if common_input != 0. {
                        return Err("Unfilled junction cannot hold common surplus.".into());
                    }
                    Storage::Separate(self.stocks(&values))
                }
            }
        };
        let snapshot = self.audit(&next)?;
        if (snapshot.total_stored_cubic_meters - self.snapshot()?.total_stored_cubic_meters - input)
            .abs()
            > tolerance(input)
        {
            return Err("Junction pulse cannot be accounted for precisely.".into());
        }
        let pulse = Pulse {
            index: next.pulse_count,
            inputs: inputs.to_vec(),
            local_retained: self.stocks(&local),
            spill_supplied: self.stocks(&supplied),
            spill_received: self.stocks(&received),
            input_to_common_storage_cubic_meters: common_input,
            snapshot,
        };
        self.checkpoint.inventory = next;
        Ok(pulse)
    }
}
