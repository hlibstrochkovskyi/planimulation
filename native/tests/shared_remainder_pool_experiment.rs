//! Test-only, closed three-recipient ownership experiment. A pool's entire
//! input is owned by its recipient set, rather than assigning an indivisible
//! remainder to one branch ID. It does not model geography or timed flow.
use planimulation_core::exact_initial_accounting::exact_units;
use serde::{Deserialize, Serialize};

const VERSION: &str = "shared-remainder-pool-lab-1";
const CHILDREN: usize = 3;
const POOLS: usize = 1 << CHILDREN;
const SHARE_DENOMINATOR: i128 = 6; // lcm(1, 2, 3)

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PoolState {
    version: String,
    child_capacity_units: i128,
    /// pools[mask] is exact whole-unit input jointly owned by mask's children.
    pools: [i128; POOLS],
    parent_units: Option<i128>,
    input_units: i128,
    event_count: u64,
}

impl PoolState {
    fn new(child_capacity_units: i128) -> Result<Self, &'static str> {
        let state = Self {
            version: VERSION.into(),
            child_capacity_units,
            pools: [0; POOLS],
            parent_units: None,
            input_units: 0,
            event_count: 0,
        };
        state.validate()?;
        Ok(state)
    }

    fn child_stock_sixths(&self, child: usize) -> Result<i128, &'static str> {
        if child >= CHILDREN || self.parent_units.is_some() {
            return Err("Child stock is not on the active frontier.");
        }
        let mut stock = 0_i128;
        for mask in 1..POOLS {
            if mask & (1 << child) == 0 {
                continue;
            }
            let divisor = (mask as u8).count_ones() as i128;
            let factor = SHARE_DENOMINATOR / divisor;
            stock = stock
                .checked_add(
                    self.pools[mask]
                        .checked_mul(factor)
                        .ok_or("Shared pool stock overflowed.")?,
                )
                .ok_or("Shared pool stock overflowed.")?;
        }
        Ok(stock)
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.version != VERSION
            || self.child_capacity_units <= 0
            || self
                .child_capacity_units
                .checked_mul(SHARE_DENOMINATOR)
                .is_none()
            || self
                .child_capacity_units
                .checked_mul(CHILDREN as i128)
                .is_none()
            || self.pools[0] != 0
            || self.pools.iter().any(|&amount| amount < 0)
            || self.input_units < 0
        {
            return Err("Invalid shared-pool checkpoint.");
        }
        let pool_total = self
            .pools
            .iter()
            .try_fold(0_i128, |sum, &amount| sum.checked_add(amount))
            .ok_or("Shared pool ledger overflowed.")?;
        if let Some(parent) = self.parent_units {
            if pool_total != 0
                || parent != self.input_units
                || parent < self.child_capacity_units * CHILDREN as i128
            {
                return Err("Invalid shared-pool parent frontier.");
            }
        } else {
            if pool_total != self.input_units {
                return Err("Shared pool input ledger does not balance.");
            }
            let capacity = self.child_capacity_units * SHARE_DENOMINATOR;
            let mut all_full = true;
            let mut stored_sixths = 0_i128;
            for child in 0..CHILDREN {
                let stock = self.child_stock_sixths(child)?;
                if stock > capacity {
                    return Err("Shared pool child exceeds capacity.");
                }
                stored_sixths = stored_sixths
                    .checked_add(stock)
                    .ok_or("Shared pool stock sum overflowed.")?;
                all_full &= stock == capacity;
            }
            if stored_sixths % SHARE_DENOMINATOR != 0
                || stored_sixths / SHARE_DENOMINATOR != self.input_units
            {
                return Err("Shared pool child stocks do not balance.");
            }
            if all_full {
                return Err("Full shared-pool children must merge.");
            }
        }
        Ok(())
    }

    fn restore(self) -> Result<Self, &'static str> {
        self.validate()?;
        Ok(self)
    }

    fn apply(&mut self, mask: usize, input_units: i128) -> Result<(), &'static str> {
        if self.parent_units.is_some() || !(1..POOLS).contains(&mask) || input_units <= 0 {
            return Err("Invalid shared-pool input or recipient set.");
        }
        let mut next = self.clone();
        next.pools[mask] = next.pools[mask]
            .checked_add(input_units)
            .ok_or("Shared pool amount overflowed.")?;
        next.input_units = next
            .input_units
            .checked_add(input_units)
            .ok_or("Shared pool input overflowed.")?;
        next.event_count = next
            .event_count
            .checked_add(1)
            .ok_or("Event count overflowed.")?;
        let capacity = next.child_capacity_units * SHARE_DENOMINATOR;
        let mut full = true;
        for child in 0..CHILDREN {
            let stock = next.child_stock_sixths(child)?;
            if stock > capacity {
                return Err("Shared pool child exceeds capacity.");
            }
            full &= stock == capacity;
        }
        if full {
            next.parent_units = Some(next.input_units);
            next.pools = [0; POOLS];
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Exact first threshold for equal sharing to this set, if the required
    /// external input is an integer number of declared fixed-point units.
    fn advance_to_first_threshold(&mut self, mask: usize) -> Result<i128, &'static str> {
        if self.parent_units.is_some() || !(1..POOLS).contains(&mask) {
            return Err("Invalid shared-pool threshold set.");
        }
        let members = (mask as u8).count_ones() as i128;
        let capacity = self.child_capacity_units * SHARE_DENOMINATOR;
        let mut first_sixths = None;
        for child in 0..CHILDREN {
            if mask & (1 << child) == 0 {
                continue;
            }
            let stock = self.child_stock_sixths(child)?;
            if stock >= capacity {
                return Err("Shared-pool recipient is already full.");
            }
            let input_sixths = (capacity - stock)
                .checked_mul(members)
                .ok_or("Shared-pool threshold overflowed.")?;
            first_sixths =
                Some(first_sixths.map_or(input_sixths, |old: i128| old.min(input_sixths)));
        }
        let first_sixths = first_sixths.ok_or("Empty shared-pool threshold set.")?;
        if first_sixths % SHARE_DENOMINATOR != 0 {
            return Err("Threshold input is below the declared fixed-point unit.");
        }
        let input = first_sixths / SHARE_DENOMINATOR;
        self.apply(mask, input)?;
        Ok(input)
    }
}

