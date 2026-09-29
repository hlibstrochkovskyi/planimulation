//! Bounded dynamic continuation from generated initial water, with a separate
//! initial-volume ledger. This does not scale the laboratory to a whole planet.
use super::{Checkpoint as DryCheckpoint, EntryWeight, MultiEntryNetwork, Setup};
use crate::{
    Recipe, World,
    initial_water_inventory::InitialWaterInventory,
    nested_reservoir::{Edge, Geometry, Input, Inventory, MAX_REGIONS, Snapshot, Stock},
    reservoir::Column,
    spill_junction::Weight,
    spill_network::{self, EXPANDED_CURVE_REFERENCES, EXPANDED_REGIONS, Receiver, SpillNetwork},
};
use serde::{Deserialize, Serialize};

use super::super::Interval;

pub const EXPERIMENT_VERSION: &str = "seeded-multi-entry-network-1";
pub const EXPANDED_EXPERIMENT_VERSION: &str = "seeded-multi-entry-network-2";
pub const EXACT_LIMIT_EXPERIMENT_VERSION: &str = "seeded-multi-entry-network-3";
pub const SHARED_STORAGE_EXPERIMENT_VERSION: &str = "seeded-multi-entry-network-4";
pub const RESOLUTION_AWARE_EXPERIMENT_VERSION: &str = "seeded-multi-entry-network-5";
pub const UNIT_WEIGHT_POLICY_VERSION: &str = "unit-branch-and-entry-weights-1";

