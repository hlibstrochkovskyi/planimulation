//! Test-only ownership experiment for simultaneous pools with different
//! recipient sets across a partial merge. It has no geographic routing,
//! timed rates, generated geometry, or production checkpoint contract.
use serde::{Deserialize, Serialize};

const VERSION: &str = "multi-pool-merge-lab-1";
const BRANCHES: usize = 5;
const LEAVES: usize = 3;
const SCALE: i128 = 6; // common denominator for accepted weight sums 1, 2, 3, 6
const MAX_POOLS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pool {
    units: i128,
    /// A recipient's share is units × weight / sum(weights).
    weights: [u8; BRANCHES],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MultiPool {
    version: String,
    /// Physical A and B merge first; physical C remains separate.
    physical_by_slot: [u8; LEAVES],
    active: [bool; BRANCHES],
    /// Canonically ordered and coalesced by weight vector.
    pools: Vec<Pool>,
    input_units: i128,
    event_count: u64,
}

#[derive(Clone, Debug)]
struct Geometry {
    capacities: [i128; BRANCHES],
    children: [Vec<usize>; BRANCHES],
}

fn geometry(physical_by_slot: [u8; LEAVES]) -> Result<Geometry, &'static str> {
    let mut seen = [false; LEAVES];
    for &physical in &physical_by_slot {
        let physical = physical as usize;
        if physical >= LEAVES || seen[physical] {
            return Err("Invalid multi-pool physical labeling.");
        }
        seen[physical] = true;
    }
    let slot = |physical: u8| {
        physical_by_slot
            .iter()
            .position(|&label| label == physical)
            .unwrap()
    };
    Ok(Geometry {
        capacities: std::array::from_fn(|branch| match branch {
            0..LEAVES if physical_by_slot[branch] == 2 => 5,
            0..LEAVES => 4,
            3 => 10,
            _ => 15,
        }),
        children: [
            vec![],
            vec![],
            vec![],
            vec![slot(0), slot(1)],
            vec![3, slot(2)],
        ],
    })
}

fn total_weight(weights: &[u8; BRANCHES]) -> Result<i128, &'static str> {
    let sum: i128 = weights.iter().map(|&weight| i128::from(weight)).sum();
    if sum == 0 || SCALE % sum != 0 {
        return Err("Unsupported multi-pool share denominator.");
    }
    Ok(sum)
}

fn canonical_weights(mut weights: [u8; BRANCHES]) -> [u8; BRANCHES] {
    let mut divisor = 0_u8;
    for &weight in &weights {
        if weight == 0 {
            continue;
        }
        let mut a = divisor;
        let mut b = weight;
        while b != 0 {
            (a, b) = (b, a % b);
        }
        divisor = a;
    }
    if divisor > 1 {
        for weight in &mut weights {
            *weight /= divisor;
        }
    }
    weights
}

