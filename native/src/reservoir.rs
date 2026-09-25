//! Isolated, quasi-static leaf reservoir experiments. Not planetary water routing.
use serde::{Deserialize, Serialize};

pub const EXPERIMENT_VERSION: &str = "isolated-reservoir-1";

// Parse checkpoint decimals exactly without changing serde_json's global float
// parser: existing world recipes must retain their versioned interpretation.
pub(crate) fn checkpoint_number<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let raw = Box::<serde_json::value::RawValue>::deserialize(d)?;
    raw.get().parse::<f64>().map_err(serde::de::Error::custom)
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Column {
    #[serde(deserialize_with = "checkpoint_number")]
    pub bed_meters: f64,
    #[serde(deserialize_with = "checkpoint_number")]
    pub area_square_meters: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum Boundary {
    Closed,
    /// An imposed sink outside this experiment, not an inferred neighboring lake.
    ExternalCollector {
        #[serde(rename = "spillLevelMeters", deserialize_with = "checkpoint_number")]
        spill_level_meters: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inventory {
    #[serde(deserialize_with = "checkpoint_number")]
    pub initial_volume_cubic_meters: f64,
    #[serde(deserialize_with = "checkpoint_number")]
    pub input_volume_cubic_meters: f64,
    #[serde(deserialize_with = "checkpoint_number")]
    pub outflow_volume_cubic_meters: f64,
    #[serde(deserialize_with = "checkpoint_number")]
    pub stored_volume_cubic_meters: f64,
    pub pulse_count: u64,
}

/// Complete experiment state. Curve indexes and the water level are derived.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub experiment_version: String,
    pub columns: Vec<Column>,
    pub boundary: Boundary,
    pub inventory: Inventory,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pulse {
    pub index: u64,
    pub input_cubic_meters: f64,
    pub previous_storage_cubic_meters: f64,
    pub stored_cubic_meters: f64,
    pub outflow_cubic_meters: f64,
    /// None when dry; the minimum bed is not a physical water surface.
    pub water_level_meters: Option<f64>,
    pub storage_residual_cubic_meters: f64,
    pub budget_residual_cubic_meters: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct Segment {
    height: f64,
    volume: f64,
    area: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reservoir {
    checkpoint: Checkpoint,
    curve: Vec<Segment>,
    capacity: Option<f64>,
}

pub(crate) fn tolerance(scale: f64) -> f64 {
    1e-9_f64.max(scale.abs() * 1e-12)
}

pub(crate) fn add(a: f64, b: f64) -> Result<f64, String> {
    let sum = a + b;
    if !sum.is_finite() || (b > 0. && sum <= a) {
        return Err("Reservoir addition overflowed or is below stock precision.".into());
    }
    Ok(sum)
}

impl Reservoir {
    pub(crate) fn new(
        columns: Vec<Column>,
        boundary: Boundary,
        initial_volume: f64,
    ) -> Result<Self, String> {
        Self::restore(Checkpoint {
            experiment_version: EXPERIMENT_VERSION.into(),
            columns,
            boundary,
            inventory: Inventory {
                initial_volume_cubic_meters: initial_volume,
                input_volume_cubic_meters: 0.,
                outflow_volume_cubic_meters: 0.,
                stored_volume_cubic_meters: initial_volume,
                pulse_count: 0,
            },
        })
    }

    /// Validates a complete isolated-column experiment, not topology or a world save.
    pub fn restore(checkpoint: Checkpoint) -> Result<Self, String> {
        if checkpoint.experiment_version != EXPERIMENT_VERSION {
            return Err("Unsupported reservoir experiment version.".into());
        }
        if checkpoint.columns.is_empty()
            || checkpoint.columns.iter().any(|c| {
                !c.bed_meters.is_finite()
                    || !c.area_square_meters.is_finite()
                    || c.area_square_meters <= 0.
            })
        {
            return Err("Invalid reservoir columns.".into());
        }
        let mut order: Vec<usize> = (0..checkpoint.columns.len()).collect();
        order.sort_by(|&a, &b| {
            checkpoint.columns[a]
                .bed_meters
                .partial_cmp(&checkpoint.columns[b].bed_meters)
                .unwrap()
                .then(a.cmp(&b))
        });
        let mut curve: Vec<Segment> = Vec::new();
        for id in order {
            let c = &checkpoint.columns[id];
            if let Some(last) = curve.last_mut() {
                if c.bed_meters == last.height {
                    last.area = add(last.area, c.area_square_meters)?;
                    continue;
                }
                let increment = last.area * (c.bed_meters - last.height);
                if !increment.is_finite() || increment <= 0. {
                    return Err("Reservoir curve cannot resolve a storage interval.".into());
                }
                let next = Segment {
                    height: c.bed_meters,
                    volume: add(last.volume, increment)?,
                    area: add(last.area, c.area_square_meters)?,
                };
                curve.push(next);
            } else {
                curve.push(Segment {
                    height: c.bed_meters,
                    volume: 0.,
                    area: c.area_square_meters,
                });
            }
        }
        let mut result = Self {
            checkpoint,
            curve,
            capacity: None,
        };
        if let Boundary::ExternalCollector { spill_level_meters } = result.checkpoint.boundary {
            if !spill_level_meters.is_finite()
                || spill_level_meters <= result.curve.last().unwrap().height
            {
                return Err("Spill must be above every isolated leaf column.".into());
            }
            let capacity = result.volume_at_level(spill_level_meters)?;
            if capacity <= 0. {
                return Err("Invalid reservoir capacity.".into());
            }
            result.capacity = Some(capacity);
        }
        result.audit(&result.checkpoint.inventory)?;
        Ok(result)
    }

    pub fn checkpoint(&self) -> Checkpoint {
        self.checkpoint.clone()
    }
    pub fn inventory(&self) -> &Inventory {
        &self.checkpoint.inventory
    }
    pub fn capacity_cubic_meters(&self) -> Option<f64> {
        self.capacity
    }

    /// Mathematical prism volume; accepts the dry datum, not levels above a spill.
    pub fn volume_at_level(&self, level: f64) -> Result<f64, String> {
        if !level.is_finite()
            || level < self.curve[0].height
            || matches!(self.checkpoint.boundary, Boundary::ExternalCollector { spill_level_meters } if level > spill_level_meters)
        {
            return Err("Level outside isolated reservoir range.".into());
        }
        let i = self.curve.partition_point(|s| s.height <= level) - 1;
        let s = &self.curve[i];
        let volume = s.volume + s.area * (level - s.height);
        if !volume.is_finite() {
            return Err("Reservoir volume overflowed.".into());
        }
        Ok(volume)
    }

    pub fn water_level_meters(&self) -> Result<Option<f64>, String> {
        self.level_for_volume(self.inventory().stored_volume_cubic_meters)
    }

    pub(crate) fn level_for_volume(&self, volume: f64) -> Result<Option<f64>, String> {
        if !volume.is_finite() || volume < 0. || self.capacity.is_some_and(|cap| volume > cap) {
            return Err("Storage outside isolated reservoir range.".into());
        }
        if volume == 0. {
            return Ok(None);
        }
        let i = self.curve.partition_point(|s| s.volume <= volume) - 1;
        let s = &self.curve[i];
        let level = if self.capacity == Some(volume) {
            match self.checkpoint.boundary {
                Boundary::ExternalCollector { spill_level_meters } => spill_level_meters,
                Boundary::Closed => unreachable!(),
            }
        } else {
            s.height + (volume - s.volume) / s.area
        };
        let resolved = self.volume_at_level(level)?;
        // Absolute water elevations lose low bits when subtracted from the bed.
        // Match initial-water level reconstruction tolerance; inventory budgets
        // below remain stricter and never replace the stock with this estimate.
        if (resolved - volume).abs() > 1e-9_f64.max(volume * 1e-10) || resolved == 0. {
            return Err("Water level cannot represent the requested storage precisely.".into());
        }
        Ok(Some(level))
    }

    fn audit(&self, state: &Inventory) -> Result<f64, String> {
        let Inventory {
            initial_volume_cubic_meters: initial,
            input_volume_cubic_meters: input,
            outflow_volume_cubic_meters: outflow,
            stored_volume_cubic_meters: stored,
            pulse_count,
        } = *state;
        if [initial, input, outflow, stored]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
            || stored < initial
            || outflow > input
            || (outflow > 0. && self.capacity != Some(stored))
            || (pulse_count == 0 && (input != 0. || outflow != 0. || stored != initial))
        {
            return Err("Invalid reservoir inventory.".into());
        }
        self.level_for_volume(stored)?;
        let supplied = initial + input;
        let accounted = stored + outflow;
        let residual = supplied - accounted;
        if !supplied.is_finite() || !accounted.is_finite() || residual.abs() > tolerance(supplied) {
            return Err("Reservoir budget does not balance.".into());
        }
        Ok(residual)
    }

    /// Instantaneous equilibration after a volume pulse, not a timed discharge law.
    /// Failures leave all inventory and counters unchanged.
    pub fn add_input(&mut self, input: f64) -> Result<Pulse, String> {
        if !input.is_finite() || input < 0. {
            return Err("Input must be a finite nonnegative volume.".into());
        }
        let old = self.inventory();
        let retained = self
            .capacity
            .map_or(input, |cap| input.min(cap - old.stored_volume_cubic_meters));
        let outflow = input - retained;
        let next = Inventory {
            initial_volume_cubic_meters: old.initial_volume_cubic_meters,
            input_volume_cubic_meters: add(old.input_volume_cubic_meters, input)?,
            outflow_volume_cubic_meters: add(old.outflow_volume_cubic_meters, outflow)?,
            stored_volume_cubic_meters: add(old.stored_volume_cubic_meters, retained)?,
            pulse_count: old
                .pulse_count
                .checked_add(1)
                .ok_or("Reservoir pulse counter overflowed.")?,
        };
        let budget_residual = self.audit(&next)?;
        let local_residual =
            (next.stored_volume_cubic_meters - old.stored_volume_cubic_meters) + outflow - input;
        if local_residual.abs() > tolerance(input) {
            return Err("Reservoir pulse cannot be accounted for precisely.".into());
        }
        let level = self.level_for_volume(next.stored_volume_cubic_meters)?;
        let resolved = level.map_or(Ok(0.), |h| self.volume_at_level(h))?;
        let pulse = Pulse {
            index: next.pulse_count,
            input_cubic_meters: input,
            previous_storage_cubic_meters: old.stored_volume_cubic_meters,
            stored_cubic_meters: next.stored_volume_cubic_meters,
            outflow_cubic_meters: outflow,
            water_level_meters: level,
            storage_residual_cubic_meters: resolved - next.stored_volume_cubic_meters,
            budget_residual_cubic_meters: budget_residual,
        };
        self.checkpoint.inventory = next;
        Ok(pulse)
    }
}
