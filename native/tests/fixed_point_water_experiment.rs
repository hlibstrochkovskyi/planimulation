//! Isolated fixed-point feasibility experiment, not a production water model.
//! Integer units remove stock-addition roundoff within a declared range but
//! cannot make an indivisible remainder symmetric among identical receivers.
use std::cmp::Ordering;

const FRACTION_BITS: u32 = 56;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RationalUnits {
    numerator: i128,
    denominator: i128,
}

fn gcd(mut left: i128, mut right: i128) -> i128 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

impl RationalUnits {
    fn new(numerator: i128, denominator: i128) -> Result<Self, &'static str> {
        if numerator < 0 || denominator <= 0 {
            return Err("Invalid rational water amount.");
        }
        let divisor = gcd(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    fn integer(units: i128) -> Self {
        Self::new(units, 1).unwrap()
    }

    fn checked_add(self, other: Self) -> Result<Self, &'static str> {
        let common = gcd(self.denominator, other.denominator);
        let left_factor = other.denominator / common;
        let right_factor = self.denominator / common;
        let numerator = self
            .numerator
            .checked_mul(left_factor)
            .and_then(|left| {
                other
                    .numerator
                    .checked_mul(right_factor)
                    .and_then(|right| left.checked_add(right))
            })
            .ok_or("Rational water numerator overflowed.")?;
        let denominator = self
            .denominator
            .checked_mul(left_factor)
            .ok_or("Rational water denominator overflowed.")?;
        Self::new(numerator, denominator)
    }

    fn checked_sub(self, other: Self) -> Result<Self, &'static str> {
        let common = gcd(self.denominator, other.denominator);
        let left_factor = other.denominator / common;
        let right_factor = self.denominator / common;
        let numerator = self
            .numerator
            .checked_mul(left_factor)
            .and_then(|left| {
                other
                    .numerator
                    .checked_mul(right_factor)
                    .and_then(|right| left.checked_sub(right))
            })
            .ok_or("Rational water numerator overflowed.")?;
        let denominator = self
            .denominator
            .checked_mul(left_factor)
            .ok_or("Rational water denominator overflowed.")?;
        Self::new(numerator, denominator)
    }

    fn split_equal(self, recipients: i128) -> Result<Self, &'static str> {
        if recipients <= 0 {
            return Err("Invalid equal split size.");
        }
        let denominator = self
            .denominator
            .checked_mul(recipients)
            .ok_or("Rational water denominator overflowed.")?;
        Self::new(self.numerator, denominator)
    }

    fn checked_mul_integer(self, factor: i128) -> Result<Self, &'static str> {
        if factor < 0 {
            return Err("Invalid rational water factor.");
        }
        let common = gcd(factor, self.denominator);
        let numerator = self
            .numerator
            .checked_mul(factor / common)
            .ok_or("Rational water numerator overflowed.")?;
        Self::new(numerator, self.denominator / common)
    }

    fn checked_div_integer(self, divisor: i128) -> Result<Self, &'static str> {
        if divisor <= 0 {
            return Err("Invalid rational water divisor.");
        }
        let common = gcd(self.numerator, divisor);
        let denominator = self
            .denominator
            .checked_mul(divisor / common)
            .ok_or("Rational water denominator overflowed.")?;
        Self::new(self.numerator / common, denominator)
    }

    fn checked_cmp(self, other: Self) -> Result<Ordering, &'static str> {
        let common = gcd(self.denominator, other.denominator);
        let left = self
            .numerator
            .checked_mul(other.denominator / common)
            .ok_or("Rational water comparison overflowed.")?;
        let right = other
            .numerator
            .checked_mul(self.denominator / common)
            .ok_or("Rational water comparison overflowed.")?;
        Ok(left.cmp(&right))
    }
}

