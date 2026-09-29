//! Isolated arithmetic experiment, not the production water stock or solver.
//! It tests whether a two-float local stock can retain contributions that a
//! single f64 stock loses. Routing and grant reconciliation are not modeled.
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExperimentalVolume {
    high: f64,
    low: f64,
}

fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    let b_rounded = sum - a;
    let error = (a - (sum - b_rounded)) + (b - b_rounded);
    (sum, error)
}

impl ExperimentalVolume {
    fn new(high: f64) -> Self {
        Self { high, low: 0. }
    }

    fn add_scalar(self, amount: f64) -> Self {
        let (high, roundoff) = two_sum(self.high, amount);
        let (high, low) = two_sum(high, self.low + roundoff);
        Self { high, low }
    }

    fn add_pair(self, other: Self) -> Self {
        self.add_scalar(other.high).add_scalar(other.low)
    }

    fn compare_to_capacity(self, capacity: f64) -> Ordering {
        match self.high.total_cmp(&capacity) {
            Ordering::Equal => self.low.total_cmp(&0.),
            order => order,
        }
    }
}

#[test]
fn sub_ulp_grants_survive_checkpoint_and_reach_the_expected_stock() {
    let initial = 500_000_000_000_000.;
    let grant = 1. / 1024.;
    assert_eq!(initial + grant, initial);
    let mut single = initial;
    let mut compensated = ExperimentalVolume::new(initial);
    for index in 0..1024 {
        single += grant;
        compensated = compensated.add_scalar(grant);
        if index == 511 {
            let serialized = serde_json::to_vec(&compensated).unwrap();
            compensated = serde_json::from_slice(&serialized).unwrap();
        }
    }
    assert_eq!(single, initial);
    assert_eq!(compensated.high, initial + 1.);
    assert_eq!(compensated.low, 0.);
}

#[test]
fn threshold_comparison_sees_a_sub_ulp_remainder() {
    let capacity = 500_000_000_000_000.;
    let one_step = 1. / 1024.;
    let below = ExperimentalVolume::new(capacity - 0.25);
    let almost_full = (0..255).fold(below, |volume, _| volume.add_scalar(one_step));
    assert_eq!(almost_full.compare_to_capacity(capacity), Ordering::Less);
    let full = almost_full.add_scalar(one_step);
    assert_eq!(full.compare_to_capacity(capacity), Ordering::Equal);
    assert_eq!(full, ExperimentalVolume::new(capacity));
    let overfull = full.add_scalar(one_step);
    assert_eq!(overfull.high, capacity);
    assert!(overfull.low > 0.);
    assert_eq!(overfull.compare_to_capacity(capacity), Ordering::Greater);
}

#[test]
fn measured_event_grants_survive_large_stock_additions() {
    let old = [
        543_587_840_477_296.5,
        854_124_433_587.218_5,
        247_378_574_803_164.2,
    ];
    let cap = 856_168_291_496.38;
    let grants = [2_043_857_909.161499, cap - old[1], 4_087_715_818.322998];
    let nominal_step = 8_175_431_636.645996;
    let observed = old
        .into_iter()
        .zip(grants)
        .map(|(stock, grant)| {
            ExperimentalVolume::new(stock)
                .add_scalar(grant)
                .add_scalar(-stock)
        })
        .fold(ExperimentalVolume::new(0.), ExperimentalVolume::add_pair);
    assert_eq!(observed.high, nominal_step);
    assert_eq!(observed.low, 0.);
}

#[test]
fn merging_saturated_children_retains_volume_rounded_out_of_parent_birth() {
    // Each child capacity is one representable level above a round base.
    // Their exact dyadic sum is not the single-float parent birth value.
    let first_capacity = 500_000_000_000_000_f64.next_up();
    let second_capacity = 100_000_000_000_000_f64.next_up();
    let rounded_parent_birth = first_capacity + second_capacity;
    let merged =
        ExperimentalVolume::new(first_capacity).add_pair(ExperimentalVolume::new(second_capacity));
    assert_eq!(merged.high, rounded_parent_birth);
    assert_ne!(merged.low, 0.);
    let exact_units = (first_capacity * 64.) as i128 + (second_capacity * 64.) as i128;
    let merged_units = (merged.high * 64.) as i128 + (merged.low * 64.) as i128;
    assert_eq!(merged_units, exact_units);
    let saved = serde_json::to_vec(&merged).unwrap();
    let restored: ExperimentalVolume = serde_json::from_slice(&saved).unwrap();
    assert_eq!(restored, merged);
}

#[test]
fn compensated_stocks_expose_a_rate_split_residual_hidden_by_f64_total() {
    let event_input = 100.;
    let share = event_input / 3.;
    let naive_grants = [share, share, share];
    assert_eq!(naive_grants.into_iter().sum::<f64>(), event_input);
    let exact_grant_sum = naive_grants
        .into_iter()
        .fold(ExperimentalVolume::new(0.), ExperimentalVolume::add_scalar);
    assert_eq!(exact_grant_sum.high, event_input);
    assert_ne!(exact_grant_sum.low, 0.);

    // This one event has an eligible last recipient, so a deterministic
    // remainder share can balance the exact dyadic sum. A saturated last
    // recipient would require a different explicit ownership policy.
    let canonical_grants = [share, share, event_input - (share + share)];
    let canonical_sum = canonical_grants
        .into_iter()
        .fold(ExperimentalVolume::new(0.), ExperimentalVolume::add_scalar);
    assert_eq!(canonical_sum, ExperimentalVolume::new(event_input));
    // If every recipient is already required to land at this exact capacity,
    // the canonical third grant would leave its branch underfilled. Balancing
    // arithmetic must not silently alter the event's saturation topology.
    assert!(canonical_grants[2] < share);
}

#[test]
fn varied_dyadic_grants_match_an_independent_integer_ledger() {
    let initial = 500_000_000_000_000.;
    let scale = 1024.;
    let mut exact_units = (initial * scale) as i128;
    let mut stock = ExperimentalVolume::new(initial);
    let mut state = 0x61_28_17_u64;
    for index in 0..10_000 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let grant_units = ((state >> 32) % 201) as i128 - 100;
        exact_units += grant_units;
        stock = stock.add_scalar(grant_units as f64 / scale);
        if index % 1000 == 0 {
            stock = serde_json::from_slice(&serde_json::to_vec(&stock).unwrap()).unwrap();
        }
        let represented_units = (stock.high * scale) as i128 + (stock.low * scale) as i128;
        assert_eq!(represented_units, exact_units, "grant {index}");
    }
}
