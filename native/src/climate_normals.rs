//! Prescribed annual climate normals and a climate-class biome map for a generated,
//! static world. Temperature is a prescribed zonal sea-level profile with a lagged
//! seasonal insolation response and a lapse-rate correction. Precipitation is a
//! diagnostic estimate: a zonal
//! circulation profile scaled by moisture supply from upwind water and by
//! windward lift. It is not a conserved water budget, a weather model or the
//! seasonal-water experiment, and biomes classify climate, not vegetation state.
use crate::{
    World,
    mountain_belts::Key,
    seasonal_temperature::{
        self, DAYS_PER_YEAR, MONTHS_PER_YEAR, daily_insolation_watts_per_square_meter,
        declination_radians,
    },
    seasonal_wind,
};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const MODEL_VERSION: &str = "climate-normals-1";
/// Annual mean sea-level temperature, °C, by absolute latitude in degrees. Prescribing
/// it stands in for the atmospheric and oceanic heat transport that is not modeled.
pub const ZONAL_TEMPERATURE: [(f64, f64); 7] = [
    (0., 26.5),
    (15., 25.),
    (30., 19.),
    (45., 10.),
    (60., 0.),
    (75., -13.),
    (90., -25.),
];
/// Seasonal response to the daily insolation anomaly, °C per W/m².
pub const SEASONAL_SENSITIVITY: f64 = 0.075;
/// Water regions damp the seasonal anomaly (mixed-layer heat capacity).
pub const WATER_SEASONAL_FACTOR: f64 = 0.35;
/// Land seasonal damping where onshore air arrives straight from water; it fades
/// with the same upwind path length as moisture supply (maritime vs continental).
pub const MARITIME_SEASONAL_FACTOR: f64 = 0.5;
pub const LAND_RESPONSE_DAYS: f64 = 20.;
pub const WATER_RESPONSE_DAYS: f64 = 60.;
pub const LAPSE_RATE_CELSIUS_PER_METER: f64 = 0.0065;
/// Zonal precipitation potential with full moisture supply, mm/year, by absolute
/// latitude in degrees: wet equatorial convergence, dry subtropical highs, wet
/// midlatitude storm tracks and dry polar air.
pub const ZONAL_PRECIPITATION: [(f64, f64); 10] = [
    (0., 2400.),
    (10., 1800.),
    (20., 800.),
    (30., 550.),
    (40., 950.),
    (50., 1150.),
    (60., 900.),
    (70., 450.),
    (80., 250.),
    (90., 150.),
];
/// E-folding length of moisture supply along the cheapest upwind path from water.
pub const SUPPLY_LENGTH_METERS: f64 = 1_700_000.;
/// Moisture supply that remains far from any water (local recycling).
pub const MINIMUM_SUPPLY: f64 = 0.15;
/// Equivalent path length added per meter of ascent along the path (rain-out).
pub const RAINOUT_METERS_PER_METER: f64 = 500.;
/// Windward precipitation gain per meter of rise from the upwind neighbor.
pub const LIFT_PER_METER: f64 = 0.0015;
pub const MAXIMUM_LIFT: f64 = 2.5;
/// Wind speed at which transport becomes half directional.
pub const DIRECTIONAL_SPEED: f64 = 3.;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    Water = 0,
    Ice = 1,
    Tundra = 2,
    BorealForest = 3,
    TemperateForest = 4,
    TemperateGrassland = 5,
    ColdDesert = 6,
    HotDesert = 7,
    Savanna = 8,
    TropicalSeasonalForest = 9,
    TropicalRainforest = 10,
}

/// Köppen-style aridity threshold in mm/year for an annual mean temperature.
pub fn aridity_threshold(mean_celsius: f64) -> f64 {
    20. * mean_celsius + 140.
}

