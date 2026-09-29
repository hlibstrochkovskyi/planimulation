//! Isolated arithmetic experiment, not the production water stock or solver.
//! It tests whether a two-float local stock can retain contributions that a
//! single f64 stock loses. A closed exclusive-merge transition validates
//! supplied grants. A guarded rate-to-grant candidate is exercised here, but
//! route selection and a general event-time policy are not modeled.
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

    fn checked_add_scalar(self, amount: f64) -> Result<Self, &'static str> {
        if !self.high.is_finite() || !self.low.is_finite() || !amount.is_finite() {
            return Err("Nonfinite experimental volume.");
        }
        let (high, first_error) = two_sum(self.high, amount);
        let (low, second_error) = two_sum(self.low, first_error);
        let (high, carried) = two_sum(high, low);
        let (low, lost) = two_sum(carried, second_error);
        if lost != 0. {
            return Err("Input is below two-float precision.");
        }
        let (high, low) = two_sum(high, low);
        let result = Self { high, low };
        if !high.is_finite() || !low.is_finite() || (amount != 0. && result == self) {
            return Err("Input is below two-float precision.");
        }
        Ok(result)
    }

    fn checked_add_pair(self, other: Self) -> Result<Self, &'static str> {
        self.checked_add_scalar(other.high)?
            .checked_add_scalar(other.low)
    }

    fn subtract_pair(self, other: Self) -> Self {
        self.add_scalar(-other.high).add_scalar(-other.low)
    }

    fn compare_to_capacity(self, capacity: f64) -> Ordering {
        match self.high.total_cmp(&capacity) {
            Ordering::Equal => self.low.total_cmp(&0.),
            order => order,
        }
    }

    fn compare_pair(self, other: Self) -> Ordering {
        match self.high.total_cmp(&other.high) {
            Ordering::Equal => self.low.total_cmp(&other.low),
            order => order,
        }
    }
}

