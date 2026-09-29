//! Isolated fixed-point feasibility experiment, not a production water model.
//! Integer units remove stock-addition roundoff within a declared range but
//! cannot make an indivisible remainder symmetric among identical receivers.

const FRACTION_BITS: u32 = 56;

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
