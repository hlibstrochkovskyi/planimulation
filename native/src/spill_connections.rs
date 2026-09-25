//! Static child/plateau incidence and potential passages, not water allocation.
use crate::{Surface, basins::Basins};
use serde::Serialize;
use std::collections::BTreeMap;

pub const ANALYSIS_VERSION: &str = "spill-connections-1";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub child_branch: usize,
    /// [Sill region, strictly lower region inside the immediate child subtree].
    pub edge: [u32; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SillPlateau {
    pub parent_branch: usize,
    pub level_meters: f64,
    pub regions: Vec<u32>,
    pub contacts: Vec<Contact>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receiver {
    pub branch: usize,
    /// Alternatives remain explicit; this is not a flow-split decision.
    pub plateaus: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Passage {
    pub source_branch: usize,
    pub receiver_branch: usize,
    pub plateau: usize,
    /// Lower source contact, equal-height sill path, lower receiving contact.
    pub regions: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpillConnections {
    analysis_version: &'static str,
    basins: Basins,
    plateaus: Vec<SillPlateau>,
    #[serde(skip)]
    child_plateaus: Vec<Vec<usize>>,
    #[serde(skip)]
    region_plateaus: Vec<usize>,
    #[serde(skip)]
    offsets: Vec<u32>,
    #[serde(skip)]
    neighbors: Vec<u32>,
}

impl SpillConnections {
    /// Requires the same validated connected reciprocal graph as Basins::build.
    /// Owns a fresh analysis so callers cannot mix a hierarchy with another bed.
    pub fn build(surface: &Surface, heights: &[f64]) -> Result<Self, String> {
        let basins = Basins::build(surface, heights)?;
        let nodes = basins.nodes();
        let owners = basins.region_nodes();
        // Iterative Euler intervals identify immediate children in logarithmic
        // sibling lookup time, without walking deep ancestry for every edge.
        let mut enter = vec![0; nodes.len()];
        let mut leave = vec![0; nodes.len()];
        let mut stack = vec![(basins.root(), false)];
        let mut clock = 0;
        while let Some((id, closing)) = stack.pop() {
            if closing {
                leave[id] = clock;
                continue;
            }
            enter[id] = clock;
            clock += 1;
            stack.push((id, true));
            for &child in nodes[id].children.iter().rev() {
                stack.push((child, false));
            }
        }
        let mut neighbors = surface.neighbors.clone();
        for range in surface.offsets.windows(2) {
            neighbors[range[0] as usize..range[1] as usize].sort_unstable();
        }
        let adjacent =
            |i: usize| &neighbors[surface.offsets[i] as usize..surface.offsets[i + 1] as usize];
        let mut region_plateaus = vec![usize::MAX; heights.len()];
        let mut child_plateaus = vec![Vec::new(); nodes.len()];
        let mut canonical = vec![None; nodes.len()];
        let mut plateaus = Vec::new();
        for start in 0..heights.len() {
            let parent = owners[start];
            if nodes[parent].children.is_empty()
                || heights[start] != nodes[parent].birth_level_meters
                || region_plateaus[start] != usize::MAX
            {
                continue;
            }
            let plateau_id = plateaus.len();
            let level = heights[start];
            let mut regions = vec![start as u32];
            region_plateaus[start] = plateau_id;
            let mut contacts = Vec::new();
            let mut cursor = 0;
            while cursor < regions.len() {
                let i = regions[cursor] as usize;
                cursor += 1;
                for &j in adjacent(i) {
                    let id = j as usize;
                    if heights[id] == level {
                        if owners[id] != parent {
                            return Err("Equal-height sill crosses analysis parents.".into());
                        }
                        if region_plateaus[id] == usize::MAX {
                            region_plateaus[id] = plateau_id;
                            regions.push(j);
                        }
                    } else if heights[id] < level {
                        let position = enter[owners[id]];
                        let children = &nodes[parent].children;
                        let count = children.partition_point(|&child| enter[child] <= position);
                        let child = count
                            .checked_sub(1)
                            .map(|k| children[k])
                            .ok_or("Sill contact is outside its parent.")?;
                        if position >= leave[child] {
                            return Err("Sill contact is outside its child subtree.".into());
                        }
                        contacts.push(Contact {
                            child_branch: child,
                            edge: [i as u32, j],
                        });
                    }
                }
            }
            regions.sort_unstable();
            contacts.sort_unstable();
            if contacts.is_empty() {
                return Err("Merge sill has no lower contacts.".into());
            }
            for contact in &contacts {
                let child = contact.child_branch;
                if child_plateaus[child].last() != Some(&plateau_id) {
                    child_plateaus[child].push(plateau_id);
                }
                canonical[child] = Some(
                    canonical[child].map_or(contact.edge, |old: [u32; 2]| old.min(contact.edge)),
                );
            }
            plateaus.push(SillPlateau {
                parent_branch: parent,
                level_meters: level,
                regions,
                contacts,
            });
        }
        if nodes
            .iter()
            .enumerate()
            .any(|(id, node)| node.spill_edge != canonical[id])
        {
            return Err("Sill contacts disagree with basin analysis witnesses.".into());
        }
        Ok(Self {
            analysis_version: ANALYSIS_VERSION,
            basins,
            plateaus,
            child_plateaus,
            region_plateaus,
            offsets: surface.offsets.clone(),
            neighbors,
        })
    }

    pub fn basins(&self) -> &Basins {
        &self.basins
    }
    pub fn plateaus(&self) -> &[SillPlateau] {
        &self.plateaus
    }

    /// All direct geometric candidates through one sill plateau. Never transit
    /// through another lower child, which may still have an unfilled inventory.
    pub fn receivers(&self, source: usize) -> Result<Vec<Receiver>, String> {
        let ids = self
            .child_plateaus
            .get(source)
            .ok_or("Invalid source branch.")?;
        let mut candidates: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for &plateau in ids {
            for contact in &self.plateaus[plateau].contacts {
                if contact.child_branch == source {
                    continue;
                }
                let alternatives = candidates.entry(contact.child_branch).or_default();
                if alternatives.last() != Some(&plateau) {
                    alternatives.push(plateau);
                }
            }
        }
        Ok(candidates
            .into_iter()
            .map(|(branch, plateaus)| Receiver { branch, plateaus })
            .collect())
    }

    /// Canonical endpoint edges, then shortest graph-hop path within the chosen
    /// plateau. Equal-length ties use ascending region IDs, not hydraulic costs.
    pub fn passage(
        &self,
        source: usize,
        receiver: usize,
        plateau: usize,
    ) -> Result<Passage, String> {
        if source == receiver {
            return Err("A spill passage requires distinct branches.".into());
        }
        let p = self.plateaus.get(plateau).ok_or("Invalid sill plateau.")?;
        let from = p
            .contacts
            .iter()
            .find(|c| c.child_branch == source)
            .ok_or("Source does not contact this plateau.")?
            .edge;
        let to = p
            .contacts
            .iter()
            .find(|c| c.child_branch == receiver)
            .ok_or("Receiver does not contact this plateau.")?
            .edge;
        let mut previous = vec![u32::MAX; self.region_plateaus.len()];
        let mut queue = vec![from[0]];
        previous[from[0] as usize] = from[0];
        let mut cursor = 0;
        while cursor < queue.len() && previous[to[0] as usize] == u32::MAX {
            let i = queue[cursor] as usize;
            cursor += 1;
            for &j in &self.neighbors[self.offsets[i] as usize..self.offsets[i + 1] as usize] {
                if self.region_plateaus[j as usize] == plateau && previous[j as usize] == u32::MAX {
                    previous[j as usize] = i as u32;
                    queue.push(j);
                }
            }
        }
        if previous[to[0] as usize] == u32::MAX {
            return Err("Disconnected sill plateau.".into());
        }
        let mut regions = vec![to[1], to[0]];
        let mut current = to[0];
        while current != from[0] {
            current = previous[current as usize];
            regions.push(current);
        }
        regions.push(from[1]);
        regions.reverse();
        Ok(Passage {
            source_branch: source,
            receiver_branch: receiver,
            plateau,
            regions,
        })
    }
}
