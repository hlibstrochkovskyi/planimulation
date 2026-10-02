//! Read-only transfer of the generated initial water into exclusive basin stocks.
//! This is not a spill-network checkpoint or a water-transport step.
use crate::{Surface, basins::Basins, reservoir::tolerance, water::Water};
use serde::{Deserialize, Serialize};

pub const IMPORT_VERSION: &str = "initial-water-inventory-1";

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InitialStock {
    pub branch: usize,
    pub level_meters: f64,
    pub volume_cubic_meters: f64,
    /// Zero for a dry leaf. Positive IDs use the generated water body's order.
    pub body_id: u32,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InitialWaterInventory {
    pub import_version: String,
    pub source_model_version: String,
    pub source_level_meters: f64,
    pub initial_volume_cubic_meters: f64,
    pub main_ocean_id: u32,
    /// Exclusive active frontier, ordered by branch ID; ancestor capacities are not added.
    pub stocks: Vec<InitialStock>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reconstruction {
    pub depth_meters: Vec<f64>,
    pub body_ids: Vec<u32>,
    pub volume_cubic_meters: f64,
    pub main_ocean_id: u32,
}

pub(crate) fn frontier(basins: &Basins, level: f64) -> (Vec<usize>, Vec<Option<usize>>) {
    let nodes = basins.nodes();
    let mut active = vec![false; nodes.len()];
    let mut pending = vec![basins.root()];
    while let Some(id) = pending.pop() {
        let node = &nodes[id];
        // A sill region has zero depth at equality. Its children remain separate
        // positive-depth bodies until the water level actually exceeds the sill.
        if node.children.is_empty() || node.birth_level_meters < level {
            active[id] = true;
        } else {
            pending.extend_from_slice(&node.children);
        }
    }
    let branches = active
        .iter()
        .enumerate()
        .filter_map(|(id, &yes)| yes.then_some(id))
        .collect();
    let mut owners = vec![None; nodes.len()];
    let mut pending = vec![(basins.root(), None)];
    while let Some((id, inherited)) = pending.pop() {
        let owner = if active[id] { Some(id) } else { inherited };
        owners[id] = owner;
        for &child in &nodes[id].children {
            pending.push((child, owner));
        }
    }
    (branches, owners)
}

fn bodies(surface: &Surface, depths: &[f64]) -> Result<(Vec<u32>, u32), String> {
    let mut ids = vec![0; depths.len()];
    let mut next_id = 0u32;
    let mut main_id = 0;
    let mut largest_area = 0.;
    let mut queue = Vec::new();
    for start in 0..depths.len() {
        if depths[start] == 0. || ids[start] != 0 {
            continue;
        }
        next_id = next_id
            .checked_add(1)
            .ok_or("Too many initial water bodies.")?;
        ids[start] = next_id;
        queue.clear();
        queue.push(start);
        let mut cursor = 0;
        let mut area = 0.;
        while cursor < queue.len() {
            let id = queue[cursor];
            cursor += 1;
            area += surface.areas[id];
            for k in surface.offsets[id] as usize..surface.offsets[id + 1] as usize {
                let neighbor = surface.neighbors[k] as usize;
                if depths[neighbor] > 0. && ids[neighbor] == 0 {
                    ids[neighbor] = next_id;
                    queue.push(neighbor);
                }
            }
        }
        if area > largest_area {
            largest_area = area;
            main_id = next_id;
        }
    }
    Ok((ids, main_id))
}

impl InitialWaterInventory {
    pub fn from_world(world: &crate::World) -> Result<Self, String> {
        let mut result = Self::from_water(
            &world.surface,
            &world.terrain.elevation,
            &world.water,
            &world.basins,
        )?;
        result.source_model_version = world.recipe.model_version.clone();
        Ok(result)
    }
    pub fn from_water(
        surface: &Surface,
        heights: &[f64],
        water: &Water,
        basins: &Basins,
    ) -> Result<Self, String> {
        if !water.level_meters.is_finite()
            || !water.resolved_volume_cubic_meters.is_finite()
            || water.resolved_volume_cubic_meters < 0.
            || water.depth_meters.len() != heights.len()
            || water.body_ids.len() != heights.len()
            || Basins::build(surface, heights)? != *basins
        {
            return Err("Initial water and basin geometry are incompatible.".into());
        }
        let (branches, owners) = frontier(basins, water.level_meters);
        let mut stocks: Vec<_> = branches
            .iter()
            .map(|&branch| InitialStock {
                branch,
                level_meters: water.level_meters,
                volume_cubic_meters: 0.,
                body_id: 0,
            })
            .collect();
        let mut index = vec![None; basins.nodes().len()];
        for (slot, &branch) in branches.iter().enumerate() {
            index[branch] = Some(slot);
        }
        for (region, &depth) in water.depth_meters.iter().enumerate() {
            let expected = (water.level_meters - heights[region]).max(0.);
            if depth != expected {
                return Err("Generated initial depth does not match its level and bed.".into());
            }
            if depth == 0. {
                if water.body_ids[region] != 0 {
                    return Err("A dry region has a water body ID.".into());
                }
                continue;
            }
            let branch = owners[basins.region_nodes()[region]]
                .ok_or("Wet region has no active basin branch.")?;
            let stock = &mut stocks[index[branch].unwrap()];
            let contribution = surface.areas[region] * depth;
            stock.volume_cubic_meters += contribution;
            if !stock.volume_cubic_meters.is_finite() {
                return Err("Initial basin volume overflowed.".into());
            }
            if stock.body_id == 0 {
                stock.body_id = water.body_ids[region];
            } else if stock.body_id != water.body_ids[region] {
                return Err("An active basin contains disconnected water bodies.".into());
            }
        }
        let result = Self {
            import_version: IMPORT_VERSION.into(),
            source_model_version: "basins-1".into(),
            source_level_meters: water.level_meters,
            initial_volume_cubic_meters: water.resolved_volume_cubic_meters,
            main_ocean_id: water.main_ocean_id,
            stocks,
        };
        let restored = result.reconstruct(surface, heights, basins)?;
        if restored.depth_meters != water.depth_meters
            || restored.body_ids != water.body_ids
            || restored.main_ocean_id != water.main_ocean_id
            || (restored.volume_cubic_meters - water.resolved_volume_cubic_meters).abs()
                > tolerance(water.resolved_volume_cubic_meters)
        {
            return Err("Initial inventory does not reproduce generated water.".into());
        }
        Ok(result)
    }

    pub fn reconstruct(
        &self,
        surface: &Surface,
        heights: &[f64],
        basins: &Basins,
    ) -> Result<Reconstruction, String> {
        if self.import_version != IMPORT_VERSION
            || (self.source_model_version != "basins-1"
                && self.source_model_version != "terrain-prep-1")
            || !self.source_level_meters.is_finite()
            || !self.initial_volume_cubic_meters.is_finite()
            || self.initial_volume_cubic_meters < 0.
            || Basins::build(surface, heights)? != *basins
        {
            return Err("Unsupported or incompatible initial inventory.".into());
        }
        let (branches, owners) = frontier(basins, self.source_level_meters);
        if self.stocks.len() != branches.len()
            || self.stocks.iter().zip(&branches).any(|(stock, &branch)| {
                stock.branch != branch
                    || stock.level_meters != self.source_level_meters
                    || !stock.volume_cubic_meters.is_finite()
                    || stock.volume_cubic_meters < 0.
            })
        {
            return Err("Initial stocks do not form the canonical exclusive frontier.".into());
        }
        let mut index = vec![None; basins.nodes().len()];
        for (slot, &branch) in branches.iter().enumerate() {
            index[branch] = Some(slot);
        }
        let mut depths = vec![0.; heights.len()];
        let mut volumes = vec![0.; branches.len()];
        let mut total = 0.;
        for region in 0..heights.len() {
            let Some(branch) = owners[basins.region_nodes()[region]] else {
                continue;
            };
            let slot = index[branch].unwrap();
            let depth = (self.stocks[slot].level_meters - heights[region]).max(0.);
            depths[region] = depth;
            let contribution = surface.areas[region] * depth;
            volumes[slot] += contribution;
            total += contribution;
        }
        if !total.is_finite()
            || (total - self.initial_volume_cubic_meters).abs()
                > tolerance(self.initial_volume_cubic_meters)
            || volumes.iter().zip(&self.stocks).any(|(&volume, stock)| {
                !volume.is_finite()
                    || (volume - stock.volume_cubic_meters).abs()
                        > tolerance(stock.volume_cubic_meters)
            })
        {
            return Err("Initial inventory stock or total volume does not balance.".into());
        }
        let (body_ids, main_ocean_id) = bodies(surface, &depths)?;
        for (region, &body_id) in body_ids.iter().enumerate() {
            if body_id == 0 {
                continue;
            }
            let branch = owners[basins.region_nodes()[region]].unwrap();
            if self.stocks[index[branch].unwrap()].body_id != body_id {
                return Err("Initial stock body labels do not reproduce connectivity.".into());
            }
        }
        if main_ocean_id != self.main_ocean_id
            || self
                .stocks
                .iter()
                .zip(&volumes)
                .any(|(stock, &volume)| (volume == 0.) != (stock.body_id == 0))
        {
            return Err("Initial inventory water body metadata is inconsistent.".into());
        }
        Ok(Reconstruction {
            depth_meters: depths,
            body_ids,
            volume_cubic_meters: total,
            main_ocean_id,
        })
    }
}
