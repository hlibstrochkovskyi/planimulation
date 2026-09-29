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
