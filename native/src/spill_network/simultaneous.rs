//! Constant concurrent forcing over a normalized interval, with global events.
//! Instantaneous routing is an explicit policy, not finite-rate hydraulics.
use super::{Receiver, SpillNetwork};
use crate::nested_reservoir::{Geometry, Input, Inventory, Snapshot, Stock};
use crate::reservoir::{add, tolerance};
use crate::spill_connections::SpillConnections;
use crate::spill_junction::Weight;
use serde::{Deserialize, Serialize};

pub mod multi_entry;

pub const EXPERIMENT_VERSION: &str = "simultaneous-network-1";
pub const POLICY_VERSION: &str = "constant-forcing-frontiers-1";

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Setup {
    pub geometry: Geometry,
    pub policy_version: String,
    pub weights: Vec<Weight>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub experiment_version: String,
    pub setup: Setup,
    /// pulse_count counts complete intervals, not internal saturation events.
    pub inventory: Inventory,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferRate {
    pub source_branch: usize,
    pub receiver: Receiver,
    pub cubic_meters_per_interval: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub duration_fraction: f64,
    pub input_cubic_meters: f64,
    /// Fixed routing during this segment; nested rates are not external input.
    pub transfers: Vec<TransferRate>,
    /// Changes in exclusive active stocks before any endpoint merges.
    pub retained: Vec<Stock>,
    pub saturated: Vec<usize>,
    pub merged: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interval {
    pub index: u64,
    /// Canonical region order; omitted regions supply zero.
    pub inputs: Vec<Input>,
    pub events: Vec<Event>,
    pub snapshot: Snapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SimultaneousNetwork {
    // Reuse geometry, frontier queries and stock validation, not ordered fill.
    core: SpillNetwork,
    /// Only the separately versioned multi-entry wrapper can enable this policy.
    entry_weights: Option<Vec<multi_entry::EntryWeight>>,
    /// Seeded v3 commits an exactly limiting event at its stored capacity.
    exact_limit_commit: bool,
}

fn commit_event_stock(
    old: f64,
    grant: f64,
    capacity: Option<f64>,
    limiting: bool,
    exact_limit_commit: bool,
) -> Result<f64, String> {
    let value = if exact_limit_commit && limiting {
        let cap = capacity.ok_or("Missing limiting capacity.")?;
        let rounded = add(old, grant)?;
        if cap <= old
            || grant != cap - old
            || !cap.next_up().is_finite()
            || (rounded - cap).abs() > cap.next_up() - cap
        {
            return Err("Concurrent limiting grant cannot be represented.".into());
        }
        // The endpoint is the already validated capacity; its reported
        // retained amount below is the actual stock difference, not `grant`.
        cap
    } else {
        add(old, grant)?
    };
    if capacity.is_some_and(|cap| value > cap) {
        return Err("Concurrent update exceeds capacity.".into());
    }
    Ok(value)
}

impl Setup {
    fn core(&self) -> Result<super::Setup, String> {
        if self.policy_version != POLICY_VERSION {
            return Err("Unsupported simultaneous network policy.".into());
        }
        Ok(super::Setup {
            geometry: self.geometry.clone(),
            policy_version: super::POLICY_VERSION.into(),
            weights: self.weights.clone(),
        })
    }
}

impl SimultaneousNetwork {
    pub fn new(setup: Setup) -> Result<Self, String> {
        Ok(Self {
            core: SpillNetwork::new(setup.core()?)?,
            entry_weights: None,
            exact_limit_commit: false,
        })
    }
    pub fn restore(checkpoint: Checkpoint) -> Result<Self, String> {
        if checkpoint.experiment_version != EXPERIMENT_VERSION {
            return Err("Unsupported simultaneous network experiment.".into());
        }
        Ok(Self {
            core: SpillNetwork::restore(super::Checkpoint {
                experiment_version: super::EXPERIMENT_VERSION.into(),
                setup: checkpoint.setup.core()?,
                inventory: checkpoint.inventory,
            })?,
            entry_weights: None,
            exact_limit_commit: false,
        })
    }
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            experiment_version: EXPERIMENT_VERSION.into(),
            setup: Setup {
                geometry: self.core.checkpoint.setup.geometry.clone(),
                policy_version: POLICY_VERSION.into(),
                weights: self.core.checkpoint.setup.weights.clone(),
            },
            inventory: self.core.checkpoint.inventory.clone(),
        }
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.core.snapshot()
    }
    pub fn connections(&self) -> &SpillConnections {
        self.core.connections()
    }

    fn supplied(&self, id: usize, inputs: &[f64]) -> Result<f64, String> {
        inputs
            .iter()
            .enumerate()
            .filter(|(leaf, _)| self.core.contains(id, *leaf))
            .try_fold(0., |sum, (_, &rate)| add(sum, rate))
    }

    /// For a fixed stock frontier, routing is linear in concurrent forcing.
    /// Full immediate children pass their forcing to reachable underfilled
    /// siblings. Recursion then combines incoming and local forcing before
    /// routing inside each receiving subtree. No recipient is advanced here.
    fn rates(
        &self,
        id: usize,
        inputs: &[f64],
        stocks: &[Option<f64>],
        retained: &mut [f64],
        transfers: &mut Vec<TransferRate>,
    ) -> Result<(), String> {
        if stocks[id].is_some() {
            if self.core.full(id, stocks) {
                return Err("A full branch must be routed by its parent.".into());
            }
            retained[id] = self.supplied(id, inputs)?;
            return Ok(());
        }
        let children = &self.core.connections.basins().nodes()[id].children;
        let mut combined = inputs.to_vec();
        for &source in children {
            if !self.core.full(source, stocks) {
                continue;
            }
            let input = self.supplied(source, inputs)?;
            if input == 0. {
                continue;
            }
            if let Some(weights) = &self.entry_weights {
                for transfer in multi_entry::allocate(&self.core, weights, source, stocks, input)? {
                    let leaf = transfer.receiver.entry_leaf;
                    combined[leaf] = add(combined[leaf], transfer.cubic_meters_per_interval)?;
                    transfers.push(transfer);
                }
                continue;
            }
            let receivers = self.core.frontier(source, stocks)?;
            if receivers.is_empty() {
                return Err("Missing concurrent receiving frontier.".into());
            }
            let max_weight = receivers
                .iter()
                .map(|r| self.core.checkpoint.setup.weights[r.branch].weight)
                .fold(0., f64::max);
            let weights: Vec<_> = receivers
                .iter()
                .map(|r| self.core.checkpoint.setup.weights[r.branch].weight / max_weight)
                .collect();
            let sum = weights.iter().try_fold(0., |v, &w| add(v, w))?;
            let mut assigned = 0.;
            for (i, receiver) in receivers.into_iter().enumerate() {
                let share = input * (weights[i] / sum);
                let rate = if i + 1 == weights.len() {
                    input - assigned
                } else {
                    share
                };
                if !share.is_finite()
                    || share <= 0.
                    || !rate.is_finite()
                    || rate <= 0.
                    || (rate - share).abs() > tolerance(input)
                {
                    return Err("Concurrent routing rate cannot be represented.".into());
                }
                assigned = add(assigned, rate)?;
                combined[receiver.entry_leaf] = add(combined[receiver.entry_leaf], rate)?;
                transfers.push(TransferRate {
                    source_branch: source,
                    receiver,
                    cubic_meters_per_interval: rate,
                });
            }
        }
        for &child in children {
            if !self.core.full(child, stocks) {
                self.rates(child, &combined, stocks, retained, transfers)?;
            }
        }
        Ok(())
    }

    /// One constant-forcing interval. Input vector order has no temporal meaning.
    /// Duplicate regions fail; separate calls intentionally express chronology.
    pub fn add_interval(&mut self, mut inputs: Vec<Input>) -> Result<Interval, String> {
        if inputs.len() > self.core.entry_leaves.len()
            || inputs.iter().any(|i| {
                i.region >= self.core.entry_leaves.len()
                    || !i.volume_cubic_meters.is_finite()
                    || i.volume_cubic_meters < 0.
            })
        {
            return Err("Invalid simultaneous region inputs.".into());
        }
        inputs.sort_by_key(|i| i.region);
        if inputs.windows(2).any(|w| w[0].region == w[1].region) {
            return Err("Specify each forcing region at most once.".into());
        }
        let n = self.core.capacities.len();
        let mut forcing = vec![0.; n];
        let mut total = 0.;
        for input in &inputs {
            let leaf = self.core.entry_leaves[input.region];
            forcing[leaf] = add(forcing[leaf], input.volume_cubic_meters)?;
            total = add(total, input.volume_cubic_meters)?;
        }
        let mut next = self.core.checkpoint.inventory.clone();
        next.input_cubic_meters = add(next.input_cubic_meters, total)?;
        next.pulse_count = next
            .pulse_count
            .checked_add(1)
            .ok_or("Interval counter overflowed.")?;
        let mut stocks = vec![None; n];
        for s in &next.active {
            stocks[s.branch] = Some(s.volume_cubic_meters);
        }
        // Use supplied volume as the event coordinate. It is proportional to
        // interval time, but avoids repeatedly subtracting normalized fractions
        // when a representable volume lands exactly on a storage threshold.
        let mut remaining = total;
        let mut events = Vec::new();
        // Each nonfinal segment saturates at least one previously unfilled
        // branch. A branch cannot unfill; at most K such events plus a tail.
        for _ in 0..=n {
            if remaining == 0. {
                break;
            }
            let mut rates = vec![0.; n];
            let mut transfers = Vec::new();
            self.rates(
                self.core.connections.basins().root(),
                &forcing,
                &stocks,
                &mut rates,
                &mut transfers,
            )?;
            let sum = rates.iter().try_fold(0., |v, &r| add(v, r))?;
            if (sum - total).abs() > tolerance(total) {
                return Err("Concurrent rates do not balance.".into());
            }
            let fractions: Vec<_> = rates.iter().map(|r| r / total).collect();
            if rates
                .iter()
                .zip(&fractions)
                .any(|(&r, &f)| r > 0. && f <= 0.)
            {
                return Err("Concurrent rate fraction is below numeric precision.".into());
            }
            let event_inputs: Vec<_> = (0..n)
                .map(|id| {
                    if rates[id] > 0. {
                        self.core.capacities[id].map_or(f64::INFINITY, |cap| {
                            (cap - stocks[id].unwrap()) / fractions[id]
                        })
                    } else {
                        f64::INFINITY
                    }
                })
                .collect();
            let step = event_inputs.iter().copied().fold(remaining, f64::min);
            if !step.is_finite()
                || step <= 0.
                || (step < remaining && remaining - step >= remaining)
            {
                return Err("Concurrent event cannot advance at current precision.".into());
            }
            let mut retained = Vec::new();
            let mut saturated = Vec::new();
            let mut actual = 0.;
            for id in 0..n {
                if rates[id] == 0. {
                    continue;
                }
                let old = stocks[id].ok_or("Rate targets an inactive branch.")?;
                let grant = if event_inputs[id] == step {
                    self.core.capacities[id].unwrap() - old
                } else {
                    fractions[id] * step
                };
                if !grant.is_finite() || grant <= 0. {
                    return Err("Concurrent grant is below numeric precision.".into());
                }
                let value = commit_event_stock(
                    old,
                    grant,
                    self.core.capacities[id],
                    event_inputs[id] == step,
                    self.exact_limit_commit,
                )?;
                stocks[id] = Some(value);
                if self.core.full(id, &stocks) {
                    saturated.push(id);
                }
                actual = add(actual, value - old)?;
                retained.push(Stock {
                    branch: id,
                    volume_cubic_meters: value - old,
                });
            }
            if (actual - step).abs() > tolerance(step) {
                return Err("Concurrent event budget does not balance.".into());
            }
            let mut merged = Vec::new();
            for (id, node) in self.core.connections.basins().nodes().iter().enumerate() {
                if !node.children.is_empty()
                    && node.children.iter().all(|&c| self.core.full(c, &stocks))
                {
                    for &child in &node.children {
                        stocks[child] = None;
                    }
                    stocks[id] = Some(self.core.birth_volumes[id]);
                    merged.push(id);
                }
            }
            if step < remaining && saturated.is_empty() {
                return Err("Nonfinal event did not saturate a branch.".into());
            }
            remaining -= step;
            events.push(Event {
                duration_fraction: step / total,
                input_cubic_meters: step,
                transfers,
                retained,
                saturated,
                merged,
            });
        }
        if remaining != 0. {
            return Err("Concurrent event bound exceeded.".into());
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
        let snapshot = self.core.audit(&next)?;
        if (snapshot.total_stored_cubic_meters - self.snapshot()?.total_stored_cubic_meters - total)
            .abs()
            > tolerance(total)
        {
            return Err("Concurrent interval budget does not balance.".into());
        }
        let report = Interval {
            index: next.pulse_count,
            inputs,
            events,
            snapshot,
        };
        self.core.checkpoint.inventory = next;
        Ok(report)
    }
}

#[cfg(test)]
mod threshold_tests {
    use super::commit_event_stock;
    use crate::reservoir::{add, tolerance};

    #[test]
    fn exact_limit_commits_only_a_checked_endpoint() {
        let old = 5.297_262_581_291_787e12;
        let cap = 2.839_427_442_824_685_5e13;
        let grant = cap - old;
        assert_eq!(old + grant, 2.839_427_442_824_686e13);
        assert_eq!(
            commit_event_stock(old, grant, Some(cap), true, false).unwrap_err(),
            "Concurrent update exceeds capacity."
        );
        assert_eq!(
            commit_event_stock(old, grant, Some(cap), true, true).unwrap(),
            cap
        );
        assert_eq!(
            commit_event_stock(old, grant, Some(cap), false, true).unwrap_err(),
            "Concurrent update exceeds capacity."
        );
        assert_eq!(
            commit_event_stock(old, grant + 100., Some(cap), true, true).unwrap_err(),
            "Concurrent limiting grant cannot be represented."
        );
    }

    #[test]
    fn generated_event_witness_has_no_one_ulp_stock_only_budget_repair() {
        // profile-15, second interval, second internal event. The limiting
        // stock is held at its validated capacity; other stocks are already
        // rounded to the nearest representable values.
        let old = [
            543_587_840_477_296.5,
            854_124_433_587.218_5,
            // This decimal rounds to the measured .1875 f64 value.
            247_378_574_803_164.2,
        ];
        let grants = [
            2_043_857_909.161499,
            2_043_857_909.161499,
            4_087_715_818.322998,
        ];
        let cap = 856_168_291_496.38;
        let values = [
            commit_event_stock(old[0], grants[0], None, false, true).unwrap(),
            commit_event_stock(old[1], cap - old[1], Some(cap), true, true).unwrap(),
            commit_event_stock(old[2], grants[2], None, false, true).unwrap(),
        ];
        let step = 8_175_431_636.645996;
        let actual = values
            .iter()
            .zip(old)
            .try_fold(0., |sum, (&value, old)| add(sum, value - old))
            .unwrap();
        assert_eq!(actual - step, 0.0155029296875);
        assert!(actual - step > tolerance(step));
        assert_eq!(values[0] - values[0].next_down(), 0.0625);
        assert_eq!(values[2] - values[2].next_down(), 0.03125);

        for first in [values[0].next_down(), values[0], values[0].next_up()] {
            for third in [values[2].next_down(), values[2], values[2].next_up()] {
                let repaired =
                    add(add(first - old[0], cap - old[1]).unwrap(), third - old[2]).unwrap();
                assert!((repaired - step).abs() > tolerance(step));
            }
        }
    }
}