/// Candidate only: preserve exact limiting grants and assign the checked
/// rounding remainder to the last nonlimiting receiver in canonical order.
/// Reject when this changes any nominal share by over eight local f64 steps.
fn guarded_rate_grants(
    supplied: ExperimentalVolume,
    rates: [f64; 3],
    limiting: [Option<ExperimentalVolume>; 3],
    available: [ExperimentalVolume; 3],
) -> Result<[ExperimentalVolume; 3], &'static str> {
    if !supplied.high.is_finite()
        || !supplied.low.is_finite()
        || supplied.compare_to_capacity(0.) != Ordering::Greater
        || rates.iter().any(|rate| !rate.is_finite() || *rate <= 0.)
        || available.iter().any(|stock| {
            !stock.high.is_finite()
                || !stock.low.is_finite()
                || stock.compare_to_capacity(0.) != Ordering::Greater
        })
    {
        return Err("Invalid candidate event input or rates.");
    }
    let total_rate: f64 = rates.into_iter().sum();
    if !total_rate.is_finite() || total_rate <= 0. {
        return Err("Invalid candidate total rate.");
    }
    let nominal = rates.map(|rate| (rate / total_rate) * supplied.high);
    if nominal
        .iter()
        .any(|share| !share.is_finite() || *share <= 0.)
    {
        return Err("Candidate rate share cannot be represented.");
    }
    let remainder_owner = limiting.iter().rposition(Option::is_none);
    let adjustable: Vec<_> = (0..3)
        .filter(|&id| limiting[id].is_none() && Some(id) != remainder_owner)
        .collect();
    let offsets = |slot: usize| {
        if slot < adjustable.len() {
            (-8..=8).collect::<Vec<i32>>()
        } else {
            vec![0]
        }
    };
    let shift = |mut value: f64, offset: i32| {
        for _ in 0..offset.unsigned_abs() {
            value = if offset < 0 {
                value.next_down()
            } else {
                value.next_up()
            };
        }
        value
    };
    let mut best: Option<(u32, [ExperimentalVolume; 3])> = None;
    let mut missing_owner = false;
    for first_offset in offsets(0) {
        for second_offset in offsets(1) {
            let mut grants = [ExperimentalVolume::new(0.); 3];
            let mut assigned = ExperimentalVolume::new(0.);
            for id in 0..3 {
                if Some(id) == remainder_owner {
                    continue;
                }
                let offset = adjustable
                    .iter()
                    .position(|&candidate| candidate == id)
                    .map_or(0, |slot| {
                        if slot == 0 {
                            first_offset
                        } else {
                            second_offset
                        }
                    });
                grants[id] = limiting[id]
                    .unwrap_or_else(|| ExperimentalVolume::new(shift(nominal[id], offset)));
                assigned = assigned.add_pair(grants[id]);
            }
            if let Some(owner) = remainder_owner {
                grants[owner] = supplied.subtract_pair(assigned);
            } else if assigned != supplied {
                missing_owner = true;
                continue;
            }
            let valid = (0..3).all(|id| {
                let grant = grants[id];
                let deviation = grant.add_scalar(-nominal[id]);
                let bound = 8. * (nominal[id].next_up() - nominal[id]);
                grant.high.is_finite()
                    && grant.low.is_finite()
                    && grant.compare_to_capacity(0.) == Ordering::Greater
                    && grant.compare_pair(available[id]) != Ordering::Greater
                    && limiting[id].is_none_or(|fixed| fixed == available[id])
                    && bound.is_finite()
                    && deviation.compare_to_capacity(bound) != Ordering::Greater
                    && deviation.compare_to_capacity(-bound) != Ordering::Less
            });
            let reconciled = grants
                .into_iter()
                .fold(ExperimentalVolume::new(0.), ExperimentalVolume::add_pair);
            if !valid || reconciled != supplied {
                continue;
            }
            let cost = first_offset.unsigned_abs() + second_offset.unsigned_abs();
            if best.as_ref().is_none_or(|(current, _)| cost < *current) {
                best = Some((cost, grants));
            }
        }
    }
    if let Some((_, grants)) = best {
        Ok(grants)
    } else if missing_owner {
        Err("No unsaturated recipient owns the event remainder.")
    } else {
        Err("No bounded capacity-safe rate allocation exists.")
    }
}

/// Minimal closed three-child accounting transition. This deliberately has
/// no geography, routing, time coordinate or production checkpoint contract.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExclusiveMergeExperiment {
    capacities: [f64; 3],
    children: Option<[ExperimentalVolume; 3]>,
    parent: Option<ExperimentalVolume>,
    accepted_input: ExperimentalVolume,
    event_count: u64,
}

impl ExclusiveMergeExperiment {
    fn new(capacities: [f64; 3]) -> Self {
        Self {
            capacities,
            children: Some([ExperimentalVolume::new(0.); 3]),
            parent: None,
            accepted_input: ExperimentalVolume::new(0.),
            event_count: 0,
        }
    }