/// Climate class from annual mean, warmest and coldest monthly temperatures, annual
/// precipitation and driest-month precipitation (mm).
pub fn classify(mean: f64, warmest: f64, coldest: f64, annual: f64, driest: f64) -> Biome {
    let threshold = aridity_threshold(mean);
    if warmest < 0. {
        Biome::Ice
    } else if warmest < 10. {
        Biome::Tundra
    } else if annual < threshold {
        if mean >= 18. {
            Biome::HotDesert
        } else {
            Biome::ColdDesert
        }
    } else if annual < 2. * threshold {
        if mean >= 18. {
            Biome::Savanna
        } else {
            Biome::TemperateGrassland
        }
    } else if coldest >= 18. {
        if driest >= 60. {
            Biome::TropicalRainforest
        } else {
            Biome::TropicalSeasonalForest
        }
    } else if coldest > -3. || warmest >= 22. {
        Biome::TemperateForest
    } else {
        Biome::BorealForest
    }
}

/// Piecewise-linear lookup in a table keyed by absolute latitude in degrees.
pub fn zonal(table: &[(f64, f64)], latitude_degrees: f64) -> f64 {
    let x = latitude_degrees.abs().min(90.);
    let k = table
        .windows(2)
        .position(|w| x <= w[1].0)
        .unwrap_or(table.len() - 2);
    let ((x0, y0), (x1, y1)) = (table[k], table[k + 1]);
    y0 + (y1 - y0) * (x - x0) / (x1 - x0)
}

/// Monthly mean temperatures for one region from the zonal profile, a lagged
/// insolation anomaly scaled by `seasonal_factor` (water, maritime or continental)
/// and the lapse-rate correction.
pub fn monthly_temperature(
    sin_latitude: f64,
    seasonal_factor: f64,
    height_above_water_meters: f64,
    day_counts: &[usize; MONTHS_PER_YEAR],
    tilt_degrees: f64,
) -> [f64; MONTHS_PER_YEAR] {
    let latitude = sin_latitude.clamp(-1., 1.).asin().to_degrees();
    let base = zonal(&ZONAL_TEMPERATURE, latitude)
        - LAPSE_RATE_CELSIUS_PER_METER * height_above_water_meters.max(0.);
    let insolation: Vec<f64> = (0..DAYS_PER_YEAR)
        .map(|day| {
            daily_insolation_watts_per_square_meter(
                sin_latitude,
                declination_radians(day, tilt_degrees),
                1360.,
            )
        })
        .collect();
    let mean = insolation.iter().sum::<f64>() / DAYS_PER_YEAR as f64;
    let wet = seasonal_factor <= WATER_SEASONAL_FACTOR;
    let gain = SEASONAL_SENSITIVITY * seasonal_factor;
    let retained = (-1.
        / if wet {
            WATER_RESPONSE_DAYS
        } else {
            LAND_RESPONSE_DAYS
        })
    .exp();
    // Periodic steady state of the lagged anomaly, as in seasonal-temperature-1.
    let mut from_zero = 0.;
    for value in &insolation {
        from_zero = retained * from_zero + (1. - retained) * gain * (value - mean);
    }
    let mut anomaly = from_zero / (1. - retained.powi(DAYS_PER_YEAR as i32));
    let mut months = [0.; MONTHS_PER_YEAR];
    let mut day = 0;
    for (month, &count) in day_counts.iter().enumerate() {
        for _ in 0..count {
            anomaly = retained * anomaly + (1. - retained) * gain * (insolation[day] - mean);
            months[month] += (base + anomaly) / count as f64;
            day += 1;
        }
    }
    months
}

#[derive(Clone, Debug, PartialEq)]
pub struct Normals {
    pub model_version: &'static str,
    pub monthly_temperature_celsius: Vec<Vec<f64>>,
    pub annual_mean_celsius: Vec<f64>,
    pub warmest_month_celsius: Vec<f64>,
    pub coldest_month_celsius: Vec<f64>,
    pub monthly_precipitation_mm: Vec<Vec<f64>>,
    pub annual_precipitation_mm: Vec<f64>,
    pub driest_month_mm: Vec<f64>,
    /// Annual zonal potential before supply and lift, mm/year.
    pub zonal_precipitation_mm: Vec<f64>,
    /// Precipitation-weighted annual mean moisture supply, 0–1.
    pub moisture_supply: Vec<f64>,
    /// Precipitation-weighted annual mean windward lift factor, ≥ 1.
    pub lift: Vec<f64>,
    pub biome: Vec<u8>,
}

