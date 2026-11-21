//! Finite-budget sediment redistribution on the prepared geography-V bed.
use crate::{
    Surface, drainage::Drainage, eroded_landscape::stack_order, mountain_belts::Key, water::Water,
};
use std::{cmp::Reverse, collections::BinaryHeap, ops::Range};

pub const MODEL_VERSION: &str = "sediment-1";
pub const MAX_CONTRIBUTION_METERS: f64 = 12_000.;
const CAPACITY_LENGTH: f64 = 30_000.;
const TRUNK_AREA: f64 = 25e9;
const PLAIN_SLOPE: f64 = 0.0015;
const PLAIN_HEIGHT: f64 = 380.;
const PLAIN_THICKNESS: f64 = 34.;
const FAN_SLOPE: f64 = 0.004;
const FAN_DONOR_SLOPE: f64 = 0.006;
const FAN_SLOPE_DROP: f64 = 2.5;
const FAN_THICKNESS: f64 = 90.;
const DELTA_AREA: f64 = 225e9;
const DELTA_RADIUS: f64 = 125_000.;
const DELTA_DEPTH: f64 = 180.;
const DELTA_THICKNESS: f64 = 20.;
const DELTA_SHARE: f64 = 0.45;
const SHELF_RADIUS: f64 = 300_000.;
const SHELF_DEPTH: f64 = 400.;
const SHELF_THICKNESS: f64 = 160.;
const WET_CLEARANCE: f64 = 8.;
const MAPPED_THICKNESS: f64 = 5.;

#[derive(Debug, Default, PartialEq)]
pub struct Budget {
    pub supplied: f64,
    pub plains: f64,
    pub fans: f64,
    pub deltas: f64,
    pub shelf: f64,
    pub basins: f64,
    pub marine_unplaced: f64,
    pub basin_unplaced: f64,
    pub plain_area: f64,
    pub fan_area: f64,
    pub delta_area: f64,
    pub delta_count: usize,
    pub dry_basins_filled: usize,
    /// (Volume, region); report anchors for the largest deposits of each kind.
    pub largest_delta: Option<(f64, usize)>,
    pub largest_fan: Option<(f64, usize)>,
}

impl Budget {
    pub fn deposited(&self) -> f64 {
        self.plains + self.fans + self.deltas + self.shelf + self.basins
    }

    pub fn closing_error(&self) -> f64 {
        self.supplied - self.deposited() - self.marine_unplaced - self.basin_unplaced
    }
}

