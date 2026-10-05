//! Read-only current owners and derived surfaces for coupled closed lakes.
//! Reference water bodies, queued input and liquid lake surfaces stay distinct.
use super::{Checkpoint, closed_lake, lake_exchange, merged_lake};
use serde::Serialize;

pub const OBSERVATION_VERSION: &str = "seasonal-closed-lake-frontier-1";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "ownerKind", rename_all = "camelCase")]
pub enum Owner {
    Leaf {
        #[serde(rename = "basinNode")]
        basin_node: usize,
        #[serde(rename = "terminalRegion")]
        terminal_region: usize,
        liquid: closed_lake::Liquid,
        surface: closed_lake::Surface,
    },
    Parent {
        contents: merged_lake::Parent,
        surface: merged_lake::Surface,
    },
}
impl Owner {
    pub fn basin_node(&self) -> usize {
        match self {
            Self::Leaf { basin_node, .. } => *basin_node,
            Self::Parent { contents, .. } => contents.basin_node,
        }
    }
    pub fn exposed_regions(&self) -> &[usize] {
        match self {
            Self::Leaf { surface, .. } => &surface.exposed_regions,
            Self::Parent { surface, .. } => &surface.exposed_regions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingInput {
    pub terminal_region: usize,
    pub owner_basin_node: usize,
    /// Owned queue, not already part of this owner's surface geometry.
    pub liquid: closed_lake::Liquid,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub observation_version: String,
    pub simulation_model_version: String,
    /// Region/basin IDs are scoped to this exact resolved generated geography.
    pub recipe: crate::Recipe,
    pub elapsed_seconds: u64,
    /// Dry independent leaves are retained with no level/exposed regions.
    /// Deactivated children are omitted, never reported as independent dry owners.
    pub owners: Vec<Owner>,
    /// Positive-depth closed-lake surfaces only. None is not an ocean/dry verdict.
    pub surface_owner_basin_nodes: Vec<Option<usize>>,
    pub pending_inputs: Vec<PendingInput>,
}

impl lake_exchange::Layout {
    /// The enclosing public Model entry point validates the complete state first.
    pub fn observe_frontier(&self, cp: &Checkpoint) -> Result<Observation, String> {
        let mut owners = Vec::new();
        for lake in self.geometry.lakes() {
            let r = lake.terminal_region();
            if self
                .merge
                .as_ref()
                .is_some_and(|m| m.terminal_active(cp, r))
            {
                continue;
            }
            let liquid = closed_lake::Liquid {
                high_kilograms: cp.terminal_water_kilograms[r],
                low_kilograms: cp.terminal_low_kilograms.as_ref().unwrap()[r],
            };
            owners.push(Owner::Leaf {
                basin_node: lake.basin_node(),
                terminal_region: r,
                liquid,
                surface: lake.surface(liquid)?,
            });
        }
        if let Some(merge) = &self.merge {
            let parents = &cp.merged_lake_state.as_ref().unwrap().parents;
            let surfaces = merge.surfaces(cp)?;
            if surfaces.len() != parents.len() {
                return Err("Closed-lake observation parent/surface count mismatch.".into());
            }
            for (contents, surface) in parents.iter().cloned().zip(surfaces) {
                if contents.basin_node != surface.basin_node {
                    return Err("Closed-lake observation parent/surface mismatch.".into());
                }
                owners.push(Owner::Parent { contents, surface });
            }
        }
        owners.sort_unstable_by_key(Owner::basin_node);
        if owners
            .windows(2)
            .any(|w| w[0].basin_node() == w[1].basin_node())
        {
            return Err("Closed-lake observation duplicates an owner.".into());
        }
        let mut surface_owner_basin_nodes = vec![None; self.by_region.len()];
        for owner in &owners {
            for &r in owner.exposed_regions() {
                if surface_owner_basin_nodes[r]
                    .replace(owner.basin_node())
                    .is_some()
                {
                    return Err("Closed-lake observation duplicates a wet region.".into());
                }
            }
        }
        let mut pending_inputs = Vec::new();
        if let Some(spill) = &cp.leaf_spill_state {
            for r in 0..self.by_region.len() {
                let stock = spill.pending_input.stock(r)?;
                if stock.high == 0. {
                    continue;
                }
                let owner_basin_node = self
                    .merge
                    .as_ref()
                    .and_then(|m| m.active_owner_node(cp, r))
                    .or_else(|| self.by_region[r].map(|b| self.geometry.lakes()[b].basin_node()))
                    .ok_or("Closed-lake observation queue has no supported owner.")?;
                pending_inputs.push(PendingInput {
                    terminal_region: r,
                    owner_basin_node,
                    liquid: closed_lake::Liquid {
                        high_kilograms: stock.high,
                        low_kilograms: stock.low,
                    },
                });
            }
        }
        Ok(Observation {
            observation_version: OBSERVATION_VERSION.into(),
            simulation_model_version: cp.model_version.clone(),
            recipe: cp.recipe.clone(),
            elapsed_seconds: cp.elapsed_seconds,
            owners,
            surface_owner_basin_nodes,
            pending_inputs,
        })
    }
}