impl MultiPool {
    fn new(physical_by_slot: [u8; LEAVES]) -> Result<Self, &'static str> {
        geometry(physical_by_slot)?;
        let state = Self {
            version: VERSION.into(),
            physical_by_slot,
            active: [true, true, true, false, false],
            pools: vec![],
            input_units: 0,
            event_count: 0,
        };
        state.validate()?;
        Ok(state)
    }

    fn stock_sixths(&self, branch: usize) -> Result<i128, &'static str> {
        if branch >= BRANCHES || !self.active[branch] {
            return Err("Inactive multi-pool branch has no stock.");
        }
        let mut stock = 0_i128;
        for pool in &self.pools {
            let divisor = total_weight(&pool.weights)?;
            let factor = i128::from(pool.weights[branch]) * (SCALE / divisor);
            stock = stock
                .checked_add(
                    pool.units
                        .checked_mul(factor)
                        .ok_or("Multi-pool stock overflowed.")?,
                )
                .ok_or("Multi-pool stock overflowed.")?;
        }
        Ok(stock)
    }

    fn validate(&self) -> Result<(), &'static str> {
        let geometry = geometry(self.physical_by_slot)?;
        if self.version != VERSION || self.pools.len() > MAX_POOLS || self.input_units < 0 {
            return Err("Invalid multi-pool checkpoint.");
        }
        let mut previous = None;
        let mut pool_total = 0_i128;
        for pool in &self.pools {
            if pool.units <= 0
                || pool.weights != canonical_weights(pool.weights)
                || previous.is_some_and(|prior| pool.weights <= prior)
                || pool
                    .weights
                    .iter()
                    .enumerate()
                    .any(|(branch, &weight)| weight > 0 && !self.active[branch])
            {
                return Err("Invalid multi-pool ownership.");
            }
            total_weight(&pool.weights)?;
            pool_total = pool_total
                .checked_add(pool.units)
                .ok_or("Multi-pool ledger overflowed.")?;
            previous = Some(pool.weights);
        }
        if pool_total != self.input_units {
            return Err("Multi-pool input ledger does not balance.");
        }

        let mut covered = [false; BRANCHES];
        let mut stored_sixths = 0_i128;
        for branch in 0..BRANCHES {
            if self.active[branch] {
                if geometry.children[branch]
                    .iter()
                    .any(|&child| covered[child])
                {
                    return Err("Invalid multi-pool exclusive frontier.");
                }
                let stock = self.stock_sixths(branch)?;
                if stock > geometry.capacities[branch] * SCALE {
                    return Err("Multi-pool branch exceeds capacity.");
                }
                stored_sixths = stored_sixths
                    .checked_add(stock)
                    .ok_or("Multi-pool stock sum overflowed.")?;
                covered[branch] = true;
            } else if !geometry.children[branch].is_empty() {
                let mut all_full = true;
                for &child in &geometry.children[branch] {
                    if !self.active[child]
                        || self.stock_sixths(child)? != geometry.capacities[child] * SCALE
                    {
                        all_full = false;
                        break;
                    }
                }
                if all_full {
                    return Err("Full multi-pool children must merge.");
                }
                covered[branch] = geometry.children[branch]
                    .iter()
                    .all(|&child| covered[child]);
            }
        }
        if !covered[BRANCHES - 1] {
            return Err("Multi-pool frontier does not cover the root.");
        }
        if stored_sixths % SCALE != 0 || stored_sixths / SCALE != self.input_units {
            return Err("Multi-pool stocks do not balance the input ledger.");
        }
        Ok(())
    }

    fn restore(self) -> Result<Self, &'static str> {
        self.validate()?;
        Ok(self)
    }

    fn coalesce(&mut self) -> Result<(), &'static str> {
        for pool in &mut self.pools {
            pool.weights = canonical_weights(pool.weights);
        }
        self.pools.sort_by_key(|pool| pool.weights);
        let mut canonical: Vec<Pool> = Vec::with_capacity(self.pools.len());
        for pool in self.pools.drain(..) {
            if let Some(last) = canonical.last_mut()
                && last.weights == pool.weights
            {
                last.units = last
                    .units
                    .checked_add(pool.units)
                    .ok_or("Multi-pool coalescing overflowed.")?;
                continue;
            }
            canonical.push(pool);
        }
        if canonical.len() > MAX_POOLS {
            return Err("Too many distinct multi-pool ownership groups.");
        }
        self.pools = canonical;
        Ok(())
    }

    fn merge_full_children(&mut self) -> Result<(), &'static str> {
        let geometry = geometry(self.physical_by_slot)?;
        for parent in LEAVES..BRANCHES {
            let children = &geometry.children[parent];
            let mut all_full = true;
            for &child in children {
                if !self.active[child]
                    || self.stock_sixths(child)? != geometry.capacities[child] * SCALE
                {
                    all_full = false;
                    break;
                }
            }
            if !all_full {
                continue;
            }
            for pool in &mut self.pools {
                let mut weight = pool.weights[parent];
                for &child in children {
                    weight = weight
                        .checked_add(pool.weights[child])
                        .ok_or("Multi-pool merge weight overflowed.")?;
                    pool.weights[child] = 0;
                }
                pool.weights[parent] = weight;
            }
            for &child in children {
                self.active[child] = false;
            }
            self.active[parent] = true;
            self.coalesce()?;
        }
        Ok(())
    }

    fn apply_pool(&mut self, weights: [u8; BRANCHES], units: i128) -> Result<(), &'static str> {
        if units <= 0 {
            return Err("Invalid multi-pool input.");
        }
        let weights = canonical_weights(weights);
        total_weight(&weights)?;
        if weights
            .iter()
            .enumerate()
            .any(|(branch, &weight)| weight > 0 && !self.active[branch])
        {
            return Err("Input has an inactive multi-pool recipient.");
        }
        let mut next = self.clone();
        next.pools.push(Pool { units, weights });
        next.coalesce()?;
        next.input_units = next
            .input_units
            .checked_add(units)
            .ok_or("Multi-pool input overflowed.")?;
        next.event_count = next
            .event_count
            .checked_add(1)
            .ok_or("Event count overflowed.")?;
        let geometry = geometry(next.physical_by_slot)?;
        for branch in 0..BRANCHES {
            if next.active[branch]
                && next.stock_sixths(branch)? > geometry.capacities[branch] * SCALE
            {
                return Err("Multi-pool branch exceeds capacity.");
            }
        }
        next.merge_full_children()?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// First capacity endpoint in external whole-unit coordinates for one
    /// fixed positive weight vector. This is not elapsed time or a rate law.
    fn advance_to_first_limit(&mut self, weights: [u8; BRANCHES]) -> Result<i128, &'static str> {
        self.validate()?;
        let weights = canonical_weights(weights);
        let total = total_weight(&weights)?;
        let geometry = geometry(self.physical_by_slot)?;
        let mut first: Option<(i128, i128)> = None;
        for (branch, &weight) in weights.iter().enumerate() {
            if weight == 0 {
                continue;
            }
            if !self.active[branch] {
                return Err("Rate targets an inactive multi-pool branch.");
            }
            let deficit_sixths = geometry.capacities[branch] * SCALE - self.stock_sixths(branch)?;
            if deficit_sixths <= 0 {
                return Err("Rate targets an already full multi-pool branch.");
            }
            let numerator = deficit_sixths
                .checked_mul(total)
                .ok_or("Multi-pool event coordinate overflowed.")?;
            let denominator = SCALE * i128::from(weight);
            let earlier = match first {
                None => true,
                Some((old_numerator, old_denominator)) => {
                    numerator
                        .checked_mul(old_denominator)
                        .ok_or("Multi-pool event comparison overflowed.")?
                        < old_numerator
                            .checked_mul(denominator)
                            .ok_or("Multi-pool event comparison overflowed.")?
                }
            };
            if earlier {
                first = Some((numerator, denominator));
            }
        }
        let (numerator, denominator) = first.ok_or("Empty multi-pool rate group.")?;
        if numerator % denominator != 0 {
            return Err("First multi-pool limit is not representable in whole units.");
        }
        let input = numerator / denominator;
        if input <= 0 {
            return Err("First multi-pool limit has no positive input.");
        }
        let mut reaches_limit = false;
        for (branch, &weight) in weights.iter().enumerate() {
            if weight == 0 {
                continue;
            }
            let grant_sixths = input
                .checked_mul(i128::from(weight))
                .and_then(|value| value.checked_mul(SCALE / total))
                .ok_or("Multi-pool event grant overflowed.")?;
            reaches_limit |= self
                .stock_sixths(branch)?
                .checked_add(grant_sixths)
                .ok_or("Multi-pool event grant overflowed.")?
                == geometry.capacities[branch] * SCALE;
        }
        if !reaches_limit {
            return Err("Multi-pool event has no exact limiting branch.");
        }
        self.apply_pool(weights, input)?;
        Ok(input)
    }
}