pub(crate) fn deposit(
    s: &Surface,
    elevation: &[f64],
    erosion: &[f64],
    water: &Water,
    mut wet_storage: f64,
) -> (Vec<f64>, Budget) {
    let n = elevation.len();
    let mut added = vec![0.; n];
    let mut budget = Budget::default();
    let wet = &water.body_ids;
    if !wet.contains(&0) || wet.iter().all(|&body| body == 0) {
        return (added, budget);
    }
    let level = water.level_meters;
    // A submerged bed's slope is irrelevant to the upstream river's capacity.
    let bed: Vec<_> = (0..n)
        .map(|i| if wet[i] == 0 { elevation[i] } else { level })
        .collect();
    let d = Drainage::build(s, &bed, wet);
    let mut load: Vec<_> = erosion
        .iter()
        .zip(&s.areas)
        .map(|(&e, &a)| -e.min(0.) * a)
        .collect();
    budget.supplied = load.iter().sum();

    let mut slopes = vec![0.; n];
    let mut donor_ceiling = vec![f64::INFINITY; n];
    let mut donor_slope = vec![0_f64; n];
    for (i, &receiver) in d.receivers.iter().enumerate() {
        let r = receiver as usize;
        if i == r {
            continue;
        }
        let edge = links(s, i).find(|&k| s.neighbors[k] == receiver).unwrap();
        slopes[i] = (bed[i] - bed[r]) / s.distances[edge];
        donor_slope[r] = donor_slope[r].max(slopes[i]);
        donor_ceiling[r] = donor_ceiling[r].min(bed[i]);
    }
    let mut order = Vec::with_capacity(n);
    stack_order(&d.receivers, wet, &mut order);
    let mut mouths = Vec::new();
    let mut dry_sinks = Vec::new();
    for i in order.into_iter().rev() {
        if wet[i] != 0 {
            continue;
        }
        let r = d.receivers[i] as usize;
        if r == i {
            if load[i] > 0. {
                dry_sinks.push(i);
            }
            continue;
        }
        let area = d.contributing_area[i];
        // A slope break promotes fan deposition. On low trunks, excess load
        // can also come from a confluence or locally supplied incision.
        let fan = donor_slope[i] >= FAN_DONOR_SLOPE
            && slopes[i] <= FAN_SLOPE
            && donor_slope[i] >= FAN_SLOPE_DROP * slopes[i];
        let plain =
            area >= TRUNK_AREA && slopes[i] <= PLAIN_SLOPE && bed[i] - level <= PLAIN_HEIGHT;
        if fan || plain {
            let capacity = CAPACITY_LENGTH * area * slopes[i];
            let thickness = if fan { FAN_THICKNESS } else { PLAIN_THICKNESS };
            // Preserve strictly descending incoming links. On a flat, leave
            // the existing equal-height receiver steps available.
            let accommodation = (donor_ceiling[i] - bed[i] - 0.01).clamp(0., thickness);
            let volume = (load[i] - capacity).max(0.).min(accommodation * s.areas[i]);
            added[i] = volume / s.areas[i];
            load[i] -= volume;
            if fan {
                budget.fans += volume;
                if added[i] >= MAPPED_THICKNESS {
                    budget.fan_area += s.areas[i];
                    if budget
                        .largest_fan
                        .is_none_or(|(largest, _)| volume > largest)
                    {
                        budget.largest_fan = Some((volume, i));
                    }
                }
            } else {
                budget.plains += volume;
                if added[i] >= MAPPED_THICKNESS {
                    budget.plain_area += s.areas[i];
                }
            }
        }
        if wet[r] != 0 && load[i] > 0. {
            mouths.push(i);
        }
        load[r] += load[i];
    }
    if !dry_sinks.is_empty() {
        fill_dry_basins(s, &bed, &d, &load, dry_sinks, &mut added, &mut budget);
    }

    // Large mouths have first claim on shared accommodation. Each river can
    // spend only the supply delivered by its own catchment.
    mouths.sort_unstable_by(|&a, &b| load[b].total_cmp(&load[a]).then(a.cmp(&b)));
    let mut shelf = Shelf::new(s, water);
    for i in mouths {
        let r = d.receivers[i] as usize;
        let supply = load[i].min(wet_storage);
        let mut placed = 0.;
        if wet[r] == water.main_ocean_id {
            if supply > 0. {
                placed = shelf.deposit(
                    i,
                    r,
                    d.contributing_area[i],
                    supply,
                    &mut added,
                    &mut budget,
                );
            }
            budget.marine_unplaced += load[i] - placed;
        } else {
            let room = (water.depth_meters[r] - WET_CLEARANCE).min(SHELF_THICKNESS) - added[r];
            let room = room.max(0.);
            placed = supply.min(room * s.areas[r]);
            added[r] += placed / s.areas[r];
            budget.basins += placed;
            budget.basin_unplaced += load[i] - placed;
        }
        wet_storage = (wet_storage - placed).max(0.);
    }
    (added, budget)
}

fn links(s: &Surface, i: usize) -> Range<usize> {
    s.offsets[i] as usize..s.offsets[i + 1] as usize
}