/// Closed three-recipient event coordinate only. Rates are positive integer
/// weights; there is no geography, transit, or policy for already full stocks.
fn first_exact_event(
    stocks: [RationalUnits; 3],
    capacities: [i128; 3],
    rates: [i128; 3],
) -> Result<(RationalUnits, [RationalUnits; 3], [bool; 3]), &'static str> {
    if rates.iter().any(|rate| *rate <= 0) || capacities.iter().any(|capacity| *capacity <= 0) {
        return Err("Invalid laboratory event inputs.");
    }
    let total_rate = rates
        .into_iter()
        .try_fold(0_i128, i128::checked_add)
        .ok_or("Laboratory rate sum overflowed.")?;
    let mut coordinate = None;
    for id in 0..3 {
        let capacity = RationalUnits::integer(capacities[id]);
        if stocks[id].checked_cmp(capacity)? != Ordering::Less {
            return Err("Laboratory receiver is already full.");
        }
        let deficit = capacity.checked_sub(stocks[id])?;
        let candidate = deficit
            .checked_mul_integer(total_rate)?
            .checked_div_integer(rates[id])?;
        let earlier = match coordinate {
            None => true,
            Some(current) => candidate.checked_cmp(current)? == Ordering::Less,
        };
        if earlier {
            coordinate = Some(candidate);
        }
    }
    let coordinate = coordinate.ok_or("No laboratory event coordinate.")?;
    let mut next = stocks;
    let mut full = [false; 3];
    let mut granted = RationalUnits::integer(0);
    for id in 0..3 {
        let grant = coordinate
            .checked_mul_integer(rates[id])?
            .checked_div_integer(total_rate)?;
        next[id] = next[id].checked_add(grant)?;
        granted = granted.checked_add(grant)?;
        match next[id].checked_cmp(RationalUnits::integer(capacities[id]))? {
            Ordering::Greater => return Err("Laboratory event exceeded capacity."),
            Ordering::Equal => full[id] = true,
            Ordering::Less => {}
        }
    }
    if granted != coordinate || !full.contains(&true) {
        return Err("Laboratory event did not conserve or reach a threshold.");
    }
    Ok((coordinate, next, full))
}

/// Preserve the exact ratios of three finite positive binary64 rates when
/// their shared integer weights and sum fit i128. This does not validate the
/// physical rate calculation that produced those binary64 values.
fn exact_rate_weights(rates: [f64; 3]) -> Result<[i128; 3], &'static str> {
    let mut reduced = [(0_u64, 0_i32); 3];
    for (id, rate) in rates.into_iter().enumerate() {
        if !rate.is_finite() || rate <= 0. {
            return Err("Invalid laboratory rate.");
        }
        let bits = rate.to_bits();
        let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1_u64 << 52) - 1);
        let (significand, exponent) = if exponent_bits == 0 {
            (fraction, -1074)
        } else {
            ((1_u64 << 52) | fraction, exponent_bits - 1023 - 52)
        };
        let trailing = significand.trailing_zeros() as i32;
        reduced[id] = (significand >> trailing, exponent + trailing);
    }
    let smallest_exponent = reduced.iter().map(|(_, exponent)| *exponent).min().unwrap();
    // Every reduced significand is odd, so the common factor of the aligned
    // weights is exactly their common odd significand factor. Remove it before
    // shifting: otherwise a valid reduced weight can overflow prematurely.
    let common = reduced
        .iter()
        .map(|(significand, _)| *significand as i128)
        .reduce(gcd)
        .unwrap();
    let mut weights = [0_i128; 3];
    for (id, (significand, exponent)) in reduced.into_iter().enumerate() {
        let shift = (exponent - smallest_exponent) as u32;
        let significand = (significand as i128) / common;
        if shift >= 127 || (significand as u128) > ((i128::MAX as u128) >> shift) {
            return Err("Exact laboratory rate weights exceed i128.");
        }
        weights[id] = significand << shift;
    }
    weights
        .into_iter()
        .try_fold(0_i128, i128::checked_add)
        .ok_or("Exact laboratory rate sum exceeds i128.")?;
    Ok(weights)
}

/// Convert only exactly representable, nonnegative f64 multiples of 2^-56 m³.
/// A production version would need a policy for all other inputs and geometry.
fn exact_units(volume_cubic_meters: f64) -> Result<i128, &'static str> {
    if !volume_cubic_meters.is_finite() || volume_cubic_meters.is_sign_negative() {
        return Err("Invalid fixed-point volume.");
    }
    if volume_cubic_meters == 0. {
        return Ok(0);
    }
    let bits = volume_cubic_meters.to_bits();
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if exponent_bits == 0 {
        (fraction, -1022)
    } else {
        ((1_u64 << 52) | fraction, exponent_bits - 1023)
    };
    let shift = exponent - 52 + FRACTION_BITS as i32;
    if shift >= 0 {
        let shift = shift as u32;
        if shift >= 127 || (significand as u128) > ((i128::MAX as u128) >> shift) {
            return Err("Fixed-point volume overflows i128.");
        }
        Ok((significand as i128) << shift)
    } else {
        let right = (-shift) as u32;
        if right >= 64 || significand & ((1_u64 << right) - 1) != 0 {
            return Err("Volume is below the fixed-point unit.");
        }
        Ok((significand >> right) as i128)
    }
}