    fn apply(&mut self, supplied: f64, grants: [f64; 3]) -> Result<(), &'static str> {
        self.apply_pair_grants(
            ExperimentalVolume::new(supplied),
            grants.map(ExperimentalVolume::new),
        )
    }

    fn apply_pair_grants(
        &mut self,
        supplied: ExperimentalVolume,
        grants: [ExperimentalVolume; 3],
    ) -> Result<(), &'static str> {
        if !supplied.high.is_finite()
            || !supplied.low.is_finite()
            || supplied.compare_to_capacity(0.) == Ordering::Less
            || grants.iter().any(|grant| {
                !grant.high.is_finite()
                    || !grant.low.is_finite()
                    || grant.compare_to_capacity(0.) == Ordering::Less
            })
        {
            return Err("Nonfinite or negative laboratory input.");
        }
        let granted = grants.into_iter().try_fold(
            ExperimentalVolume::new(0.),
            ExperimentalVolume::checked_add_pair,
        )?;
        if granted != supplied {
            return Err("Laboratory grants do not match supplied input.");
        }
        let mut next = self.clone();
        let children = next.children.as_mut().ok_or("Parent is already active.")?;
        for (id, grant) in grants.into_iter().enumerate() {
            children[id] = children[id].checked_add_pair(grant)?;
            if children[id].compare_to_capacity(next.capacities[id]) == Ordering::Greater {
                return Err("Laboratory child exceeds capacity.");
            }
        }
        if children
            .iter()
            .zip(next.capacities)
            .all(|(child, capacity)| child.compare_to_capacity(capacity) == Ordering::Equal)
        {
            next.parent = Some(children.iter().copied().try_fold(
                ExperimentalVolume::new(0.),
                ExperimentalVolume::checked_add_pair,
            )?);
            next.children = None;
        }
        next.accepted_input = next.accepted_input.checked_add_pair(supplied)?;
        let active_total = if let Some(parent) = next.parent {
            parent
        } else {
            next.children.unwrap().into_iter().try_fold(
                ExperimentalVolume::new(0.),
                ExperimentalVolume::checked_add_pair,
            )?
        };
        if active_total != next.accepted_input {
            return Err("Laboratory exclusive stocks do not balance input.");
        }
        next.event_count += 1;
        *self = next;
        Ok(())
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

#[test]
fn exclusive_merge_waits_for_exact_capacity_and_replays_atomically() {
    let supplied = 100.;
    let capacity = supplied / 3.;
    let grants = [capacity, capacity, supplied - (capacity + capacity)];
    let mut model = ExclusiveMergeExperiment::new([capacity; 3]);
    model.apply(supplied, grants).unwrap();
    assert!(model.parent.is_none());
    assert_eq!(model.event_count, 1);
    assert_eq!(
        model.children.unwrap()[2].compare_to_capacity(capacity),
        Ordering::Less
    );
    let serialized = serde_json::to_vec(&model).unwrap();
    let mut replay: ExclusiveMergeExperiment = serde_json::from_slice(&serialized).unwrap();
    assert_eq!(replay, model);

    let before_rejection = model.clone();
    assert!(model.apply(supplied, [capacity; 3]).is_err());
    assert_eq!(model, before_rejection);
    let missing = capacity - grants[2];
    assert!(missing > 0.);
    assert_eq!(
        model.apply(missing * 2., [0., 0., missing * 2.]),
        Err("Laboratory child exceeds capacity.")
    );
    assert_eq!(model, before_rejection);

    model.apply(missing, [0., 0., missing]).unwrap();
    replay.apply(missing, [0., 0., missing]).unwrap();
    assert_eq!(model, replay);
    assert!(model.children.is_none());
    assert_eq!(model.event_count, 2);
    assert_eq!(model.parent, Some(model.accepted_input));
    let scale = 2_f64.powi(47);
    let expected_units = 3 * ((capacity * scale) as i128);
    let parent = model.parent.unwrap();
    let represented_units = (parent.high * scale) as i128 + (parent.low * scale) as i128;
    assert_eq!(represented_units, expected_units);
}

#[test]
fn guarded_rate_grants_balance_and_preserve_limiting_endpoints() {
    let input = ExperimentalVolume::new(100.);
    let share = 100. / 3.;
    let grants = guarded_rate_grants(
        input,
        [1.; 3],
        [None; 3],
        [ExperimentalVolume::new(share); 3],
    )
    .unwrap();
    let mut partial = ExclusiveMergeExperiment::new([share; 3]);
    partial.apply_pair_grants(input, grants).unwrap();
    assert!(partial.parent.is_none());
    assert!(
        partial
            .children
            .unwrap()
            .iter()
            .any(|child| child.compare_to_capacity(share) == Ordering::Less)
    );

    let fixed = [None, None, Some(ExperimentalVolume::new(share))];
    let grants = guarded_rate_grants(
        input,
        [1.; 3],
        fixed,
        [
            ExperimentalVolume::new(100.),
            ExperimentalVolume::new(100.),
            ExperimentalVolume::new(share),
        ],
    )
    .unwrap();
    assert_eq!(grants[2], ExperimentalVolume::new(share));
    let mut one_full = ExclusiveMergeExperiment::new([100., 100., share]);
    one_full.apply_pair_grants(input, grants).unwrap();
    assert_eq!(
        one_full.children.unwrap()[2].compare_to_capacity(share),
        Ordering::Equal
    );

    let exact_caps = [25., 25., 50.];
    let full = exact_caps.map(|cap| Some(ExperimentalVolume::new(cap)));
    let grants = guarded_rate_grants(
        input,
        [1., 1., 2.],
        full,
        exact_caps.map(ExperimentalVolume::new),
    )
    .unwrap();
    let mut merged = ExclusiveMergeExperiment::new(exact_caps);
    merged.apply_pair_grants(input, grants).unwrap();
    assert_eq!(merged.parent, Some(input));
}

