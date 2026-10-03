//! Area-weighted headless checks for the static seasonal-temperature model.
use planimulation_core::{
    Recipe, World,
    seasonal_temperature::{Normals, Settings},
};
use serde_json::{Value, json};
use std::io::Write;

fn mean(values: &[f64], areas: &[f64], selected: impl Fn(usize) -> bool) -> Option<f64> {
    let mut weighted = 0.;
    let mut area = 0.;
    for (region, &value) in values.iter().enumerate() {
        if selected(region) {
            weighted += value * areas[region];
            area += areas[region];
        }
    }
    (area > 0.).then_some(weighted / area)
}

fn report(recipe: Recipe) -> Result<Value, String> {
    let world = World::generate(recipe)?;
    let normals = Normals::from_world(&world, Settings::default())?;
    let centers = &world.surface.centers;
    let areas = &world.surface.areas;
    let monthly_global: Vec<_> = normals
        .monthly_temperature_celsius
        .iter()
        .map(|month| mean(month, areas, |_| true))
        .collect();
    let monthly_north: Vec<_> = normals
        .monthly_temperature_celsius
        .iter()
        .map(|month| mean(month, areas, |id| centers[id][1] >= 0.))
        .collect();
    let monthly_south: Vec<_> = normals
        .monthly_temperature_celsius
        .iter()
        .map(|month| mean(month, areas, |id| centers[id][1] < 0.))
        .collect();
    let annual_land = mean(&normals.annual_mean_celsius, areas, |id| {
        world.water.depth_meters[id] == 0.
    });
    let annual_water = mean(&normals.annual_mean_celsius, areas, |id| {
        world.water.depth_meters[id] > 0.
    });
    Ok(json!({
        "reportVersion": 1,
        "recipe": world.recipe,
        "temperatureModelVersion": normals.model_version,
        "settings": normals.settings,
        "daysPerYear": 365,
        "monthlyDayCounts": normals.monthly_day_counts,
        "regionCount": centers.len(),
        "monthlyGlobalMeanCelsius": monthly_global,
        "monthlyNorthMeanCelsius": monthly_north,
        "monthlySouthMeanCelsius": monthly_south,
        "annualLandMeanCelsius": annual_land,
        "annualWaterMeanCelsius": annual_water,
        "scope": "Periodic temperature normals on fixed initial geography; no weather, energy conservation, moisture, or timed water dynamics"
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Usage: seasonal_temperature_report RECIPE.json")?;
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
    fn report_repeats_and_has_opposite_hemispheric_seasons() {
        let recipe: Recipe = serde_json::from_str(include_str!(
            "../../docs/scenarios/seasonal-temperature.json"
        ))
        .unwrap();
        let first = report(recipe.clone()).unwrap();
        assert_eq!(first, report(recipe).unwrap());
        assert_eq!(first["temperatureModelVersion"], "seasonal-temperature-1");
        let north = first["monthlyNorthMeanCelsius"].as_array().unwrap();
        let south = first["monthlySouthMeanCelsius"].as_array().unwrap();
        assert!(north[3].as_f64().unwrap() > north[9].as_f64().unwrap());
        assert!(south[3].as_f64().unwrap() < south[9].as_f64().unwrap());
    }
}
