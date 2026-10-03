//! Deterministic seasonal temperature normals on a generated, static world.
//! This is a prescribed response model, not a conserved energy budget.
use crate::World;
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

pub const MODEL_VERSION: &str = "seasonal-temperature-1";
pub const DAYS_PER_YEAR: usize = 365;
pub const MONTHS_PER_YEAR: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub axial_tilt_degrees: f64,
    pub solar_irradiance_watts_per_square_meter: f64,
    pub reference_temperature_celsius: f64,
    pub sensitivity_celsius_per_watt_per_square_meter: f64,
    pub lapse_rate_celsius_per_meter: f64,
    pub land_response_days: f64,
    pub water_response_days: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            axial_tilt_degrees: 23.44,
            solar_irradiance_watts_per_square_meter: 1360.,
            reference_temperature_celsius: 15.,
            sensitivity_celsius_per_watt_per_square_meter: 0.09,
            lapse_rate_celsius_per_meter: 0.0065,
            land_response_days: 20.,
            water_response_days: 60.,
        }
    }
}

impl Settings {
    pub fn validate(self) -> Result<(), String> {
        let ranges = [
            (self.axial_tilt_degrees, 0., 45.),
            (self.solar_irradiance_watts_per_square_meter, 800., 1800.),
            (self.reference_temperature_celsius, -20., 40.),
            (self.sensitivity_celsius_per_watt_per_square_meter, 0., 0.2),
            (self.lapse_rate_celsius_per_meter, 0., 0.01),
            (self.land_response_days, 1., 120.),
            (self.water_response_days, 1., 365.),
        ];
        if ranges
            .iter()
            .any(|&(value, low, high)| !value.is_finite() || value < low || value > high)
            || self.water_response_days < self.land_response_days
        {
            return Err("Invalid seasonal-temperature settings.".into());
        }
        Ok(())
    }
}

/// Day zero is a northern spring equinox in a circular 365-day orbit.
pub fn declination_radians(day: usize, axial_tilt_degrees: f64) -> f64 {
    let phase = (day % DAYS_PER_YEAR) as f64 / DAYS_PER_YEAR as f64;
    (axial_tilt_degrees.to_radians().sin() * (TAU * phase).sin()).asin()
}

/// Top-of-atmosphere daily-mean flux for a horizontal patch; no refraction or atmosphere.
/// `sin_latitude` is used directly to avoid a longitude seam and pole singularities.
pub fn daily_insolation_watts_per_square_meter(
    sin_latitude: f64,
    declination_radians: f64,
    solar_irradiance: f64,
) -> f64 {
    let cos_latitude = (1. - sin_latitude * sin_latitude).max(0.).sqrt();
    let a = cos_latitude * declination_radians.cos();
    let b = sin_latitude * declination_radians.sin();
    let half_day = if a <= 1e-14 {
        if b > 0. { PI } else { 0. }
    } else {
        (-b / a).clamp(-1., 1.).acos()
    };
    (solar_irradiance / PI * (half_day * b + a * half_day.sin())).max(0.)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Normals {
    pub model_version: &'static str,
    pub settings: Settings,
    pub monthly_temperature_celsius: Vec<Vec<f64>>,
    pub annual_mean_celsius: Vec<f64>,
    pub annual_minimum_celsius: Vec<f64>,
    pub annual_maximum_celsius: Vec<f64>,
    pub monthly_day_counts: [usize; MONTHS_PER_YEAR],
}

impl Normals {
    pub fn from_world(world: &World, settings: Settings) -> Result<Self, String> {
        settings.validate()?;
        let n = world.surface.centers.len();
        if world.terrain.elevation.len() != n || world.water.depth_meters.len() != n {
            return Err("Seasonal-temperature input arrays do not match the surface.".into());
        }
        let declinations: Vec<f64> = (0..DAYS_PER_YEAR)
            .map(|day| declination_radians(day, settings.axial_tilt_degrees))
            .collect();
        let mut monthly_temperature_celsius = vec![vec![0.; n]; MONTHS_PER_YEAR];
        let mut annual_mean_celsius = vec![0.; n];
        let mut annual_minimum_celsius = vec![f64::INFINITY; n];
        let mut annual_maximum_celsius = vec![f64::NEG_INFINITY; n];
        let mut monthly_day_counts = [0; MONTHS_PER_YEAR];
        for day in 0..DAYS_PER_YEAR {
            monthly_day_counts[day * MONTHS_PER_YEAR / DAYS_PER_YEAR] += 1;
        }
        for region in 0..n {
            let sin_latitude = world.surface.centers[region][1];
            let wet = world.water.depth_meters[region] > 0.;
            let elevation_penalty = if wet {
                0.
            } else {
                settings.lapse_rate_celsius_per_meter * world.terrain.elevation[region].max(0.)
            };
            let response_days = if wet {
                settings.water_response_days
            } else {
                settings.land_response_days
            };
            let retained = (-1. / response_days).exp();
            let response = 1. - retained;
            let target = |day: usize| {
                let insolation = daily_insolation_watts_per_square_meter(
                    sin_latitude,
                    declinations[day],
                    settings.solar_irradiance_watts_per_square_meter,
                );
                settings.reference_temperature_celsius
                    + settings.sensitivity_celsius_per_watt_per_square_meter
                        * (insolation - settings.solar_irradiance_watts_per_square_meter / 4.)
                    - elevation_penalty
            };
            // The daily update is affine. Solve its annual fixed point exactly,
            // rather than choosing an arbitrary spin-up year count.
            let mut from_zero = 0.;
            for day in 0..DAYS_PER_YEAR {
                from_zero = retained * from_zero + response * target(day);
            }
            let initial = from_zero / (1. - retained.powi(DAYS_PER_YEAR as i32));
            let mut temperature = initial;
            for day in 0..DAYS_PER_YEAR {
                temperature = retained * temperature + response * target(day);
                let month = day * MONTHS_PER_YEAR / DAYS_PER_YEAR;
                monthly_temperature_celsius[month][region] += temperature;
                annual_mean_celsius[region] += temperature;
                annual_minimum_celsius[region] = annual_minimum_celsius[region].min(temperature);
                annual_maximum_celsius[region] = annual_maximum_celsius[region].max(temperature);
            }
            annual_mean_celsius[region] /= DAYS_PER_YEAR as f64;
        }
        for (month, values) in monthly_temperature_celsius.iter_mut().enumerate() {
            for temperature in values {
                *temperature /= monthly_day_counts[month] as f64;
            }
        }
        Ok(Self {
            model_version: MODEL_VERSION,
            settings,
            monthly_temperature_celsius,
            annual_mean_celsius,
            annual_minimum_celsius,
            annual_maximum_celsius,
            monthly_day_counts,
        })
    }
}