#[test]
fn the_declared_unit_covers_the_volume_recipe_ceiling_but_rejects_smaller_inputs() {
    let max_radius_meters = 20_000_000.0_f64;
    let max_global_layer_meters = 20_000.0_f64;
    let volume_recipe_ceiling =
        4. * std::f64::consts::PI * max_radius_meters.powi(2) * max_global_layer_meters;
    let ceiling_units = exact_units(volume_recipe_ceiling).unwrap();
    assert!(ceiling_units > 0);
    assert!(ceiling_units.checked_mul(16).is_some());
    assert_eq!(exact_units(1. / (1_u64 << FRACTION_BITS) as f64), Ok(1));
    assert_eq!(
        exact_units(1e-18),
        Err("Volume is below the fixed-point unit.")
    );
    assert_eq!(
        exact_units(f64::MAX),
        Err("Fixed-point volume overflows i128.")
    );
    assert_eq!(exact_units(-0.), Err("Invalid fixed-point volume."));
}

#[test]
fn sub_ulp_grants_accumulate_exactly_at_large_stocks() {
    let initial = 500_000_000_000_000.;
    let grant = 1. / 1024.;
    assert_eq!(initial + grant, initial);
    let mut stock = exact_units(initial).unwrap();
    let grant_units = exact_units(grant).unwrap();
    for _ in 0..1024 {
        stock = stock.checked_add(grant_units).unwrap();
    }
    assert_eq!(stock, exact_units(initial + 1.).unwrap());
}

#[test]
fn exact_units_cross_the_tied_followup_but_still_break_label_symmetry() {
    let capacity = exact_units(100. / 3.).unwrap();
    let supplied = exact_units(100.).unwrap();
    let aggregate_headroom = capacity.checked_mul(3).unwrap() - supplied;
    assert_eq!(aggregate_headroom, 512);

    // Integer conservation requires 512 indivisible units of headroom to be
    // distributed among three otherwise identical receivers. It cannot make
    // their exact stocks equal because 512 is not divisible by three.
    let base = aggregate_headroom / 3;
    let deficits = [base, base + 1, base + 1];
    assert_eq!(deficits.iter().sum::<i128>(), aggregate_headroom);
    let stocks = deficits.map(|deficit| capacity - deficit);
    assert_eq!(stocks.iter().sum::<i128>(), supplied);
    assert!(stocks.iter().all(|stock| *stock < capacity));

    // Unlike the two-float experiment, every equal follow-up grant is kept.
    // Yet the canonical one-unit remainder determines which branch fills.
    let followup = base;
    let advanced = stocks.map(|stock| stock.checked_add(followup).unwrap());
    assert!(advanced.iter().all(|stock| *stock <= capacity));
    let full = advanced.map(|stock| stock == capacity);
    assert_eq!(full, [true, false, false]);
    assert_eq!(advanced.iter().sum::<i128>(), supplied + 3 * followup);
    assert_ne!([full[1], full[2], full[0]], full);
}

