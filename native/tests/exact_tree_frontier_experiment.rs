//! Test-only exact frontier experiment on a small, closed merge tree.
//! A topology-derived finite lattice handles selected equal splits, not
//! arbitrary rates, geography, transport time, or generated-world water.
use serde::{Deserialize, Serialize};

const VERSION: &str = "exact-tree-frontier-lab-1";
const MAX_NODES: usize = 16;
const MAX_DENOMINATOR: i128 = 1_000_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Node {
    children: Vec<usize>,
    capacity_units: i128,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClosedTree {
    version: String,
    nodes: Vec<Node>,
    /// Every stock and ledger entry is counted in 1/denominator base units.
    denominator: i128,
    /// Only an exclusive frontier has Some(stock); inactive branches have None.
    active: Vec<Option<i128>>,
    input_units: i128,
    event_count: u64,
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn lattice_denominator(nodes: &[Node]) -> Result<i128, &'static str> {
    if nodes.is_empty() || nodes.len() > MAX_NODES {
        return Err("Unsupported laboratory tree size.");
    }
    let mut parent_counts = vec![0_u8; nodes.len()];
    let mut denominator = 1_i128;
    for (id, node) in nodes.iter().enumerate() {
        if node.capacity_units <= 0 || node.children.len() == 1 {
            return Err("Invalid laboratory branch capacity or fanout.");
        }
        if !node.children.is_empty() {
            let fanout = node.children.len() as i128;
            denominator = denominator
                .checked_mul(fanout / gcd(denominator, fanout))
                .ok_or("Laboratory lattice denominator overflowed.")?;
            if denominator > MAX_DENOMINATOR {
                return Err("Laboratory lattice denominator exceeds its limit.");
            }
        }
        let mut child_capacity = 0_i128;
        for &child in &node.children {
            if child >= id {
                return Err("Laboratory tree must be topologically ordered.");
            }
            parent_counts[child] = parent_counts[child]
                .checked_add(1)
                .ok_or("Laboratory child has multiple parents.")?;
            if parent_counts[child] > 1 {
                return Err("Laboratory child has multiple parents.");
            }
            child_capacity = child_capacity
                .checked_add(nodes[child].capacity_units)
                .ok_or("Laboratory branch capacity overflowed.")?;
        }
        if !node.children.is_empty() && child_capacity > node.capacity_units {
            return Err("Laboratory parent is smaller than its child birth stock.");
        }
    }
    if parent_counts[..nodes.len() - 1]
        .iter()
        .any(|&count| count != 1)
        || parent_counts[nodes.len() - 1] != 0
    {
        return Err("Laboratory branches do not form one rooted tree.");
    }
    if nodes
        .iter()
        .any(|node| node.capacity_units.checked_mul(denominator).is_none())
    {
        return Err("Laboratory scaled capacity overflowed.");
    }
    Ok(denominator)
}

impl ClosedTree {
    fn new(nodes: Vec<Node>) -> Result<Self, &'static str> {
        let denominator = lattice_denominator(&nodes)?;
        let active = nodes
            .iter()
            .map(|node| node.children.is_empty().then_some(0))
            .collect();
        let result = Self {
            version: VERSION.into(),
            nodes,
            denominator,
            active,
            input_units: 0,
            event_count: 0,
        };
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> Result<(), &'static str> {
        if self.version != VERSION
            || lattice_denominator(&self.nodes)? != self.denominator
            || self.active.len() != self.nodes.len()
            || self.input_units < 0
        {
            return Err("Invalid laboratory tree checkpoint.");
        }
        let mut covered = vec![false; self.nodes.len()];
        let mut stored = 0_i128;
        for (id, node) in self.nodes.iter().enumerate() {
            if let Some(stock) = self.active[id] {
                let capacity = node
                    .capacity_units
                    .checked_mul(self.denominator)
                    .ok_or("Laboratory scaled capacity overflowed.")?;
                if stock < 0 || stock > capacity || node.children.iter().any(|&c| covered[c]) {
                    return Err("Invalid laboratory exclusive frontier.");
                }
                stored = stored
                    .checked_add(stock)
                    .ok_or("Laboratory stock sum overflowed.")?;
                covered[id] = true;
            } else if !node.children.is_empty() {
                if node.children.iter().all(|&c| {
                    self.active[c] == Some(self.nodes[c].capacity_units * self.denominator)
                }) {
                    return Err("Full laboratory children must merge.");
                }
                covered[id] = node.children.iter().all(|&c| covered[c]);
            }
        }
        if !covered[self.nodes.len() - 1] {
            return Err("Laboratory frontier does not cover the root.");
        }
        if stored != self.input_units {
            return Err("Laboratory exact input ledger does not balance.");
        }
        Ok(())
    }

    fn restore(self) -> Result<Self, &'static str> {
        self.validate()?;
        Ok(self)
    }

    fn apply_grants(&mut self, grants: &[(usize, i128)], input: i128) -> Result<(), &'static str> {
        if input <= 0 || grants.is_empty() {
            return Err("Invalid laboratory external input.");
        }
        let mut next = self.clone();
        let mut granted = 0_i128;
        let mut seen = vec![false; next.nodes.len()];
        for &(branch, grant) in grants {
            if branch >= next.nodes.len() || seen[branch] || grant <= 0 {
                return Err("Invalid laboratory recipient or grant.");
            }
            seen[branch] = true;
            let stock = next.active[branch]
                .as_mut()
                .ok_or("Laboratory recipient is not active.")?;
            *stock = stock
                .checked_add(grant)
                .ok_or("Laboratory stock overflowed.")?;
            granted = granted
                .checked_add(grant)
                .ok_or("Laboratory input sum overflowed.")?;
        }
        if granted != input {
            return Err("Laboratory grants do not equal external input.");
        }
        next.input_units = next
            .input_units
            .checked_add(input)
            .ok_or("Laboratory ledger overflowed.")?;
        next.event_count = next
            .event_count
            .checked_add(1)
            .ok_or("Event count overflowed.")?;
        next.merge_full_children()?;
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn apply_equal_whole_units(
        &mut self,
        recipients: &[usize],
        whole_units: i128,
    ) -> Result<(), &'static str> {
        if whole_units <= 0 || recipients.is_empty() {
            return Err("Invalid laboratory equal input.");
        }
        let input = whole_units
            .checked_mul(self.denominator)
            .ok_or("Laboratory equal input overflowed.")?;
        let count = recipients.len() as i128;
        if input % count != 0 {
            return Err("Equal shares are below the declared lattice unit.");
        }
        let grants: Vec<_> = recipients.iter().map(|&id| (id, input / count)).collect();
        self.apply_grants(&grants, input)
    }

    fn merge_full_children(&mut self) -> Result<(), &'static str> {
        for id in 0..self.nodes.len() {
            let children = &self.nodes[id].children;
            if children.is_empty() || self.active[id].is_some() {
                continue;
            }
            if children.iter().all(|&child| {
                self.active[child] == Some(self.nodes[child].capacity_units * self.denominator)
            }) {
                let mut birth = 0_i128;
                for &child in children {
                    birth = birth
                        .checked_add(self.active[child].take().unwrap())
                        .ok_or("Laboratory merge overflowed.")?;
                }
                self.active[id] = Some(birth);
            }
        }
        Ok(())
    }
}

