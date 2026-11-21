//! Monthly climate-normal runoff and analytical spill routing on frozen geography.
use crate::{
    World,
    climate_normals::Normals,
    seasonal_temperature::{
        DAYS_PER_YEAR, MONTHS_PER_YEAR, Settings as Orbit, daily_insolation_watts_per_square_meter,
        declination_radians,
    },
};
use serde::Serialize;
use std::{array, cmp::Reverse, collections::BinaryHeap};

pub const MODEL_VERSION: &str = "rivers-1";
const MONTHS: usize = MONTHS_PER_YEAR;
const SECONDS_PER_DAY: f64 = 86_400.;
const SOIL_STEPS_PER_MONTH: usize = 4;
const SOIL_TOLERANCE_MM: f64 = 1e-10;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub soil_capacity_mm: f64,
    pub soil_recharge_exponent: u32,
    pub soil_et_full_fraction: f64,
    pub soil_steps_per_month: usize,
    pub snow_melt_mm_per_degree_day: f64,
    pub snow_threshold_celsius: f64,
    pub pet_temperature_offset_celsius: f64,
    pub pet_temperature_scale_celsius: f64,
    pub latent_heat_mj_per_kg: f64,
    pub axial_tilt_degrees: f64,
    pub solar_irradiance_watts_per_square_meter: f64,
    pub lake_pet_factor: f64,
    pub dry_lake_area_fraction: f64,
    pub river_threshold_cubic_meters_per_second: f64,
}

impl Default for Settings {
    fn default() -> Self {
        let orbit = Orbit::default();
        Self {
            soil_capacity_mm: 100.,
            soil_recharge_exponent: 2,
            soil_et_full_fraction: 0.7,
            soil_steps_per_month: SOIL_STEPS_PER_MONTH,
            snow_melt_mm_per_degree_day: 3.,
            snow_threshold_celsius: 0.,
            pet_temperature_offset_celsius: 5.,
            pet_temperature_scale_celsius: 100.,
            latent_heat_mj_per_kg: 2.45,
            axial_tilt_degrees: orbit.axial_tilt_degrees,
            solar_irradiance_watts_per_square_meter: orbit.solar_irradiance_watts_per_square_meter,
            lake_pet_factor: 1.15,
            dry_lake_area_fraction: 0.1,
            river_threshold_cubic_meters_per_second: 1.,
        }
    }
}

