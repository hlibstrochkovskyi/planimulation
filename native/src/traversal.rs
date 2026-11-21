//! Read-only foot traversal cost and least-time travel field (`traversal-1`).
//!
//! Computes directed edge travel times (in days) across the region graph for local
//! summer and local winter from Tobler's hiking function, a coarse-grid neighborhood
//! ruggedness proxy, land-cover multipliers (using active `vegetation-2` woody cover
//! when present, otherwise `climate-normals-1` biome classes), winter snowpack from
//! monthly temperature and precipitation normals, deep-winter lake/river freezing,
//! and a drainage-area river-crossing proxy designed to be replaced by hydraulic
//! discharge later.
//!
//! Not part of the world recipe or fingerprint; never alters terrain, water, climate
//! or vegetation state.
use crate::{
    World,
    climate_normals::{Biome, Normals, aridity_threshold},
    mountain_belts::Key,
    vegetation::State as VegetationState,
};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const MODEL_VERSION: &str = "traversal-1";
pub const MAX_SOURCES: usize = 64;
/// Floor on Tobler hiking speed (km/h) to keep extreme synthetic slopes finite.
pub const MIN_TOBLER_KM_PER_HOUR: f64 = 0.05;
/// Flat-ground cover multipliers (dimensionless speed factors relative to open grassland).
pub const ICE_COVER_FACTOR: f64 = 0.30;
pub const TUNDRA_COVER_FACTOR: f64 = 0.55;
pub const HOT_DESERT_COVER_FACTOR: f64 = 0.58;
pub const COLD_DESERT_COVER_FACTOR: f64 = 0.78;
pub const TROPICAL_OPEN_COVER_FACTOR: f64 = 0.92;
/// Woody obstruction slopes: `woody_factor = 1 - drag * woody`.
pub const TEMPERATE_WOODY_DRAG: f64 = 0.35;
pub const BOREAL_WOODY_DRAG: f64 = 0.42;
pub const RAINFOREST_WOODY_DRAG: f64 = 0.51;

/// Bit flags packed per region for wire transport and inspection.
pub const FLAG_PASSABLE_SUMMER: u32 = 1 << 0;
pub const FLAG_PASSABLE_WINTER: u32 = 1 << 1;
pub const FLAG_FROZEN_LAKE_WINTER: u32 = 1 << 2;
pub const FLAG_FROZEN_RIVER_WINTER: u32 = 1 << 3;
pub const FLAG_FREEZING_MONTHS_SHIFT: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Season {
    Summer,
    Winter,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    /// Sustained walking hours per day for foot groups.
    pub hours_per_day: f64,
    /// Mean neighbor elevation difference (m) at which the sub-grid ruggedness factor
    /// halves speed: `1 / (1 + local_relief / relief_scale_meters)`.
    pub relief_scale_meters: f64,
    /// Contributing dry-land area (m²) above which river crossings add delay.
    pub river_threshold_square_meters: f64,
    /// Extra crossing hours per order-of-magnitude (log10) above the area threshold.
    pub river_crossing_hours_per_decade: f64,
    /// Fraction of river-crossing delay paid when walking along a drainage edge.
    pub along_river_factor: f64,
    /// Maximum winter snow slowdown weight: `m_snow = 1 / (1 + snow_slowdown_max * snow_index)`.
    pub snow_slowdown_max: f64,
    /// Freezing-month count at which winter snow duration saturates.
    pub snow_months_saturation: f64,
    /// Cold-month precipitation (mm) at which winter snowpack depth saturates.
    pub snow_precip_saturation_mm: f64,
    /// Coldest-month temperature (°C) at or below which inland lakes and rivers freeze solid.
    pub freeze_temp_celsius: f64,
    /// Minimum count of months below 0 °C required for solid lake/river freezing.
    pub freeze_min_months: u32,
    /// Cover speed factor on deep-winter frozen inland lake ice.
    pub frozen_lake_cover_factor: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hours_per_day: 8.0,
            relief_scale_meters: 600.0,
            river_threshold_square_meters: 1e11,
            river_crossing_hours_per_decade: 8.0,
            along_river_factor: 0.25,
            snow_slowdown_max: 1.5,
            snow_months_saturation: 6.0,
            snow_precip_saturation_mm: 300.0,
            freeze_temp_celsius: -10.0,
            freeze_min_months: 3,
            frozen_lake_cover_factor: 0.75,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        let positive = |v: f64, high: f64| v.is_finite() && v > 0.0 && v <= high;
        if !positive(self.hours_per_day, 24.0)
            || !positive(self.relief_scale_meters, 20_000.0)
            || !positive(self.river_threshold_square_meters, 1e15)
            || !(self.river_crossing_hours_per_decade.is_finite()
                && (0.0..=100.0).contains(&self.river_crossing_hours_per_decade))
            || !(self.along_river_factor.is_finite()
                && (0.0..=1.0).contains(&self.along_river_factor))
            || !(self.snow_slowdown_max.is_finite()
                && (0.0..=10.0).contains(&self.snow_slowdown_max))
            || !positive(self.snow_months_saturation, 12.0)
            || !positive(self.snow_precip_saturation_mm, 5_000.0)
            || !(self.freeze_temp_celsius.is_finite()
                && (-80.0..=0.0).contains(&self.freeze_temp_celsius))
            || self.freeze_min_months > 12
            || !positive(self.frozen_lake_cover_factor, 1.0)
        {
            return Err("Traversal settings are outside the supported range.".into());
        }
        Ok(())
    }
}

