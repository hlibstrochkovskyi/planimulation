//! Read-only screening of two C3f/C3g structural restrictions on generated beds.
//! Passing this screen does not establish numerical or dynamic readiness.
use crate::{
    Surface, drainage::Drainage, nested_reservoir::MAX_REGIONS, spill_connections::SpillConnections,
};
use serde::Serialize;

pub const ANALYSIS_VERSION: &str = "spill-readiness-1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryWitness {
    pub plateau: usize,
    /// Actual [sill, lower receiving region] adjacency.
    pub edge: [u32; 2],
    pub terminal_region: u32,
    pub terminal_leaf: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmbiguousEntry {
    pub branch: usize,
    pub parent_branch: usize,
    pub first: EntryWitness,
    pub different: EntryWitness,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screening {
    pub analysis_version: &'static str,
    pub region_count: usize,
    pub branch_count: usize,
    pub leaf_count: usize,
    /// Root has depth zero.
    pub maximum_branch_depth: usize,
    pub maximum_children: usize,
    pub connecting_plateau_count: usize,
    pub dead_end_plateau_count: usize,
    pub checked_contact_count: usize,
    pub laboratory_region_limit: usize,
    pub within_laboratory_region_limit: bool,
    pub unambiguous_nested_entries: bool,
    /// One canonical pair of conflicting entry paths per affected branch.
    pub ambiguous_entries: Vec<AmbiguousEntry>,
    /// Logical element counts of C3f's representation, not allocated bytes.
    pub laboratory_membership_matrix_entries: u64,
    pub laboratory_curve_column_references: u64,
    pub not_checked: Vec<&'static str>,
}

impl Screening {
    /// Like C2a/C3c, requires an already validated connected reciprocal graph.
    /// Rebuild from the actual bed, never from a separately supplied hierarchy.
    pub fn build(surface: &Surface, heights: &[f64]) -> Result<Self, String> {
        if surface.distances.len() != surface.neighbors.len()
            || surface.distances.iter().any(|d| !d.is_finite() || *d <= 0.)
        {
            return Err("Screening requires finite positive adjacency distances.".into());
        }
        let connections = SpillConnections::build(surface, heights)?;
        let basins = connections.basins();
        let nodes = basins.nodes();
        let k = nodes.len();
        // The laboratory Geometry adapter sorts adjacency together with its
        // distances. Match that dry descent without touching the caller's data.
        let mut dry_surface = Surface {
            centers: vec![],
            faces: vec![],
            offsets: surface.offsets.clone(),
            neighbors: vec![],
            distances: vec![],
            areas: surface.areas.clone(),
            boundary_offsets: vec![],
            boundaries: vec![],
        };
        for range in surface.offsets.windows(2) {
            let mut edges: Vec<_> = (range[0] as usize..range[1] as usize)
                .map(|i| (surface.neighbors[i], surface.distances[i]))
                .collect();
            edges.sort_unstable_by_key(|e| e.0);
            for (neighbor, distance) in edges {
                dry_surface.neighbors.push(neighbor);
                dry_surface.distances.push(distance);
            }
        }
        let drainage = Drainage::build(&dry_surface, heights, &vec![0; heights.len()]);
        let mut enter = vec![0; k];
        let mut leave = vec![0; k];
        let mut stack = vec![(basins.root(), false)];
        let mut clock = 0;
        while let Some((id, closing)) = stack.pop() {
            if closing {
                leave[id] = clock;
                continue;
            }
            enter[id] = clock;
            clock += 1;
            stack.push((id, true));
            for &child in nodes[id].children.iter().rev() {
                stack.push((child, false));
            }
        }
        let mut first: Vec<Option<EntryWitness>> = vec![None; k];
        let mut conflicts = vec![None; k];
        let mut connecting = 0;
        let mut dead_end = 0;
        let mut checked = 0;
        for (plateau, p) in connections.plateaus().iter().enumerate() {
            let child = p.contacts[0].child_branch;
            if p.contacts.iter().all(|c| c.child_branch == child) {
                dead_end += 1;
                continue;
            }
            connecting += 1;
            for contact in &p.contacts {
                checked += 1;
                let branch = contact.child_branch;
                let terminal_region = drainage.outlets[contact.edge[1] as usize];
                let terminal_leaf = basins.region_nodes()[terminal_region as usize];
                if !nodes[terminal_leaf].children.is_empty()
                    || enter[terminal_leaf] < enter[branch]
                    || enter[terminal_leaf] >= leave[branch]
                {
                    return Err("Dry contact descent is outside its receiving subtree.".into());
                }
                let witness = EntryWitness {
                    plateau,
                    edge: contact.edge,
                    terminal_region,
                    terminal_leaf,
                };
                if let Some(previous) = &first[branch] {
                    if previous.terminal_leaf != terminal_leaf && conflicts[branch].is_none() {
                        conflicts[branch] = Some(AmbiguousEntry {
                            branch,
                            parent_branch: p.parent_branch,
                            first: previous.clone(),
                            different: witness,
                        });
                    }
                } else {
                    first[branch] = Some(witness);
                }
            }
        }
        let ambiguous_entries: Vec<_> = conflicts.into_iter().flatten().collect();
        let mut depth = vec![0; k];
        for id in (0..k).rev() {
            if let Some(parent) = nodes[id].parent {
                depth[id] = depth[parent] + 1;
            }
        }
        let mut subtree_columns = vec![0_u64; k];
        for &owner in basins.region_nodes() {
            subtree_columns[owner] += 1;
        }
        for id in 0..k {
            if let Some(parent) = nodes[id].parent {
                subtree_columns[parent] = subtree_columns[parent]
                    .checked_add(subtree_columns[id])
                    .ok_or("Subtree column count overflowed.")?;
            }
        }
        let curve_columns = subtree_columns
            .iter()
            .try_fold(0_u64, |sum, &v| sum.checked_add(v))
            .ok_or("Curve column count overflowed.")?;
        Ok(Self {
            analysis_version: ANALYSIS_VERSION,
            region_count: heights.len(),
            branch_count: k,
            leaf_count: nodes.iter().filter(|n| n.children.is_empty()).count(),
            maximum_branch_depth: depth.into_iter().max().unwrap_or(0),
            maximum_children: nodes.iter().map(|n| n.children.len()).max().unwrap_or(0),
            connecting_plateau_count: connecting,
            dead_end_plateau_count: dead_end,
            checked_contact_count: checked,
            laboratory_region_limit: MAX_REGIONS,
            within_laboratory_region_limit: heights.len() <= MAX_REGIONS,
            unambiguous_nested_entries: ambiguous_entries.is_empty(),
            ambiguous_entries,
            laboratory_membership_matrix_entries: (k as u64)
                .checked_mul(k as u64)
                .ok_or("Matrix count overflowed.")?,
            laboratory_curve_column_references: curve_columns,
            not_checked: vec![
                "initial-water-inventory-mapping",
                "receiver-weight-policy",
                "stock-and-event-numerics",
                "pending-input-timing-and-accounting",
                "dynamic-runtime-and-memory",
                "drying-and-splitting",
            ],
        })
    }
}