fn fixture() -> Vec<Node> {
    vec![
        Node {
            children: vec![],
            capacity_units: 10,
        },
        Node {
            children: vec![],
            capacity_units: 10,
        },
        Node {
            children: vec![],
            capacity_units: 10,
        },
        Node {
            children: vec![],
            capacity_units: 5,
        },
        Node {
            children: vec![],
            capacity_units: 5,
        },
        Node {
            children: vec![0, 1, 2],
            capacity_units: 40,
        },
        Node {
            children: vec![3, 4],
            capacity_units: 14,
        },
        Node {
            children: vec![5, 6],
            capacity_units: 64,
        },
    ]
}

#[test]
fn exact_nested_frontier_replays_multiway_and_binary_merges_without_double_counting() {
    let mut tree = ClosedTree::new(fixture()).unwrap();
    assert_eq!(tree.denominator, 6);
    tree.apply_equal_whole_units(&[0, 1, 2], 29).unwrap();
    assert_eq!(&tree.active[..3], &[Some(58); 3]);
    let checkpoint = serde_json::to_string(&tree).unwrap();
    tree = serde_json::from_str::<ClosedTree>(&checkpoint)
        .unwrap()
        .restore()
        .unwrap();
    tree.apply_equal_whole_units(&[0, 1, 2], 1).unwrap();
    assert_eq!(tree.active[5], Some(180));
    assert!(tree.active[7].is_none());
    let before = tree.clone();
    assert_eq!(
        tree.apply_grants(&[(0, 6)], 6),
        Err("Laboratory recipient is not active.")
    );
    assert_eq!(tree, before);
    assert_eq!(
        tree.apply_grants(&[(5, 61)], 61),
        Err("Invalid laboratory exclusive frontier.")
    );
    assert_eq!(tree, before);

    tree.apply_grants(&[(5, 60)], 60).unwrap();
    tree.apply_equal_whole_units(&[3, 4], 10).unwrap();
    assert_eq!(tree.active[6], Some(60));
    assert!(tree.active[7].is_none());
    tree.apply_grants(&[(6, 24)], 24).unwrap();
    assert_eq!(
        tree.active,
        vec![None, None, None, None, None, None, None, Some(324)]
    );
    assert_eq!(tree.input_units, 324);
    tree.apply_grants(&[(7, 60)], 60).unwrap();
    assert_eq!(tree.active[7], Some(384));
    assert_eq!(tree.input_units, 384);
    assert_eq!(tree.event_count, 6);
}

