//! Reproducible area-weighted summaries of prescribed monthly surface winds.
use planimulation_core::{
    Recipe, World,
    seasonal_wind::{Normals, Settings},
};
use serde_json::{Value, json};
use std::io::Write;

fn report(recipe: Recipe) -> Result<Value, String> {
    let world = World::generate(recipe)?;
    let tilt = planimulation_core::seasonal_temperature::Settings::default().axial_tilt_degrees;
    let wind = Normals::from_world(&world, Settings::default(), tilt)?;
    let total_area: f64 = world.surface.areas.iter().sum();
    let mut monthly_mean_speed = Vec::with_capacity(12);
    let mut monthly_mean_east = Vec::with_capacity(12);
    let mut monthly_mean_north = Vec::with_capacity(12);
    for month in 0..12 {
        let east = &wind.monthly_east_meters_per_second[month];
        let north = &wind.monthly_north_meters_per_second[month];
        let weighted = |values: &[f64]| {
            values
                .iter()
                .zip(&world.surface.areas)
                .map(|(value, area)| value * area)
                .sum::<f64>()
                / total_area
        };
        monthly_mean_east.push(weighted(east));
        monthly_mean_north.push(weighted(north));
        monthly_mean_speed.push(
            east.iter()
                .zip(north)
                .zip(&world.surface.areas)
                .map(|((&u, &v), area)| u.hypot(v) * area)
                .sum::<f64>()
                / total_area,
        );
    }
    Ok(json!({
        "reportVersion": 1,
        "recipe": world.recipe,
        "windModelVersion": wind.model_version,
        "temperatureModelVersion": wind.temperature_model_version,
        "axialTiltDegrees": wind.axial_tilt_degrees,
        "settings": wind.settings,
        "monthlyDayCounts": wind.monthly_day_counts,
        "regionCount": world.surface.centers.len(),
        "monthlyAreaWeightedMeanSpeedMetersPerSecond": monthly_mean_speed,
        "monthlyAreaWeightedMeanEastMetersPerSecond": monthly_mean_east,
        "monthlyAreaWeightedMeanNorthMetersPerSecond": monthly_mean_north,
        "scope": "Prescribed zonally symmetric surface-wind normals; no atmospheric mass, momentum, terrain deflection, or moisture transport"
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Usage: seasonal_wind_report RECIPE.json")?;
    let recipe: Recipe = serde_json::from_slice(&std::fs::read(path)?)?;
    let output = report(recipe)?;
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &output)?;
    writeln!(out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_replays_and_has_positive_area_weighted_speed() {
        let recipe: Recipe = serde_json::from_str(include_str!(
            "../../docs/scenarios/seasonal-temperature.json"
        ))
        .unwrap();
        let first = report(recipe.clone()).unwrap();
        assert_eq!(first, report(recipe).unwrap());
        assert_eq!(first["windModelVersion"], "seasonal-wind-1");
        assert!(
            first["monthlyAreaWeightedMeanSpeedMetersPerSecond"]
                .as_array()
                .unwrap()
                .iter()
                .all(|value| value.as_f64().unwrap() > 0.)
        );
    }
}
