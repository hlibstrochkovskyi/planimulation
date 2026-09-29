//! One region ordering shared by all branch storage queries. The checkpoint
//! owns the columns; this index stores only region IDs and subtree ranges.
use crate::{basins::Basins, reservoir::Column};
use std::ops::Range;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SharedStorageIndex {
    ordered_regions: Vec<usize>,
    ranges: Vec<Range<usize>>,
    minimum: Vec<f64>,
    maximum: Vec<f64>,
    heights: Vec<f64>,
}

#[derive(Default)]
struct Sum {
    value: f64,
    correction: f64,
}

impl Sum {
    fn add(&mut self, term: f64) -> Result<(), String> {
        if !term.is_finite() || term < 0. {
            return Err("Shared storage sum overflowed.".into());
        }
        let next = self.value + term;
        if !next.is_finite() {
            return Err("Shared storage sum overflowed.".into());
        }
        self.correction += if self.value >= term {
            (self.value - next) + term
        } else {
            (term - next) + self.value
        };
        self.value = next;
        Ok(())
    }

    fn total(&self) -> Result<f64, String> {
        let total = self.value + self.correction;
        if !total.is_finite() {
            return Err("Shared storage sum overflowed.".into());
        }
        Ok(total)
    }
}

impl SharedStorageIndex {
    pub(super) fn new(
        columns: &[Column],
        basins: &Basins,
        enter: &[usize],
        leave: &[usize],
    ) -> Result<Self, String> {
        let k = basins.nodes().len();
        if columns.len() != basins.region_nodes().len() || enter.len() != k || leave.len() != k {
            return Err("Shared storage geometry and branch index differ.".into());
        }
        let mut owned = vec![Vec::new(); k];
        let mut minimum = vec![f64::INFINITY; k];
        let mut maximum = vec![f64::NEG_INFINITY; k];
        for (region, (&owner, column)) in basins.region_nodes().iter().zip(columns).enumerate() {
            owned[enter[owner]].push(region);
            minimum[owner] = minimum[owner].min(column.bed_meters);
            maximum[owner] = maximum[owner].max(column.bed_meters);
        }
        let mut ordered_regions = Vec::with_capacity(columns.len());
        let mut offsets = Vec::with_capacity(k + 1);
        for group in owned {
            offsets.push(ordered_regions.len());
            ordered_regions.extend(group);
        }
        offsets.push(ordered_regions.len());
        if ordered_regions.len() != columns.len() {
            return Err("Shared storage omitted a regional column.".into());
        }
        for (id, node) in basins.nodes().iter().enumerate() {
            for &child in &node.children {
                minimum[id] = minimum[id].min(minimum[child]);
                maximum[id] = maximum[id].max(maximum[child]);
            }
        }
        if minimum.iter().any(|h| !h.is_finite()) || maximum.iter().any(|h| !h.is_finite()) {
            return Err("Shared storage has an empty branch.".into());
        }
        let ranges = (0..k)
            .map(|id| offsets[enter[id]]..offsets[leave[id]])
            .collect();
        let mut heights: Vec<_> = columns.iter().map(|c| c.bed_meters).collect();
        heights.sort_by(|a, b| a.total_cmp(b));
        heights.dedup();
        Ok(Self {
            ordered_regions,
            ranges,
            minimum,
            maximum,
            heights,
        })
    }

    pub(super) fn volume_at_level(
        &self,
        columns: &[Column],
        branch: usize,
        level: f64,
    ) -> Result<f64, String> {
        if !level.is_finite() || level < self.minimum[branch] {
            return Err("Level outside shared storage range.".into());
        }
        let mut volume = Sum::default();
        for &region in &self.ordered_regions[self.ranges[branch].clone()] {
            let c = &columns[region];
            if c.bed_meters < level {
                volume.add(c.area_square_meters * (level - c.bed_meters))?;
            }
        }
        volume.total()
    }

    fn wetted_area(&self, columns: &[Column], branch: usize, level: f64) -> Result<f64, String> {
        let mut area = Sum::default();
        for &region in &self.ordered_regions[self.ranges[branch].clone()] {
            let c = &columns[region];
            if c.bed_meters <= level {
                area.add(c.area_square_meters)?;
            }
        }
        area.total()
    }