#[test]
fn a_shared_thirds_denominator_crosses_the_same_tie_symmetrically() {
    let capacity_units = exact_units(100. / 3.).unwrap();
    let initial_input_units = exact_units(100.).unwrap();
    let capacity_thirds = capacity_units.checked_mul(3).unwrap();
    let remaining_total_units = capacity_thirds - initial_input_units;
    assert_eq!(remaining_total_units, 512);

    // Each local stock owns its own numerator in thirds of a fixed-point
    // unit. An integer external input split equally adds the same numerator
    // to each child; summing three child numerators and dividing by three
    // recovers the exact external ledger without choosing a remainder owner.
    let mut children_thirds = [initial_input_units; 3];
    assert_eq!(
        children_thirds.iter().sum::<i128>() / 3,
        initial_input_units
    );
    assert!(children_thirds.iter().all(|stock| *stock < capacity_thirds));

    let first_followup_units = 510;
    for child in &mut children_thirds {
        *child = child.checked_add(first_followup_units).unwrap();
    }
    assert!(children_thirds.iter().all(|stock| *stock < capacity_thirds));
    assert_eq!(
        children_thirds.iter().sum::<i128>() / 3,
        initial_input_units + first_followup_units
    );
    assert_eq!(capacity_thirds - children_thirds[0], 2);

    let final_followup_units = 2;
    for child in &mut children_thirds {
        *child = child.checked_add(final_followup_units).unwrap();
    }
    assert_eq!(children_thirds, [capacity_thirds; 3]);
    assert_eq!(
        initial_input_units + first_followup_units + final_followup_units,
        capacity_units * 3
    );
}

#[test]
fn local_rationals_keep_the_ledger_when_one_child_breaks_the_tie() {
    let capacity = RationalUnits::integer(exact_units(100. / 3.).unwrap());
    let initial = RationalUnits::integer(exact_units(100.).unwrap());
    let shared = initial.split_equal(3).unwrap();
    let mut children = [shared; 3];
    let sum = |stocks: [RationalUnits; 3]| {
        stocks
            .into_iter()
            .try_fold(RationalUnits::integer(0), RationalUnits::checked_add)
            .unwrap()
    };
    assert_eq!(sum(children), initial);

    // This input is physically addressed to child zero, so subsequent
    // asymmetry is causal rather than introduced by a remainder owner.
    let local_input = RationalUnits::integer(1);
    children[0] = children[0].checked_add(local_input).unwrap();
    let common_input = RationalUnits::integer(509);
    let equal_share = common_input.split_equal(3).unwrap();
    for child in &mut children {
        *child = child.checked_add(equal_share).unwrap();
    }
    assert_eq!(children[0], capacity);
    assert_eq!(
        capacity.checked_sub(children[1]),
        Ok(RationalUnits::integer(1))
    );
    assert_eq!(
        capacity.checked_sub(children[2]),
        Ok(RationalUnits::integer(1))
    );
    assert_eq!(
        sum(children),
        initial
            .checked_add(local_input)
            .unwrap()
            .checked_add(common_input)
            .unwrap()
    );

    children[1] = children[1].checked_add(RationalUnits::integer(1)).unwrap();
    children[2] = children[2].checked_add(RationalUnits::integer(1)).unwrap();
    assert_eq!(children, [capacity; 3]);
    assert_eq!(
        sum(children),
        RationalUnits::integer(capacity.numerator * 3)
    );
}

#[test]
fn repeated_exact_splits_hit_an_explicit_denominator_bound() {
    let mut branch = RationalUnits::integer(1);
    let mut successful_splits = 0;
    loop {
        let before = branch;
        match branch.split_equal(3) {
            Ok(share) => {
                let reconstructed = [share; 3]
                    .into_iter()
                    .try_fold(RationalUnits::integer(0), RationalUnits::checked_add)
                    .unwrap();
                assert_eq!(reconstructed, before);
                branch = share;
                successful_splits += 1;
            }
            Err(error) => {
                assert_eq!(error, "Rational water denominator overflowed.");
                assert_eq!(branch, before);
                break;
            }
        }
    }
    assert_eq!(successful_splits, 80);
    assert!(branch.denominator > 1_000_000_000_000_000_000);
}