impl Normals {
    pub fn from_world(world: &World) -> Result<Self, String> {
        let s = &world.surface;
        let n = s.centers.len();
        if world.water.body_ids.len() != n || world.terrain.elevation.len() != n {
            return Err("Climate-normal input arrays do not match the surface.".into());
        }
        let settings = seasonal_temperature::Settings::default();
        let mut day_counts = [0; MONTHS_PER_YEAR];
        for day in 0..DAYS_PER_YEAR {
            day_counts[day * MONTHS_PER_YEAR / DAYS_PER_YEAR] += 1;
        }
        let wind_settings = seasonal_wind::Settings::default();
        let wind =
            seasonal_wind::Normals::from_world(world, wind_settings, settings.axial_tilt_degrees)?;
        let wet = |i: usize| world.water.body_ids[i] != 0;
        let level = world.water.level_meters;
        let height: Vec<f64> = (0..n)
            .map(|i| {
                if wet(i) {
                    level
                } else {
                    world.terrain.elevation[i].max(level)
                }
            })
            .collect();
        let mut monthly = vec![vec![0.; n]; MONTHS_PER_YEAR];
        let (mut potential_sum, mut supply_sum, mut lift_sum) =
            (vec![0.; n], vec![0.; n], vec![0.; n]);
        let mut maritime = vec![0.; n];
        let mut middle_day = 0;
        for month in 0..MONTHS_PER_YEAR {
            let days = day_counts[month];
            let declination =
                declination_radians(middle_day + days / 2, settings.axial_tilt_degrees);
            middle_day += days;
            let east = &wind.monthly_east_meters_per_second[month];
            let north = &wind.monthly_north_meters_per_second[month];
            let velocity =
                |i: usize| seasonal_wind::tangent_vector(s.centers[i], east[i], north[i]);
            // Cheapest upwind path from water; moving against the wind and climbing cost more.
            let mut distance = vec![f64::INFINITY; n];
            let mut queue = BinaryHeap::new();
            for i in (0..n).filter(|&i| wet(i)) {
                distance[i] = 0.;
                queue.push(Reverse(Key(0., i)));
            }
            while let Some(Reverse(Key(d, j))) = queue.pop() {
                if d > distance[j] {
                    continue;
                }
                for k in s.offsets[j] as usize..s.offsets[j + 1] as usize {
                    let i = s.neighbors[k] as usize;
                    let cost = d
                        + edge_cost(s.centers[j], s.centers[i], velocity(j), s.distances[k])
                        + (height[i] - height[j]).max(0.) * RAINOUT_METERS_PER_METER;
                    if cost < distance[i] {
                        distance[i] = cost;
                        queue.push(Reverse(Key(cost, i)));
                    }
                }
            }
            for i in 0..n {
                let latitude = s.centers[i][1].clamp(-1., 1.).asin();
                let shifted = latitude - wind_settings.itcz_shift_fraction * declination;
                let potential = zonal(&ZONAL_PRECIPITATION, shifted.to_degrees()) * days as f64
                    / DAYS_PER_YEAR as f64;
                maritime[i] += (-distance[i] / SUPPLY_LENGTH_METERS).exp() / MONTHS_PER_YEAR as f64;
                let (supply, lift) = if wet(i) {
                    (1., 1.)
                } else {
                    let supply = if distance[i].is_finite() {
                        MINIMUM_SUPPLY
                            + (1. - MINIMUM_SUPPLY) * (-distance[i] / SUPPLY_LENGTH_METERS).exp()
                    } else {
                        MINIMUM_SUPPLY
                    };
                    (supply, windward_lift(world, &height, i, velocity(i)))
                };
                monthly[month][i] = potential * supply * lift;
                potential_sum[i] += potential;
                supply_sum[i] += potential * supply;
                lift_sum[i] += potential * supply * lift;
            }
        }
        let annual: Vec<f64> = (0..n).map(|i| monthly.iter().map(|m| m[i]).sum()).collect();
        let driest: Vec<f64> = (0..n)
            .map(|i| monthly.iter().map(|m| m[i]).fold(f64::INFINITY, f64::min))
            .collect();
        let mut temperature = vec![vec![0.; n]; MONTHS_PER_YEAR];
        for i in 0..n {
            let months = monthly_temperature(
                s.centers[i][1],
                if wet(i) {
                    WATER_SEASONAL_FACTOR
                } else {
                    1. - (1. - MARITIME_SEASONAL_FACTOR) * maritime[i]
                },
                height[i] - level,
                &day_counts,
                settings.axial_tilt_degrees,
            );
            for (month, value) in months.into_iter().enumerate() {
                temperature[month][i] = value;
            }
        }
        let annual_mean: Vec<f64> = (0..n)
            .map(|i| {
                (0..MONTHS_PER_YEAR)
                    .map(|m| temperature[m][i] * day_counts[m] as f64)
                    .sum::<f64>()
                    / DAYS_PER_YEAR as f64
            })
            .collect();
        let warmest: Vec<f64> = (0..n)
            .map(|i| {
                temperature
                    .iter()
                    .map(|m| m[i])
                    .fold(f64::NEG_INFINITY, f64::max)
            })
            .collect();
        let coldest: Vec<f64> = (0..n)
            .map(|i| {
                temperature
                    .iter()
                    .map(|m| m[i])
                    .fold(f64::INFINITY, f64::min)
            })
            .collect();
        let biome = (0..n)
            .map(|i| {
                if wet(i) {
                    Biome::Water as u8
                } else {
                    classify(annual_mean[i], warmest[i], coldest[i], annual[i], driest[i]) as u8
                }
            })
            .collect();
        Ok(Self {
            model_version: MODEL_VERSION,
            moisture_supply: (0..n)
                .map(|i| supply_sum[i] / potential_sum[i].max(1e-12))
                .collect(),
            lift: (0..n)
                .map(|i| lift_sum[i] / supply_sum[i].max(1e-12))
                .collect(),
            zonal_precipitation_mm: potential_sum,
            warmest_month_celsius: warmest,
            coldest_month_celsius: coldest,
            monthly_precipitation_mm: monthly,
            annual_precipitation_mm: annual,
            driest_month_mm: driest,
            monthly_temperature_celsius: temperature,
            annual_mean_celsius: annual_mean,
            biome,
        })
    }
}