fn leaf_weights(permutation: [u8; LEAVES], physical: &[u8]) -> [u8; BRANCHES] {
    let mut weights = [0; BRANCHES];
    for (slot, &label) in permutation.iter().enumerate() {
        if physical.contains(&label) {
            weights[slot] = 1;
        }
    }
    weights
}

#[test]
fn two_distinct_pools_survive_partial_merge_and_a_third_pool_finishes_the_root() {
    for permutation in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut state = MultiPool::new(permutation).unwrap();
        state
            .apply_pool(leaf_weights(permutation, &[0, 1, 2]), 9)
            .unwrap();
        let early = serde_json::to_string(&state).unwrap();
        state = serde_json::from_str::<MultiPool>(&early)
            .unwrap()
            .restore()
            .unwrap();
        state
            .apply_pool(leaf_weights(permutation, &[0, 1]), 2)
            .unwrap();

        let outside = permutation.iter().position(|&label| label == 2).unwrap();
        let mut expected_active = [false; BRANCHES];
        expected_active[outside] = true;
        expected_active[3] = true;
        assert_eq!(state.active, expected_active);
        assert_eq!(state.stock_sixths(3), Ok(48)); // 6 + 2 units
        assert_eq!(state.stock_sixths(outside), Ok(18)); // 3 units
        assert_eq!(state.input_units, 11);
        assert_eq!(state.pools.len(), 2);

        let before_rejection = state.clone();
        let mut overfilled_weights = [0; BRANCHES];
        overfilled_weights[3] = 2;
        overfilled_weights[outside] = 1;
        assert_eq!(
            state.apply_pool(overfilled_weights, 4),
            Err("Multi-pool branch exceeds capacity.")
        );
        assert_eq!(state, before_rejection);

        let middle = serde_json::to_string(&state).unwrap();
        state = serde_json::from_str::<MultiPool>(&middle)
            .unwrap()
            .restore()
            .unwrap();
        let mut final_weights = [0; BRANCHES];
        final_weights[3] = 1;
        final_weights[outside] = 1;
        state.apply_pool(final_weights, 4).unwrap();
        assert_eq!(state.active, [false, false, false, false, true]);
        assert_eq!(state.stock_sixths(4), Ok(90));
        assert_eq!(state.input_units, 15);
        assert_eq!(state.event_count, 3);
        assert_eq!(state.pools.len(), 1); // all singleton root claims coalesce
        assert_eq!(state.pools[0].units, 15);
    }
}

