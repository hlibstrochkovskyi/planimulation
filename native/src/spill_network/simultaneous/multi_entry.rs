//! Explicit branch-then-terminal-entry weighting for concurrent spill networks.
use super::{Interval, SimultaneousNetwork, TransferRate};
use crate::{
    drainage::Drainage,
    nested_reservoir::{Geometry, Input, Inventory, Snapshot},
    reservoir::{add, checkpoint_number, tolerance},
    spill_connections::SpillConnections,
    spill_junction::Weight,
    spill_network::{self, Receiver, SpillNetwork},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const EXPERIMENT_VERSION: &str = "multi-entry-network-1";
pub const POLICY_VERSION: &str = "branch-then-entry-weights-1";

pub mod seeded;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryTarget {
    pub branch: usize,
    pub leaf: usize,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntryWeight {
    pub branch: usize,
    pub leaf: usize,
    #[serde(deserialize_with = "checkpoint_number")]
    pub weight: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Setup {
    pub geometry: Geometry,
    pub policy_version: String,
    pub weights: Vec<Weight>,
    /// One positive weight per distinct (receiving branch, dry terminal leaf),
    /// sorted by that pair. Geometry discovers targets, never implicit weights.
    pub entry_weights: Vec<EntryWeight>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub experiment_version: String,
    pub setup: Setup,
    pub inventory: Inventory,
}

/// Inspect required targets without assuming weights or running a solver.
pub fn entry_targets(geometry: &Geometry) -> Result<Vec<EntryTarget>, String> {
    entry_targets_with_limit(geometry, crate::nested_reservoir::MAX_REGIONS)
}

fn entry_targets_with_limit(
    geometry: &Geometry,
    max_regions: usize,
) -> Result<Vec<EntryTarget>, String> {
    let surface = geometry.surface_with_limit(max_regions)?;
    let heights: Vec<_> = geometry.columns.iter().map(|c| c.bed_meters).collect();
    let connections = SpillConnections::build(&surface, &heights)?;
    let drainage = Drainage::build(&surface, &heights, &vec![0; heights.len()]);
    let leaves: Vec<_> = drainage
        .outlets
        .iter()
        .map(|&r| connections.basins().region_nodes()[r as usize])
        .collect();
    Ok(targets(&connections, &leaves))
}

fn targets(connections: &SpillConnections, leaves: &[usize]) -> Vec<EntryTarget> {
    let mut targets = BTreeSet::new();
    for p in connections.plateaus() {
        if p.contacts
            .iter()
            .all(|c| c.child_branch == p.contacts[0].child_branch)
        {
            continue;
        }
        for c in &p.contacts {
            targets.insert(EntryTarget {
                branch: c.child_branch,
                leaf: leaves[c.edge[1] as usize],
            });
        }
    }
    targets.into_iter().collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultiEntryNetwork {
    model: SimultaneousNetwork,
}

impl Setup {
    fn core(&self) -> Result<spill_network::Setup, String> {
        if self.policy_version != POLICY_VERSION {
            return Err("Unsupported multi-entry policy.".into());
        }
        Ok(spill_network::Setup {
            geometry: self.geometry.clone(),
            policy_version: spill_network::POLICY_VERSION.into(),
            weights: self.weights.clone(),
        })
    }
}

impl MultiEntryNetwork {
    pub fn new(setup: Setup) -> Result<Self, String> {
        let core = SpillNetwork::new_with_entries(setup.core()?, true)?;
        Self::finish(core, setup.entry_weights)
    }
    pub fn restore(checkpoint: Checkpoint) -> Result<Self, String> {
        if checkpoint.experiment_version != EXPERIMENT_VERSION {
            return Err("Unsupported multi-entry experiment.".into());
        }
        let core = SpillNetwork::restore_with_entries(
            spill_network::Checkpoint {
                experiment_version: spill_network::EXPERIMENT_VERSION.into(),
                setup: checkpoint.setup.core()?,
                inventory: checkpoint.inventory,
            },
            true,
        )?;
        Self::finish(core, checkpoint.setup.entry_weights)
    }
    fn finish(core: SpillNetwork, entry_weights: Vec<EntryWeight>) -> Result<Self, String> {
        let required = targets(&core.connections, &core.entry_leaves);
        if entry_weights.len() != required.len()
            || entry_weights.iter().zip(required).any(|(w, t)| {
                w.branch != t.branch || w.leaf != t.leaf || !w.weight.is_finite() || w.weight <= 0.
            })
        {
            return Err(
                "Specify one finite positive entry weight per canonical branch/leaf target.".into(),
            );
        }
        Ok(Self {
            model: SimultaneousNetwork {
                core,
                entry_weights: Some(entry_weights),
                exact_limit_commit: false,
            },
        })
    }
    fn finish_with_exact_limit(
        core: SpillNetwork,
        entry_weights: Vec<EntryWeight>,
    ) -> Result<Self, String> {
        let mut result = Self::finish(core, entry_weights)?;
        result.model.exact_limit_commit = true;
        Ok(result)
    }
    pub fn checkpoint(&self) -> Checkpoint {
        let core = self.model.core.checkpoint();
        Checkpoint {
            experiment_version: EXPERIMENT_VERSION.into(),
            setup: Setup {
                geometry: core.setup.geometry,
                policy_version: POLICY_VERSION.into(),
                weights: core.setup.weights,
                entry_weights: self
                    .model
                    .entry_weights
                    .clone()
                    .expect("Multi-entry model has validated weights."),
            },
            inventory: core.inventory,
        }
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.model.snapshot()
    }
    pub fn connections(&self) -> &SpillConnections {
        self.model.connections()
    }
    pub fn add_interval(&mut self, inputs: Vec<Input>) -> Result<Interval, String> {
        self.model.add_interval(inputs)
    }
    pub fn receivers(&self, source: usize) -> Result<Vec<Receiver>, String> {
        let core = &self.model.core;
        let mut stocks = vec![None; core.capacities.len()];
        for s in &core.checkpoint.inventory.active {
            stocks[s.branch] = Some(s.volume_cubic_meters);
        }
        frontier(core, source, &stocks)
    }
}

/// Deduplicate receiving terminal leaves, not lower contact edges. Underfilled
/// siblings stop transit but must retain EVERY entry on a reachable plateau.
fn frontier(
    core: &SpillNetwork,
    source: usize,
    stocks: &[Option<f64>],
) -> Result<Vec<Receiver>, String> {
    if source >= stocks.len() || !core.full(source, stocks) {
        return Err("Multi-entry source must be a saturated non-root branch.".into());
    }
    let mut seen = vec![false; stocks.len()];
    let mut seen_plateau = vec![false; core.connections.plateaus().len()];
    let mut previous = vec![None; stocks.len()];
    let mut queue = vec![source];
    seen[source] = true;
    let mut found = BTreeMap::new();
    let mut cursor = 0;
    while cursor < queue.len() {
        let from = queue[cursor];
        cursor += 1;
        for &plateau in &core.incident[from] {
            if seen_plateau[plateau] {
                continue;
            }
            seen_plateau[plateau] = true;
            for contact in &core.connections.plateaus()[plateau].contacts {
                let child = contact.child_branch;
                if core.full(child, stocks) {
                    if !seen[child] {
                        seen[child] = true;
                        previous[child] = Some((from, plateau));
                        queue.push(child);
                    }
                    continue;
                }
                let leaf = core.entry_leaves[contact.edge[1] as usize];
                let key = (child, leaf);
                if found.contains_key(&key) {
                    continue;
                }
                let mut transit = vec![from];
                let mut plateaus = vec![plateau];
                let mut id = from;
                while id != source {
                    let (parent, via) = previous[id].ok_or("Broken multi-entry route witness.")?;
                    transit.push(parent);
                    plateaus.push(via);
                    id = parent;
                }
                transit.reverse();
                plateaus.reverse();
                found.insert(
                    key,
                    Receiver {
                        branch: child,
                        entry_leaf: leaf,
                        entry_region: contact.edge[1],
                        transit_branches: transit,
                        plateaus,
                    },
                );
            }
        }
    }
    Ok(found.into_values().collect())
}

fn shares(input: f64, weights: &[f64]) -> Result<Vec<f64>, String> {
    if weights.is_empty() {
        return Err("Missing weighted receiving frontier.".into());
    }
    let max = weights.iter().copied().fold(0., f64::max);
    let normalized: Vec<_> = weights.iter().map(|w| w / max).collect();
    let sum = normalized.iter().try_fold(0., |v, &w| add(v, w))?;
    let mut assigned = 0.;
    let mut result = Vec::new();
    for (i, w) in normalized.iter().enumerate() {
        let share = input * (w / sum);
        let value = if i + 1 == weights.len() {
            input - assigned
        } else {
            share
        };
        if !share.is_finite()
            || share <= 0.
            || !value.is_finite()
            || value <= 0.
            || (value - share).abs() > tolerance(input)
        {
            return Err("Multi-entry rate cannot be represented.".into());
        }
        assigned = add(assigned, value)?;
        result.push(value);
    }
    Ok(result)
}

pub(super) fn allocate(
    core: &SpillNetwork,
    weights: &[EntryWeight],
    source: usize,
    stocks: &[Option<f64>],
    input: f64,
) -> Result<Vec<TransferRate>, String> {
    let mut grouped: BTreeMap<usize, Vec<Receiver>> = BTreeMap::new();
    for receiver in frontier(core, source, stocks)? {
        grouped.entry(receiver.branch).or_default().push(receiver);
    }
    let outer = shares(
        input,
        &grouped
            .keys()
            .map(|&branch| core.checkpoint.setup.weights[branch].weight)
            .collect::<Vec<_>>(),
    )?;
    let mut result = Vec::new();
    for ((_, entries), rate) in grouped.into_iter().zip(outer) {
        let inner_weights: Vec<_> = entries
            .iter()
            .map(|entry| {
                weights
                    .binary_search_by_key(&(entry.branch, entry.entry_leaf), |w| (w.branch, w.leaf))
                    .map(|i| weights[i].weight)
                    .map_err(|_| "Missing reachable entry weight.")
            })
            .collect::<Result<_, _>>()?;
        for (receiver, rate) in entries.into_iter().zip(shares(rate, &inner_weights)?) {
            result.push(TransferRate {
                source_branch: source,
                receiver,
                cubic_meters_per_interval: rate,
            });
        }
    }
    Ok(result)
}