#[test]
fn guarded_rate_grants_reject_missing_owner_or_large_rate_deviation() {
    let input = ExperimentalVolume::new(100.);
    let share = 100. / 3.;
    let all_limiting = [Some(ExperimentalVolume::new(share)); 3];
    assert_eq!(
        guarded_rate_grants(
            input,
            [1.; 3],
            all_limiting,
            [ExperimentalVolume::new(share); 3],
        ),
        Err("No unsaturated recipient owns the event remainder.")
    );
    assert!(
        guarded_rate_grants(
            input,
            [1.; 3],
            [Some(ExperimentalVolume::new(1.)), None, None],
            [
                ExperimentalVolume::new(1.),
                ExperimentalVolume::new(100.),
                ExperimentalVolume::new(100.),
            ],
        )
        .is_err()
    );
}

#[test]
fn guarded_rate_grants_reconcile_the_measured_three_recipient_event() {
    let supplied = ExperimentalVolume::new(8_175_431_636.645996);
    let limiting = [
        None,
        Some(ExperimentalVolume::new(2_043_857_909.161499)),
        None,
    ];
    let grants = guarded_rate_grants(
        supplied,
        [1., 1., 2.],
        limiting,
        [
            ExperimentalVolume::new(1e10),
            limiting[1].unwrap(),
            ExperimentalVolume::new(1e10),
        ],
    )
    .unwrap();
    assert_eq!(grants[1], limiting[1].unwrap());
    let total = grants
        .into_iter()
        .fold(ExperimentalVolume::new(0.), ExperimentalVolume::add_pair);
    assert_eq!(total, supplied);
}

#[test]
fn canonical_remainder_owner_exposes_saturation_label_bias() {
    let supplied = ExperimentalVolume::new(100.);
    let capacity = 100. / 3.;
    let grants = guarded_rate_grants(
        supplied,
        [1.; 3],
        [None; 3],
        [ExperimentalVolume::new(capacity); 3],
    )
    .unwrap();
    let full = grants.map(|grant| grant.compare_to_capacity(capacity) == Ordering::Equal);
    assert!(full.contains(&true) && full.contains(&false));

    // All three physical receivers are indistinguishable here. Reordering
    // their IDs leaves the input arrays unchanged, yet moves the one-ULP
    // deficit to a different physical receiver after mapping IDs back.
    let relabeled_back = [full[1], full[2], full[0]];
    assert_ne!(full, relabeled_back);
}

#[test]
fn distributed_tie_deficit_preserves_saturation_topology() {
    let supplied = ExperimentalVolume::new(100.);
    let capacity = 100. / 3.;
    let total_capacity = [capacity; 3]
        .into_iter()
        .fold(ExperimentalVolume::new(0.), ExperimentalVolume::add_scalar);
    let deficit = total_capacity.subtract_pair(supplied);
    assert_eq!(deficit.high, 7.105_427_357_601_002e-15);
    let equal_deficit = deficit.high / 3.;
    let first = ExperimentalVolume::new(capacity).add_scalar(-equal_deficit);
    let third = supplied.subtract_pair(first.add_pair(first));
    let grants = [first, first, third];
    let total = grants
        .into_iter()
        .fold(ExperimentalVolume::new(0.), ExperimentalVolume::add_pair);
    assert_eq!(total, supplied);
    assert!(
        grants
            .iter()
            .all(|grant| grant.compare_to_capacity(capacity) == Ordering::Less)
    );
    assert_ne!(first, third); // Exact bitwise symmetry is still unavailable.
    let mut model = ExclusiveMergeExperiment::new([capacity; 3]);
    model.apply_pair_grants(supplied, grants).unwrap();
    assert!(model.parent.is_none());
    assert!(
        model
            .children
            .unwrap()
            .iter()
            .all(|child| child.compare_to_capacity(capacity) == Ordering::Less)
    );
}