    pub(super) fn level_for_volume(
        &self,
        columns: &[Column],
        branch: usize,
        volume: f64,
        resolution_aware: bool,
    ) -> Result<Option<f64>, String> {
        if !volume.is_finite() || volume < 0. {
            return Err("Storage outside shared storage range.".into());
        }
        if volume == 0. {
            return Ok(None);
        }
        let start = self.heights.partition_point(|&h| h < self.minimum[branch]);
        let end = self.heights.partition_point(|&h| h <= self.maximum[branch]);
        let mut low = start + 1;
        let mut high = end;
        while low < high {
            let middle = low + (high - low) / 2;
            if self.volume_at_level(columns, branch, self.heights[middle])? >= volume {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        let lower = self.heights[if low == end { end - 1 } else { low - 1 }];
        let base = self.volume_at_level(columns, branch, lower)?;
        let area = self.wetted_area(columns, branch, lower)?;
        if area <= 0. {
            return Err("Shared storage has no wetted area.".into());
        }
        let level = lower + (volume - base) / area;
        let resolved = self.volume_at_level(columns, branch, level)?;
        let strict_tolerance = 1e-9_f64.max(volume * 1e-10);
        if resolved != 0. && (resolved - volume).abs() <= strict_tolerance {
            return Ok(Some(level));
        }
        if resolution_aware {
            // An absolute f64 level has a finite spacing. Accept only the
            // nearest representable level, and only while its volume error
            // remains small relative to the authoritative stored volume.
            let candidates = [level.next_down(), level, level.next_up()];
            let mut best = None;
            for candidate in candidates {
                if candidate < self.minimum[branch] {
                    continue;
                }
                let represented = self.volume_at_level(columns, branch, candidate)?;
                if represented == 0. {
                    continue;
                }
                let previous = self.volume_at_level(columns, branch, candidate.next_down())?;
                let next = self.volume_at_level(columns, branch, candidate.next_up())?;
                if volume < previous || volume > next {
                    continue;
                }
                let error = (represented - volume).abs();
                if best.is_none_or(|(_, current_error)| error < current_error) {
                    best = Some((candidate, error));
                }
            }
            if let Some((candidate, error)) = best
                && error <= volume * 1e-9
            {
                return Ok(Some(candidate));
            }
        }
        if (resolved - volume).abs() > strict_tolerance || resolved == 0. {
            return Err("Shared level cannot represent the requested storage precisely.".into());
        }
        Ok(Some(level))
    }
}

#[cfg(test)]
mod tests {
    use super::SharedStorageIndex;
    use crate::{
        Surface,
        basins::Basins,
        reservoir::{Boundary, Column, Reservoir},
    };

    #[test]
    fn resolution_aware_inverse_keeps_stock_without_accepting_unresolvable_tiny_input() {
        let bed = -4053.65;
        let columns = [Column {
            bed_meters: bed,
            area_square_meters: 46_600_000_000.,
        }];
        let index = SharedStorageIndex {
            ordered_regions: vec![0],
            ranges: std::iter::once(0..1).collect(),
            minimum: vec![bed],
            maximum: vec![bed],
            heights: vec![bed],
        };
        let volume = 50_000_000.;
        assert!(index.level_for_volume(&columns, 0, volume, false).is_err());
        let level = index
            .level_for_volume(&columns, 0, volume, true)
            .unwrap()
            .unwrap();
        let represented = index.volume_at_level(&columns, 0, level).unwrap();
        assert!((represented - volume).abs() <= volume * 1e-9);
        assert!(index.level_for_volume(&columns, 0, 0.0001, true).is_err());
    }

    #[test]
    fn one_region_order_reconstructs_each_nested_prism() {
        let surface = Surface {
            centers: vec![],
            faces: vec![],
            offsets: vec![0, 1, 3, 4],
            neighbors: vec![1, 0, 2, 1],
            distances: vec![1.; 4],
            areas: vec![1., 2., 3.],
            boundary_offsets: vec![],
            boundaries: vec![],
        };
        let columns = [
            Column {
                bed_meters: 0.,
                area_square_meters: 1.,
            },
            Column {
                bed_meters: 2.,
                area_square_meters: 2.,
            },
            Column {
                bed_meters: 0.,
                area_square_meters: 3.,
            },
        ];
        let basins = Basins::build(&surface, &[0., 2., 0.]).unwrap();
        let k = basins.nodes().len();
        let mut enter = vec![0; k];
        let mut leave = vec![0; k];
        let mut clock = 0;
        let mut pending = vec![(basins.root(), false)];
        while let Some((id, closing)) = pending.pop() {
            if closing {
                leave[id] = clock;
            } else {
                enter[id] = clock;
                clock += 1;
                pending.push((id, true));
                for &child in basins.nodes()[id].children.iter().rev() {
                    pending.push((child, false));
                }
            }
        }
        let index = SharedStorageIndex::new(&columns, &basins, &enter, &leave).unwrap();
        let mut once = index.ordered_regions.clone();
        once.sort();
        assert_eq!(once, [0, 1, 2]);
        for id in 0..k {
            let branch_columns: Vec<_> = (0..columns.len())
                .filter(|&region| {
                    let mut owner = Some(basins.region_nodes()[region]);
                    while let Some(node) = owner {
                        if node == id {
                            return true;
                        }
                        owner = basins.nodes()[node].parent;
                    }
                    false
                })
                .map(|region| columns[region].clone())
                .collect();
            assert_eq!(index.ranges[id].len(), branch_columns.len());
            let reference = Reservoir::new(branch_columns, Boundary::Closed, 0.).unwrap();
            for level in [0., 0.5, 2., 3.] {
                assert_eq!(
                    index.volume_at_level(&columns, id, level).unwrap(),
                    reference.volume_at_level(level).unwrap()
                );
            }
            for volume in [0.5, 2., 10.] {
                let level = index
                    .level_for_volume(&columns, id, volume, false)
                    .unwrap()
                    .unwrap();
                assert!(
                    (index.volume_at_level(&columns, id, level).unwrap() - volume).abs() < 1e-12
                );
            }
        }
    }
}