#[test]
fn changing_weights_recompute_first_limit_after_partial_merge() {
    for permutation in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut state = MultiPool::new(permutation).unwrap();
        state
            .apply_pool(leaf_weights(permutation, &[0, 1, 2]), 9)
            .unwrap();
        state
            .apply_pool(leaf_weights(permutation, &[0, 1]), 2)
            .unwrap();

        let outside = permutation.iter().position(|&label| label == 2).unwrap();
        let mut rates = [0; BRANCHES];
        rates[3] = 2;
        rates[outside] = 1;
        assert_eq!(state.advance_to_first_limit(rates), Ok(3));
        assert_eq!(state.stock_sixths(3), Ok(60));
        assert_eq!(state.stock_sixths(outside), Ok(24));
        assert_eq!(state.input_units, 14);
        assert!(state.active[3] && state.active[outside] && !state.active[4]);

        let before_rejection = state.clone();
        assert_eq!(
            state.advance_to_first_limit(rates),
            Err("Rate targets an already full multi-pool branch.")
        );
        assert_eq!(state, before_rejection);

        let checkpoint = serde_json::to_string(&state).unwrap();
        state = serde_json::from_str::<MultiPool>(&checkpoint)
            .unwrap()
            .restore()
            .unwrap();
        let mut remaining_rate = [0; BRANCHES];
        remaining_rate[outside] = 1;
        assert_eq!(state.advance_to_first_limit(remaining_rate), Ok(1));
        assert_eq!(state.active, [false, false, false, false, true]);
        assert_eq!(state.stock_sixths(4), Ok(90));
        assert_eq!(state.input_units, 15);
        assert_eq!(state.event_count, 4);
        assert_eq!(state.pools.len(), 1);
        assert_eq!(state.pools[0].units, 15);
    }
}