type Monthly = [f64; MONTHS];

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub land_area: f64,
    pub land_precipitation: f64,
    pub land_aet: f64,
    pub land_runoff: f64,
    pub land_storage: f64,
    pub ocean_discharge: f64,
    pub land_runoff_to_ocean: f64,
    pub land_runoff_to_endorheic: f64,
    pub land_runoff_evaporated_in_spilling_lakes: f64,
    pub endorheic_area: f64,
    pub lake_precipitation: f64,
    pub lake_evaporation: f64,
    pub terminal_storage: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LakeBalance {
    pub region: u32,
    pub outlet: Option<[u32; 2]>,
    pub sill_elevation: f64,
    pub area: f64,
    pub inflow: f64,
    pub precipitation: f64,
    pub potential_evaporation: f64,
    pub evaporation: f64,
    pub spill: f64,
    pub storage: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct SpillSegment {
    pub from: u32,
    pub to: u32,
    pub mean_discharge: f64,
    pub class: u32,
}

#[derive(Debug)]
pub struct Rivers {
    pub settings: Settings,
    pub month_days: [usize; MONTHS],
    pub ocean_terminal: Option<u32>,
    pub pet_mm: Vec<f64>,
    pub aet_mm: Vec<f64>,
    pub runoff_mm: Vec<f64>,
    pub storage_mm: Vec<f64>,
    pub mean_discharge: Vec<f64>,
    pub monthly_runoff_mm: Vec<Monthly>,
    pub monthly_discharge: Vec<Monthly>,
    pub class: Vec<u32>,
    pub terminal: Vec<u32>,
    pub lakes: Vec<LakeBalance>,
    pub spill_segments: Vec<SpillSegment>,
    pub totals: Totals,
}

fn seconds(days: usize) -> f64 {
    days as f64 * SECONDS_PER_DAY
}

fn potential_et(temperature: f64, insolation: f64, days: usize, s: Settings) -> f64 {
    let radiation = insolation * seconds(days) * 1e-6; // MJ/m²
    radiation / s.latent_heat_mj_per_kg
        * ((temperature + s.pet_temperature_offset_celsius) / s.pet_temperature_scale_celsius)
            .max(0.)
}

fn land_balance(
    temperature: Monthly,
    precipitation: Monthly,
    pet: Monthly,
    days: [usize; MONTHS],
    s: Settings,
) -> (Monthly, f64, f64) {
    let snow_delta: Monthly = array::from_fn(|m| {
        let warmth = temperature[m] - s.snow_threshold_celsius;
        if warmth <= 0. {
            precipitation[m]
        } else {
            -s.snow_melt_mm_per_degree_day * warmth * days[m] as f64
        }
    });
    // The annual snow map is max(start + sum(delta), F(0)). Starting at F(0)
    // gives a repeating cycle or, if snowfall outruns melt, explicit ice storage.
    let start_snow = snow_delta
        .iter()
        .fold(0_f64, |snow, &delta| (snow + delta).max(0.));
    let mut snow = start_snow;
    let liquid: Monthly = array::from_fn(|m| {
        let before = snow;
        snow = (snow + snow_delta[m]).max(0.);
        if temperature[m] <= s.snow_threshold_celsius {
            0.
        } else {
            precipitation[m] + (before - snow)
        }
    });

    let (runoff, aet) = soil_balance(liquid, pet, s);
    (runoff, aet, snow - start_snow)
}

/// Integrate HBV recharge (dS/dL = 1 - (S/C)²) over incoming liquid L.
fn infiltrate(soil: f64, capacity: f64, wetting: f64) -> f64 {
    let saturation = soil / capacity;
    (capacity - soil) * (1. + saturation) * wetting / (1. + saturation * wetting)
}

fn soil_month(
    soil: &mut f64,
    capacity: f64,
    full_et: f64,
    input: f64,
    wetting: f64,
    demand: f64,
) -> (f64, f64) {
    let (mut runoff, mut aet) = (0., 0.);
    for _ in 0..SOIL_STEPS_PER_MONTH {
        let entered = infiltrate(*soil, capacity, wetting)
            .min(input)
            .min(capacity - *soil);
        runoff += input - entered;
        *soil += entered;
        let used = soil.min(demand * (*soil / full_et).min(1.));
        aet += used;
        *soil -= used;
    }
    (runoff, aet)
}

fn soil_balance(liquid: Monthly, pet: Monthly, s: Settings) -> (Monthly, f64) {
    // With no demand, a full bucket is already periodic and passes all input.
    if pet.iter().all(|&mm| mm == 0.) {
        return (liquid, 0.);
    }
    let capacity = s.soil_capacity_mm;
    let full_et = capacity * s.soil_et_full_fraction;
    let input = liquid.map(|mm| mm / SOIL_STEPS_PER_MONTH as f64);
    let wetting = input.map(|mm| (mm / capacity).tanh());
    let demand = pet.map(|mm| mm / SOIL_STEPS_PER_MONTH as f64);
    let year = |mut soil: f64| {
        let mut aet = 0.;
        let runoff = array::from_fn(|m| {
            let (runoff, used) = soil_month(
                &mut soil, capacity, full_et, input[m], wetting[m], demand[m],
            );
            aet += used;
            runoff
        });
        (soil, runoff, aet)
    };
    // The annual soil map is monotone and nonexpansive. Iteration from zero
    // usually closes quickly. Bisection bounds the work for weak annual forcing.
    let mut low = 0.;
    for _ in 0..4 {
        let (end, runoff, aet) = year(low);
        if (end - low).abs() <= SOIL_TOLERANCE_MM {
            return (runoff, aet);
        }
        low = end;
    }
    let mut high = capacity;
    while high - low > SOIL_TOLERANCE_MM {
        let mid = (low + high) / 2.;
        if year(mid).0 > mid {
            low = mid;
        } else {
            high = mid;
        }
    }
    let (_, runoff, aet) = year((low + high) / 2.);
    (runoff, aet)
}

fn classify(volume: &Monthly, days: &[usize; MONTHS], s: Settings) -> u32 {
    let flows = volume.iter().zip(days).map(|(v, &days)| v / seconds(days));
    let threshold = s.river_threshold_cubic_meters_per_second;
    if flows.clone().all(|flow| flow >= threshold) {
        2
    } else if flows.into_iter().any(|flow| flow >= threshold) {
        1
    } else {
        0
    }
}

fn mean_discharge(volume: &Monthly) -> f64 {
    volume.iter().sum::<f64>() / seconds(DAYS_PER_YEAR)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Level(f64);
impl Eq for Level {}
impl Ord for Level {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).expect("finite bed levels")
    }
}
impl PartialOrd for Level {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy)]
struct Saddle {
    from: u32,
    to: u32,
    level: Level,
    hierarchy_contact: bool,
}

