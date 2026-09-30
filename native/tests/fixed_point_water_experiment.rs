//! Isolated fixed-point feasibility experiment, not a production water model.
//! Integer units remove stock-addition roundoff within a declared range but
//! cannot make an indivisible remainder symmetric among identical receivers.

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