/// Tobler's hiking function (1993): `v = 6 exp(-3.5 |dh/dx + 0.05|)` km/h.
/// Peaks at 6.0 km/h on a 5% downhill grade and yields ~5.04 km/h on flat ground.
pub fn tobler_speed_km_per_hour(slope: f64) -> f64 {
    (6.0 * (-3.5 * (slope + 0.05).abs()).exp()).max(MIN_TOBLER_KM_PER_HOUR)
}

/// Coarse-grid sub-grid ruggedness multiplier from mean neighbor elevation difference.
pub fn ruggedness_factor(local_relief_meters: f64, relief_scale_meters: f64) -> f64 {
    1.0 / (1.0 + local_relief_meters.max(0.0) / relief_scale_meters)
}

/// Climate-only surface speed factor capturing ice, tundra bog, desert heat/sand stress,
/// and tropical warmth before woody obstruction.
pub fn climate_surface_factor(
    annual_mean_celsius: f64,
    warmest_month_celsius: f64,
    annual_precipitation_mm: f64,
) -> f64 {
    if warmest_month_celsius < 0.0 {
        ICE_COVER_FACTOR
    } else if warmest_month_celsius < 10.0 {
        TUNDRA_COVER_FACTOR
    } else if annual_precipitation_mm < aridity_threshold(annual_mean_celsius) {
        if annual_mean_celsius >= 18.0 {
            HOT_DESERT_COVER_FACTOR
        } else {
            COLD_DESERT_COVER_FACTOR
        }
    } else if annual_mean_celsius >= 18.0 {
        TROPICAL_OPEN_COVER_FACTOR
    } else {
        1.0
    }
}

/// Woody obstruction strength by thermal/moisture climate regime.
pub fn woody_drag(
    warmest_month_celsius: f64,
    coldest_month_celsius: f64,
    driest_month_mm: f64,
) -> f64 {
    if warmest_month_celsius < 10.0 {
        0.0
    } else if coldest_month_celsius >= 18.0 && driest_month_mm >= 60.0 {
        RAINFOREST_WOODY_DRAG
    } else if coldest_month_celsius <= -3.0 && warmest_month_celsius < 22.0 {
        BOREAL_WOODY_DRAG
    } else {
        TEMPERATE_WOODY_DRAG
    }
}

/// Speed multiplier from woody cover fraction `w in [0, 1]`. Strictly monotonic in `w`.
pub fn woody_speed_factor(woody: f64, drag: f64) -> f64 {
    1.0 - drag * woody.clamp(0.0, 1.0)
}

/// Fallback woody fraction inferred from `climate-normals-1` biome when no vegetation
/// simulation state is active.
pub fn biome_fallback_woody(biome: u8) -> f64 {
    if biome == Biome::BorealForest as u8
        || biome == Biome::TemperateForest as u8
        || biome == Biome::TropicalSeasonalForest as u8
        || biome == Biome::TropicalRainforest as u8
    {
        0.95
    } else if biome == Biome::Savanna as u8 {
        0.15
    } else {
        0.0
    }
}

/// Dimensionless winter snow severity in `[0, 1]` from freezing-month count and
/// precipitation falling during freezing months. Monotonic in both arguments.
pub fn winter_snow_index(freezing_months: u8, winter_snow_mm: f64, settings: &Settings) -> f64 {
    if freezing_months == 0 {
        return 0.0;
    }
    let duration = (f64::from(freezing_months) / settings.snow_months_saturation).min(1.0);
    let depth = (winter_snow_mm.max(0.0) / settings.snow_precip_saturation_mm).min(1.0);
    duration * (0.35 + 0.65 * depth)
}

/// Winter speed multiplier from snow severity index in `[0, 1]`.
pub fn snow_speed_factor(snow_index: f64, settings: &Settings) -> f64 {
    1.0 / (1.0 + settings.snow_slowdown_max * snow_index.clamp(0.0, 1.0))
}

/// True when deep winter freezes inland lakes and rivers solid enough to cross on foot.
pub fn is_deep_winter_frozen(
    coldest_month_celsius: f64,
    freezing_months: u8,
    settings: &Settings,
) -> bool {
    coldest_month_celsius <= settings.freeze_temp_celsius
        && u32::from(freezing_months) >= settings.freeze_min_months
}

/// Unfrozen river-crossing delay (hours) proxied from static drainage contributing area.
/// Uses `max(river_threshold_square_meters, 8 * region_area)` so headwater cells on
/// coarse meshes do not trigger spurious river penalties, while subdivision 6 (~12,450 km²
/// per region) uses the physical 100,000 km² threshold directly.
/// Kept as a standalone function so a future discharge field (m³/s) can replace it directly.
pub fn river_crossing_delay_from_area(
    contributing_area_m2: f64,
    region_area_m2: f64,
    is_wet: bool,
    settings: &Settings,
) -> f64 {
    let threshold = settings
        .river_threshold_square_meters
        .max(8.0 * region_area_m2.max(0.0));
    if is_wet || contributing_area_m2 <= threshold {
        return 0.0;
    }
    settings.river_crossing_hours_per_decade * (contributing_area_m2 / threshold).log10()
}