struct SpillTree {
    order: Vec<usize>,
    exits: Vec<Option<Saddle>>,
}

fn spill_tree(w: &World, root: usize) -> SpillTree {
    let outlets = &w.drainage.outlets;
    let heights = &w.terrain.elevation;
    let n = outlets.len();
    let mut hierarchy_contacts: Vec<_> = w
        .basins
        .nodes()
        .iter()
        .filter_map(|branch| branch.spill_edge.map(|[a, b]| (a.min(b), a.max(b))))
        .collect();
    hierarchy_contacts.sort_unstable();
    let mut edges = vec![Vec::new(); n];
    // Contract static catchments. The lowest cross-catchment contacts recover
    // the sublevel merge connections, including contacts omitted by a single
    // stored branch edge and wet bodies that split an original bed branch.
    for (i, bounds) in w.surface.offsets.windows(2).enumerate() {
        for &j in &w.surface.neighbors[bounds[0] as usize..bounds[1] as usize] {
            let a = outlets[i];
            let b = outlets[j as usize];
            if a == b || i as u32 >= j {
                continue;
            }
            let level = Level(heights[i].max(heights[j as usize]));
            let hierarchy_contact = hierarchy_contacts.binary_search(&(i as u32, j)).is_ok();
            edges[a as usize].push(Saddle {
                from: j,
                to: i as u32,
                level,
                hierarchy_contact,
            });
            edges[b as usize].push(Saddle {
                from: i as u32,
                to: j,
                level,
                hierarchy_contact,
            });
        }
    }
    let mut order = vec![root];
    let mut exits = vec![None; n];
    let mut queue = BinaryHeap::new();
    let mut push_neighbors = |source: usize, escape: Level, queue: &mut BinaryHeap<_>| {
        for &edge in &edges[source] {
            queue.push(Reverse((
                escape.max(edge.level),
                edge.level,
                !edge.hierarchy_contact,
                edge.from,
                edge.to,
            )));
        }
        edges[source].clear();
    };
    push_neighbors(root, Level(f64::NEG_INFINITY), &mut queue);
    while let Some(Reverse((escape, level, off_hierarchy, from, to))) = queue.pop() {
        let source = outlets[from as usize] as usize;
        if source == root || exits[source].is_some() {
            continue;
        }
        exits[source] = Some(Saddle {
            from,
            to,
            level,
            hierarchy_contact: !off_hierarchy,
        });
        order.push(source);
        push_neighbors(source, escape, &mut queue);
    }
    SpillTree { order, exits }
}

fn drainage_groups(w: &World) -> Vec<Vec<usize>> {
    let receivers = &w.drainage.receivers;
    let mut incoming = vec![0; receivers.len()];
    for (i, &r) in receivers.iter().enumerate() {
        if r as usize != i {
            incoming[r as usize] += 1;
        }
    }
    let mut order: Vec<_> = (0..receivers.len()).filter(|&i| incoming[i] == 0).collect();
    let mut groups = vec![Vec::new(); receivers.len()];
    let mut cursor = 0;
    while cursor < order.len() {
        let i = order[cursor];
        cursor += 1;
        groups[w.drainage.outlets[i] as usize].push(i);
        let r = receivers[i] as usize;
        if r != i {
            incoming[r] -= 1;
            if incoming[r] == 0 {
                order.push(r);
            }
        }
    }
    groups
}

impl Rivers {
    pub fn from_world(w: &World, climate: &Normals) -> Result<Self, String> {
        Self::with_dry_lake_area_fraction(w, climate, Settings::default().dry_lake_area_fraction)
    }