fn fill_dry_basins(
    s: &Surface,
    bed: &[f64],
    d: &Drainage,
    load: &[f64],
    sinks: Vec<usize>,
    added: &mut [f64],
    budget: &mut Budget,
) {
    let mut rims = vec![None; bed.len()];
    for &root in &sinks {
        rims[root] = Some(f64::INFINITY);
    }
    for (i, &outlet) in d.outlets.iter().enumerate() {
        let Some(rim) = &mut rims[outlet as usize] else {
            continue;
        };
        for k in links(s, i) {
            let j = s.neighbors[k] as usize;
            if d.outlets[j] != outlet {
                *rim = rim.min((bed[i] + added[i]).max(bed[j] + added[j]));
            }
        }
    }
    let mut floors = vec![Vec::new(); bed.len()];
    for (i, &outlet) in d.outlets.iter().enumerate() {
        let root = outlet as usize;
        if rims[root].is_some_and(|rim| bed[i] + added[i] < rim) {
            floors[root].push(i);
        }
    }
    for root in sinks {
        let floor = &mut floors[root];
        let height = |i: usize| bed[i] + added[i];
        floor.sort_unstable_by(|&a, &b| height(a).total_cmp(&height(b)).then(a.cmp(&b)));
        let mut remaining = load[root];
        let mut area = 0.;
        let mut stage = bed[root];
        let spill = rims[root].unwrap();
        let rim = spill.min(stage + MAX_CONTRIBUTION_METERS);
        for (index, next) in floor
            .iter()
            .map(|&i| height(i))
            .take_while(|&h| h < rim)
            .chain([rim])
            .enumerate()
        {
            let volume = area * (next - stage);
            if volume > remaining {
                stage += remaining / area;
                break;
            }
            remaining -= volume;
            stage = next;
            // The last iteration goes to the rim, which adds no surface area.
            if stage < rim {
                area += s.areas[floor[index]];
            }
        }
        let mut placed = 0.;
        for &i in floor.iter() {
            let thickness = (stage - bed[i]).max(0.);
            placed += (thickness - added[i]).max(0.) * s.areas[i];
            added[i] = thickness.max(added[i]);
        }
        budget.basins += placed;
        budget.basin_unplaced += load[root] - placed;
        budget.dry_basins_filled += usize::from(stage == spill);
    }
}

struct Shelf<'a> {
    surface: &'a Surface,
    water: &'a Water,
    distance: Vec<f64>,
    touched: Vec<usize>,
    queue: BinaryHeap<Reverse<Key>>,
    cells: Vec<(f64, usize)>,
    delta_cells: Vec<bool>,
}

impl<'a> Shelf<'a> {
    fn new(surface: &'a Surface, water: &'a Water) -> Self {
        let n = surface.areas.len();
        Self {
            surface,
            water,
            distance: vec![f64::INFINITY; n],
            touched: Vec::new(),
            queue: BinaryHeap::new(),
            cells: Vec::new(),
            delta_cells: vec![false; n],
        }
    }

    fn scan(&mut self, start: usize, radius: f64, depth: f64) {
        let (s, water) = (self.surface, self.water);
        for i in self.touched.drain(..) {
            self.distance[i] = f64::INFINITY;
        }
        self.cells.clear();
        if water.depth_meters[start] > depth {
            return;
        }
        self.distance[start] = 0.;
        self.touched.push(start);
        self.queue.push(Reverse(Key(0., start)));
        while let Some(Reverse(Key(distance, i))) = self.queue.pop() {
            if distance > self.distance[i] {
                continue;
            }
            self.cells.push((distance, i));
            for k in links(s, i) {
                let j = s.neighbors[k] as usize;
                let next = distance + s.distances[k];
                if water.body_ids[j] == water.main_ocean_id
                    && water.depth_meters[j] <= depth
                    && next < radius
                    && next < self.distance[j]
                {
                    if self.distance[j].is_infinite() {
                        self.touched.push(j);
                    }
                    self.distance[j] = next;
                    self.queue.push(Reverse(Key(next, j)));
                }
            }
        }
    }