/// Result of deterministic least-time Dijkstra over the region graph for one season.
#[derive(Clone, Debug, PartialEq)]
pub struct TravelField {
    pub season: Season,
    pub sources: Vec<u32>,
    /// Least travel time from the nearest passable source, in days (`INFINITY` if unreachable).
    pub days: Vec<f64>,
    /// Accumulated spherical distance along the least-time route, in meters (`INFINITY` if unreachable).
    pub distance_meters: Vec<f64>,
    /// Predecessor region along the least-time route (`u32::MAX` if unreachable, self for passable source).
    pub predecessor: Vec<u32>,
}

/// Deterministic Dijkstra on a directed weighted CSR graph.
/// Edge cost ties prefer the smaller predecessor ID; heap pops break ties by smaller region ID.
pub fn dijkstra_graph(
    n: usize,
    offsets: &[u32],
    neighbors: &[u32],
    edge_days: &[f64],
    edge_distances: &[f64],
    passable: &[bool],
    sources: &[usize],
) -> (Vec<f64>, Vec<f64>, Vec<u32>) {
    let mut days = vec![f64::INFINITY; n];
    let mut distance_meters = vec![f64::INFINITY; n];
    let mut predecessor = vec![u32::MAX; n];
    let mut heap = BinaryHeap::new();
    for &s in sources {
        if passable[s] && days[s].is_infinite() {
            days[s] = 0.0;
            distance_meters[s] = 0.0;
            predecessor[s] = s as u32;
            heap.push(Reverse(Key(0.0, s)));
        }
    }
    while let Some(Reverse(Key(cost, u))) = heap.pop() {
        if cost > days[u] {
            continue;
        }
        let start = offsets[u] as usize;
        let end = offsets[u + 1] as usize;
        for k in start..end {
            let w = edge_days[k];
            if !w.is_finite() {
                continue;
            }
            let v = neighbors[k] as usize;
            let next = cost + w;
            let next_dist = distance_meters[u] + edge_distances[k];
            if next < days[v]
                || (next == days[v]
                    && (next_dist < distance_meters[v]
                        || (next_dist == distance_meters[v] && (u as u32) < predecessor[v])))
            {
                days[v] = next;
                distance_meters[v] = next_dist;
                predecessor[v] = u as u32;
                heap.push(Reverse(Key(next, v)));
            }
        }
    }
    (days, distance_meters, predecessor)
}

/// Precomputed foot-traversal cost model for summer and winter on a static generated world.
#[derive(Clone, Debug)]
pub struct Model {
    settings: Settings,
    uses_vegetation: bool,
    vegetation_year: Option<u32>,
    offsets: Vec<u32>,
    neighbors: Vec<u32>,
    distances: Vec<f64>,
    pub local_relief_meters: Vec<f64>,
    pub ruggedness_factor: Vec<f64>,
    pub slope_factor_summer: Vec<f64>,
    pub slope_factor_winter: Vec<f64>,
    pub cover_factor_summer: Vec<f64>,
    pub cover_factor_winter: Vec<f64>,
    pub snow_index_winter: Vec<f64>,
    pub snow_factor_winter: Vec<f64>,
    pub freezing_months: Vec<u8>,
    pub river_delay_summer_hours: Vec<f64>,
    pub river_delay_winter_hours: Vec<f64>,
    pub passable_summer: Vec<bool>,
    pub passable_winter: Vec<bool>,
    pub flags: Vec<u32>,
    pub hours_per_100km_summer: Vec<f64>,
    pub hours_per_100km_winter: Vec<f64>,
    pub edge_days_summer: Vec<f64>,
    pub edge_days_winter: Vec<f64>,
}

impl Model {
    pub fn from_world(world: &World, settings: Settings) -> Result<Self, String> {
        let normals = Normals::from_world(world)?;
        Self::new(world, &normals, None, settings)
    }

