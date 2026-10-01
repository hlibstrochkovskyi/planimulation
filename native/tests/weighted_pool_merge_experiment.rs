//! Test-only partial merge of one spatially owned, equally divided pool.
//! Merging two recipients adds their claim weights without rounding the pool.
//! This is not a general multi-pool routing or generated-world water solver.
use serde::{Deserialize, Serialize};

const VERSION: &str = "weighted-pool-merge-lab-1";
const BRANCHES: usize = 5;
const LEAVES: usize = 3;
const TOTAL_WEIGHT: i128 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Branch {
    children: Vec<usize>,
    capacity_units: i128,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WeightedPool {
    version: String,
    /// Physical labels 0 and 1 merge first; physical label 2 remains outside.
    physical_by_slot: [u8; LEAVES],
    branches: [Branch; BRANCHES],
    /// Active branch weight in the one shared pool; None means inactive.
    active_weights: [Option<i128>; BRANCHES],
    pool_units: i128,
    input_units: i128,
    event_count: u64,
}

fn geometry(physical_by_slot: [u8; LEAVES]) -> Result<[Branch; BRANCHES], &'static str> {
    let mut seen = [false; LEAVES];
    for &physical in &physical_by_slot {
        let physical = physical as usize;
        if physical >= LEAVES || seen[physical] {
            return Err("Invalid laboratory physical labeling.");
        }
        seen[physical] = true;
    }
    let physical_slot = |physical: u8| {
        physical_by_slot
            .iter()
            .position(|&label| label == physical)
            .unwrap()
    };
    Ok([
        Branch {
            children: vec![],
            capacity_units: if physical_by_slot[0] == 2 { 5 } else { 3 },
        },
        Branch {
            children: vec![],
            capacity_units: if physical_by_slot[1] == 2 { 5 } else { 3 },
        },
        Branch {
            children: vec![],
            capacity_units: if physical_by_slot[2] == 2 { 5 } else { 3 },
        },
        Branch {
            children: vec![physical_slot(0), physical_slot(1)],
            capacity_units: 10,
        },
        Branch {
            children: vec![3, physical_slot(2)],
            capacity_units: 15,
        },
    ])
}

impl WeightedPool {
    fn new(physical_by_slot: [u8; LEAVES]) -> Result<Self, &'static str> {
        let state = Self {
            version: VERSION.into(),
            physical_by_slot,
            branches: geometry(physical_by_slot)?,
            active_weights: [Some(1), Some(1), Some(1), None, None],
            pool_units: 0,
            input_units: 0,
            event_count: 0,
        };
        state.validate()?;
        Ok(state)
    }

    /// Exact branch stock numerator with denominator TOTAL_WEIGHT.
    fn stock_thirds(&self, branch: usize) -> Result<i128, &'static str> {
        let weight = self
            .active_weights
            .get(branch)
            .and_then(|&weight| weight)
            .ok_or("Inactive laboratory branch has no stock.")?;
        self.pool_units
            .checked_mul(weight)
            .ok_or("Weighted pool stock overflowed.")
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.version != VERSION
            || self.branches != geometry(self.physical_by_slot)?
            || self.pool_units < 0
            || self.input_units != self.pool_units
        {
            return Err("Invalid weighted-pool checkpoint.");
        }
        let mut covered = [false; BRANCHES];
        let mut weight_sum = 0_i128;
        let mut stock_numerator_sum = 0_i128;
        for (branch, node) in self.branches.iter().enumerate() {
            if let Some(weight) = self.active_weights[branch] {
                if weight <= 0 || node.children.iter().any(|&child| covered[child]) {
                    return Err("Invalid weighted-pool frontier.");
                }
                let stock = self.stock_thirds(branch)?;
                if stock > node.capacity_units * TOTAL_WEIGHT {
                    return Err("Weighted pool branch exceeds capacity.");
                }
                stock_numerator_sum = stock_numerator_sum
                    .checked_add(stock)
                    .ok_or("Weighted pool stock sum overflowed.")?;
                weight_sum = weight_sum
                    .checked_add(weight)
                    .ok_or("Weighted pool weight sum overflowed.")?;
                covered[branch] = true;
            } else if !node.children.is_empty() {
                let mut all_children_full = true;
                for &child in &node.children {
                    if self.active_weights[child].is_none()
                        || self.stock_thirds(child)?
                            != self.branches[child].capacity_units * TOTAL_WEIGHT
                    {
                        all_children_full = false;
                        break;
                    }
                }
                if all_children_full {
                    return Err("Full weighted-pool children must merge.");
                }
                covered[branch] = node.children.iter().all(|&child| covered[child]);
            }
        }
        if !covered[BRANCHES - 1] || weight_sum != TOTAL_WEIGHT {
            return Err("Weighted pool does not cover the root exactly.");
        }
        if stock_numerator_sum % TOTAL_WEIGHT != 0
            || stock_numerator_sum / TOTAL_WEIGHT != self.input_units
        {
            return Err("Weighted pool input ledger does not balance.");
        }
        Ok(())
    }

    fn restore(self) -> Result<Self, &'static str> {
        self.validate()?;
        Ok(self)
    }

    fn merge_full_children(&mut self) -> Result<(), &'static str> {
        for parent in LEAVES..BRANCHES {
            let children = &self.branches[parent].children;
            let mut all_children_full = true;
            for &child in children {
                if self.active_weights[child].is_none()
                    || self.stock_thirds(child)?
                        != self.branches[child].capacity_units * TOTAL_WEIGHT
                {
                    all_children_full = false;
                    break;
                }
            }
            if all_children_full {
                let mut merged_weight = 0_i128;
                for &child in children {
                    merged_weight = merged_weight
                        .checked_add(self.active_weights[child].take().unwrap())
                        .ok_or("Weighted pool merge overflowed.")?;
                }
                self.active_weights[parent] = Some(merged_weight);
            }
        }
        Ok(())
    }

    fn apply(&mut self, input_units: i128) -> Result<(), &'static str> {
        if input_units <= 0 {
            return Err("Invalid weighted-pool input.");
        }
        let mut next = self.clone();
        next.pool_units = next
            .pool_units
            .checked_add(input_units)
            .ok_or("Weighted pool input overflowed.")?;
        next.input_units = next
            .input_units
            .checked_add(input_units)
            .ok_or("Weighted pool ledger overflowed.")?;
        next.event_count = next
            .event_count
            .checked_add(1)
            .ok_or("Event count overflowed.")?;
        for (branch, node) in next.branches.iter().enumerate() {
            if next.active_weights[branch].is_some()
                && next.stock_thirds(branch)? > node.capacity_units * TOTAL_WEIGHT
            {
                return Err("Weighted pool branch exceeds capacity.");
            }
        }
        next.merge_full_children()?;
        next.validate()?;
        *self = next;
        Ok(())
    }
}