    fn deposit(
        &mut self,
        mouth: usize,
        receiver: usize,
        area: f64,
        supply: f64,
        added: &mut [f64],
        budget: &mut Budget,
    ) -> f64 {
        let (s, water) = (self.surface, self.water);
        let room = |i: usize, stored: f64| (water.depth_meters[i] - WET_CLEARANCE - stored).max(0.);
        let mut delta = 0.;
        let mut remaining = DELTA_SHARE * supply;
        if area >= DELTA_AREA && remaining >= DELTA_THICKNESS * s.areas[receiver] {
            self.scan(receiver, DELTA_RADIUS, DELTA_DEPTH);
            for &(_, i) in &self.cells {
                let accommodation = room(i, added[i]);
                if accommodation < DELTA_THICKNESS {
                    continue;
                }
                let volume = remaining.min(accommodation * s.areas[i]);
                if volume < DELTA_THICKNESS * s.areas[i] {
                    break;
                }
                added[i] += volume / s.areas[i];
                if !self.delta_cells[i] {
                    budget.delta_area += s.areas[i];
                    self.delta_cells[i] = true;
                }
                remaining -= volume;
                delta += volume;
            }
        }
        if delta > 0. {
            budget.delta_count += 1;
            budget.deltas += delta;
            if budget
                .largest_delta
                .is_none_or(|(largest, _)| delta > largest)
            {
                budget.largest_delta = Some((delta, mouth));
            }
        }

        self.scan(receiver, SHELF_RADIUS, SHELF_DEPTH);
        let capacity = |distance: f64, i: usize, stored: f64| {
            let taper = (1. - distance / SHELF_RADIUS) * (1. - water.depth_meters[i] / SHELF_DEPTH);
            room(i, stored).min((SHELF_THICKNESS - stored).max(0.)) * taper * s.areas[i]
        };
        let total: f64 = self
            .cells
            .iter()
            .map(|&(d, i)| capacity(d, i, added[i]))
            .sum();
        let fraction = if total > 0. {
            ((supply - delta) / total).min(1.)
        } else {
            0.
        };
        let mut shelf = 0.;
        for &(distance, i) in &self.cells {
            let volume = fraction * capacity(distance, i, added[i]);
            added[i] += volume / s.areas[i];
            shelf += volume;
        }
        budget.shelf += shelf;
        delta + shelf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Recipe, World, eroded_landscape, water::WaterSettings};

    fn chain(n: usize) -> Surface {
        let mut offsets = vec![0];
        let mut neighbors = Vec::new();
        for i in 0..n {
            if i > 0 {
                neighbors.push((i - 1) as u32);
            }
            if i + 1 < n {
                neighbors.push((i + 1) as u32);
            }
            offsets.push(neighbors.len() as u32);
        }
        Surface {
            centers: vec![[1., 0., 0.]; n],
            faces: vec![],
            offsets,
            distances: vec![50_000.; neighbors.len()],
            neighbors,
            areas: vec![50e9; n],
            boundary_offsets: vec![],
            boundaries: vec![],
        }
    }

    fn fitted(s: &Surface, bed: &[f64], fraction: f64) -> Water {
        Water::generate(s, bed, &WaterSettings::Coverage { fraction }).unwrap()
    }

    fn check_budget(s: &Surface, added: &[f64], b: &Budget) {
        let geometry: f64 = added.iter().zip(&s.areas).map(|(h, a)| h * a).sum();
        let tolerance = b.supplied * 1e-11;
        assert!((geometry - b.deposited()).abs() <= tolerance, "{b:?}");
        assert!(geometry <= b.supplied + tolerance, "{b:?}");
        assert!(b.closing_error().abs() <= tolerance, "{b:?}");
        assert!(
            added
                .iter()
                .all(|h| (0. ..=MAX_CONTRIBUTION_METERS).contains(h))
        );
    }

    #[test]
    fn capacity_drop_places_fans_and_plains_and_feeds_a_delta() {
        let bed = [850., 450., 350., 300., 250., -100., -150., -260., -450.];
        let s = chain(bed.len());
        let water = Water {
            level_meters: 0.,
            depth_meters: bed.map(|h: f64| (-h).max(0.)).into(),
            ..fitted(&s, &bed, 4. / 9.)
        };
        let mut source = [0.; 9];
        source[0] = -1000.;
        let (added, b) = deposit(&s, &bed, &source, &water, f64::INFINITY);
        assert_eq!(added[0], 0., "steep headwater must pass its load");
        assert!(b.fans > 0. && added[1] > 0.);
        assert!(b.plains > 0. && added[3] > 0.);
        assert_eq!(added[4], 0., "steep terminal reach passes load through");
        assert_eq!(b.delta_count, 1);
        assert_eq!(b.largest_delta.unwrap().1, 4);
        assert!(added[5] >= DELTA_THICKNESS && added[6] >= DELTA_THICKNESS);
        assert!(b.shelf > 0. && added[7] > 0.);
        assert_eq!(added[8], 0.);
        check_budget(&s, &added, &b);
        assert_eq!(
            (added, b),
            deposit(&s, &bed, &source, &water, f64::INFINITY)
        );

        source[0] = 0.;
        source[4] = -2.;
        let (added, b) = deposit(&s, &bed, &source, &water, f64::INFINITY);
        assert_eq!(
            b.delta_count, 0,
            "a mouth with little supply only feeds the shelf"
        );
        assert!(b.shelf > 0.);
        check_budget(&s, &added, &b);
    }