    pub fn new(
        world: &World,
        normals: &Normals,
        vegetation: Option<&VegetationState>,
        settings: Settings,
    ) -> Result<Self, String> {
        settings.validate()?;
        let s = &world.surface;
        let n = s.centers.len();
        if normals.biome.len() != n
            || normals.monthly_temperature_celsius.len() != 12
            || normals.monthly_precipitation_mm.len() != 12
            || world.terrain.elevation.len() != n
            || world.water.body_ids.len() != n
            || world.drainage.contributing_area.len() != n
            || world.drainage.receivers.len() != n
            || vegetation.is_some_and(|v| v.woody.len() != n)
        {
            return Err("Traversal input arrays do not match the surface.".into());
        }

        let level = world.water.level_meters;
        let main_ocean = world.water.main_ocean_id;
        let surface_height: Vec<f64> = (0..n)
            .map(|i| {
                if world.water.body_ids[i] != 0 {
                    level
                } else {
                    world.terrain.elevation[i]
                }
            })
            .collect();

        let mut local_relief_meters = vec![0.0; n];
        let mut rugged = vec![1.0; n];
        let mut cover_factor_summer = vec![0.0; n];
        let mut cover_factor_winter = vec![0.0; n];
        let mut snow_index_winter = vec![0.0; n];
        let mut snow_factor_winter = vec![1.0; n];
        let mut freezing_months = vec![0u8; n];
        let mut river_delay_summer_hours = vec![0.0; n];
        let mut river_delay_winter_hours = vec![0.0; n];
        let mut passable_summer = vec![false; n];
        let mut passable_winter = vec![false; n];
        let mut flags = vec![0u32; n];

        for i in 0..n {
            let body = world.water.body_ids[i];
            let is_wet = body != 0;
            let is_ocean = is_wet && body == main_ocean;
            let is_inland_lake = is_wet && !is_ocean;

            let mut cold_months = 0u8;
            let mut cold_precip = 0.0;
            for month in 0..12 {
                if normals.monthly_temperature_celsius[month][i] < 0.0 {
                    cold_months += 1;
                    cold_precip += normals.monthly_precipitation_mm[month][i];
                }
            }
            freezing_months[i] = cold_months;
            let snow_idx = winter_snow_index(cold_months, cold_precip, &settings);
            snow_index_winter[i] = snow_idx;
            snow_factor_winter[i] = snow_speed_factor(snow_idx, &settings);

            let deep_freeze =
                is_deep_winter_frozen(normals.coldest_month_celsius[i], cold_months, &settings);
            let frozen_lake = is_inland_lake && deep_freeze;
            passable_summer[i] = !is_wet;
            passable_winter[i] = !is_wet || frozen_lake;

            if !is_wet {
                let start = s.offsets[i] as usize;
                let end = s.offsets[i + 1] as usize;
                let count = (end - start).max(1) as f64;
                let relief_sum: f64 = (start..end)
                    .map(|k| (surface_height[s.neighbors[k] as usize] - surface_height[i]).abs())
                    .sum();
                local_relief_meters[i] = relief_sum / count;
                rugged[i] = ruggedness_factor(local_relief_meters[i], settings.relief_scale_meters);

                let climate_factor = climate_surface_factor(
                    normals.annual_mean_celsius[i],
                    normals.warmest_month_celsius[i],
                    normals.annual_precipitation_mm[i],
                );
                let drag = woody_drag(
                    normals.warmest_month_celsius[i],
                    normals.coldest_month_celsius[i],
                    normals.driest_month_mm[i],
                );
                let woody = match vegetation {
                    Some(v) => v.woody[i].clamp(0.0, 1.0),
                    None => biome_fallback_woody(normals.biome[i]),
                };
                let cover = climate_factor * woody_speed_factor(woody, drag);
                cover_factor_summer[i] = cover;
                cover_factor_winter[i] = cover;
            } else if frozen_lake {
                local_relief_meters[i] = 0.0;
                rugged[i] = 1.0;
                cover_factor_summer[i] = 0.0;
                cover_factor_winter[i] = settings.frozen_lake_cover_factor;
            }

            let river_summer = river_crossing_delay_from_area(
                world.drainage.contributing_area[i],
                s.areas[i],
                is_wet,
                &settings,
            );
            let frozen_river = river_summer > 0.0 && deep_freeze;
            river_delay_summer_hours[i] = river_summer;
            river_delay_winter_hours[i] = if frozen_river { 0.0 } else { river_summer };

            let mut f = u32::from(cold_months) << FLAG_FREEZING_MONTHS_SHIFT;
            if passable_summer[i] {
                f |= FLAG_PASSABLE_SUMMER;
            }
            if passable_winter[i] {
                f |= FLAG_PASSABLE_WINTER;
            }
            if frozen_lake {
                f |= FLAG_FROZEN_LAKE_WINTER;
            }
            if frozen_river {
                f |= FLAG_FROZEN_RIVER_WINTER;
            }
            flags[i] = f;
        }

        let m = s.neighbors.len();
        let mut edge_days_summer = vec![f64::INFINITY; m];
        let mut edge_days_winter = vec![f64::INFINITY; m];
        let mut hours_per_100km_summer = vec![f64::INFINITY; n];
        let mut hours_per_100km_winter = vec![f64::INFINITY; n];
        let mut slope_factor_summer = vec![0.0; n];
        let mut slope_factor_winter = vec![0.0; n];
        let flat_tobler = tobler_speed_km_per_hour(0.0);

        let compute_edge_hours = |u: usize,
                                  v: usize,
                                  d_meters: f64,
                                  passable: &[bool],
                                  cover: &[f64],
                                  snow: Option<&[f64]>,
                                  river_delay: &[f64]|
         -> f64 {
            if !passable[u] || !passable[v] {
                return f64::INFINITY;
            }
            let slope = (surface_height[v] - surface_height[u]) / d_meters;
            let v_tobler = tobler_speed_km_per_hour(slope);
            let half_km = 0.5 * d_meters / 1000.0;
            let s_u = snow.map_or(1.0, |a| a[u]);
            let s_v = snow.map_or(1.0, |a| a[v]);
            let m_u = rugged[u] * cover[u] * s_u;
            let m_v = rugged[v] * cover[v] * s_v;
            let walk = half_km / (v_tobler * m_u) + half_km / (v_tobler * m_v);
            let along = world.drainage.receivers[u] as usize == v
                || world.drainage.receivers[v] as usize == u;
            let river = 0.5
                * (river_delay[u] + river_delay[v])
                * if along {
                    settings.along_river_factor
                } else {
                    1.0
                };
            walk + river
        };

        for u in 0..n {
            let start = s.offsets[u] as usize;
            let end = s.offsets[u + 1] as usize;
            let (mut sum_s, mut slope_s, mut cnt_s) = (0.0, 0.0, 0usize);
            let (mut sum_w, mut slope_w, mut cnt_w) = (0.0, 0.0, 0usize);
            for k in start..end {
                let v = s.neighbors[k] as usize;
                let d = s.distances[k];
                let h_s_out = compute_edge_hours(
                    u,
                    v,
                    d,
                    &passable_summer,
                    &cover_factor_summer,
                    None,
                    &river_delay_summer_hours,
                );
                if h_s_out.is_finite() {
                    edge_days_summer[k] = h_s_out / settings.hours_per_day;
                    let h_s_in = compute_edge_hours(
                        v,
                        u,
                        d,
                        &passable_summer,
                        &cover_factor_summer,
                        None,
                        &river_delay_summer_hours,
                    );
                    sum_s += 0.5 * (h_s_out + h_s_in) / (d / 100_000.0);
                    let sl = (surface_height[v] - surface_height[u]) / d;
                    let mean_tobler_ratio = 0.5
                        * (tobler_speed_km_per_hour(sl) + tobler_speed_km_per_hour(-sl))
                        / flat_tobler;
                    slope_s += mean_tobler_ratio * rugged[u];
                    cnt_s += 1;
                }
                let h_w_out = compute_edge_hours(
                    u,
                    v,
                    d,
                    &passable_winter,
                    &cover_factor_winter,
                    Some(&snow_factor_winter),
                    &river_delay_winter_hours,
                );
                if h_w_out.is_finite() {
                    edge_days_winter[k] = h_w_out / settings.hours_per_day;
                    let h_w_in = compute_edge_hours(
                        v,
                        u,
                        d,
                        &passable_winter,
                        &cover_factor_winter,
                        Some(&snow_factor_winter),
                        &river_delay_winter_hours,
                    );
                    sum_w += 0.5 * (h_w_out + h_w_in) / (d / 100_000.0);
                    let sl = (surface_height[v] - surface_height[u]) / d;
                    let mean_tobler_ratio = 0.5
                        * (tobler_speed_km_per_hour(sl) + tobler_speed_km_per_hour(-sl))
                        / flat_tobler;
                    slope_w += mean_tobler_ratio * rugged[u];
                    cnt_w += 1;
                }
            }
            if passable_summer[u] {
                if cnt_s > 0 {
                    hours_per_100km_summer[u] = sum_s / cnt_s as f64;
                    slope_factor_summer[u] = slope_s / cnt_s as f64;
                } else {
                    hours_per_100km_summer[u] =
                        100.0 / (flat_tobler * rugged[u] * cover_factor_summer[u]);
                    slope_factor_summer[u] = rugged[u];
                }
            }
            if passable_winter[u] {
                if cnt_w > 0 {
                    hours_per_100km_winter[u] = sum_w / cnt_w as f64;
                    slope_factor_winter[u] = slope_w / cnt_w as f64;
                } else {
                    hours_per_100km_winter[u] = 100.0
                        / (flat_tobler
                            * rugged[u]
                            * cover_factor_winter[u]
                            * snow_factor_winter[u]);
                    slope_factor_winter[u] = rugged[u];
                }
            }
        }

        Ok(Self {
            settings,
            uses_vegetation: vegetation.is_some(),
            vegetation_year: vegetation.map(|v| v.year),
            offsets: s.offsets.clone(),
            neighbors: s.neighbors.clone(),
            distances: s.distances.clone(),
            local_relief_meters,
            ruggedness_factor: rugged,
            slope_factor_summer,
            slope_factor_winter,
            cover_factor_summer,
            cover_factor_winter,
            snow_index_winter,
            snow_factor_winter,
            freezing_months,
            river_delay_summer_hours,
            river_delay_winter_hours,
            passable_summer,
            passable_winter,
            flags,
            hours_per_100km_summer,
            hours_per_100km_winter,
            edge_days_summer,
            edge_days_winter,
        })
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    pub fn uses_vegetation(&self) -> bool {
        self.uses_vegetation
    }

    pub fn vegetation_year(&self) -> Option<u32> {
        self.vegetation_year
    }

    pub fn region_count(&self) -> usize {
        self.passable_summer.len()
    }

    pub fn edge_days(&self, season: Season) -> &[f64] {
        match season {
            Season::Summer => &self.edge_days_summer,
            Season::Winter => &self.edge_days_winter,
        }
    }

    pub fn passable(&self, season: Season) -> &[bool] {
        match season {
            Season::Summer => &self.passable_summer,
            Season::Winter => &self.passable_winter,
        }
    }

    pub fn hours_per_100km(&self, season: Season) -> &[f64] {
        match season {
            Season::Summer => &self.hours_per_100km_summer,
            Season::Winter => &self.hours_per_100km_winter,
        }
    }

    /// Least-time travel field in days from one source region.
    pub fn travel_time(&self, season: Season, source: usize) -> Result<TravelField, String> {
        self.travel_time_multi(season, &[source])
    }

    /// Least-time travel field in days from one or more source regions.
    pub fn travel_time_multi(
        &self,
        season: Season,
        sources: &[usize],
    ) -> Result<TravelField, String> {
        let n = self.region_count();
        if sources.is_empty() || sources.len() > MAX_SOURCES {
            return Err(format!(
                "Travel time requires 1–{MAX_SOURCES} origin regions."
            ));
        }
        if sources.iter().any(|&s| s >= n) {
            return Err("Travel origin is outside the generated world.".into());
        }
        let (days, distance_meters, predecessor) = dijkstra_graph(
            n,
            &self.offsets,
            &self.neighbors,
            self.edge_days(season),
            &self.distances,
            self.passable(season),
            sources,
        );
        Ok(TravelField {
            season,
            sources: sources.iter().map(|&s| s as u32).collect(),
            days,
            distance_meters,
            predecessor,
        })
    }

    /// Trace the least-time path from the nearest source to `target`.
    pub fn trace_route(&self, field: &TravelField, target: usize) -> Option<Vec<usize>> {
        let n = self.region_count();
        if target >= n || !field.days[target].is_finite() || field.predecessor[target] == u32::MAX {
            return None;
        }
        let mut path = Vec::new();
        let mut cur = target;
        for _ in 0..=n {
            path.push(cur);
            let prev = field.predecessor[cur] as usize;
            if prev == cur {
                path.reverse();
                return Some(path);
            }
            cur = prev;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Recipe;
    use crate::vegetation::Model as VegetationModel;

    fn test_world(seed: &str, subdivision: u32) -> World {
        let recipe: Recipe = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "modelVersion": "multiscale-geography-4",
            "randomVersion": "fnv1a-utf8-mulberry32-1",
            "seed": seed,
            "subdivision": subdivision,
            "radiusMeters": 6_371_000.0,
            "plateCount": 12,
            "maxPlateSpeedCmPerYear": 8.0,
            "continentalFraction": 0.38,
            "continentalScale": 1.0,
            "reliefScale": 1.0,
            "boundaryWidthKm": 300.0,
            "detailAmplitudeMeters": 300.0,
            "terrainPreparationPasses": 4,
            "water": { "mode": "coverage", "fraction": 0.71 }
        }))
        .unwrap();
        World::generate(recipe).unwrap()
    }