/// Path cost of carrying moisture from `from` to `to`: the edge length in calm air or
/// with the wind, up to three times longer against a strong wind.
fn edge_cost(from: [f64; 3], to: [f64; 3], wind: [f64; 3], length: f64) -> f64 {
    let step = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let (speed, step_length) = (norm(wind), norm(step));
    if speed < 1e-12 || step_length < 1e-15 {
        return length;
    }
    let alignment =
        (wind[0] * step[0] + wind[1] * step[1] + wind[2] * step[2]) / (speed * step_length);
    length * (1. + (1. - alignment) * speed / (speed + DIRECTIONAL_SPEED))
}

/// Precipitation gain from rising above the most upwind neighbor.
fn windward_lift(world: &World, height: &[f64], i: usize, wind: [f64; 3]) -> f64 {
    let s = &world.surface;
    let speed = norm(wind);
    if speed < 1e-12 {
        return 1.;
    }
    let c = s.centers[i];
    let mut best = (0., i);
    for k in s.offsets[i] as usize..s.offsets[i + 1] as usize {
        let j = s.neighbors[k] as usize;
        let q = s.centers[j];
        let toward = [c[0] - q[0], c[1] - q[1], c[2] - q[2]];
        let alignment = (wind[0] * toward[0] + wind[1] * toward[1] + wind[2] * toward[2])
            / (speed * norm(toward).max(1e-15));
        if alignment > best.0 {
            best = (alignment, j);
        }
    }
    let rise = (height[i] - height[best.1]).max(0.) * best.0 * speed / (speed + DIRECTIONAL_SPEED);
    (1. + LIFT_PER_METER * rise).min(MAXIMUM_LIFT)
}