/// Explicit laboratory policy for a small generated world. Unit weights are
/// not inferred hydraulic conductance or a calibrated planetary law.
pub fn generated_unit_setup(world: &World) -> Result<Setup, String> {
    let n = world.surface.areas.len();
    if n > EXPANDED_REGIONS {
        return Err("Generated world exceeds the bounded seeded-network laboratory.".into());
    }
    let geometry = Geometry {
        columns: (0..n)
            .map(|region| Column {
                bed_meters: world.terrain.elevation[region],
                area_square_meters: world.surface.areas[region],
            })
            .collect(),
        edges: (0..n)
            .flat_map(|region| {
                (world.surface.offsets[region] as usize..world.surface.offsets[region + 1] as usize)
                    .filter_map(move |edge| {
                        let neighbor = world.surface.neighbors[edge] as usize;
                        (region < neighbor).then_some(Edge {
                            regions: [region, neighbor],
                            distance_meters: world.surface.distances[edge],
                        })
                    })
            })
            .collect(),
    };
    let targets = super::entry_targets_with_limit(&geometry, EXPANDED_REGIONS)?;
    Ok(Setup {
        geometry,
        policy_version: super::POLICY_VERSION.into(),
        weights: (0..world.basins.nodes().len() - 1)
            .map(|branch| Weight { branch, weight: 1. })
            .collect(),
        entry_weights: targets
            .into_iter()
            .map(|target| EntryWeight {
                branch: target.branch,
                leaf: target.leaf,
                weight: 1.,
            })
            .collect(),
    })
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub experiment_version: String,
    /// Present for generated starts; replay verifies this provenance against
    /// the complete stored bounded geometry and initial water.
    pub origin_recipe: Option<Recipe>,
    pub setup: Setup,
    pub initial_water: InitialWaterInventory,
    /// `input_cubic_meters` counts only inputs after initialization.
    pub inventory: Inventory,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeededNetwork {
    model: MultiEntryNetwork,
    initial_water: InitialWaterInventory,
    origin_recipe: Option<Recipe>,
    experiment_version: String,
}

impl SeededNetwork {
    pub fn new(setup: Setup, initial_water: InitialWaterInventory) -> Result<Self, String> {
        Self::start(setup, initial_water, None)
    }

    pub fn from_generated(world: &World, setup: Setup) -> Result<Self, String> {
        Self::from_generated_with_version(world, setup, None)
    }

    /// Explicitly opt into a seeded experiment version. `None` preserves the
    /// established size-dependent default and its checkpoint semantics.
    pub fn from_generated_with_version(
        world: &World,
        setup: Setup,
        requested_version: Option<&str>,
    ) -> Result<Self, String> {
        let initial_water = InitialWaterInventory::from_water(
            &world.surface,
            &world.terrain.elevation,
            &world.water,
            &world.basins,
        )?;
        Self::start_with_version(
            setup,
            initial_water,
            Some(world.recipe.clone()),
            requested_version,
        )
    }

    fn start(
        setup: Setup,
        initial_water: InitialWaterInventory,
        origin_recipe: Option<Recipe>,
    ) -> Result<Self, String> {
        Self::start_with_version(setup, initial_water, origin_recipe, None)
    }

    fn start_with_version(
        setup: Setup,
        initial_water: InitialWaterInventory,
        origin_recipe: Option<Recipe>,
        requested_version: Option<&str>,
    ) -> Result<Self, String> {
        let inventory = Inventory {
            active: initial_water
                .stocks
                .iter()
                .map(|stock| Stock {
                    branch: stock.branch,
                    volume_cubic_meters: stock.volume_cubic_meters,
                })
                .collect(),
            input_cubic_meters: 0.,
            pulse_count: 0,
        };
        Self::restore(Checkpoint {
            experiment_version: requested_version.map(str::to_owned).unwrap_or_else(|| {
                if setup.geometry.columns.len() > MAX_REGIONS {
                    SHARED_STORAGE_EXPERIMENT_VERSION.into()
                } else {
                    EXPERIMENT_VERSION.into()
                }
            }),
            origin_recipe,
            setup,
            initial_water,
            inventory,
        })
    }

    pub fn restore(checkpoint: Checkpoint) -> Result<Self, String> {
        let (
            max_regions,
            max_curve_references,
            max_subdivision,
            shared_storage,
            resolution_aware_level,
        ) = match checkpoint.experiment_version.as_str() {
            EXPERIMENT_VERSION => (MAX_REGIONS, usize::MAX, 1, false, false),
            EXPANDED_EXPERIMENT_VERSION => {
                (EXPANDED_REGIONS, EXPANDED_CURVE_REFERENCES, 5, false, false)
            }
            EXACT_LIMIT_EXPERIMENT_VERSION => {
                (EXPANDED_REGIONS, EXPANDED_CURVE_REFERENCES, 5, false, false)
            }
            SHARED_STORAGE_EXPERIMENT_VERSION => (EXPANDED_REGIONS, usize::MAX, 5, true, false),
            RESOLUTION_AWARE_EXPERIMENT_VERSION => (EXPANDED_REGIONS, usize::MAX, 5, true, true),
            _ => return Err("Unsupported seeded-network experiment version.".into()),
        };
        if let Some(recipe) = &checkpoint.origin_recipe {
            if recipe.subdivision > max_subdivision {
                return Err(
                    "Generated world exceeds the bounded seeded-network laboratory.".into(),
                );
            }
            let generated = World::generate(recipe.clone())?;
            let expected_geometry = generated_unit_setup(&generated)?.geometry;
            let expected_water = InitialWaterInventory::from_water(
                &generated.surface,
                &generated.terrain.elevation,
                &generated.water,
                &generated.basins,
            )?;
            if checkpoint.setup.geometry != expected_geometry
                || checkpoint.initial_water != expected_water
            {
                return Err("Seeded checkpoint does not match its generating recipe.".into());
            }
        }
        let geometry = &checkpoint.setup.geometry;
        let surface = geometry.surface_with_limit(max_regions)?;
        let heights: Vec<_> = geometry.columns.iter().map(|c| c.bed_meters).collect();
        let basins = crate::basins::Basins::build(&surface, &heights)?;
        checkpoint
            .initial_water
            .reconstruct(&surface, &heights, &basins)?;
        if basins.nodes().iter().any(|node| {
            !node.children.is_empty()
                && node.birth_level_meters == checkpoint.initial_water.source_level_meters
        }) {
            return Err("Initial water exactly at a dry merging sill is not yet supported by the dynamic frontier.".into());
        }
        if checkpoint.inventory.pulse_count == 0
            && (checkpoint.inventory.input_cubic_meters != 0.
                || checkpoint.inventory.active
                    != checkpoint
                        .initial_water
                        .stocks
                        .iter()
                        .map(|stock| Stock {
                            branch: stock.branch,
                            volume_cubic_meters: stock.volume_cubic_meters,
                        })
                        .collect::<Vec<_>>())
        {
            return Err("Initial seeded checkpoint does not match imported water.".into());
        }
        let core = SpillNetwork::restore_with_initial(
            spill_network::Checkpoint {
                experiment_version: spill_network::EXPERIMENT_VERSION.into(),
                setup: checkpoint.setup.core()?,
                inventory: checkpoint.inventory,
            },
            true,
            checkpoint.initial_water.initial_volume_cubic_meters,
            max_regions,
            max_curve_references,
            shared_storage,
            resolution_aware_level,
        )?;
        let model = if checkpoint.experiment_version == EXACT_LIMIT_EXPERIMENT_VERSION
            || checkpoint.experiment_version == SHARED_STORAGE_EXPERIMENT_VERSION
            || checkpoint.experiment_version == RESOLUTION_AWARE_EXPERIMENT_VERSION
        {
            MultiEntryNetwork::finish_with_exact_limit(core, checkpoint.setup.entry_weights)?
        } else {
            MultiEntryNetwork::finish(core, checkpoint.setup.entry_weights)?
        };
        if model.checkpoint().inventory.pulse_count == 0 {
            let snapshot = model.snapshot()?;
            for active in &snapshot.active {
                if let Some(level) = active.water_level_meters
                    && (level - checkpoint.initial_water.source_level_meters).abs()
                        > 1e-9_f64.max(checkpoint.initial_water.source_level_meters.abs() * 1e-12)
                {
                    return Err("Dynamic initial level differs from generated water.".into());
                }
            }
        }
        Ok(Self {
            model,
            initial_water: checkpoint.initial_water,
            origin_recipe: checkpoint.origin_recipe,
            experiment_version: checkpoint.experiment_version,
        })
    }

    pub fn checkpoint(&self) -> Checkpoint {
        let DryCheckpoint {
            setup, inventory, ..
        } = self.model.checkpoint();
        Checkpoint {
            experiment_version: self.experiment_version.clone(),
            origin_recipe: self.origin_recipe.clone(),
            setup,
            initial_water: self.initial_water.clone(),
            inventory,
        }
    }

    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.model.snapshot()
    }

    pub fn experiment_version(&self) -> &str {
        &self.experiment_version
    }

    pub fn add_interval(&mut self, inputs: Vec<Input>) -> Result<Interval, String> {
        self.model.add_interval(inputs)
    }

    pub fn receivers(&self, source: usize) -> Result<Vec<Receiver>, String> {
        self.model.receivers(source)
    }
}