    #[test]
    fn tobler_flat_speed_and_uphill_downhill_asymmetry() {
        let flat = tobler_speed_km_per_hour(0.0);
        assert!(
            (flat - 5.036746).abs() < 1e-4,
            "flat Tobler speed should be ~5.04 km/h, got {flat}"
        );
        let peak = tobler_speed_km_per_hour(-0.05);
        assert!((peak - 6.0).abs() < 1e-12);
        // Uphill is strictly slower than equal-magnitude downhill for all positive grades.
        for grade in [0.02, 0.05, 0.10, 0.20, 0.35] {
            let up = tobler_speed_km_per_hour(grade);
            let down = tobler_speed_km_per_hour(-grade);
            assert!(
                up < down && up < flat,
                "grade {grade}: up {up} vs down {down}"
            );
        }
        // Very steep downhill eventually slows below flat as well.
        assert!(tobler_speed_km_per_hour(-0.25) < flat);
    }

    #[test]
    fn dijkstra_matches_brute_force_on_small_directed_graph() {
        // 6-node directed asymmetric graph with unreachable node 5.
        let n = 6;
        let edges: [Vec<(usize, f64, f64)>; 6] = [
            vec![(1, 2.5, 100.0), (2, 6.0, 220.0)],
            vec![(0, 3.0, 100.0), (2, 2.0, 90.0), (3, 7.5, 310.0)],
            vec![(3, 1.5, 80.0), (4, 8.0, 350.0)],
            vec![(4, 2.2, 110.0), (1, 1.0, 310.0)],
            vec![(0, 9.0, 400.0)],
            vec![],
        ];
        let (mut offsets, mut neighbors, mut weights, mut dists) =
            (vec![0u32], Vec::new(), Vec::new(), Vec::new());
        for adj in &edges {
            for &(v, w, d) in adj {
                neighbors.push(v as u32);
                weights.push(w);
                dists.push(d);
            }
            offsets.push(neighbors.len() as u32);
        }
        let passable = [true, true, true, true, true, false];

        // Brute-force all simple paths from source to target.
        fn brute(
            u: usize,
            target: usize,
            visited: u32,
            acc: f64,
            edges: &[Vec<(usize, f64, f64)>],
        ) -> f64 {
            if u == target {
                return acc;
            }
            let mut best = f64::INFINITY;
            for &(v, w, _) in &edges[u] {
                if (visited & (1 << v)) == 0 {
                    best = best.min(brute(v, target, visited | (1 << v), acc + w, edges));
                }
            }
            best
        }

        for src in 0..5 {
            let (days, _, pred) =
                dijkstra_graph(n, &offsets, &neighbors, &weights, &dists, &passable, &[src]);
            assert_eq!(days[src], 0.0);
            assert_eq!(pred[src], src as u32);
            assert!(days[5].is_infinite() && pred[5] == u32::MAX);
            for dst in 0..5 {
                let exact = brute(src, dst, 1 << src, 0.0, &edges);
                assert!(
                    (days[dst] - exact).abs() < 1e-12,
                    "src {src} -> dst {dst}: dijkstra {} vs brute {exact}",
                    days[dst]
                );
            }
        }
    }

