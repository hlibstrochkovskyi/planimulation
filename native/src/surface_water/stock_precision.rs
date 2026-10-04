//! A normalized two-component stock, not a spare reservoir or final repair.
//! Small increments are retained in the same owned stock before later updates.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stock {
    pub high: f64,
    pub low: f64,
}
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let sum = a + b;
    let b_virtual = sum - a;
    (sum, (a - (sum - b_virtual)) + (b - b_virtual))
}
impl Stock {
    pub fn new(high: f64, low: f64, capacity: f64) -> Result<Self, String> {
        let (normalized, remainder) = two_sum(high, low);
        if !high.is_finite()
            || !low.is_finite()
            || !capacity.is_finite()
            || capacity <= 0.
            || high < 0.
            || high > capacity
            || normalized != high
            || remainder != low
            || (high == 0. && low != 0.)
            || (high == capacity && low > 0.)
        {
            return Err("Invalid compensated stock or normalization.".into());
        }
        Ok(Self { high, low })
    }
    /// Add to this stock only. The caller must account for the opposite transfer.
    fn add(&mut self, increment: f64) {
        let (sum, error) = two_sum(self.high, increment);
        (self.high, self.low) = two_sum(sum, self.low + error);
    }
    /// Uncapped input (snowfall): overflow rejects rather than clipping the input.
    pub fn credit(&mut self, increment: f64) -> Result<(), String> {
        self.add(increment);
        Self::new(self.high, self.low, f64::MAX).map(|_| ())
    }
    /// Largest representable value no greater than the represented stock.
    fn floor(self) -> f64 {
        if self.low < 0. {
            self.high.next_down()
        } else {
            self.high
        }
    }
    pub fn available(self) -> f64 {
        self.floor()
    }
    pub fn withdraw(&mut self, request: f64) -> f64 {
        let grant = request.min(self.floor());
        self.add(-grant);
        grant
    }
    pub fn deposit(&mut self, request: f64, capacity: f64) -> f64 {
        let (high, low) = two_sum(capacity, -self.high);
        let mut room = Self { high, low };
        room.add(-self.low);
        let grant = request.min(room.floor());
        self.add(grant);
        grant
    }
}

pub(crate) fn validate(high: f64, low: f64, capacity: f64) -> Result<(), String> {
    Stock::new(high, low, capacity).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tiny_changes_survive_and_signed_low_parts_cannot_overdraw_or_overfill() {
        let mut stock = Stock::new(2_f64.powi(53), 0., 2_f64.powi(55)).unwrap();
        for _ in 0..1000 {
            assert_eq!(stock.deposit(1., 2_f64.powi(55)), 1.);
        }
        assert_eq!(stock.high, 2_f64.powi(53) + 1000.);
        assert_eq!(stock.low, 0.);
        let mut negative = Stock::new(1., -2_f64.powi(-54), 2.).unwrap();
        assert_eq!(negative.withdraw(1.), 1_f64.next_down());
        assert_eq!(negative.high, 2_f64.powi(-54));
        assert_eq!(negative.low, 0.);
        let mut positive = Stock::new(1., 2_f64.powi(-54), 2.).unwrap();
        assert_eq!(positive.withdraw(1.), 1.);
        assert_eq!(positive.high, 2_f64.powi(-54));
        assert_eq!(positive.low, 0.);
        let mut nearly_full = Stock::new(2., -2_f64.powi(-54), 2.).unwrap();
        assert_eq!(nearly_full.deposit(1., 2.), 2_f64.powi(-54));
        assert_eq!(nearly_full.high, 2.);
        assert_eq!(nearly_full.low, 0.);
    }
    #[test]
    fn malformed_components_reject() {
        for (high, low) in [
            (0., 1e-30),
            (1., 0.1),
            (2., 1e-30),
            (1., f64::NAN),
            (-1., 0.),
        ] {
            assert!(Stock::new(high, low, 2.).is_err());
        }
    }
    #[test]
    fn many_small_updates_match_an_independent_integer_oracle() {
        let mut exact = 1_i128 << 70;
        let capacity = (1_u128 << 72) as f64;
        let mut soil = Stock::new(exact as f64, 0., capacity).unwrap();
        let mut random = 17_u64;
        let mut retained_low = false;
        for _ in 0..100_000 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let amount = ((random >> 32) % 1000 + 1) as i128;
            if random & 1 == 0 {
                if random & 2 == 0 {
                    soil.credit(amount as f64).unwrap();
                } else {
                    assert_eq!(soil.deposit(amount as f64, capacity), amount as f64);
                }
                exact += amount;
            } else {
                assert_eq!(soil.withdraw(amount as f64), amount as f64);
                exact -= amount;
            }
            assert_eq!(soil.high as i128 + soil.low as i128, exact);
            assert_eq!(soil.low, (soil.low as i128) as f64);
            Stock::new(soil.high, soil.low, capacity).unwrap();
            retained_low |= soil.low != 0.;
        }
        assert!(retained_low);
    }

    #[test]
    fn uncapped_credit_rejects_overflow_and_retains_subnormal_mass() {
        let mut stock = Stock::new(f64::MAX, 0., f64::MAX).unwrap();
        assert!(stock.credit(f64::MAX).is_err());
        let tiny = f64::from_bits(1);
        let mut stock = Stock::new(tiny, 0., f64::MAX).unwrap();
        stock.credit(tiny).unwrap();
        assert_eq!(stock.withdraw(tiny), tiny);
        assert_eq!(stock.high, tiny);
        assert_eq!(stock.withdraw(tiny), tiny);
        assert_eq!(stock.high, 0.);
        assert_eq!(stock.low, 0.);
    }
}
