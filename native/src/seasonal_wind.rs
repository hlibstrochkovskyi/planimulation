//! Prescribed, smooth surface-wind belts on a sphere; no momentum or air-mass solver.
use crate::{
    World,
    seasonal_temperature::{DAYS_PER_YEAR, MONTHS_PER_YEAR, declination_radians},
};
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "seasonal-wind-1";

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub itcz_shift_fraction: f64,
    pub trade_easterly_meters_per_second: f64,
    pub midlatitude_westerly_meters_per_second: f64,
    pub polar_easterly_meters_per_second: f64,
    pub tropical_convergence_meters_per_second: f64,
    pub midlatitude_poleward_meters_per_second: f64,
    pub polar_equatorward_meters_per_second: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            itcz_shift_fraction: 0.5,
            trade_easterly_meters_per_second: 7.,
            midlatitude_westerly_meters_per_second: 10.,
            polar_easterly_meters_per_second: 5.,
            tropical_convergence_meters_per_second: 2.,
            midlatitude_poleward_meters_per_second: 1.,
            polar_equatorward_meters_per_second: 0.5,
        }
    }
}

impl Settings {
    pub fn validate(self) -> Result<(), String> {
        if !self.itcz_shift_fraction.is_finite()
            || !(0. ..=1.).contains(&self.itcz_shift_fraction)
            || [
                self.trade_easterly_meters_per_second,
                self.midlatitude_westerly_meters_per_second,
                self.polar_easterly_meters_per_second,
                self.tropical_convergence_meters_per_second,
                self.midlatitude_poleward_meters_per_second,
                self.polar_equatorward_meters_per_second,
            ]
            .iter()
            .any(|&speed| !speed.is_finite() || !(0. ..=40.).contains(&speed))
        {
            return Err("Invalid seasonal-wind settings.".into());
        }
        Ok(())
    }
}

fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

fn belt_value(distance_degrees: f64, peaks: [f64; 3]) -> f64 {
    let distance = distance_degrees.clamp(0., 90.);
    let segment = (distance / 15.).floor().min(5.) as usize;
    let knots = [0., peaks[0], 0., peaks[1], 0., peaks[2], 0.];
    let fraction = smoothstep((distance - segment as f64 * 15.) / 15.);
    knots[segment] + (knots[segment + 1] - knots[segment]) * fraction
}

/// East and north velocity components in m/s at a latitude and subsolar declination.
/// Longitude, terrain, and water do not modify this first zonally symmetric field.
pub fn velocity_at(
    latitude_radians: f64,
    declination_radians: f64,
    settings: Settings,
) -> [f64; 2] {
    let shifted = latitude_radians - settings.itcz_shift_fraction * declination_radians;
    let distance = shifted.abs().to_degrees();
    let taper = latitude_radians.cos().max(0.);
    let east = belt_value(
        distance,
        [
            -settings.trade_easterly_meters_per_second,
            settings.midlatitude_westerly_meters_per_second,
            -settings.polar_easterly_meters_per_second,
        ],
    ) * taper;
    let north = shifted.signum()
        * belt_value(
            distance,
            [
                -settings.tropical_convergence_meters_per_second,
                settings.midlatitude_poleward_meters_per_second,
                -settings.polar_equatorward_meters_per_second,
            ],
        )
        * taper;
    [east, north]
}

/// Convert local east/north components into a 3D tangent vector for later edge fluxes.
pub fn tangent_vector(center: [f64; 3], east: f64, north: f64) -> [f64; 3] {
    let rho = center[0].hypot(center[2]);
    if rho <= 1e-12 {
        return [0.; 3];
    }
    let east_basis = [-center[2] / rho, 0., center[0] / rho];
    let north_basis = [
        -center[0] * center[1] / rho,
        rho,
        -center[2] * center[1] / rho,
    ];
    [0, 1, 2].map(|axis| east * east_basis[axis] + north * north_basis[axis])
}

#[derive(Clone, Debug, PartialEq)]
pub struct Normals {
    pub model_version: &'static str,
    pub settings: Settings,
    pub temperature_model_version: &'static str,
    pub axial_tilt_degrees: f64,
    pub monthly_day_counts: [usize; MONTHS_PER_YEAR],
    pub monthly_east_meters_per_second: Vec<Vec<f64>>,
    pub monthly_north_meters_per_second: Vec<Vec<f64>>,
}

impl Normals {
    pub fn from_world(
        world: &World,
        settings: Settings,
        axial_tilt_degrees: f64,
    ) -> Result<Self, String> {
        settings.validate()?;
        if !axial_tilt_degrees.is_finite() || !(0. ..=45.).contains(&axial_tilt_degrees) {
            return Err("Invalid axial tilt for seasonal wind.".into());
        }
        let n = world.surface.centers.len();
        let latitudes: Vec<f64> = world
            .surface
            .centers
            .iter()
            .map(|center| center[1].clamp(-1., 1.).asin())
            .collect();
        let mut east = vec![vec![0.; n]; MONTHS_PER_YEAR];
        let mut north = vec![vec![0.; n]; MONTHS_PER_YEAR];
        let mut day_counts = [0; MONTHS_PER_YEAR];
        for day in 0..DAYS_PER_YEAR {
            let month = day * MONTHS_PER_YEAR / DAYS_PER_YEAR;
            day_counts[month] += 1;
            let declination = declination_radians(day, axial_tilt_degrees);
            for (region, &latitude) in latitudes.iter().enumerate() {
                let velocity = velocity_at(latitude, declination, settings);
                east[month][region] += velocity[0];
                north[month][region] += velocity[1];
            }
        }
        for month in 0..MONTHS_PER_YEAR {
            for region in 0..n {
                east[month][region] /= day_counts[month] as f64;
                north[month][region] /= day_counts[month] as f64;
            }
        }
        Ok(Self {
            model_version: MODEL_VERSION,
            settings,
            temperature_model_version: crate::seasonal_temperature::MODEL_VERSION,
            axial_tilt_degrees,
            monthly_day_counts: day_counts,
            monthly_east_meters_per_second: east,
            monthly_north_meters_per_second: north,
        })
    }
}
