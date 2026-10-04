//! Bounded one-level, all-dry common-sill parents. No drying/splitting or next spill.
use super::{Checkpoint as SeasonalCheckpoint, closed_lake, leaf_spill, reference_pool};
use crate::{World, moisture_transport::total_mass, reservoir, surface_water::CompensatedStock};
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "common-sill-parent-1";

#[cfg(test)]
mod tests;

/// One exclusive parent owner with inherited contents and an incremental layer.
/// Birth contents are not reconstructed from a rounded parent geometric volume.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Parent {
    pub basin_node: usize,
    pub birth_high_kilograms: f64,
    pub birth_low_kilograms: f64,
    pub surplus_high_kilograms: f64,
    pub surplus_low_kilograms: f64,
}
impl Parent {
    pub(super) fn components(&self) -> [f64; 4] {
        [
            self.birth_high_kilograms,
            self.birth_low_kilograms,
            self.surplus_high_kilograms,
            self.surplus_low_kilograms,
        ]
    }
    fn surplus(&self, capacity: f64) -> Result<CompensatedStock, String> {
        CompensatedStock::new(
            self.surplus_high_kilograms,
            self.surplus_low_kilograms,
            capacity,
        )
    }
    fn set_surplus(&mut self, stock: CompensatedStock) {
        self.surplus_high_kilograms = stock.high;
        self.surplus_low_kilograms = stock.low;
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub model_version: String,
    /// Strictly increasing basin nodes; children cease owning liquid at activation.
    pub parents: Vec<Parent>,
}
impl Checkpoint {
    pub(super) fn empty() -> Self {
        Self {
            model_version: MODEL_VERSION.into(),
            parents: Vec::new(),
        }
    }
    pub(super) fn total(&self) -> f64 {
        total_mass(
            &self
                .parents
                .iter()
                .flat_map(Parent::components)
                .collect::<Vec<_>>(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub basin_node: usize,
    pub child_terminals: Vec<usize>,
    pub child_capacities_kilograms: Vec<f64>,
    pub regions: Vec<usize>,
    pub birth_level_meters: f64,
    pub surplus_capacity_kilograms: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Surface {
    pub basin_node: usize,
    pub relative_height_above_birth_meters: f64,
    pub absolute_level_meters: Option<f64>,
    pub exposed_regions: Vec<usize>,
    pub exposed_area_square_meters: f64,
    pub reconstruction_residual_cubic_meters: f64,
}

struct Group {
    description: Candidate,
    birth: CompensatedStock,
    // Bed offsets retain negative values for birth-level wetness.
    beds: Vec<f64>,
    areas: Vec<f64>,
    curve: reservoir::Reservoir,
}
impl Group {
    fn capacity(&self) -> f64 {
        self.description
            .surplus_capacity_kilograms
            .unwrap_or(f64::MAX)
    }
    fn surface(&self, parent: &Parent) -> Result<Surface, String> {
        let stock = parent.surplus(self.capacity())?;
        let volume = (stock.high + stock.low) / 1000.;
        if stock.high > 0. && volume == 0. {
            return Err("Merged lake surplus is below derived-volume resolution.".into());
        }
        let height = self.curve.level_for_volume(volume)?.unwrap_or(0.);
        if stock.high > 0. && height == 0. {
            return Err("Merged lake surplus is below derived-height resolution.".into());
        }
        let reconstructed = self.curve.volume_at_level(height)?;
        let residual = reconstructed - volume;
        if !residual.is_finite() || residual.abs() > 1e-12 * volume.max(1.) {
            return Err("Merged lake level reconstruction exceeds its local tolerance.".into());
        }
        let mut exposed_regions = Vec::new();
        let mut wet_areas = Vec::new();
        for ((&r, &bed), &area) in self
            .description
            .regions
            .iter()
            .zip(&self.beds)
            .zip(&self.areas)
        {
            if bed < height {
                exposed_regions.push(r);
                wet_areas.push(area);
            }
        }
        let absolute = self.description.birth_level_meters + height;
        let absolute_level_meters = (absolute.is_finite()
            && (height == 0. || absolute > self.description.birth_level_meters)
            && ((absolute - self.description.birth_level_meters) - height).abs()
                <= 1e-12 * height.max(1.))
        .then_some(absolute);
        Ok(Surface {
            basin_node: parent.basin_node,
            relative_height_above_birth_meters: height,
            absolute_level_meters,
            exposed_regions,
            exposed_area_square_meters: total_mass(&wet_areas),
            reconstruction_residual_cubic_meters: residual,
        })
    }
}

pub(super) struct Layout {
    groups: Vec<Group>,
    by_terminal: Vec<Option<usize>>,
    by_region: Vec<Option<usize>>,
}
impl Layout {
    pub fn from_world(world: &World, leaves: &closed_lake::Layout) -> Result<Self, String> {
        let nodes = world.basins.nodes();
        let mut leaf_by_node = vec![None; nodes.len()];
        for lake in leaves.lakes() {
            leaf_by_node[lake.basin_node()] = Some(lake);
        }
        let mut candidates = Vec::new();
        let mut by_node = vec![None; nodes.len()];
        for (basin_node, node) in nodes.iter().enumerate() {
            if node.children.len() < 2
                || node
                    .children
                    .iter()
                    .any(|&c| !nodes[c].children.is_empty() || leaf_by_node[c].is_none())
            {
                continue;
            }
            let group = candidates.len();
            by_node[basin_node] = Some(group);
            let mut pairs = Vec::new();
            for &c in &node.children {
                if nodes[c].spill_level_meters != Some(node.birth_level_meters) {
                    return Err("Merged lake child does not meet its parent birth level.".into());
                }
                by_node[c] = Some(group);
                let leaf = leaf_by_node[c].unwrap();
                let cap = leaf
                    .capacity_cubic_meters()
                    .ok_or("Merged child has no finite capacity.")?
                    * 1000.;
                if !cap.is_finite() || cap <= 0. {
                    return Err("Invalid merged child capacity.".into());
                }
                pairs.push((leaf.terminal_region(), cap));
            }
            pairs.sort_unstable_by_key(|p| p.0);
            candidates.push(Candidate {
                basin_node,
                child_terminals: pairs.iter().map(|p| p.0).collect(),
                child_capacities_kilograms: pairs.iter().map(|p| p.1).collect(),
                regions: Vec::new(),
                birth_level_meters: node.birth_level_meters,
                surplus_capacity_kilograms: None,
            });
        }
        let mut wet = vec![false; candidates.len()];
        for (r, &node) in world.basins.region_nodes().iter().enumerate() {
            if let Some(g) = by_node[node] {
                candidates[g].regions.push(r);
                wet[g] |= world.water.body_ids[r] != 0;
            }
        }
        let mut groups = Vec::new();
        let mut by_terminal = vec![None; world.surface.areas.len()];
        let mut by_region = by_terminal.clone();
        for (mut description, is_wet) in candidates.into_iter().zip(wet) {
            if is_wet {
                continue;
            }
            let mut birth = CompensatedStock::new(0., 0., f64::MAX)?;
            for &cap in &description.child_capacities_kilograms {
                let before = birth;
                birth.credit(cap)?;
                if (birth.high, birth.low) == (before.high, before.low) {
                    return Err(
                        "Merged child capacity is below the birth owner's pair resolution.".into(),
                    );
                }
                let residual =
                    total_mass(&[birth.high - before.high, birth.low - before.low, -cap]);
                if !residual.is_finite()
                    || residual.abs() > 32. * f64::EPSILON * before.high.max(cap).max(1.)
                {
                    return Err(
                        "Merged birth addition exceeds its local arithmetic tolerance.".into(),
                    );
                }
            }
            let beds: Vec<_> = description
                .regions
                .iter()
                .map(|&r| world.terrain.elevation[r] - description.birth_level_meters)
                .collect();
            let areas: Vec<_> = description
                .regions
                .iter()
                .map(|&r| world.surface.areas[r])
                .collect();
            let columns = beds
                .iter()
                .zip(&areas)
                .map(|(&bed, &area)| reservoir::Column {
                    bed_meters: bed.max(0.),
                    area_square_meters: area,
                })
                .collect();
            let boundary = match nodes[description.basin_node].spill_level_meters {
                Some(level) => reservoir::Boundary::ExternalCollector {
                    spill_level_meters: level - description.birth_level_meters,
                },
                None => reservoir::Boundary::Closed,
            };
            // The boundary marks capacity only; no external collector receives water.
            let curve = reservoir::Reservoir::new(columns, boundary, 0.)?;
            description.surplus_capacity_kilograms =
                curve.capacity_cubic_meters().map(|v| v * 1000.);
            if description
                .surplus_capacity_kilograms
                .is_some_and(|c| !c.is_finite() || c <= 0.)
            {
                return Err("Invalid merged lake incremental capacity.".into());
            }
            let g = groups.len();
            for &r in &description.child_terminals {
                by_terminal[r] = Some(g);
            }
            for &r in &description.regions {
                if by_region[r].replace(g).is_some() {
                    return Err("Overlapping merged lake groups.".into());
                }
            }
            groups.push(Group {
                description,
                birth,
                beds,
                areas,
                curve,
            });
        }
        Ok(Self {
            groups,
            by_terminal,
            by_region,
        })
    }
    pub fn candidates(&self) -> Vec<Candidate> {
        self.groups.iter().map(|g| g.description.clone()).collect()
    }
    fn active(&self, cp: &SeasonalCheckpoint, g: usize) -> Option<usize> {
        cp.merged_lake_state
            .as_ref()?
            .parents
            .binary_search_by_key(&self.groups[g].description.basin_node, |p| p.basin_node)
            .ok()
    }
    pub fn terminal_active(&self, cp: &SeasonalCheckpoint, r: usize) -> bool {
        self.by_terminal[r].is_some_and(|g| self.active(cp, g).is_some())
    }
    pub fn owner(&self, cp: &SeasonalCheckpoint, r: usize) -> Option<usize> {
        let g = self.by_region[r]?;
        self.active(cp, g)?;
        Some(self.groups[g].description.child_terminals[0])
    }
    pub fn surfaces(&self, cp: &SeasonalCheckpoint) -> Result<Vec<Surface>, String> {
        self.groups
            .iter()
            .enumerate()
            .filter_map(|(g, group)| {
                self.active(cp, g)
                    .map(|p| group.surface(&cp.merged_lake_state.as_ref().unwrap().parents[p]))
            })
            .collect()
    }
    /// Transfer complete child contents exactly once, then apply all owned queues.
    /// Only called on the enclosing seasonal operator's provisional checkpoint.
    pub fn settle(&self, cp: &mut SeasonalCheckpoint) -> Result<(), String> {
        for g in 0..self.groups.len() {
            self.settle_group(cp, g)?;
        }
        Ok(())
    }
    pub fn settle_at(&self, cp: &mut SeasonalCheckpoint, terminal: usize) -> Result<(), String> {
        if let Some(g) = self.by_terminal[terminal] {
            self.settle_group(cp, g)?;
        }
        Ok(())
    }
    fn settle_group(&self, cp: &mut SeasonalCheckpoint, g: usize) -> Result<(), String> {
        let group = &self.groups[g];
        let d = &group.description;
        if self.active(cp, g).is_none()
            && d.child_terminals
                .iter()
                .zip(&d.child_capacities_kilograms)
                .all(|(&r, &cap)| {
                    cp.terminal_water_kilograms[r] == cap
                        && cp.terminal_low_kilograms.as_ref().unwrap()[r] == 0.
                })
        {
            let parent = Parent {
                basin_node: d.basin_node,
                birth_high_kilograms: group.birth.high,
                birth_low_kilograms: group.birth.low,
                surplus_high_kilograms: 0.,
                surplus_low_kilograms: 0.,
            };
            let parents = &mut cp.merged_lake_state.as_mut().unwrap().parents;
            let position = parents
                .binary_search_by_key(&d.basin_node, |p| p.basin_node)
                .unwrap_err();
            parents.insert(position, parent);
            for &r in &d.child_terminals {
                cp.terminal_water_kilograms[r] = 0.;
                cp.terminal_low_kilograms.as_mut().unwrap()[r] = 0.;
            }
        }
        if let Some(p) = self.active(cp, g) {
            let mut surplus =
                cp.merged_lake_state.as_ref().unwrap().parents[p].surplus(group.capacity())?;
            for &r in &d.child_terminals {
                let mut input = cp
                    .leaf_spill_state
                    .as_ref()
                    .unwrap()
                    .pending_input
                    .stock(r)?;
                leaf_spill::fill(&mut input, &mut surplus, group.capacity())?;
                if input.high != 0. || input.low != 0. {
                    return Err("Merged lake requires next-parent spill; input remains owned but the interval is refused.".into());
                }
                cp.leaf_spill_state
                    .as_mut()
                    .unwrap()
                    .pending_input
                    .set(r, input);
            }
            cp.merged_lake_state.as_mut().unwrap().parents[p].set_surplus(surplus);
        }
        Ok(())
    }
    pub fn evaporate(
        &self,
        cp: &mut SeasonalCheckpoint,
        demand: &[f64],
        regional: &mut [f64],
    ) -> Result<f64, String> {
        let mut maximum: f64 = 0.;
        for (g, group) in self.groups.iter().enumerate() {
            let Some(p) = self.active(cp, g) else {
                continue;
            };
            let requests: Vec<_> = group
                .description
                .regions
                .iter()
                .map(|&r| demand[r])
                .collect();
            let donor =
                cp.merged_lake_state.as_ref().unwrap().parents[p].surplus(group.capacity())?;
            if total_mass(&requests) > donor.available() {
                return Err(
                    "Merged lake evaporation requires drying/splitting below the common sill."
                        .into(),
                );
            }
            let (after, grants, residual) = reference_pool::allocate(donor, &requests)?;
            let mut checked = donor;
            for &grant in &grants {
                let before = checked;
                if checked.withdraw(grant) != grant
                    || (grant > 0. && (checked.high, checked.low) == (before.high, before.low))
                {
                    return Err(
                        "Merged lake evaporation is below the donor pair's resolution.".into(),
                    );
                }
            }
            if (checked.high, checked.low) != (after.high, after.low) {
                return Err("Merged lake evaporation allocation differs from its debits.".into());
            }
            cp.merged_lake_state.as_mut().unwrap().parents[p].set_surplus(after);
            for (&r, grant) in group.description.regions.iter().zip(grants) {
                regional[r] = grant;
            }
            maximum = maximum.max(residual.abs());
        }
        Ok(maximum)
    }
    pub fn validate(&self, cp: &SeasonalCheckpoint) -> Result<(f64, usize), String> {
        let state = cp
            .merged_lake_state
            .as_ref()
            .ok_or("Missing merged lake state.")?;
        if state.model_version != MODEL_VERSION
            || state.parents.len() > self.groups.len()
            || state
                .parents
                .windows(2)
                .any(|w| w[0].basin_node >= w[1].basin_node)
            || (cp.elapsed_seconds == 0 && !state.parents.is_empty())
        {
            return Err("Invalid merged lake frontier or version.".into());
        }
        let mut maximum = (0., 0);
        for parent in &state.parents {
            let index = self
                .groups
                .binary_search_by_key(&parent.basin_node, |g| g.description.basin_node)
                .map_err(|_| "Merged lake is not an eligible all-dry, one-level parent.")?;
            let group = &self.groups[index];
            if (parent.birth_high_kilograms, parent.birth_low_kilograms)
                != (group.birth.high, group.birth.low)
            {
                return Err(
                    "Merged lake birth contents differ from complete child contents.".into(),
                );
            }
            parent.surplus(group.capacity())?;
            let spill = cp
                .leaf_spill_state
                .as_ref()
                .ok_or("Missing merged lake input accounting.")?;
            let mut terms = parent.components().to_vec();
            let mut flows = Vec::new();
            for &r in &group.description.child_terminals {
                if cp.terminal_water_kilograms[r] != 0.
                    || cp.terminal_low_kilograms.as_ref().unwrap()[r] != 0.
                {
                    return Err("Merged lake child still owns liquid.".into());
                }
                let pending = spill.pending_input.stock(r)?;
                terms.extend([pending.high, pending.low]);
                flows.extend([pending.high, pending.low]);
            }
            for &r in &group.description.regions {
                let h = cp.cumulative_lake_capture_kilograms.as_ref().unwrap()[r];
                let l = cp.cumulative_lake_capture_low_kilograms.as_ref().unwrap()[r];
                let route = cp.cumulative_runoff_transfers[r];
                let incoming = spill.cumulative_incoming.stock(r)?;
                let outgoing = spill.cumulative_outgoing.stock(r)?;
                terms.extend([
                    -h,
                    -l,
                    -route.terminal_delivery,
                    route.terminal_evaporation,
                    -incoming.high,
                    -incoming.low,
                    outgoing.high,
                    outgoing.low,
                ]);
                flows.extend([
                    h,
                    l,
                    route.terminal_delivery,
                    route.terminal_evaporation,
                    incoming.high,
                    incoming.low,
                    outgoing.high,
                    outgoing.low,
                ]);
            }
            let residual = total_mass(&terms);
            let scale = total_mass(&parent.components())
                .max(total_mass(&flows))
                .max(1.);
            if !residual.is_finite() || !scale.is_finite() {
                return Err("Nonfinite merged lake identity.".into());
            }
            if residual.abs() / scale > maximum.0 {
                maximum = (residual.abs() / scale, group.description.child_terminals[0]);
            }
        }
        Ok(maximum)
    }
}