#[test]
fn partial_merge_preserves_a_cross_boundary_pool_without_rounding_or_duplication() {
    for permutation in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut state = WeightedPool::new(permutation).unwrap();
        state.apply(8).unwrap();
        assert_eq!(state.active_weights[..LEAVES], [Some(1); LEAVES]);
        for leaf in 0..LEAVES {
            assert_eq!(state.stock_thirds(leaf), Ok(8));
        }
        let serialized = serde_json::to_string(&state).unwrap();
        state = serde_json::from_str::<WeightedPool>(&serialized)
            .unwrap()
            .restore()
            .unwrap();

        state.apply(1).unwrap();
        let outside = permutation
            .iter()
            .position(|&physical| physical == 2)
            .unwrap();
        assert_eq!(state.active_weights[3], Some(2));
        assert_eq!(state.stock_thirds(3), Ok(18)); // 6 units
        assert_eq!(state.active_weights[outside], Some(1));
        assert_eq!(state.stock_thirds(outside), Ok(9)); // 3 units
        assert!(state.active_weights[4].is_none());
        assert_eq!(state.input_units, 9);

        let before_rejection = state.clone();
        assert_eq!(
            state.apply(7),
            Err("Weighted pool branch exceeds capacity.")
        );
        assert_eq!(state, before_rejection);

        let serialized = serde_json::to_string(&state).unwrap();
        state = serde_json::from_str::<WeightedPool>(&serialized)
            .unwrap()
            .restore()
            .unwrap();
        state.apply(1).unwrap();
        assert_eq!(state.stock_thirds(3), Ok(20)); // 6 + 2/3 units
        assert_eq!(state.stock_thirds(outside), Ok(10)); // 3 + 1/3 units
        assert_eq!(state.input_units, 10);

        state.apply(5).unwrap();
        assert_eq!(state.active_weights, [None, None, None, None, Some(3)]);
        assert_eq!(state.stock_thirds(4), Ok(45)); // 15 units
        assert_eq!(state.input_units, 15);
        assert_eq!(state.event_count, 4);
    }
}

#[test]
fn malformed_weight_ownership_and_frontiers_reject_on_restore() {
    let state = WeightedPool::new([0, 1, 2]).unwrap();
    let mut bad = state.clone();
    bad.active_weights[0] = Some(2);
    assert_eq!(
        bad.restore(),
        Err("Weighted pool does not cover the root exactly.")
    );
    let mut bad = state.clone();
    bad.pool_units = 2;
    bad.input_units = 2;
    bad.active_weights[0] = Some(i128::MAX);
    assert_eq!(bad.restore(), Err("Weighted pool stock overflowed."));
    let mut bad = state.clone();
    bad.active_weights[3] = Some(1);
    assert_eq!(bad.restore(), Err("Invalid weighted-pool frontier."));
    let mut bad = state.clone();
    bad.input_units = 1;
    assert_eq!(bad.restore(), Err("Invalid weighted-pool checkpoint."));
    let mut bad = state.clone();
    bad.physical_by_slot = [0, 0, 2];
    assert_eq!(bad.restore(), Err("Invalid laboratory physical labeling."));
    let mut bad = state.clone();
    bad.branches[3].capacity_units = 9;
    assert_eq!(bad.restore(), Err("Invalid weighted-pool checkpoint."));
    let mut bad = state.clone();
    bad.version = "future".into();
    assert_eq!(bad.restore(), Err("Invalid weighted-pool checkpoint."));

    let mut state = WeightedPool::new([0, 1, 2]).unwrap();
    state.apply(9).unwrap();
    let mut bad = state.clone();
    bad.active_weights[3] = None;
    bad.active_weights[0] = Some(1);
    bad.active_weights[1] = Some(1);
    assert_eq!(
        bad.restore(),
        Err("Full weighted-pool children must merge.")
    );
}