    /// Evaluate sensitivity to the dry-sink evaporation footprint.
    pub fn with_dry_lake_area_fraction(
        w: &World,
        climate: &Normals,
        dry_lake_area_fraction: f64,
    ) -> Result<Self, String> {
        if !(0. ..=1.).contains(&dry_lake_area_fraction) {
            return Err("Dry-lake area fraction must be between zero and one.".into());
        }
        let n = w.surface.areas.len();
        if climate.model_version != crate::climate_normals::MODEL_VERSION
            || [
                &climate.monthly_temperature_celsius,
                &climate.monthly_precipitation_mm,
            ]
            .iter()
            .any(|months| months.len() != MONTHS || months.iter().any(|m| m.len() != n))
        {
            return Err("River climate normals do not match the surface.".into());
        }
        let settings = Settings {
            dry_lake_area_fraction,
            ..Settings::default()
        };
        let mut month_days = [0; MONTHS];
        for day in 0..DAYS_PER_YEAR {
            month_days[day * MONTHS / DAYS_PER_YEAR] += 1;
        }
        let mut first_day = 0;
        let declination: Monthly = month_days.map(|days| {
            let result = declination_radians(first_day + days / 2, settings.axial_tilt_degrees);
            first_day += days;
            result
        });
        let ocean_terminal = w
            .water
            .body_ids
            .iter()
            .position(|&body| w.water.main_ocean_id != 0 && body == w.water.main_ocean_id);
        let root = ocean_terminal.unwrap_or_else(|| {
            w.terrain
                .elevation
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.total_cmp(b))
                .unwrap()
                .0
        });
        let root = w.drainage.outlets[root] as usize;
        let tree = spill_tree(w, root);
        let groups = drainage_groups(w);
        let mut result = Self {
            settings,
            month_days,
            ocean_terminal: ocean_terminal.map(|_| root as u32),
            pet_mm: vec![0.; n],
            aet_mm: vec![0.; n],
            runoff_mm: vec![0.; n],
            storage_mm: vec![0.; n],
            mean_discharge: vec![0.; n],
            monthly_runoff_mm: vec![[0.; MONTHS]; n],
            monthly_discharge: vec![[0.; MONTHS]; n],
            terminal: w.drainage.outlets.clone(),
            class: vec![0; n],
            lakes: Vec::new(),
            spill_segments: Vec::new(),
            totals: Totals::default(),
        };
        let mut volume = vec![[0.; MONTHS]; n];
        let mut land_volume = vec![0.; n];
        let mut catchment_area = vec![0.; n];
        let mut direct_rain = vec![[0.; MONTHS]; n];
        let mut lake_area = vec![0.; n];
        let mut evaporation = vec![0.; n];
        let highest = w
            .terrain
            .elevation
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        for i in 0..n {
            let area = w.surface.areas[i];
            let sin_latitude = w.surface.centers[i][1];
            let temperature: Monthly =
                array::from_fn(|m| climate.monthly_temperature_celsius[m][i]);
            let precipitation: Monthly = array::from_fn(|m| climate.monthly_precipitation_mm[m][i]);
            let pet: Monthly = array::from_fn(|m| {
                let insolation = daily_insolation_watts_per_square_meter(
                    sin_latitude,
                    declination[m],
                    settings.solar_irradiance_watts_per_square_meter,
                );
                potential_et(temperature[m], insolation, month_days[m], settings)
            });
            result.pet_mm[i] = pet.iter().sum();
            let outlet = w.drainage.outlets[i] as usize;
            let body = w.water.body_ids[i];
            let terminal_is_dry = w.water.body_ids[outlet] == 0;
            let open_pet = (0..MONTHS)
                .filter(|&m| temperature[m] > settings.snow_threshold_celsius)
                .map(|m| pet[m] * settings.lake_pet_factor)
                .sum::<f64>();
            if body != 0 {
                if Some(outlet as u32) != result.ocean_terminal {
                    lake_area[outlet] += area;
                    evaporation[outlet] += open_pet * area * 0.001;
                    for (rain, p) in direct_rain[outlet].iter_mut().zip(precipitation) {
                        *rain += p * area * 0.001;
                    }
                }
                continue;
            }
            let (runoff, aet, storage) =
                land_balance(temperature, precipitation, pet, month_days, settings);
            result.aet_mm[i] = aet;
            result.storage_mm[i] = storage;
            result.runoff_mm[i] = runoff.iter().sum();
            result.monthly_runoff_mm[i] = runoff;
            volume[i] = runoff.map(|mm| mm * area * 0.001);
            land_volume[i] = result.runoff_mm[i] * area * 0.001;
            catchment_area[outlet] += area;
            result.totals.land_precipitation += precipitation.iter().sum::<f64>() * area * 0.001;
            result.totals.land_aet += aet * area * 0.001;
            result.totals.land_storage += storage * area * 0.001;
            result.totals.land_runoff += land_volume[i];
            let sill = tree.exits[outlet].map_or(highest, |edge| edge.level.0);
            if terminal_is_dry && (w.terrain.elevation[i] < sill || i == outlet) {
                lake_area[outlet] += area;
                evaporation[outlet] += (open_pet - aet).max(0.) * area * 0.001;
            }
        }
        result.totals.land_area = catchment_area.iter().sum();
        let mut spilled = vec![false; n];
        for &outlet in tree.order.iter().rev() {
            let mut inflow = [0.; MONTHS];
            let mut land_inflow = 0.;
            for &i in &groups[outlet] {
                let r = w.drainage.receivers[i] as usize;
                if i == r {
                    add(&mut inflow, volume[i]);
                    land_inflow += land_volume[i];
                } else {
                    let flow = volume[i];
                    add(&mut volume[r], flow);
                    land_volume[r] += land_volume[i];
                }
            }
            if Some(outlet as u32) == result.ocean_terminal {
                result.totals.ocean_discharge = inflow.iter().sum();
                result.totals.land_runoff_to_ocean = land_inflow;
                continue;
            }
            let mut area = lake_area[outlet];
            let mut potential_evaporation = evaporation[outlet];
            if w.water.body_ids[outlet] == 0 && area > 0. {
                let fraction =
                    (settings.dry_lake_area_fraction * catchment_area[outlet] / area).min(1.);
                area *= fraction;
                potential_evaporation *= fraction;
            }
            let (lake_evaporation, spill, storage) = lake_balance(
                inflow,
                direct_rain[outlet],
                potential_evaporation,
                tree.exits[outlet].is_some(),
            );
            let total_spill: f64 = spill.iter().sum();
            result.totals.lake_precipitation += direct_rain[outlet].iter().sum::<f64>();
            result.totals.lake_evaporation += lake_evaporation;
            result.totals.terminal_storage += storage;
            let edge = tree.exits[outlet];
            result.lakes.push(LakeBalance {
                region: outlet as u32,
                outlet: edge.map(|e| [e.from, e.to]),
                sill_elevation: edge.map_or(0., |e| e.level.0),
                area,
                inflow: inflow.iter().sum(),
                precipitation: direct_rain[outlet].iter().sum(),
                potential_evaporation,
                evaporation: lake_evaporation,
                spill: total_spill,
                storage,
            });
            if let Some(edge) = edge.filter(|_| total_spill > 0.) {
                spilled[outlet] = true;
                add(&mut volume[edge.to as usize], spill);
                let land_spill = land_inflow * (total_spill / (lake_evaporation + total_spill));
                land_volume[edge.to as usize] += land_spill;
                result.totals.land_runoff_evaporated_in_spilling_lakes += land_inflow - land_spill;
                let mean_discharge = mean_discharge(&spill);
                let class = classify(&spill, &month_days, settings);
                result.spill_segments.push(SpillSegment {
                    from: edge.from,
                    to: edge.to,
                    mean_discharge,
                    class,
                });
                let mut from = edge.from;
                while w.drainage.receivers[from as usize] != from {
                    let to = from;
                    from = w.drainage.receivers[from as usize];
                    result.spill_segments.push(SpillSegment {
                        from,
                        to,
                        mean_discharge,
                        class,
                    });
                }
            } else {
                result.totals.land_runoff_to_endorheic += land_inflow;
            }
        }

