use planimulation_core::{
    Recipe, Surface, World,
    seasonal_temperature::{
        DAYS_PER_YEAR, Normals, Settings, daily_insolation_watts_per_square_meter,
        declination_radians,
    },
};
use std::f64::consts::PI;

fn world() -> World {
    let recipe: Recipe = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1, "modelVersion": "terrain-prep-1", "randomVersion": "fnv1a-utf8-mulberry32-1",
        "seed": "seasonal-test", "subdivision": 2, "radiusMeters": 6371000., "plateCount": 12,
        "maxPlateSpeedCmPerYear": 8., "continentalFraction": 0.38, "continentalScale": 1.,
        "reliefScale": 1., "boundaryWidthKm": 300., "detailAmplitudeMeters": 300.,
        "terrainPreparationPasses": 4, "water": { "mode": "coverage", "fraction": 0.71 }
    }))
    .unwrap();
    World::generate(recipe).unwrap()
}

#[test]
fn spherical_insolation_handles_equinox_solstices_and_polar_night() {
    let settings = Settings::default();
    let equinox = declination_radians(0, settings.axial_tilt_degrees);
    assert_eq!(equinox, 0.);
    assert!(
        (daily_insolation_watts_per_square_meter(0., equinox, 1360.) - 1360. / PI).abs() < 1e-12
    );
    assert_eq!(
        daily_insolation_watts_per_square_meter(1., equinox, 1360.),
        0.
    );
    let northern_summer = declination_radians(91, settings.axial_tilt_degrees);
    assert!(northern_summer > 0.);
    assert!(daily_insolation_watts_per_square_meter(1., northern_summer, 1360.) > 0.);
    assert_eq!(
        daily_insolation_watts_per_square_meter(-1., northern_summer, 1360.),
        0.
    );
    let northern_winter = declination_radians(274, settings.axial_tilt_degrees);
    assert!(northern_winter < 0.);
    assert_eq!(
        daily_insolation_watts_per_square_meter(1., northern_winter, 1360.),
        0.
    );
    assert!(daily_insolation_watts_per_square_meter(-1., northern_winter, 1360.) > 0.);
    assert_eq!(
        declination_radians(DAYS_PER_YEAR, settings.axial_tilt_degrees),
        0.
    );
}

#[test]
fn daily_global_insolation_converges_to_one_quarter_of_incident_flux() {
    for level in [2, 4, 6] {
        let surface = Surface::build(level, 6_371_000.);
        let declination = declination_radians(91, 23.44);
        let total_area: f64 = surface.areas.iter().sum();
        let global: f64 = surface
            .centers
            .iter()
            .zip(&surface.areas)
            .map(|(center, area)| {
                area * daily_insolation_watts_per_square_meter(center[1], declination, 1360.)
            })
            .sum::<f64>()
            / total_area;
        assert!((global - 340.).abs() < if level == 2 { 2. } else { 0.2 });
    }
}

#[test]
fn fixed_seasonal_normals_are_reproducible_and_reconstruct_the_annual_mean() {
    let world = world();
    let settings = Settings::default();
    let a = Normals::from_world(&world, settings).unwrap();
    assert_eq!(a, Normals::from_world(&world, settings).unwrap());
    assert_eq!(a.monthly_day_counts.iter().sum::<usize>(), DAYS_PER_YEAR);
    for region in 0..world.surface.centers.len() {
        let weighted = a
            .monthly_temperature_celsius
            .iter()
            .enumerate()
            .map(|(month, values)| values[region] * a.monthly_day_counts[month] as f64)
            .sum::<f64>()
            / DAYS_PER_YEAR as f64;
        assert!((weighted - a.annual_mean_celsius[region]).abs() < 1e-10);
        assert!(a.annual_minimum_celsius[region] <= a.annual_mean_celsius[region]);
        assert!(a.annual_mean_celsius[region] <= a.annual_maximum_celsius[region]);
    }
}

#[test]
fn seasonal_phase_inertia_and_lapse_response_have_controlled_effects() {
    let mut base = world();
    base.terrain.elevation.fill(0.);
    base.water.depth_meters.fill(0.);
    let settings = Settings::default();
    let land = Normals::from_world(&base, settings).unwrap();
    let northern = base
        .surface
        .centers
        .iter()
        .enumerate()
        .filter(|(_, center)| center[1] > 0.5 && center[1] < 0.9)
        .max_by(|(_, a), (_, b)| a[1].total_cmp(&b[1]))
        .unwrap()
        .0;
    let southern = base
        .surface
        .centers
        .iter()
        .enumerate()
        .filter(|(_, center)| center[1] < -0.5 && center[1] > -0.9)
        .min_by(|(_, a), (_, b)| a[1].total_cmp(&b[1]))
        .unwrap()
        .0;
    assert!(
        land.monthly_temperature_celsius[3][northern]
            > land.monthly_temperature_celsius[3][southern]
    );
    assert!(
        land.monthly_temperature_celsius[9][northern]
            < land.monthly_temperature_celsius[9][southern]
    );

    base.water.depth_meters.fill(1.);
    let water = Normals::from_world(&base, settings).unwrap();
    let land_range = land.annual_maximum_celsius[northern] - land.annual_minimum_celsius[northern];
    let water_range =
        water.annual_maximum_celsius[northern] - water.annual_minimum_celsius[northern];
    assert!(water_range < land_range);
    base.water.depth_meters.fill(0.);
    base.terrain.elevation.fill(1000.);
    let high = Normals::from_world(&base, settings).unwrap();
    assert!(
        (land.annual_mean_celsius[northern] - high.annual_mean_celsius[northern] - 6.5).abs()
            < 1e-10
    );

    base.terrain.elevation.fill(0.);
    let no_tilt = Normals::from_world(
        &base,
        Settings {
            axial_tilt_degrees: 0.,
            ..settings
        },
    )
    .unwrap();
    assert!(
        (no_tilt.annual_maximum_celsius[northern] - no_tilt.annual_minimum_celsius[northern]).abs()
            < 1e-10
    );
}

#[test]
fn invalid_settings_reject_before_building_normals() {
    let world = world();
    for settings in [
        Settings {
            axial_tilt_degrees: -1.,
            ..Settings::default()
        },
        Settings {
            land_response_days: 100.,
            water_response_days: 20.,
            ..Settings::default()
        },
        Settings {
            solar_irradiance_watts_per_square_meter: f64::NAN,
            ..Settings::default()
        },
    ] {
        assert!(Normals::from_world(&world, settings).is_err());
    }
}