fn norm(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Recipe, Surface};

    #[test]
    fn classes_follow_the_documented_rules() {
        assert_eq!(classify(-20., -2., -40., 300., 10.), Biome::Ice);
        assert_eq!(classify(-8., 8., -30., 300., 10.), Biome::Tundra);
        assert_eq!(classify(25., 30., 20., 300., 0.), Biome::HotDesert);
        assert_eq!(classify(8., 22., -8., 150., 5.), Biome::ColdDesert);
        assert_eq!(classify(25., 30., 20., 800., 0.), Biome::Savanna);
        assert_eq!(classify(10., 20., 0., 400., 10.), Biome::TemperateGrassland);
        assert_eq!(
            classify(26., 28., 24., 2500., 80.),
            Biome::TropicalRainforest
        );
        assert_eq!(
            classify(26., 28., 24., 1500., 10.),
            Biome::TropicalSeasonalForest
        );
        assert_eq!(classify(12., 20., 2., 900., 50.), Biome::TemperateForest);
        assert_eq!(classify(0., 16., -20., 500., 20.), Biome::BorealForest);
        assert_eq!(aridity_threshold(10.), 340.);
    }

    #[test]
    fn zonal_tables_interpolate_symmetrically() {
        assert_eq!(zonal(&ZONAL_PRECIPITATION, 0.), 2400.);
        assert_eq!(zonal(&ZONAL_PRECIPITATION, -90.), 150.);
        assert_eq!(zonal(&ZONAL_PRECIPITATION, 25.), 675.);
        assert_eq!(
            zonal(&ZONAL_TEMPERATURE, 45.),
            zonal(&ZONAL_TEMPERATURE, -45.)
        );
    }

    #[test]
    fn moisture_paths_are_cheapest_with_the_wind() {
        let (a, b) = ([1., 0., 0.], [0.999, 0.0447, 0.]);
        let toward = [0., 10., 0.];
        let with = edge_cost(a, b, toward, 100.);
        let calm = edge_cost(a, b, [0.; 3], 100.);
        let across = edge_cost(a, b, [0., 0., 10.], 100.);
        let against = edge_cost(a, b, [0., -10., 0.], 100.);
        assert!((with - 100.).abs() < 0.1 && calm == 100.);
        assert!(with < across && across < against && against < 300.);
    }

    #[test]
    fn normals_are_bounded_deterministic_and_classify_only_dry_land() {
        let recipe: Recipe = serde_json::from_value(serde_json::json!({"schemaVersion":1,
            "modelVersion":"multiscale-geography-4","randomVersion":"fnv1a-utf8-mulberry32-1","seed":"climate",
            "subdivision":4,"radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,
            "continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,
            "detailAmplitudeMeters":300.,"terrainPreparationPasses":4,"water":{"mode":"coverage","fraction":0.71}}))
            .unwrap();
        let world = World::generate(recipe).unwrap();
        let normals = Normals::from_world(&world).unwrap();
        assert_eq!(normals, Normals::from_world(&world).unwrap());
        let n = world.surface.centers.len();
        assert_eq!(n, Surface::build(4, 1.).centers.len());
        let mut classes = std::collections::BTreeSet::new();
        for i in 0..n {
            let monthly: f64 = normals.monthly_precipitation_mm.iter().map(|m| m[i]).sum();
            assert!((monthly - normals.annual_precipitation_mm[i]).abs() < 1e-9);
            assert!(normals.driest_month_mm[i] >= 0. && normals.driest_month_mm[i] <= monthly);
            assert!(normals.coldest_month_celsius[i] <= normals.annual_mean_celsius[i] + 1e-9);
            assert!(normals.annual_mean_celsius[i] <= normals.warmest_month_celsius[i] + 1e-9);
            assert!(
                normals.moisture_supply[i] >= MINIMUM_SUPPLY
                    && normals.moisture_supply[i] <= 1. + 1e-12
            );
            assert!(normals.lift[i] >= 1. && normals.lift[i] <= MAXIMUM_LIFT);
            let wet = world.water.body_ids[i] != 0;
            assert_eq!(normals.biome[i] == Biome::Water as u8, wet);
            if wet {
                assert_eq!(normals.moisture_supply[i], 1.);
            } else {
                classes.insert(normals.biome[i]);
            }
        }
        assert!(
            classes.len() >= 5,
            "An Earth-like world has several climate classes."
        );
    }
}