        let mut destinations = w.drainage.outlets.clone();
        for &outlet in &tree.order {
            if spilled[outlet] {
                let next = tree.exits[outlet].unwrap().to as usize;
                destinations[outlet] = destinations[w.drainage.outlets[next] as usize];
            }
            if Some(destinations[outlet]) != result.ocean_terminal {
                result.totals.endorheic_area += catchment_area[outlet];
            }
        }
        for i in 0..n {
            result.terminal[i] = destinations[w.drainage.outlets[i] as usize];
            result.mean_discharge[i] = mean_discharge(&volume[i]);
            result.class[i] = classify(&volume[i], &month_days, settings);
            result.monthly_discharge[i] = array::from_fn(|m| volume[i][m] / seconds(month_days[m]));
        }
        result.lakes.sort_unstable_by_key(|lake| lake.region);
        Ok(result)
    }
}

fn add(target: &mut Monthly, source: Monthly) {
    for (target, source) in target.iter_mut().zip(source) {
        *target += source;
    }
}

fn lake_balance(
    inflow: Monthly,
    rain: Monthly,
    capacity: f64,
    has_exit: bool,
) -> (f64, Monthly, f64) {
    let available: Monthly = array::from_fn(|m| inflow[m] + rain[m]);
    let annual: f64 = available.iter().sum();
    let evaporation = capacity.min(annual);
    let surplus = annual - evaporation;
    if has_exit && surplus > 0. {
        (evaporation, available.map(|v| v * (surplus / annual)), 0.)
    } else {
        (evaporation, [0.; MONTHS], surplus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Recipe, Surface, basins::Basins, drainage::Drainage};

    fn near(a: f64, b: f64) {
        assert!(
            (a - b).abs() <= 1e-8 + a.abs().max(b.abs()) * 1e-12,
            "{a} != {b}"
        );
    }

    fn world(fraction: f64) -> World {
        let recipe: Recipe = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1, "modelVersion": "basins-1", "randomVersion": "fnv1a-utf8-mulberry32-1",
            "seed": "river-test", "subdivision": 2, "radiusMeters": 6371000, "plateCount": 12,
            "maxPlateSpeedCmPerYear": 8, "continentalFraction": 0.38,
            "continentalScale": 1., "reliefScale": 1., "boundaryWidthKm": 300., "detailAmplitudeMeters": 300.,
            "water": {"mode": "coverage", "fraction": fraction}
        })).unwrap();
        World::generate(recipe).unwrap()
    }

    fn chain(heights: &[f64], bodies: &[u32]) -> (World, Normals) {
        let mut w = world(0.71);
        let mut climate = Normals::from_world(&w).unwrap();
        let n = heights.len();
        let mut offsets = vec![0];
        let mut neighbors = Vec::new();
        for i in 0..n {
            if i > 0 {
                neighbors.push(i as u32 - 1);
            }
            if i + 1 < n {
                neighbors.push(i as u32 + 1);
            }
            offsets.push(neighbors.len() as u32);
        }
        w.surface = Surface {
            centers: vec![[1., 0., 0.]; n],
            areas: vec![1e9; n],
            faces: Vec::new(),
            offsets,
            distances: vec![1.; neighbors.len()],
            neighbors,
            boundary_offsets: Vec::new(),
            boundaries: Vec::new(),
        };
        w.terrain.elevation = heights.to_vec();
        w.water.body_ids = bodies.to_vec();
        w.water.main_ocean_id = u32::from(bodies.contains(&1));
        w.drainage = Drainage::build(&w.surface, heights, bodies);
        w.basins = Basins::build(&w.surface, heights).unwrap();
        climate.monthly_temperature_celsius = vec![vec![25.; n]; MONTHS];
        climate.monthly_precipitation_mm = vec![vec![0.; n]; MONTHS];
        (w, climate)
    }

    fn audit_budgets(w: &World, climate: &Normals, result: &Rivers) {
        let t = result.totals;
        near(
            t.land_precipitation,
            t.land_aet + t.land_runoff + t.land_storage,
        );
        near(
            t.land_runoff + t.lake_precipitation,
            t.ocean_discharge + t.lake_evaporation + t.terminal_storage,
        );
        near(
            t.land_runoff,
            t.land_runoff_to_ocean
                + t.land_runoff_to_endorheic
                + t.land_runoff_evaporated_in_spilling_lakes,
        );
        for (i, &r) in w.drainage.receivers.iter().enumerate() {
            if w.water.body_ids[i] == 0 {
                let p: f64 = climate.monthly_precipitation_mm.iter().map(|m| m[i]).sum();
                near(
                    p,
                    result.aet_mm[i] + result.runoff_mm[i] + result.storage_mm[i],
                );
                assert!(result.aet_mm[i] <= p.min(result.pet_mm[i]) + 1e-8);
                assert!(result.monthly_runoff_mm[i].iter().all(|&v| v >= 0.));
            }
            for m in 0..MONTHS {
                assert!(result.monthly_discharge[i][m] <= result.monthly_discharge[r as usize][m]);
            }
        }
        for lake in &result.lakes {
            near(
                lake.inflow + lake.precipitation,
                lake.evaporation + lake.spill + lake.storage,
            );
            assert!(lake.evaporation <= lake.potential_evaporation);
        }
    }

    #[test]
    fn oudin_reference() {
        let s = Settings::default();
        near(potential_et(20., 30. / 0.0864, 1, s), 30. / 2.45 / 4.);
        assert_eq!(potential_et(-6., 300., 31, s), 0.);
        assert_eq!(potential_et(20., 0., 31, s), 0.);
    }

    #[test]
    fn soil_recharge_increases_with_saturation_and_closes_monthly() {
        let capacity = Settings::default().soil_capacity_mm;
        let full_et = capacity * Settings::default().soil_et_full_fraction;
        let input = 25.;
        let wetting = (input / capacity).tanh();
        let demand = 12.;
        let mut previous = -1.;
        for start in [0., 10., 30., 50., 70., 90., capacity] {
            let mut soil = start;
            let (runoff, aet) = soil_month(&mut soil, capacity, full_et, input, wetting, demand);
            assert!(runoff > previous);
            assert!((0. ..=capacity).contains(&soil));
            assert!((0. ..=4. * demand).contains(&aet));
            near(start + 4. * input, soil + aet + runoff);
            previous = runoff;
        }
        near(infiltrate(0., capacity, wetting), capacity * wetting);
        assert_eq!(infiltrate(capacity, capacity, wetting), 0.);
    }

    #[test]
    fn soil_budget_repeats_across_a_seasonal_year() {
        let mut p = [0.; MONTHS];
        p[11] = 300.;
        let (runoff, aet, storage) = land_balance(
            [10.; MONTHS],
            p,
            [20.; MONTHS],
            [30; MONTHS],
            Settings::default(),
        );
        assert!(runoff[11] > 0. && runoff[..11].iter().all(|&q| q == 0.));
        near(aet + runoff.iter().sum::<f64>(), 300.);
        assert!(aet > 20. && aet <= 240.);
        near(storage, 0.);
        // Weak forcing exercises the bracketed solve when spin-up is slow.
        let (runoff, aet) = soil_balance([0.001; MONTHS], [0.001; MONTHS], Settings::default());
        near(aet + runoff.iter().sum::<f64>(), 0.012);
        assert!(aet > 0. && runoff[0] > 0.);
    }

    #[test]
    fn generated_land_et_is_within_the_fu_band() {
        let w = world(0.71);
        let climate = Normals::from_world(&w).unwrap();
        let result = Rivers::from_world(&w, &climate).unwrap();
        let mut volumes = [0.; 3];
        for (i, &area) in w.surface.areas.iter().enumerate() {
            let p = climate.annual_precipitation_mm[i];
            if w.water.body_ids[i] != 0 || p <= 0. || result.storage_mm[i] > 1e-6 {
                continue;
            }
            let phi = result.pet_mm[i] / p;
            let fu = |omega: f64| 1. + phi - (1. + phi.powf(omega)).powf(1. / omega);
            volumes[0] += p * area * fu(2.);
            volumes[1] += result.aet_mm[i] * area;
            volumes[2] += p * area * fu(4.);
        }
        assert!(
            volumes[0] < volumes[1] && volumes[1] < volumes[2],
            "{volumes:?}"
        );
    }

    #[test]
    fn snow_crosses_the_year_boundary_and_permanent_ice_accumulates() {
        let s = Settings::default();
        let mut p = [0.; MONTHS];
        p[10] = 180.;
        let mut t = [-4.; MONTHS];
        t[1] = 4.;
        let (runoff, aet, storage) = land_balance(t, p, [0.; MONTHS], [30; MONTHS], s);
        assert_eq!(runoff[10], 0.);
        assert_eq!(runoff[1], 180.);
        near(aet + storage, 0.);
        let (runoff, aet, storage) = land_balance([-1.; MONTHS], p, [10.; MONTHS], [30; MONTHS], s);
        assert_eq!(runoff, [0.; MONTHS]);
        near(aet, 0.);
        near(storage, 180.);
        t[1] = 1.;
        let (runoff, _, storage) = land_balance(t, p, [0.; MONTHS], [30; MONTHS], s);
        near(storage, 90.);
        near(runoff[1], 90.);
    }

    #[test]
    fn coast_receives_exact_local_catchment_sum_and_zero_precipitation_stays_dry() {
        let (w, mut climate) = chain(&[8., 5., 3., 2., -1.], &[0, 0, 0, 0, 1]);
        let dry = Rivers::from_world(&w, &climate).unwrap();
        assert!(
            dry.monthly_discharge
                .iter()
                .flatten()
                .all(|&flow| flow == 0.)
        );
        climate.monthly_precipitation_mm[3][0] = 400.;
        climate.monthly_precipitation_mm[3][2] = 900.;
        let river = Rivers::from_world(&w, &climate).unwrap();
        audit_budgets(&w, &climate, &river);
        assert_eq!(river.class[4], 1);
        assert_eq!(river.terminal, vec![4; 5]);
        let volume: f64 = (0..4)
            .map(|i| river.monthly_runoff_mm[i][3] * w.surface.areas[i] * 0.001)
            .sum();
        near(
            river.monthly_discharge[4][3] * seconds(river.month_days[3]),
            volume,
        );
        near(
            river.mean_discharge[4] * seconds(DAYS_PER_YEAR),
            river.totals.land_runoff,
        );
    }

    #[test]
    fn humid_dry_sink_spills_through_a_lake_and_arid_basins_remain_closed() {
        let (w, mut climate) = chain(&[9., 1., 8., 0., 7., -2.], &[0, 0, 0, 2, 0, 1]);
        climate.monthly_precipitation_mm[0][0] = 260.;
        let arid = Rivers::from_world(&w, &climate).unwrap();
        audit_budgets(&w, &climate, &arid);
        assert!(arid.lakes[0].inflow > 0.);
        assert_eq!(arid.lakes[0].spill, 0.);
        assert_eq!(arid.terminal[0], 1);
        assert_eq!(
            arid.totals.land_runoff_to_endorheic,
            arid.totals.land_runoff
        );
        assert_eq!(arid.totals.ocean_discharge, 0.);
        for m in &mut climate.monthly_precipitation_mm {
            m[0] = 600.;
            m[3] = 40.;
        }
        let humid = Rivers::from_world(&w, &climate).unwrap();
        audit_budgets(&w, &climate, &humid);
        let [sink, lake] = &humid.lakes[..] else {
            panic!("expected two inland terminals")
        };
        assert_eq!(sink.outlet, Some([1, 2]));
        assert_eq!(sink.sill_elevation, 8.);
        assert_eq!(lake.outlet, Some([3, 4]));
        assert_eq!(lake.sill_elevation, 7.);
        near(sink.spill, lake.inflow);
        near(lake.precipitation, 480e6);
        near(lake.spill, humid.totals.ocean_discharge);
        assert_eq!(humid.terminal, vec![5; 6]);
        assert_eq!(humid.class[5], 2);
        assert_eq!(humid.spill_segments.len(), 2);
    }

    #[test]
    fn oceanless_root_accounts_for_surplus_and_wet_world_has_no_land_runoff() {
        let (w, mut climate) = chain(&[9., 1., 8., 0.], &[0; 4]);
        climate.monthly_precipitation_mm[0][0] = 2_000.;
        let closed = Rivers::from_world(&w, &climate).unwrap();
        audit_budgets(&w, &climate, &closed);
        assert!(closed.ocean_terminal.is_none());
        assert!(closed.totals.terminal_storage > 0.);
        near(closed.totals.endorheic_area, closed.totals.land_area);
        assert_eq!(closed.terminal, vec![3; 4]);
        let wet = world(1.);
        let climate = Normals::from_world(&wet).unwrap();
        let result = Rivers::from_world(&wet, &climate).unwrap();
        audit_budgets(&wet, &climate, &result);
        assert_eq!(result.totals.land_area, 0.);
        assert_eq!(result.totals.ocean_discharge, 0.);
    }

    #[test]
    fn generated_surface_balances_close_and_spill_edges_are_adjacent() {
        let w = world(0.71);
        let climate = Normals::from_world(&w).unwrap();
        let result = Rivers::from_world(&w, &climate).unwrap();
        audit_budgets(&w, &climate, &result);
        for segment in result.spill_segments {
            let i = segment.from as usize;
            assert!(
                w.surface.neighbors
                    [w.surface.offsets[i] as usize..w.surface.offsets[i + 1] as usize]
                    .contains(&segment.to)
            );
        }
    }
}