#[test]
fn exact_event_time_keeps_a_tie_and_a_causally_broken_tie_distinct() {
    let capacity = exact_units(100. / 3.).unwrap();
    let initial = RationalUnits::integer(exact_units(100.).unwrap())
        .split_equal(3)
        .unwrap();
    let stocks = [initial; 3];
    let (coordinate, next, full) = first_exact_event(stocks, [capacity; 3], [1; 3]).unwrap();
    assert_eq!(coordinate, RationalUnits::integer(512));
    assert_eq!(next, [RationalUnits::integer(capacity); 3]);
    assert_eq!(full, [true; 3]);

    let mut physically_broken = stocks;
    physically_broken[0] = physically_broken[0]
        .checked_add(RationalUnits::integer(1))
        .unwrap();
    let (coordinate, next, full) =
        first_exact_event(physically_broken, [capacity; 3], [1; 3]).unwrap();
    assert_eq!(coordinate, RationalUnits::integer(509));
    assert_eq!(full, [true, false, false]);
    assert_eq!(next[0], RationalUnits::integer(capacity));
    assert_eq!(
        RationalUnits::integer(capacity).checked_sub(next[1]),
        Ok(RationalUnits::integer(1))
    );
    assert_eq!(next[1], next[2]);

    for permutation in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let (other_coordinate, permuted_next, permuted_full) = first_exact_event(
            permutation.map(|id| physically_broken[id]),
            [capacity; 3],
            [1; 3],
        )
        .unwrap();
        assert_eq!(other_coordinate, coordinate);
        let mut physical_next = [RationalUnits::integer(0); 3];
        let mut physical_full = [false; 3];
        for (slot, id) in permutation.into_iter().enumerate() {
            physical_next[id] = permuted_next[slot];
            physical_full[id] = permuted_full[slot];
        }
        assert_eq!(physical_next, next);
        assert_eq!(physical_full, full);
    }
}

#[test]
fn exact_event_time_marks_simultaneous_weighted_limits_without_id_priority() {
    let stocks = [RationalUnits::integer(0); 3];
    let capacities = [10, 20, 60];
    let rates = [1, 2, 3];
    let (coordinate, next, full) = first_exact_event(stocks, capacities, rates).unwrap();
    assert_eq!(coordinate, RationalUnits::integer(60));
    assert_eq!(next, [10, 20, 30].map(RationalUnits::integer));
    assert_eq!(full, [true, true, false]);
    for permutation in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let (other_coordinate, permuted_next, permuted_full) = first_exact_event(
            permutation.map(|id| stocks[id]),
            permutation.map(|id| capacities[id]),
            permutation.map(|id| rates[id]),
        )
        .unwrap();
        let mut physical_next = stocks;
        let mut physical_full = [false; 3];
        for (slot, id) in permutation.into_iter().enumerate() {
            physical_next[id] = permuted_next[slot];
            physical_full[id] = permuted_full[slot];
        }
        assert_eq!(other_coordinate, coordinate);
        assert_eq!(physical_next, next);
        assert_eq!(physical_full, full);
    }
    assert_eq!(
        first_exact_event(stocks, [10, 20, 60], [i128::MAX, 1, 1]),
        Err("Laboratory rate sum overflowed.")
    );
}

#[test]
fn finite_binary64_rates_have_exact_weights_only_within_a_bounded_range() {
    assert_eq!(exact_rate_weights([1., 1., 2.]), Ok([1, 1, 2]));
    let next = 1_f64.next_up();
    assert_eq!(
        exact_rate_weights([1., next, 2.]),
        Ok([1_i128 << 52, (1_i128 << 52) + 1, 1_i128 << 53])
    );
    assert_eq!(
        exact_rate_weights([0., 1., 2.]),
        Err("Invalid laboratory rate.")
    );
    assert_eq!(
        exact_rate_weights([f64::NAN, 1., 2.]),
        Err("Invalid laboratory rate.")
    );
    let smallest_subnormal = f64::from_bits(1);
    assert_eq!(
        exact_rate_weights([smallest_subnormal, f64::from_bits(2), smallest_subnormal]),
        Ok([1, 2, 1])
    );
    assert_eq!(
        exact_rate_weights([1e-300, 1., 1e300]),
        Err("Exact laboratory rate weights exceed i128.")
    );
    // The unreduced 3 * 2^126 does not fit i128, but the exact rate ratio does.
    assert_eq!(
        exact_rate_weights([3., 3. * 2_f64.powi(126), 3.]),
        Ok([1, 1_i128 << 126, 1])
    );
    assert_eq!(
        exact_rate_weights([1., 2_f64.powi(126), 2_f64.powi(126)]),
        Err("Exact laboratory rate sum exceeds i128.")
    );

    let rates = exact_rate_weights([1., next, 1.]).unwrap();
    let (_, stocks, full) =
        first_exact_event([RationalUnits::integer(0); 3], [10; 3], rates).unwrap();
    assert_eq!(full, [false, true, false]);
    assert_eq!(stocks[1], RationalUnits::integer(10));
}
