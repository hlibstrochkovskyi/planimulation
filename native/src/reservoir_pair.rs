//! Closed two-bowl, one-sill experiment. No routing through a planetary hierarchy.
use crate::reservoir::{Boundary, Column, Reservoir, add, checkpoint_number, tolerance};
use serde::{Deserialize, Serialize};

pub const EXPERIMENT_VERSION: &str = "reservoir-pair-1";

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Geometry {
    pub left: Vec<Column>,
    pub right: Vec<Column>,
    /// Exclusive columns at/above the sill, counted only in common storage.
    pub connection: Vec<Column>,
    #[serde(deserialize_with = "checkpoint_number")]
    pub sill_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Volumes {
    #[serde(deserialize_with = "checkpoint_number")]
    pub left: f64,
    #[serde(deserialize_with = "checkpoint_number")]
    pub right: f64,
}
impl Volumes {
    fn total(&self) -> Result<f64, String> {
        add(self.left, self.right)
    }
    fn valid(&self) -> bool {
        [self.left, self.right]
            .iter()
            .all(|v| v.is_finite() && *v >= 0.)
    }
}

/// No duplicate child stock once the pair shares one reservoir.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum Storage {
    Separate(Volumes),
    Connected {
        #[serde(deserialize_with = "checkpoint_number")]
        volume: f64,
    },
}
impl Storage {
    fn total(&self) -> Result<f64, String> {
        match self {
            Self::Separate(v) => v.total(),
            Self::Connected { volume } => Ok(*volume),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inventory {
    pub initial: Volumes,
    pub input: Volumes,
    pub storage: Storage,
    pub pulse_count: u64,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub experiment_version: String,
    pub geometry: Geometry,
    pub inventory: Inventory,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub phase: &'static str,
    pub storage: Storage,
    pub levels_meters: [Option<f64>; 2],
    pub total_stored_cubic_meters: f64,
    pub storage_residual_cubic_meters: f64,
    pub budget_residual_cubic_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pulse {
    pub index: u64,
    pub input_cubic_meters: Volumes,
    pub left_to_right_cubic_meters: f64,
    pub right_to_left_cubic_meters: f64,
    /// Surplus after both bowls reach the sill, not a directional edge flux.
    pub input_to_common_storage_cubic_meters: f64,
    pub snapshot: Snapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReservoirPair {
    checkpoint: Checkpoint,
    // Reuse C3a's indexed geometry queries, not its external-collector ledgers.
    sides: [Reservoir; 2],
    common: Reservoir,
    capacities: [f64; 2],
    connection_volume: f64,
}

impl ReservoirPair {
    pub fn new(geometry: Geometry, initial: Volumes) -> Result<Self, String> {
        let checkpoint = Checkpoint {
            experiment_version: EXPERIMENT_VERSION.into(),
            geometry,
            inventory: Inventory {
                storage: Storage::Separate(initial.clone()),
                initial,
                input: Volumes {
                    left: 0.,
                    right: 0.,
                },
                pulse_count: 0,
            },
        };
        // Construction alone canonicalizes an exactly full pair. Restore must
        // reject a noncanonical saved state instead of silently changing it.
        let mut pair = Self::prepare(checkpoint)?;
        if pair.checkpoint.inventory.initial
            == (Volumes {
                left: pair.capacities[0],
                right: pair.capacities[1],
            })
        {
            pair.checkpoint.inventory.storage = Storage::Connected {
                volume: pair.connection_volume,
            };
        }
        pair.snapshot()?;
        Ok(pair)
    }

    pub fn restore(checkpoint: Checkpoint) -> Result<Self, String> {
        let pair = Self::prepare(checkpoint)?;
        pair.snapshot()?;
        Ok(pair)
    }

    fn prepare(checkpoint: Checkpoint) -> Result<Self, String> {
        if checkpoint.experiment_version != EXPERIMENT_VERSION {
            return Err("Unsupported reservoir-pair version.".into());
        }
        let g = &checkpoint.geometry;
        if !g.sill_meters.is_finite()
            || g.connection.is_empty()
            || g.connection
                .iter()
                .any(|c| !c.bed_meters.is_finite() || c.bed_meters < g.sill_meters)
            || !g.connection.iter().any(|c| c.bed_meters == g.sill_meters)
        {
            return Err(
                "Pair requires explicit connection columns at and above a finite sill.".into(),
            );
        }
        let boundary = Boundary::ExternalCollector {
            spill_level_meters: g.sill_meters,
        };
        let sides = [
            Reservoir::new(g.left.clone(), boundary.clone(), 0.)?,
            Reservoir::new(g.right.clone(), boundary, 0.)?,
        ];
        let capacities = sides.each_ref().map(|r| r.capacity_cubic_meters().unwrap());
        let connection_volume = add(capacities[0], capacities[1])?;
        let columns = g
            .left
            .iter()
            .chain(&g.right)
            .chain(&g.connection)
            .cloned()
            .collect();
        let common = Reservoir::new(columns, Boundary::Closed, 0.)?;
        let resolved = common.volume_at_level(g.sill_meters)?;
        if (resolved - connection_volume).abs() > tolerance(connection_volume) {
            return Err("Pair connection capacities disagree with common geometry.".into());
        }
        Ok(Self {
            checkpoint,
            sides,
            common,
            capacities,
            connection_volume,
        })
    }

    pub fn checkpoint(&self) -> Checkpoint {
        self.checkpoint.clone()
    }
    pub fn capacities_cubic_meters(&self) -> [f64; 2] {
        self.capacities
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.audit(&self.checkpoint.inventory)
    }

    fn audit(&self, i: &Inventory) -> Result<Snapshot, String> {
        if !i.initial.valid()
            || !i.input.valid()
            || i.initial.left > self.capacities[0]
            || i.initial.right > self.capacities[1]
            || (i.pulse_count == 0 && (i.input.left != 0. || i.input.right != 0.))
        {
            return Err("Invalid pair inventory.".into());
        }
        let (phase, levels, resolved) = match &i.storage {
            Storage::Separate(v) => {
                if !v.valid()
                    || v.left < i.initial.left
                    || v.right < i.initial.right
                    || (v.left == self.capacities[0] && v.right == self.capacities[1])
                    || (i.pulse_count == 0 && *v != i.initial)
                {
                    return Err("Invalid separate pair storage.".into());
                }
                let levels = [
                    self.sides[0].level_for_volume(v.left)?,
                    self.sides[1].level_for_volume(v.right)?,
                ];
                let mut resolved = 0.;
                for (side, level) in self.sides.iter().zip(levels) {
                    if let Some(h) = level {
                        resolved += side.volume_at_level(h)?;
                    }
                }
                ("separate", levels, resolved)
            }
            Storage::Connected { volume } => {
                if !volume.is_finite()
                    || *volume < self.connection_volume
                    || (i.pulse_count == 0
                        && (i.initial.left != self.capacities[0]
                            || i.initial.right != self.capacities[1]))
                {
                    return Err("Connected pair requires both bowls filled to the sill.".into());
                }
                let at_sill = *volume == self.connection_volume;
                let level = if at_sill {
                    self.checkpoint.geometry.sill_meters
                } else {
                    self.common
                        .level_for_volume(*volume)?
                        .ok_or("Missing common level.")?
                };
                if !at_sill && level <= self.checkpoint.geometry.sill_meters {
                    return Err("Common depth above sill is below level precision.".into());
                }
                (
                    if at_sill { "atSill" } else { "merged" },
                    [Some(level); 2],
                    self.common.volume_at_level(level)?,
                )
            }
        };
        let stored = i.storage.total()?;
        let supplied = add(i.initial.total()?, i.input.total()?)?;
        let residual = supplied - stored;
        if !resolved.is_finite()
            || !stored.is_finite()
            || residual.abs() > tolerance(supplied)
            || (resolved - stored).abs() > 1e-9_f64.max(stored * 1e-10)
        {
            return Err("Pair storage or water budget does not balance.".into());
        }
        Ok(Snapshot {
            phase,
            storage: i.storage.clone(),
            levels_meters: levels,
            total_stored_cubic_meters: stored,
            storage_residual_cubic_meters: resolved - stored,
            budget_residual_cubic_meters: residual,
        })
    }

    /// Simultaneous nonnegative volumes into both sides; atomic on error.
    /// Own input first fills its side, then excess fills the other side's deficit.
    pub fn add_input(&mut self, input: Volumes) -> Result<Pulse, String> {
        if !input.valid() {
            return Err("Pair input must be finite nonnegative volumes.".into());
        }
        let old = &self.checkpoint.inventory;
        let mut next = old.clone();
        next.input.left = add(old.input.left, input.left)?;
        next.input.right = add(old.input.right, input.right)?;
        next.pulse_count = old
            .pulse_count
            .checked_add(1)
            .ok_or("Pair pulse counter overflowed.")?;
        let mut transfers = [0.; 2];
        let common_input;
        next.storage = match &old.storage {
            Storage::Connected { volume } => {
                common_input = input.total()?;
                Storage::Connected {
                    volume: add(*volume, common_input)?,
                }
            }
            Storage::Separate(v) => {
                let mut stocks = [v.left, v.right];
                let mut excess = [0.; 2];
                for (id, incoming) in [input.left, input.right].into_iter().enumerate() {
                    let retained = incoming.min(self.capacities[id] - stocks[id]);
                    stocks[id] = add(stocks[id], retained)?;
                    if stocks[id] > self.capacities[id] {
                        return Err("Pair fill exceeds capacity at current precision.".into());
                    }
                    excess[id] = incoming - retained;
                }
                for id in 0..2 {
                    let other = 1 - id;
                    transfers[id] = excess[id].min(self.capacities[other] - stocks[other]);
                    stocks[other] = add(stocks[other], transfers[id])?;
                    if stocks[other] > self.capacities[other] {
                        return Err("Pair transfer exceeds capacity at current precision.".into());
                    }
                    excess[id] -= transfers[id];
                }
                common_input = add(excess[0], excess[1])?;
                if stocks == self.capacities {
                    Storage::Connected {
                        volume: add(self.connection_volume, common_input)?,
                    }
                } else {
                    if common_input != 0. {
                        return Err("Pair surplus exists before both sides reach the sill.".into());
                    }
                    Storage::Separate(Volumes {
                        left: stocks[0],
                        right: stocks[1],
                    })
                }
            }
        };
        let snapshot = self.audit(&next)?;
        let local_residual =
            snapshot.total_stored_cubic_meters - old.storage.total()? - input.total()?;
        if local_residual.abs() > tolerance(input.total()?) {
            return Err("Pair pulse cannot be accounted for precisely.".into());
        }
        let pulse = Pulse {
            index: next.pulse_count,
            input_cubic_meters: input,
            left_to_right_cubic_meters: transfers[0],
            right_to_left_cubic_meters: transfers[1],
            input_to_common_storage_cubic_meters: common_input,
            snapshot,
        };
        self.checkpoint.inventory = next;
        Ok(pulse)
    }
}