    #[test]
    fn triangle_inequality_holds_on_generated_world() {
        let world = test_world("first-light", 3);
        let normals = Normals::from_world(&world).unwrap();
        let model = Model::new(&world, &normals, None, Settings::default()).unwrap();
        let land: Vec<usize> = (0..model.region_count())
            .filter(|&i| model.passable_summer[i])
            .take(20)
            .collect();
        assert!(land.len() >= 10);
        for season in [Season::Summer, Season::Winter] {
            let fields: Vec<TravelField> = land
                .iter()
                .map(|&s| model.travel_time(season, s).unwrap())
                .collect();
            for (ia, fa) in fields.iter().enumerate() {
                for (ib, &b) in land.iter().enumerate() {
                    if !fa.days[b].is_finite() {
                        continue;
                    }
                    let fb = &fields[ib];
                    for &c in &land {
                        if fb.days[c].is_finite() {
                            assert!(
                                fa.days[c] <= fa.days[b] + fb.days[c] + 1e-9,
                                "triangle inequality violated ({season:?}): {} -> {} -> {}: {} > {} + {}",
                                land[ia],
                                b,
                                c,
                                fa.days[c],
                                fa.days[b],
                                fb.days[c]
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn impassable_water_blocks_travel_between_disconnected_landmasses() {
        let world = test_world("first-light", 4);
        let normals = Normals::from_world(&world).unwrap();
        let model = Model::new(&world, &normals, None, Settings::default()).unwrap();
        let n = model.region_count();

        // Find connected components of dry land in summer.
        let mut comp = vec![usize::MAX; n];
        let mut sizes = Vec::new();
        let mut reps = Vec::new();
        for start in 0..n {
            if !model.passable_summer[start] || comp[start] != usize::MAX {
                continue;
            }
            let id = sizes.len();
            let mut stack = vec![start];
            comp[start] = id;
            let mut count = 0;
            while let Some(u) = stack.pop() {
                count += 1;
                for k in world.surface.offsets[u] as usize..world.surface.offsets[u + 1] as usize {
                    let v = world.surface.neighbors[k] as usize;
                    if model.passable_summer[v] && comp[v] == usize::MAX {
                        comp[v] = id;
                        stack.push(v);
                    }
                }
            }
            sizes.push(count);
            reps.push(start);
        }
        assert!(
            reps.len() >= 2,
            "world should have at least two separated landmasses"
        );
        let field = model.travel_time(Season::Summer, reps[0]).unwrap();
        for i in 0..n {
            if world.water.body_ids[i] != 0 {
                assert!(
                    field.days[i].is_infinite(),
                    "water region {i} must be unreachable in summer"
                );
            } else if comp[i] == 0 {
                assert!(
                    field.days[i].is_finite(),
                    "same-component land {i} must be reachable"
                );
            } else {
                assert!(
                    field.days[i].is_infinite(),
                    "separate-component land {i} must be blocked by water"
                );
            }
        }
        // Main ocean must stay impassable in winter too.
        let winter = model.travel_time(Season::Winter, reps[0]).unwrap();
        for i in 0..n {
            if world.water.body_ids[i] == world.water.main_ocean_id {
                assert!(
                    winter.days[i].is_infinite(),
                    "ocean region {i} must stay impassable in winter"
                );
            }
        }
    }

    #[test]
    fn monotonic_slowdown_with_woody_cover_and_snow() {
        let s = Settings::default();
        // Pure factor monotonicity.
        for drag in [
            TEMPERATE_WOODY_DRAG,
            BOREAL_WOODY_DRAG,
            RAINFOREST_WOODY_DRAG,
        ] {
            let mut prev = f64::INFINITY;
            for step in 0..=10 {
                let f = woody_speed_factor(step as f64 / 10.0, drag);
                assert!(f < prev, "woody factor must strictly decrease");
                prev = f;
            }
        }
        let mut prev_snow = f64::INFINITY;
        for months in 0..=8u8 {
            let idx = winter_snow_index(months, f64::from(months) * 40.0, &s);
            let f = snow_speed_factor(idx, &s);
            assert!(
                f <= prev_snow && (months == 0 || f < prev_snow || idx == 1.0),
                "snow factor must decrease with freezing months and snowpack"
            );
            prev_snow = f;
        }
        // End-to-end world check: raising woody cover everywhere slows travel on wooded edges.
        let world = test_world("first-light", 3);
        let normals = Normals::from_world(&world).unwrap();
        let (_, mut sparse) = VegetationModel::new(&world, &normals, Default::default()).unwrap();
        let mut dense = sparse.clone();
        for i in 0..world.surface.centers.len() {
            if world.water.body_ids[i] == 0 && normals.warmest_month_celsius[i] >= 10.0 {
                sparse.woody[i] = 0.0;
                dense.woody[i] = 1.0;
            }
        }
        let m_sparse = Model::new(&world, &normals, Some(&sparse), s).unwrap();
        let m_dense = Model::new(&world, &normals, Some(&dense), s).unwrap();
        let mut checked = 0;
        for i in 0..world.surface.centers.len() {
            if m_sparse.passable_summer[i] && normals.warmest_month_celsius[i] >= 10.0 {
                assert!(
                    m_dense.hours_per_100km_summer[i] > m_sparse.hours_per_100km_summer[i],
                    "region {i}: dense woody cost {} must exceed sparse {}",
                    m_dense.hours_per_100km_summer[i],
                    m_sparse.hours_per_100km_summer[i]
                );
                checked += 1;
            }
        }
        assert!(checked > 20);
        // Winter snow slows every land region with freezing months and no frozen river offset.
        let mut snow_checked = 0;
        for i in 0..world.surface.centers.len() {
            if m_sparse.passable_summer[i]
                && m_sparse.freezing_months[i] > 0
                && m_sparse.river_delay_summer_hours[i] == 0.0
                && (world.surface.offsets[i]..world.surface.offsets[i + 1]).all(|k| {
                    let j = world.surface.neighbors[k as usize] as usize;
                    m_sparse.passable_summer[j] && m_sparse.river_delay_summer_hours[j] == 0.0
                })
            {
                assert!(
                    m_sparse.hours_per_100km_winter[i] > m_sparse.hours_per_100km_summer[i],
                    "region {i}: winter cost {} must exceed summer {}",
                    m_sparse.hours_per_100km_winter[i],
                    m_sparse.hours_per_100km_summer[i]
                );
                snow_checked += 1;
            }
        }
        assert!(snow_checked > 10);
    }

    #[test]
    fn deterministic_costs_fields_and_routes() {
        let world = test_world("5c151f46", 3);
        let normals = Normals::from_world(&world).unwrap();
        let a = Model::new(&world, &normals, None, Settings::default()).unwrap();
        let b = Model::new(&world, &normals, None, Settings::default()).unwrap();
        assert_eq!(a.hours_per_100km_summer, b.hours_per_100km_summer);
        assert_eq!(a.hours_per_100km_winter, b.hours_per_100km_winter);
        assert_eq!(a.edge_days_summer, b.edge_days_summer);
        assert_eq!(a.edge_days_winter, b.edge_days_winter);
        let origin = (0..a.region_count())
            .find(|&i| a.passable_summer[i])
            .unwrap();
        let fa = a.travel_time(Season::Summer, origin).unwrap();
        let fb = b.travel_time(Season::Summer, origin).unwrap();
        assert_eq!(fa, fb);
        let target = (0..a.region_count())
            .rev()
            .find(|&i| i != origin && fa.days[i].is_finite())
            .unwrap();
        let route = a.trace_route(&fa, target).unwrap();
        assert_eq!(route[0], origin);
        assert_eq!(*route.last().unwrap(), target);
        // Reconstructed route sum matches Dijkstra distance within floating-point tolerance.
        let mut sum_days = 0.0;
        let mut sum_dist = 0.0;
        for w in route.windows(2) {
            let (u, v) = (w[0], w[1]);
            let k = (a.offsets[u] as usize..a.offsets[u + 1] as usize)
                .find(|&k| a.neighbors[k] as usize == v)
                .expect("route step must be a graph edge");
            sum_days += a.edge_days_summer[k];
            sum_dist += a.distances[k];
        }
        assert!((sum_days - fa.days[target]).abs() < 1e-11);
        assert!((sum_dist - fa.distance_meters[target]).abs() < 1e-6);
    }

    #[test]
    fn frozen_lakes_rivers_and_multi_source_dijkstra() {
        let s = Settings::default();
        assert!(is_deep_winter_frozen(-15.0, 4, &s));
        assert!(!is_deep_winter_frozen(-5.0, 4, &s));
        assert!(!is_deep_winter_frozen(-15.0, 2, &s));
        assert_eq!(river_crossing_delay_from_area(5e10, 0.0, false, &s), 0.0);
        assert!((river_crossing_delay_from_area(1e12, 0.0, false, &s) - 8.0).abs() < 1e-12);
        assert_eq!(river_crossing_delay_from_area(1e12, 0.0, true, &s), 0.0);

        // Multi-source Dijkstra equals elementwise minimum of single-source runs.
        let world = test_world("first-light", 3);
        let normals = Normals::from_world(&world).unwrap();
        let model = Model::new(&world, &normals, None, s).unwrap();
        let land: Vec<usize> = (0..model.region_count())
            .filter(|&i| model.passable_summer[i])
            .collect();
        let (a, b) = (land[0], land[land.len() / 2]);
        let fa = model.travel_time(Season::Summer, a).unwrap();
        let fb = model.travel_time(Season::Summer, b).unwrap();
        let fab = model.travel_time_multi(Season::Summer, &[a, b]).unwrap();
        for i in 0..model.region_count() {
            assert_eq!(fab.days[i], fa.days[i].min(fb.days[i]));
        }
    }
}