#[test]
fn first_limit_rejects_fractional_external_input_without_mutation() {
    let mut state = MultiPool::new([0, 1, 2]).unwrap();
    state.apply_pool([1, 1, 1, 0, 0], 1).unwrap();
    let before_rejection = state.clone();
    assert_eq!(
        state.advance_to_first_limit([1, 1, 0, 0, 0]),
        Err("First multi-pool limit is not representable in whole units.")
    );
    assert_eq!(state, before_rejection);
}

#[test]
fn unsupported_groups_and_corrupt_ownership_reject_atomically() {
    let mut state = MultiPool::new([0, 1, 2]).unwrap();
    let original = state.clone();
    assert_eq!(
        state.apply_pool([1, 1, 2, 0, 0], 1),
        Err("Unsupported multi-pool share denominator.")
    );
    assert_eq!(state, original);
    assert_eq!(
        state.apply_pool([0, 0, 0, 1, 0], 1),
        Err("Input has an inactive multi-pool recipient.")
    );
    assert_eq!(state, original);
    assert_eq!(
        state.apply_pool([1, 0, 0, 0, 0], i128::MAX),
        Err("Multi-pool stock overflowed.")
    );
    assert_eq!(state, original);
    state.apply_pool([1, 1, 1, 0, 0], 9).unwrap();
    let before_rejection = state.clone();
    assert_eq!(
        state.apply_pool([1, 1, 0, 0, 0], 3),
        Err("Multi-pool branch exceeds capacity.")
    );
    assert_eq!(state, before_rejection);

    let mut bad = state.clone();
    bad.input_units += 1;
    assert_eq!(
        bad.restore(),
        Err("Multi-pool input ledger does not balance.")
    );
    let mut bad = state.clone();
    bad.pools[0].weights[3] = 1;
    assert_eq!(bad.restore(), Err("Invalid multi-pool ownership."));
    let mut bad = state.clone();
    bad.pools[0].weights = [2, 2, 2, 0, 0];
    assert_eq!(bad.restore(), Err("Invalid multi-pool ownership."));
    let mut bad = state.clone();
    bad.pools.push(bad.pools[0].clone());
    assert_eq!(bad.restore(), Err("Invalid multi-pool ownership."));
    let mut bad = state.clone();
    bad.active[3] = true;
    assert_eq!(bad.restore(), Err("Invalid multi-pool exclusive frontier."));
    let mut bad = state.clone();
    bad.version = "future".into();
    assert_eq!(bad.restore(), Err("Invalid multi-pool checkpoint."));

    let mut bounded = MultiPool::new([0, 1, 2]).unwrap();
    bounded.apply_pool([1, 0, 0, 0, 0], 1).unwrap();
    bounded.apply_pool([0, 1, 0, 0, 0], 1).unwrap();
    bounded.apply_pool([0, 0, 1, 0, 0], 1).unwrap();
    bounded.apply_pool([1, 1, 0, 0, 0], 1).unwrap();
    assert_eq!(bounded.pools.len(), MAX_POOLS);

    let mut normalized = MultiPool::new([0, 1, 2]).unwrap();
    normalized.apply_pool([2, 2, 0, 0, 0], 1).unwrap();
    normalized.apply_pool([1, 1, 0, 0, 0], 1).unwrap();
    assert_eq!(normalized.pools.len(), 1);
    assert_eq!(normalized.pools[0].weights, [1, 1, 0, 0, 0]);
    assert_eq!(normalized.pools[0].units, 2);
    let before_rejection = bounded.clone();
    assert_eq!(
        bounded.apply_pool([1, 0, 1, 0, 0], 1),
        Err("Too many distinct multi-pool ownership groups.")
    );
    assert_eq!(bounded, before_rejection);
    bounded.apply_pool([1, 1, 0, 0, 0], 1).unwrap();
    assert_eq!(bounded.pools.len(), MAX_POOLS);
}
