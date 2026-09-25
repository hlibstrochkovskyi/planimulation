//! Bounded, closed binary-hierarchy experiment with unambiguous sill contacts.
//! Ordered volume pulses, not hydraulic time integration or planetary routing.
use crate::reservoir::{Boundary, Column, Reservoir, add, checkpoint_number, tolerance};
use crate::{Surface, drainage::Drainage, spill_connections::SpillConnections};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const EXPERIMENT_VERSION: &str = "nested-reservoir-1";
pub const MAX_REGIONS: usize = 128;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Edge {
    pub regions: [usize; 2],
    #[serde(deserialize_with = "checkpoint_number")]
    pub distance_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry {
    pub columns: Vec<Column>,
    pub edges: Vec<Edge>,
}

impl Geometry {
    fn surface(&self) -> Result<Surface, String> {
        let n = self.columns.len();
        if n == 0
            || n > MAX_REGIONS
            || self.edges.len() > n * (n - 1) / 2
            || self.columns.iter().any(|c| {
                !c.bed_meters.is_finite()
                    || !c.area_square_meters.is_finite()
                    || c.area_square_meters <= 0.
            })
        {
            return Err("Nested experiment requires 1–128 finite positive-area columns.".into());
        }
        let mut seen = BTreeSet::new();
        let mut lists = vec![Vec::new(); n];
        for e in &self.edges {
            let [a, b] = e.regions;
            if a >= n
                || b >= n
                || a == b
                || !e.distance_meters.is_finite()
                || e.distance_meters <= 0.
                || !seen.insert([a.min(b), a.max(b)])
                || !(self.columns[a].bed_meters - self.columns[b].bed_meters).is_finite()
                || !((self.columns[a].bed_meters - self.columns[b].bed_meters) / e.distance_meters)
                    .is_finite()
            {
                return Err("Invalid or duplicate nested experiment edge.".into());
            }
            lists[a].push((b as u32, e.distance_meters));
            lists[b].push((a as u32, e.distance_meters));
        }
        let mut s = Surface {
            centers: vec![],
            faces: vec![],
            offsets: vec![0],
            neighbors: vec![],
            distances: vec![],
            areas: self.columns.iter().map(|c| c.area_square_meters).collect(),
            boundary_offsets: vec![],
            boundaries: vec![],
        };
        for mut list in lists {
            list.sort_by_key(|&(id, _)| id);
            for (id, distance) in list {
                s.neighbors.push(id);
                s.distances.push(distance);
            }
            s.offsets.push(s.neighbors.len() as u32);
        }
        // Basins::build checks connectivity before any drainage traversal.
        Ok(s)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stock {
    pub branch: usize,
    #[serde(deserialize_with = "checkpoint_number")]
    pub volume_cubic_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inventory {
    /// Sorted exclusive frontier: an active parent replaces all child stocks.
    pub active: Vec<Stock>,
    #[serde(deserialize_with = "checkpoint_number")]
    pub input_cubic_meters: f64,
    pub pulse_count: u64,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub experiment_version: String,
    pub geometry: Geometry,
    pub inventory: Inventory,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub region: usize,
    #[serde(deserialize_with = "checkpoint_number")]
    pub volume_cubic_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    pub source_branch: usize,
    pub receiver_branch: usize,
    pub plateau: usize,
    pub passage_regions: Vec<u32>,
    pub descent_regions: Vec<u32>,
    pub entry_leaf: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub source_branch: usize,
    pub receiver_branch: usize,
    /// Accepted by the receiving subtree; nested transfers must not be summed
    /// as independent inputs. This is not a timed edge-discharge measurement.
    pub volume_cubic_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveLevel {
    pub stock: Stock,
    pub water_level_meters: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub active: Vec<ActiveLevel>,
    pub total_stored_cubic_meters: f64,
    pub storage_residual_cubic_meters: f64,
    pub budget_residual_cubic_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pulse {
    pub index: u64,
    pub input: Input,
    pub transfers: Vec<Transfer>,
    pub snapshot: Snapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NestedReservoir {
    checkpoint: Checkpoint,
    connections: SpillConnections,
    curves: Vec<Reservoir>,
    capacities: Vec<Option<f64>>,
    birth_volumes: Vec<f64>,
    /// Inclusive subtree membership, intentionally bounded by MAX_REGIONS.
    contains: Vec<Vec<bool>>,
    entry_leaves: Vec<usize>,
    routes: Vec<Option<Route>>,
}

impl NestedReservoir {
    /// All reservoirs start dry. Nonzero continuation uses validated checkpoints.
    pub fn new(geometry: Geometry) -> Result<Self, String> {
        let mut result = Self::prepare(Checkpoint {
            experiment_version: EXPERIMENT_VERSION.into(),
            geometry,
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
        if checkpoint.experiment_version != EXPERIMENT_VERSION {
            return Err("Unsupported nested-reservoir version.".into());
        }
        let s = checkpoint.geometry.surface()?;
        let heights: Vec<_> = checkpoint
            .geometry
            .columns
            .iter()
            .map(|c| c.bed_meters)
            .collect();
        let connections = SpillConnections::build(&s, &heights)?;
        let b = connections.basins();
        let nodes = b.nodes();
        if nodes
            .iter()
            .any(|n| !n.children.is_empty() && n.children.len() != 2)
        {
            return Err("Multiway spill allocation is not supported by this experiment.".into());
        }
        let k = nodes.len();
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
        let drainage = Drainage::build(&s, &heights, &vec![0; heights.len()]);
        let entry_leaves: Vec<_> = drainage
            .outlets
            .iter()
            .map(|&r| b.region_nodes()[r as usize])
            .collect();
        if entry_leaves
            .iter()
            .any(|&id| !nodes[id].children.is_empty())
        {
            return Err("Bed descent did not terminate in a minimum branch.".into());
        }
        let mut routes = vec![None; k];
        for source in 0..k {
            if nodes[source].parent.is_none() {
                continue;
            }
            let candidates = connections.receivers(source)?;
            if candidates.len() != 1 || candidates[0].plateaus.len() != 1 {
                return Err(
                    "Alternative receiving branches or sills require an allocation policy.".into(),
                );
            }
            let receiver = candidates[0].branch;
            let plateau = candidates[0].plateaus[0];
            let contacts = &connections.plateaus()[plateau].contacts;
            if contacts.iter().filter(|c| c.child_branch == source).count() != 1
                || contacts
                    .iter()
                    .filter(|c| c.child_branch == receiver)
                    .count()
                    != 1
            {
                return Err("Multiple lower contacts require an entry allocation policy.".into());
            }
            let passage = connections.passage(source, receiver, plateau)?;
            let entry = *passage.regions.last().unwrap() as usize;
            let leaf = entry_leaves[entry];
            if !contains[receiver][leaf] {
                return Err("Spill descent left its receiving subtree.".into());
            }
            let mut descent = vec![entry as u32];
            let mut current = entry;
            while drainage.receivers[current] as usize != current {
                current = drainage.receivers[current] as usize;
                descent.push(current as u32);
            }
            routes[source] = Some(Route {
                source_branch: source,
                receiver_branch: receiver,
                plateau,
                passage_regions: passage.regions,
                descent_regions: descent,
                entry_leaf: leaf,
            });
        }
        let mut curves = Vec::new();
        let mut capacities: Vec<Option<f64>> = Vec::new();
        let mut birth_volumes = Vec::new();
        for id in 0..k {
            let columns = checkpoint
                .geometry
                .columns
                .iter()
                .enumerate()
                .filter(|(r, _)| contains[id][b.region_nodes()[*r]])
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
            let measured = curve.volume_at_level(nodes[id].birth_level_meters)?;
            if (measured - birth).abs() > tolerance(birth)
                || capacity.is_some_and(|cap| cap <= birth)
            {
                return Err("Nested storage interval cannot be resolved precisely.".into());
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
            routes,
        })
    }

    pub fn checkpoint(&self) -> Checkpoint {
        self.checkpoint.clone()
    }
    pub fn connections(&self) -> &SpillConnections {
        &self.connections
    }
    pub fn routes(&self) -> &[Option<Route>] {
        &self.routes
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.audit(&self.checkpoint.inventory)
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

    /// Recursion is bounded by the validated <=128-region binary hierarchy.
    /// Calls descend to children; one final active-parent call follows a merge.
    fn fill(
        &self,
        id: usize,
        leaf: usize,
        input: f64,
        stocks: &mut [Option<f64>],
        transfers: &mut Vec<Transfer>,
    ) -> Result<f64, String> {
        if let Some(stock) = stocks[id] {
            let retained = self.capacities[id].map_or(input, |cap| input.min(cap - stock));
            let next = add(stock, retained)?;
            if self.capacities[id].is_some_and(|cap| next > cap) {
                return Err("Nested fill exceeds capacity at current precision.".into());
            }
            stocks[id] = Some(next);
            return Ok(input - retained);
        }
        let children = &self.connections.basins().nodes()[id].children;
        let source = if self.contains[children[0]][leaf] {
            children[0]
        } else {
            children[1]
        };
        let receiver = if source == children[0] {
            children[1]
        } else {
            children[0]
        };
        let mut excess = self.fill(source, leaf, input, stocks, transfers)?;
        if excess > 0. {
            if stocks[source] != self.capacities[source] {
                return Err("Only a saturated source can spill.".into());
            }
            let route = self.routes[source]
                .as_ref()
                .ok_or("Missing nested spill route.")?;
            let remaining = self.fill(receiver, route.entry_leaf, excess, stocks, transfers)?;
            let accepted = excess - remaining;
            if accepted > 0. {
                transfers.push(Transfer {
                    source_branch: source,
                    receiver_branch: receiver,
                    volume_cubic_meters: accepted,
                });
            }
            excess = remaining;
        }
        if children.iter().all(|&c| stocks[c] == self.capacities[c]) {
            for &child in children {
                stocks[child] = None;
            }
            stocks[id] = Some(self.birth_volumes[id]);
            // The newly active parent now has exactly one authoritative stock.
            self.fill(id, leaf, excess, stocks, transfers)
        } else if excess == 0. {
            Ok(0.)
        } else {
            Err("Surplus cannot cross an unfilled nested receiver.".into())
        }
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
        let mut transfers = Vec::new();
        let excess = self.fill(
            self.connections.basins().root(),
            self.entry_leaves[input.region],
            input.volume_cubic_meters,
            &mut stocks,
            &mut transfers,
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
            transfers,
            snapshot,
        };
        self.checkpoint.inventory = next;
        Ok(pulse)
    }
}