#[test]
fn physical_tie_break_does_not_assign_a_label_priority_or_merge_early() {
    for permutation in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut tree = ClosedTree::new(fixture()).unwrap();
        let local = permutation
            .iter()
            .position(|&physical| physical == 0)
            .unwrap();
        tree.apply_grants(&[(local, 6)], 6).unwrap();
        tree.apply_equal_whole_units(&[0, 1, 2], 27).unwrap();
        assert!(tree.active[5].is_none());
        let mut physical = [0_i128; 3];
        for (slot, physical_id) in permutation.into_iter().enumerate() {
            physical[physical_id] = tree.active[slot].unwrap();
        }
        assert_eq!(physical, [60, 54, 54]);
        let remaining: Vec<_> = (0..3).filter(|&id| id != local).collect();
        tree.apply_equal_whole_units(&remaining, 2).unwrap();
        assert_eq!(tree.active[5], Some(180));
        assert_eq!(tree.input_units, 180);
    }
}

#[test]
fn unsupported_splits_and_corrupt_frontiers_reject_without_mutation() {
    let mut tree = ClosedTree::new(fixture()).unwrap();
    let initial = tree.clone();
    assert_eq!(
        tree.apply_equal_whole_units(&[0, 1, 3, 4], 1),
        Err("Equal shares are below the declared lattice unit.")
    );
    assert_eq!(tree, initial);
    assert_eq!(
        tree.apply_grants(&[(0, 6), (0, 6)], 12),
        Err("Invalid laboratory recipient or grant.")
    );
    assert_eq!(tree, initial);
    assert_eq!(
        tree.apply_grants(&[(0, 6)], 12),
        Err("Laboratory grants do not equal external input.")
    );
    assert_eq!(tree, initial);

    let mut bad = tree.clone();
    bad.input_units = 1;
    assert_eq!(
        bad.restore(),
        Err("Laboratory exact input ledger does not balance.")
    );
    let mut bad = tree.clone();
    bad.active[5] = Some(0);
    assert_eq!(bad.restore(), Err("Invalid laboratory exclusive frontier."));
    let mut bad = tree.clone();
    bad.denominator = 3;
    assert_eq!(bad.restore(), Err("Invalid laboratory tree checkpoint."));
    let mut bad = tree.clone();
    bad.version = "future".into();
    assert_eq!(bad.restore(), Err("Invalid laboratory tree checkpoint."));

    let mut invalid = fixture();
    invalid[7].children = vec![5, 5];
    assert_eq!(
        ClosedTree::new(invalid),
        Err("Laboratory child has multiple parents.")
    );
    let mut invalid = fixture();
    invalid[7].capacity_units = 53;
    assert_eq!(
        ClosedTree::new(invalid),
        Err("Laboratory parent is smaller than its child birth stock.")
    );
}