    #[test]
    fn basin_filling_consumes_local_supply_and_respects_the_sill() {
        let bed = [-300., 500., 120., 100., 900.];
        let s = chain(bed.len());
        let water = fitted(&s, &bed, 0.2);
        let mut source = [0.; 5];
        source[4] = -100.;
        let (added, b) = deposit(&s, &bed, &source, &water, f64::INFINITY);
        assert_eq!(added, [0., 0., 40., 60., 0.]);
        assert_eq!(b.dry_basins_filled, 0);
        let final_bed: Vec<_> = bed.into_iter().zip(&added).map(|(h, d)| h + d).collect();
        assert_eq!(
            Drainage::build(&s, &final_bed, &water.body_ids).outlets[4],
            2
        );
        check_budget(&s, &added, &b);

        source[4] = -1500.;
        let (added, b) = deposit(&s, &bed, &source, &water, f64::INFINITY);
        assert_eq!(added, [0., 0., 380., 400., 0.]);
        assert_eq!(b.dry_basins_filled, 1);
        assert!(b.basin_unplaced > 0.);
        let final_bed: Vec<_> = bed.into_iter().zip(&added).map(|(h, d)| h + d).collect();
        assert_eq!(
            Drainage::build(&s, &final_bed, &water.body_ids).outlets[4],
            0
        );
        check_budget(&s, &added, &b);
    }

    #[test]
    fn generated_sediment_preserves_the_fit_and_closes_its_ledger() {
        let base: Recipe = serde_json::from_value(serde_json::json!({"schemaVersion":1,"modelVersion":eroded_landscape::MODEL_VERSION,
            "randomVersion":"fnv1a-utf8-mulberry32-1","seed":"first-light","subdivision":5,
            "radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,
            "continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,"detailAmplitudeMeters":300.,
            "terrainPreparationPasses":4,"water":{"mode":"coverage","fraction":0.71}})).unwrap();
        let coverage = World::generate(base.clone()).unwrap();
        let volume = Recipe {
            water: WaterSettings::Volume {
                volume_cubic_meters: coverage.water.resolved_volume_cubic_meters,
            },
            ..base
        };
        for world in [coverage, World::generate(volume).unwrap()] {
            let (s, t, b) = (
                &world.surface,
                &world.terrain,
                world.sediment_budget.as_ref().unwrap(),
            );
            check_budget(s, &t.sediment, b);
            assert!(b.supplied > b.deposited() && b.deposited() > 0.);
            let bed: Vec<_> = t
                .elevation
                .iter()
                .zip(&t.sediment)
                .map(|(h, d)| h - d)
                .collect();
            let prior = Water::generate(s, &bed, &world.recipe.water).unwrap();
            assert_eq!(prior.body_ids, world.water.body_ids);
            match world.recipe.water {
                WaterSettings::Coverage { fraction } => {
                    let area: f64 = world
                        .water
                        .body_ids
                        .iter()
                        .zip(&s.areas)
                        .filter(|(id, _)| **id != 0)
                        .map(|(_, a)| a)
                        .sum();
                    assert!(
                        (area / s.areas.iter().sum::<f64>() - fraction).abs()
                            <= 0.5 * s.areas.iter().copied().fold(0., f64::max)
                                / s.areas.iter().sum::<f64>()
                    );
                }
                WaterSettings::Volume {
                    volume_cubic_meters,
                } => {
                    assert!(
                        (world.water.resolved_volume_cubic_meters / volume_cubic_meters - 1.).abs()
                            < 1e-10
                    );
                }
            }
        }
    }
}