#[test]
fn compensated_stock_exposes_a_capacity_deficit_hidden_by_f64() {
    let capacity = 500_000_000_000_000.;
    let missing = 1. / 1024.;
    let stock = ExperimentalVolume::new(capacity).add_scalar(-missing);
    assert_eq!(stock.high, capacity);
    assert_eq!(capacity - stock.high, 0.);
    let deficit = ExperimentalVolume::new(capacity).subtract_pair(stock);
    assert_eq!(deficit, ExperimentalVolume::new(missing));
    assert_eq!(stock.compare_to_capacity(capacity), Ordering::Less);
    let filled = stock.add_pair(deficit);
    assert_eq!(filled, ExperimentalVolume::new(capacity));
}

#[test]
fn a_two_float_stock_still_has_an_explicit_subprecision_boundary() {
    let large = 500_000_000_000_000.;
    let stock = ExperimentalVolume::new(large).add_scalar(1e-300);
    assert_eq!(stock.high, large);
    assert_eq!(stock.low, 1e-300);
    let tiny_positive_input = 1e-320;
    assert!(tiny_positive_input > 0.);
    // The current experimental add is unchecked: this positive input is
    // lost when added to an already nonzero low component. A production
    // transition must reject or explicitly queue it, never commit it.
    assert_eq!(stock.add_scalar(tiny_positive_input), stock);
    assert_eq!(
        stock.checked_add_scalar(tiny_positive_input),
        Err("Input is below two-float precision.")
    );
}

#[test]
fn subprecision_input_rejects_without_changing_the_exclusive_checkpoint() {
    let mut model = ExclusiveMergeExperiment::new([2e15, 1., 1.]);
    model.apply(1e15, [1e15, 0., 0.]).unwrap();
    model.apply(1e-300, [1e-300, 0., 0.]).unwrap();
    let checkpoint = model.clone();
    assert_eq!(
        model.apply(1e-320, [1e-320, 0., 0.]),
        Err("Input is below two-float precision.")
    );
    assert_eq!(model, checkpoint);
}

#[test]
fn diverse_guarded_grants_are_conservative_or_explicitly_rejected() {
    let mut state = 0x7d_65_f8_u64;
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    for index in 0..512 {
        let mut draw = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            state >> 32
        };
        let rates = [
            1. + (draw() % 997) as f64,
            1. + (draw() % 997) as f64,
            1. + (draw() % 997) as f64,
        ];
        let scale = 10_f64.powi(index % 9);
        let supplied = ExperimentalVolume::new((1 + draw() % 1_000_000) as f64 * scale);
        let plan = guarded_rate_grants(supplied, rates, [None; 3], [supplied; 3]);
        match plan {
            Ok(grants) => {
                accepted += 1;
                assert!(grants.iter().all(|grant| {
                    grant.compare_to_capacity(0.) == Ordering::Greater
                        && grant.compare_pair(supplied) != Ordering::Greater
                }));
                let total = grants
                    .into_iter()
                    .fold(ExperimentalVolume::new(0.), ExperimentalVolume::add_pair);
                assert_eq!(total, supplied, "case {index}");
            }
            Err(_) => rejected += 1,
        }
    }
    assert_eq!(accepted + rejected, 512);
    assert_eq!(accepted, 505);
    assert_eq!(rejected, 7);
}
