//! Bed-only sublevel connectivity and storage analysis, not water transport.
use crate::Surface;
use serde::Serialize;
use std::collections::BTreeMap;

pub const ANALYSIS_VERSION: &str = "basin-analysis-1";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BasinNode {
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Minimum height for a leaf; connection threshold for a merged branch.
    pub birth_level_meters: f64,
    /// None for the global root: the closed planet has no external drain.
    pub spill_level_meters: Option<f64>,
    /// [New sill region, adjacent region inside this child], not a flow direction.
    pub spill_edge: Option<[u32; 2]>,
    /// Subtree footprint; this is not the wetted area at arbitrary lower levels.
    pub support_area_square_meters: f64,
    /// Total subtree storage at the next merge, including all child storage.
    pub capacity_cubic_meters: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Basins {
    analysis_version: &'static str,
    root: usize,
    nodes: Vec<BasinNode>,
    /// Exclusive branch ownership; this is not the downhill catchment label.
    region_nodes: Vec<usize>,
    #[serde(skip)]
    members: Vec<Vec<usize>>,
    #[serde(skip)]
    heights: Vec<f64>,
    #[serde(skip)]
    areas: Vec<f64>,
}

struct UnionFind {
    parents: Vec<usize>,
    sizes: Vec<usize>,
}
impl UnionFind {
    fn find(&mut self, mut i: usize) -> usize {
        while self.parents[i] != i {
            self.parents[i] = self.parents[self.parents[i]];
            i = self.parents[i];
        }
        i
    }
    fn join(&mut self, a: usize, b: usize) {
        let (mut a, mut b) = (self.find(a), self.find(b));
        if a == b {
            return;
        }
        if self.sizes[a] < self.sizes[b] || (self.sizes[a] == self.sizes[b] && a > b) {
            std::mem::swap(&mut a, &mut b);
        }
        self.parents[b] = a;
        self.sizes[a] += self.sizes[b];
    }
}

impl Basins {
    /// Requires the core's validated, connected, reciprocal surface adjacency.
    pub fn build(s: &Surface, heights: &[f64]) -> Result<Self, String> {
        let n = heights.len();
        if n == 0
            || n != s.areas.len()
            || s.offsets.len() != n + 1
            || heights.iter().any(|h| !h.is_finite())
            || s.areas.iter().any(|a| !a.is_finite() || *a <= 0.)
        {
            return Err("Invalid basin analysis fields.".into());
        }
        let mut result = Self {
            analysis_version: ANALYSIS_VERSION,
            root: 0,
            nodes: Vec::new(),
            region_nodes: vec![usize::MAX; n],
            members: Vec::new(),
            heights: heights.to_vec(),
            areas: s.areas.clone(),
        };
        let mut uf = UnionFind {
            parents: (0..n).collect(),
            sizes: vec![1; n],
        };
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| heights[a].partial_cmp(&heights[b]).unwrap().then(a.cmp(&b)));
        let mut tops = vec![usize::MAX; n];
        // For each live branch, volume at its last visited elevation. Raising it
        // before adding zero-depth cells avoids subtracting large datum moments.
        let mut volumes: Vec<f64> = Vec::new();
        let mut levels: Vec<f64> = Vec::new();
        let mut begin = 0;
        while begin < n {
            let height = heights[order[begin]];
            let mut end = begin + 1;
            while end < n && heights[order[end]] == height {
                end += 1;
            }
            let batch = &order[begin..end];
            let mut contacts = Vec::new();
            // Snapshot all lower-component identities before any unions at this
            // height, including disconnected sill pieces linked through a basin.
            for &i in batch {
                for &j in &s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize] {
                    let j = j as usize;
                    if heights[j] < height {
                        contacts.push((i, tops[uf.find(j)], [i as u32, j as u32]));
                    }
                }
            }
            for &i in batch {
                for &j in &s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize] {
                    let j = j as usize;
                    if heights[j] <= height {
                        uf.join(i, j);
                    }
                }
            }
            let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for &i in batch {
                groups.entry(uf.find(i)).or_default().push(i);
            }
            let mut children: BTreeMap<usize, BTreeMap<usize, [u32; 2]>> = BTreeMap::new();
            for (i, child, edge) in contacts {
                children
                    .entry(uf.find(i))
                    .or_default()
                    .entry(child)
                    .and_modify(|old| *old = (*old).min(edge))
                    .or_insert(edge);
            }
            let mut groups: Vec<_> = groups.into_iter().collect();
            groups.sort_by_key(|(_, cells)| cells[0]);
            for (root, cells) in groups {
                let links = children.remove(&root).unwrap_or_default();
                let mut volume = 0.;
                let mut area = 0.;
                for &child in links.keys() {
                    volumes[child] +=
                        result.nodes[child].support_area_square_meters * (height - levels[child]);
                    levels[child] = height;
                    volume += volumes[child];
                    area += result.nodes[child].support_area_square_meters;
                }
                let node = if links.len() == 1 {
                    *links.keys().next().unwrap()
                } else {
                    let id = result.nodes.len();
                    for (&child, &edge) in &links {
                        let c = &mut result.nodes[child];
                        c.parent = Some(id);
                        c.spill_level_meters = Some(height);
                        c.spill_edge = Some(edge);
                        c.capacity_cubic_meters = Some(volumes[child]);
                    }
                    result.nodes.push(BasinNode {
                        parent: None,
                        children: links.keys().copied().collect(),
                        birth_level_meters: height,
                        spill_level_meters: None,
                        spill_edge: None,
                        support_area_square_meters: area,
                        capacity_cubic_meters: None,
                    });
                    result.members.push(Vec::new());
                    volumes.push(volume);
                    levels.push(height);
                    id
                };
                for i in cells {
                    result.region_nodes[i] = node;
                    result.members[node].push(i);
                    result.nodes[node].support_area_square_meters += s.areas[i];
                }
                tops[root] = node;
            }
            begin = end;
        }
        let root = uf.find(0);
        if (0..n).any(|i| uf.find(i) != root) {
            return Err("Basin analysis requires a connected surface.".into());
        }
        result.root = tops[root];
        if volumes.iter().any(|v| !v.is_finite())
            || result
                .nodes
                .iter()
                .any(|node| !node.support_area_square_meters.is_finite())
        {
            return Err("Basin analysis overflowed.".into());
        }
        Ok(result)
    }

    pub fn root(&self) -> usize {
        self.root
    }
    pub fn nodes(&self) -> &[BasinNode] {
        &self.nodes
    }
    pub fn region_nodes(&self) -> &[usize] {
        &self.region_nodes
    }

    /// Detach one minimum basin for a prescribed-input experiment. A non-root
    /// spill is an imposed external collector, never automatic sibling routing.
    /// Merged branches are rejected: their children need independent inventories.
    pub fn isolated_leaf_reservoir(
        &self,
        node: usize,
        initial_volume: f64,
    ) -> Result<crate::reservoir::Reservoir, String> {
        use crate::reservoir::{Boundary, Column, Reservoir};
        let branch = self.nodes.get(node).ok_or("Invalid basin node.")?;
        if !branch.children.is_empty() {
            return Err("An isolated reservoir requires a leaf basin, not a merged branch.".into());
        }
        let columns = self.members[node]
            .iter()
            .map(|&i| Column {
                bed_meters: self.heights[i],
                area_square_meters: self.areas[i],
            })
            .collect();
        let boundary = branch
            .spill_level_meters
            .map_or(Boundary::Closed, |spill_level_meters| {
                Boundary::ExternalCollector { spill_level_meters }
            });
        Reservoir::new(columns, boundary, initial_volume)
    }

    /// Total connected-subtree prism volume, not added rainfall or available capacity.
    /// At exact merge height the sill has zero depth: this is a limiting threshold.
    /// Iterative O(subtree size) query, intended for inspection, not every time step.
    pub fn volume_at_level(&self, node: usize, level: f64) -> Result<f64, String> {
        let branch = self.nodes.get(node).ok_or("Invalid basin node.")?;
        if !level.is_finite()
            || level < branch.birth_level_meters
            || branch.spill_level_meters.is_some_and(|spill| level > spill)
        {
            return Err("Level outside the basin branch's connected range.".into());
        }
        let mut pending = vec![node];
        let mut volume = 0.;
        while let Some(id) = pending.pop() {
            for &i in &self.members[id] {
                volume += self.areas[i] * (level - self.heights[i]).max(0.);
            }
            pending.extend_from_slice(&self.nodes[id].children);
        }
        if !volume.is_finite() {
            return Err("Basin storage query overflowed.".into());
        }
        Ok(volume)
    }
}
