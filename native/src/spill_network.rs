//! Bounded event-driven frontier allocation through nested basin geometry.
//! Ordered pulses and prescribed weights, not a timed hydraulic network.
use crate::drainage::Drainage;
use crate::nested_reservoir::{ActiveLevel, Geometry, Input, Inventory, Snapshot, Stock};
use crate::reservoir::{Boundary, Reservoir, add, tolerance};
use crate::spill_connections::SpillConnections;
use crate::spill_junction::Weight;
use serde::{Deserialize, Serialize};

pub const EXPERIMENT_VERSION: &str = "spill-network-1";
pub const POLICY_VERSION: &str = "frontier-weighted-events-1";

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Setup {
    pub geometry: Geometry,
    pub policy_version: String,
    /// One prescribed weight per non-root branch, in ascending branch order.
    pub weights: Vec<Weight>,
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
pub struct Receiver {
    pub branch: usize,
    pub entry_region: u32,
    pub entry_leaf: usize,
    /// Saturated child branches, from the source through the last transit bowl.
    pub transit_branches: Vec<usize>,
    /// One plateau per transition, including the final receiver entry.
    pub plateaus: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub parent_branch: usize,
    pub source_branch: usize,
    /// Before this stage: underfilled frontier reachable through full children.
    pub receivers: Vec<Receiver>,
    pub accepted: Vec<Stock>,
    pub newly_saturated: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pulse {
    pub index: u64,
    pub input: Input,
    pub stages: Vec<Stage>,
    pub snapshot: Snapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpillNetwork {
    checkpoint: Checkpoint,
    connections: SpillConnections,
    curves: Vec<Reservoir>,
    capacities: Vec<Option<f64>>,
    birth_volumes: Vec<f64>,
    contains: Vec<Vec<bool>>,
    entry_leaves: Vec<usize>,
    incident: Vec<Vec<usize>>,
    receiver_leaves: Vec<Option<usize>>,
}

impl SpillNetwork {
    pub fn new(setup: Setup) -> Result<Self, String> {
        let mut result = Self::prepare(Checkpoint {
            experiment_version: EXPERIMENT_VERSION.into(),
            setup,
            inventory: Inventory {
                active: vec![],
                input_cubic_meters: 0.,
                pulse_count: 0,
            },
        })?;
        result.checkpoint.inventory.active = result
            .connections
            .basins()
            .nodes()
            .iter()
            .enumerate()
            .filter(|(_, n)| n.children.is_empty())
            .map(|(branch, _)| Stock {
                branch,
                volume_cubic_meters: 0.,
            })
            .collect();
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
            return Err("Unsupported spill-network experiment or policy version.".into());
        }
        let g = &checkpoint.setup.geometry;
        let surface = g.surface()?;
        let heights: Vec<_> = g.columns.iter().map(|c| c.bed_meters).collect();
        let connections = SpillConnections::build(&surface, &heights)?;
        let basins = connections.basins();
        let nodes = basins.nodes();
        let k = nodes.len();
        // C2a creates parents after children and the single root last.
        if checkpoint.setup.weights.len() != k - 1
            || checkpoint
                .setup
                .weights
                .iter()
                .enumerate()
                .any(|(i, w)| w.branch != i || !w.weight.is_finite() || w.weight <= 0.)
        {
            return Err(
                "Specify a finite positive weight for every ordered non-root branch.".into(),
            );
        }
        let mut contains = vec![vec![false; k]; k];
        for id in 0..k {
            contains[id][id] = true;
            for &child in &nodes[id].children {
                let (earlier, current) = contains.split_at_mut(id);
                for (included, &descendant) in current[0].iter_mut().zip(&earlier[child]) {
                    *included |= descendant;
                }
            }
        }
        let drainage = Drainage::build(&surface, &heights, &vec![0; heights.len()]);
        let entry_leaves: Vec<_> = drainage
            .outlets
            .iter()
            .map(|&r| basins.region_nodes()[r as usize])
            .collect();
        if entry_leaves
            .iter()
            .any(|&id| !nodes[id].children.is_empty())
        {
            return Err("Bed descent did not terminate in a minimum branch.".into());
        }
        let mut incident = vec![Vec::new(); k];
        let mut receiver_leaves = vec![None; k];
        for (plateau, p) in connections.plateaus().iter().enumerate() {
            let first = p.contacts[0].child_branch;
            if p.contacts.iter().all(|c| c.child_branch == first) {
                continue;
            }
            for contact in &p.contacts {
                let child = contact.child_branch;
                let leaf = entry_leaves[contact.edge[1] as usize];
                if !contains[child][leaf] || receiver_leaves[child].is_some_and(|old| old != leaf) {
                    return Err("Alternative entries into different nested leaves require a separate entry policy.".into());
                }
                receiver_leaves[child] = Some(leaf);
                if incident[child].last() != Some(&plateau) {
                    incident[child].push(plateau);
                }
            }
        }
        let mut curves = Vec::new();
        let mut capacities: Vec<Option<f64>> = Vec::new();
        let mut birth_volumes = Vec::new();
        for id in 0..k {
            let columns = g
                .columns
                .iter()
                .enumerate()
                .filter(|(r, _)| contains[id][basins.region_nodes()[*r]])
                .map(|(_, c)| c.clone())
                .collect();
            let curve = Reservoir::new(columns, Boundary::Closed, 0.)?;
            let capacity = nodes[id]
                .spill_level_meters
                .map(|h| curve.volume_at_level(h))
                .transpose()?;
            let birth = nodes[id]
                .children
                .iter()
                .try_fold(0., |v, &child| add(v, capacities[child].unwrap()))?;
            if (curve.volume_at_level(nodes[id].birth_level_meters)? - birth).abs()
                > tolerance(birth)
                || capacity.is_some_and(|cap| cap <= birth)
            {
                return Err("Network storage interval cannot be resolved precisely.".into());
            }
            curves.push(curve);
            capacities.push(capacity);
            birth_volumes.push(birth);
        }
        Ok(Self {
            checkpoint,
            connections,
            curves,
            capacities,
            birth_volumes,
            contains,
            entry_leaves,
            incident,
            receiver_leaves,
        })
    }
    pub fn checkpoint(&self) -> Checkpoint {
        self.checkpoint.clone()
    }
    pub fn connections(&self) -> &SpillConnections {
        &self.connections
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.audit(&self.checkpoint.inventory)
    }
    /// Inspect the current receiving frontier without advancing the experiment.
    /// A retired child, an underfilled branch, and the closed root cannot spill.
    pub fn receivers(&self, source: usize) -> Result<Vec<Receiver>, String> {
        if source >= self.curves.len() {
            return Err("Invalid network source branch.".into());
        }
        let mut stocks = vec![None; self.curves.len()];
        for stock in &self.checkpoint.inventory.active {
            stocks[stock.branch] = Some(stock.volume_cubic_meters);
        }
        self.frontier(source, &stocks)
    }
    fn full(&self, id: usize, stocks: &[Option<f64>]) -> bool {
        self.capacities[id].is_some() && stocks[id] == self.capacities[id]
    }
    fn total_in(&self, id: usize, stocks: &[Option<f64>]) -> Result<f64, String> {
        stocks
            .iter()
            .enumerate()
            .filter(|(child, _)| self.contains[id][*child])
            .try_fold(0., |total, (_, stock)| add(total, stock.unwrap_or(0.)))
    }

    /// Traverse sill plateaus and ONLY saturated lower children. Underfilled
    /// children are absorbing frontier receivers, never transit shortcuts.
    fn frontier(&self, source: usize, stocks: &[Option<f64>]) -> Result<Vec<Receiver>, String> {
        if !self.full(source, stocks) {
            return Err("A network source must be saturated.".into());
        }
        let mut seen = vec![false; stocks.len()];
        let mut seen_plateau = vec![false; self.connections.plateaus().len()];
        let mut previous = vec![None; stocks.len()];
        let mut queue = vec![source];
        let mut found = Vec::new();
        seen[source] = true;
        let mut cursor = 0;
        while cursor < queue.len() {
            let from = queue[cursor];
            cursor += 1;
            for &plateau in &self.incident[from] {
                if seen_plateau[plateau] {
                    continue;
                }
                seen_plateau[plateau] = true;
                for contact in &self.connections.plateaus()[plateau].contacts {
                    let child = contact.child_branch;
                    if seen[child] {
                        continue;
                    }
                    seen[child] = true;
                    previous[child] = Some((from, plateau));
                    if self.full(child, stocks) {
                        queue.push(child);
                        continue;
                    }
                    let mut transit = Vec::new();
                    let mut plateaus = Vec::new();
                    let mut id = child;
                    while id != source {
                        let (parent, via) =
                            previous[id].ok_or("Broken network reachability witness.")?;
                        transit.push(parent);
                        plateaus.push(via);
                        id = parent;
                    }
                    transit.reverse();
                    plateaus.reverse();
                    found.push(Receiver {
                        branch: child,
                        entry_region: contact.edge[1],
                        entry_leaf: self.receiver_leaves[child]
                            .ok_or("Missing network receiving leaf.")?,
                        transit_branches: transit,
                        plateaus,
                    });
                }
            }
        }
        found.sort_by_key(|r| r.branch);
        Ok(found)
    }

    /// Stop at the FIRST saturation event, not after reallocating all remaining
    /// water across a stale frontier. Exact limiting deficits define the event.
    fn grants(
        &self,
        receivers: &[Receiver],
        stocks: &[Option<f64>],
        input: f64,
    ) -> Result<Vec<f64>, String> {
        if receivers.is_empty() {
            return Err("Saturated component has no receiving frontier.".into());
        }
        let deficits: Vec<_> = receivers
            .iter()
            .map(|r| {
                let d = self.capacities[r.branch].ok_or("Root cannot be a sibling receiver.")?
                    - self.total_in(r.branch, stocks)?;
                if !d.is_finite() || d <= 0. {
                    return Err("Nonpositive receiver deficit.".into());
                }
                Ok(d)
            })
            .collect::<Result<_, String>>()?;
        let max_weight = receivers
            .iter()
            .map(|r| self.checkpoint.setup.weights[r.branch].weight)
            .fold(0., f64::max);
        let weights: Vec<_> = receivers
            .iter()
            .map(|r| self.checkpoint.setup.weights[r.branch].weight / max_weight)
            .collect();
        let sum_weight = weights.iter().try_fold(0., |sum, &w| add(sum, w))?;
        let shares: Vec<_> = weights.iter().map(|&w| input * w / sum_weight).collect();
        if shares.iter().any(|v| !v.is_finite() || *v <= 0.) {
            return Err("Network allocation is below numeric precision.".into());
        }
        let ratios: Vec<_> = deficits.iter().zip(&shares).map(|(d, s)| d / s).collect();
        let factor = ratios.iter().copied().fold(1., f64::min);
        let mut grants: Vec<_> = shares
            .iter()
            .zip(&deficits)
            .zip(&ratios)
            .map(|((&s, &d), &ratio)| {
                if ratio <= 1. && ratio == factor {
                    d
                } else {
                    s * factor
                }
            })
            .collect();
        if factor == 1. {
            let last = grants.len() - 1;
            let assigned = grants[..last].iter().try_fold(0., |sum, &v| add(sum, v))?;
            let remainder = input - assigned;
            if (remainder - grants[last]).abs() > tolerance(input) {
                return Err("Network allocation remainder is not rounding error.".into());
            }
            grants[last] = remainder;
        }
        if grants
            .iter()
            .zip(&deficits)
            .any(|(&v, &d)| !v.is_finite() || v <= 0. || v > d)
        {
            return Err("Network event cannot represent positive bounded grants.".into());
        }
        Ok(grants)
    }

    /// Every nonfinal frontier event saturates a new immediate child. Recursive
    /// calls descend the <=128-region hierarchy; cycles exist only in geometry.
    fn fill(
        &self,
        id: usize,
        leaf: usize,
        input: f64,
        stocks: &mut [Option<f64>],
        stages: &mut Vec<Stage>,
    ) -> Result<f64, String> {
        if let Some(stock) = stocks[id] {
            let retained = self.capacities[id].map_or(input, |cap| input.min(cap - stock));
            let next = add(stock, retained)?;
            if self.capacities[id].is_some_and(|cap| next > cap) {
                return Err("Network fill exceeds capacity at current precision.".into());
            }
            stocks[id] = Some(next);
            return Ok(input - retained);
        }
        let children = &self.connections.basins().nodes()[id].children;
        let source = *children
            .iter()
            .find(|&&c| self.contains[c][leaf])
            .ok_or("Entry is outside the requested subtree.")?;
        let mut excess = self.fill(source, leaf, input, stocks, stages)?;
        for _ in 0..children.len() {
            if excess == 0. || children.iter().all(|&c| self.full(c, stocks)) {
                break;
            }
            let receivers = self.frontier(source, stocks)?;
            let grants = self.grants(&receivers, stocks, excess)?;
            let mut accepted = Vec::new();
            let mut newly_saturated = Vec::new();
            let mut consumed = 0.;
            for (receiver, &grant) in receivers.iter().zip(&grants) {
                if self.fill(receiver.branch, receiver.entry_leaf, grant, stocks, stages)? != 0. {
                    return Err("A bounded frontier grant unexpectedly overflowed.".into());
                }
                consumed = add(consumed, grant)?;
                accepted.push(Stock {
                    branch: receiver.branch,
                    volume_cubic_meters: grant,
                });
                if self.full(receiver.branch, stocks) {
                    newly_saturated.push(receiver.branch);
                }
            }
            if consumed > excess || consumed <= 0. || excess - consumed >= excess {
                return Err("Network event cannot make conservative numeric progress.".into());
            }
            excess -= consumed;
            if excess > 0. && newly_saturated.is_empty() {
                return Err("A nonfinal network event must saturate a receiver.".into());
            }
            stages.push(Stage {
                parent_branch: id,
                source_branch: source,
                receivers,
                accepted,
                newly_saturated,
            });
        }
        if children.iter().all(|&c| self.full(c, stocks)) {
            for &child in children {
                stocks[child] = None;
            }
            stocks[id] = Some(self.birth_volumes[id]);
            self.fill(id, leaf, excess, stocks, stages)
        } else if excess == 0. {
            Ok(0.)
        } else {
            Err("Network exceeded its finite saturation-event bound.".into())
        }
    }

    fn audit(&self, i: &Inventory) -> Result<Snapshot, String> {
        let nodes = self.connections.basins().nodes();
        if !i.input_cubic_meters.is_finite()
            || i.input_cubic_meters < 0.
            || (i.pulse_count == 0 && i.input_cubic_meters != 0.)
            || i.active.windows(2).any(|w| w[0].branch >= w[1].branch)
        {
            return Err("Invalid nested inventory ledger or ordering.".into());
        }
        let mut covered = vec![false; nodes.len()];
        let mut slots = vec![None; nodes.len()];
        let mut active = Vec::new();
        let mut total = 0.;
        let mut resolved = 0.;
        for stock in &i.active {
            let id = stock.branch;
            let v = stock.volume_cubic_meters;
            if id >= nodes.len()
                || !v.is_finite()
                || v < self.birth_volumes[id]
                || self.capacities[id].is_some_and(|cap| v > cap)
                || (i.pulse_count == 0 && (v != 0. || !nodes[id].children.is_empty()))
            {
                return Err("Active stock is outside its branch interval.".into());
            }
            for (leaf, n) in nodes.iter().enumerate() {
                if n.children.is_empty() && self.contains[id][leaf] {
                    if covered[leaf] {
                        return Err("Nested stock is counted more than once.".into());
                    }
                    covered[leaf] = true;
                }
            }
            slots[id] = Some(v);
            let level = if v == 0. {
                None
            } else if v == self.birth_volumes[id] {
                Some(nodes[id].birth_level_meters)
            } else if self.capacities[id] == Some(v) {
                nodes[id].spill_level_meters
            } else {
                self.curves[id].level_for_volume(v)?
            };
            if let Some(h) = level {
                if (v > self.birth_volumes[id] && h <= nodes[id].birth_level_meters)
                    || (self.capacities[id].is_some_and(|cap| v < cap)
                        && nodes[id].spill_level_meters.is_some_and(|spill| h >= spill))
                {
                    return Err("Nested level cannot resolve its threshold interval.".into());
                }
                resolved = add(resolved, self.curves[id].volume_at_level(h)?)?;
            }
            total = add(total, v)?;
            active.push(ActiveLevel {
                stock: stock.clone(),
                water_level_meters: level,
            });
        }
        for (id, n) in nodes.iter().enumerate() {
            if n.children.is_empty() && !covered[id] {
                return Err("Missing leaf inventory.".into());
            }
            if !n.children.is_empty() && n.children.iter().all(|&c| slots[c] == self.capacities[c])
            {
                return Err("Full siblings must be represented by their active parent.".into());
            }
        }
        let residual = i.input_cubic_meters - total;
        if residual.abs() > tolerance(i.input_cubic_meters)
            || (resolved - total).abs() > 1e-9_f64.max(total * 1e-10)
        {
            return Err("Nested storage or water budget does not balance.".into());
        }
        Ok(Snapshot {
            active,
            total_stored_cubic_meters: total,
            storage_residual_cubic_meters: resolved - total,
            budget_residual_cubic_meters: residual,
        })
    }

    /// One ordered entry pulse, audited before committing any inventory change.
    pub fn add_input(&mut self, input: Input) -> Result<Pulse, String> {
        if input.region >= self.entry_leaves.len()
            || !input.volume_cubic_meters.is_finite()
            || input.volume_cubic_meters < 0.
        {
            return Err("Invalid nested input region or volume.".into());
        }
        let old = &self.checkpoint.inventory;
        let mut next = old.clone();
        next.input_cubic_meters = add(old.input_cubic_meters, input.volume_cubic_meters)?;
        next.pulse_count = old
            .pulse_count
            .checked_add(1)
            .ok_or("Nested pulse counter overflowed.")?;
        let mut stocks = vec![None; self.curves.len()];
        for s in &old.active {
            stocks[s.branch] = Some(s.volume_cubic_meters);
        }
        let mut stages = Vec::new();
        let excess = self.fill(
            self.connections.basins().root(),
            self.entry_leaves[input.region],
            input.volume_cubic_meters,
            &mut stocks,
            &mut stages,
        )?;
        if excess != 0. {
            return Err("Closed nested experiment cannot discard overflow.".into());
        }
        next.active = stocks
            .into_iter()
            .enumerate()
            .filter_map(|(branch, volume)| {
                volume.map(|volume_cubic_meters| Stock {
                    branch,
                    volume_cubic_meters,
                })
            })
            .collect();
        let snapshot = self.audit(&next)?;
        let previous = self.snapshot()?.total_stored_cubic_meters;
        if (snapshot.total_stored_cubic_meters - previous - input.volume_cubic_meters).abs()
            > tolerance(input.volume_cubic_meters)
        {
            return Err("Nested pulse cannot be accounted for precisely.".into());
        }
        let pulse = Pulse {
            index: next.pulse_count,
            input,
            stages,
            snapshot,
        };
        self.checkpoint.inventory = next;
        Ok(pulse)
    }
}