#[test]
fn shared_group_remainder_preserves_symmetric_saturation_and_exact_merge() {
    let capacity = exact_units(100. / 3.).unwrap();
    let initial = exact_units(100.).unwrap();
    let mut state = PoolState::new(capacity).unwrap();
    state.apply(0b111, initial).unwrap();
    assert_eq!(state.pools[0b111], initial);
    assert_eq!(state.child_stock_sixths(0), state.child_stock_sixths(1));
    assert_eq!(state.child_stock_sixths(1), state.child_stock_sixths(2));
    assert_eq!(state.advance_to_first_threshold(0b111), Ok(512));
    assert_eq!(state.parent_units, Some(capacity * 3));
    assert_eq!(state.input_units, capacity * 3);
    assert_eq!(state.pools, [0; POOLS]);
}

#[test]
fn local_tie_break_and_subset_pool_replay_identically_under_relabeling() {
    let capacity = exact_units(100. / 3.).unwrap();
    let initial = exact_units(100.).unwrap();
    for permutation in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut state = PoolState::new(capacity).unwrap();
        let local = permutation
            .iter()
            .position(|&physical| physical == 0)
            .unwrap();
        state.apply(0b111, initial).unwrap();
        state.apply(1 << local, 1).unwrap();
        assert_eq!(state.advance_to_first_threshold(0b111), Ok(509));
        assert!(state.parent_units.is_none());
        let mut physical_stocks = [0_i128; CHILDREN];
        for (slot, physical) in permutation.into_iter().enumerate() {
            physical_stocks[physical] = state.child_stock_sixths(slot).unwrap();
        }
        assert_eq!(
            physical_stocks,
            [capacity * 6, (capacity - 1) * 6, (capacity - 1) * 6]
        );
        let before_rejection = state.clone();
        assert_eq!(
            state.advance_to_first_threshold(0b111),
            Err("Shared-pool recipient is already full.")
        );
        assert_eq!(state, before_rejection);

        let serialized = serde_json::to_string(&state).unwrap();
        state = serde_json::from_str::<PoolState>(&serialized)
            .unwrap()
            .restore()
            .unwrap();
        let remaining = 0b111 ^ (1 << local);
        assert_eq!(state.advance_to_first_threshold(remaining), Ok(2));
        assert_eq!(state.parent_units, Some(capacity * 3));
        assert_eq!(state.input_units, capacity * 3);
        assert_eq!(state.event_count, 4);
    }
}

#[test]
fn unsupported_thresholds_and_corrupt_owned_remainders_reject_atomically() {
    let mut state = PoolState::new(2).unwrap();
    state.apply(0b111, 1).unwrap();
    let before_rejection = state.clone();
    assert_eq!(
        state.advance_to_first_threshold(0b011),
        Err("Threshold input is below the declared fixed-point unit.")
    );
    assert_eq!(state, before_rejection);
    assert_eq!(
        state.apply(0, 1),
        Err("Invalid shared-pool input or recipient set.")
    );
    assert_eq!(state, before_rejection);
    assert_eq!(
        state.apply(0b001, 3),
        Err("Shared pool child exceeds capacity.")
    );
    assert_eq!(state, before_rejection);

    let mut invalid = state.clone();
    invalid.input_units += 1;
    assert_eq!(
        invalid.restore(),
        Err("Shared pool input ledger does not balance.")
    );
    let mut invalid = state.clone();
    invalid.pools[0] = 1;
    assert_eq!(invalid.restore(), Err("Invalid shared-pool checkpoint."));
    let mut invalid = state.clone();
    invalid.version = "future".into();
    assert_eq!(invalid.restore(), Err("Invalid shared-pool checkpoint."));
    let mut invalid = state.clone();
    invalid.parent_units = Some(1);
    assert_eq!(
        invalid.restore(),
        Err("Invalid shared-pool parent frontier.")
    );
}
