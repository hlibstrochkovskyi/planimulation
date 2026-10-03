use planimulation_core::{
    Recipe, World,
    seasonal_wind::{Normals, Settings, tangent_vector, velocity_at},
};
use std::f64::consts::PI;

fn world() -> World {
    let recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    World::generate(recipe).unwrap()
}

#[test]
fn idealized_belts_have_expected_zonal_and_meridional_signs() {
    let settings = Settings::default();
    let at = |degrees: f64| velocity_at(degrees.to_radians(), 0., settings);
    assert_eq!(at(0.), [0.; 2]);
    assert!(at(15.)[0] < 0. && at(15.)[1] < 0.);
    assert!(at(-15.)[0] < 0. && at(-15.)[1] > 0.);
    assert!(at(45.)[0] > 0. && at(45.)[1] > 0.);
    assert!(at(-45.)[0] > 0. && at(-45.)[1] < 0.);
    assert!(at(75.)[0] < 0. && at(75.)[1] < 0.);
    assert_eq!(at(30.)[0], 0.);
    assert_eq!(at(60.)[0], 0.);
    assert!(at(90.).iter().all(|speed| speed.abs() < 1e-12));
}

#[test]
fn convergence_belt_moves_with_declination_without_randomness() {
    let settings = Settings::default();
    let summer_declination = 23.44_f64.to_radians();
    let winter_declination = -summer_declination;
    let north_itcz = settings.itcz_shift_fraction * summer_declination;
    let south_itcz = settings.itcz_shift_fraction * winter_declination;
    assert!(
        velocity_at(north_itcz, summer_declination, settings)
            .iter()
            .all(|v| v.abs() < 1e-12)
    );
    assert!(
        velocity_at(south_itcz, winter_declination, settings)
            .iter()
            .all(|v| v.abs() < 1e-12)
    );
    assert!(velocity_at(north_itcz, winter_declination, settings)[0] < 0.);
    assert!(velocity_at(south_itcz, summer_declination, settings)[0] < 0.);
}

#[test]
fn local_components_convert_to_tangent_vectors_across_seam_and_poles() {
    for longitude in [-PI + 1e-10, PI - 1e-10, -1., 0., 1.] {
        let latitude: f64 = 0.65;
        let center = [
            latitude.cos() * longitude.cos(),
            latitude.sin(),
            latitude.cos() * longitude.sin(),
        ];
        let vector = tangent_vector(center, 4., -2.);
        let dot: f64 = center.iter().zip(vector).map(|(x, y)| x * y).sum();
        let norm = vector.iter().map(|value| value * value).sum::<f64>().sqrt();
        assert!(dot.abs() < 1e-12);
        assert!((norm - 20_f64.sqrt()).abs() < 1e-12);
    }
    assert_eq!(tangent_vector([0., 1., 0.], 4., 2.), [0.; 3]);
}

#[test]
fn monthly_normals_replay_and_share_calendar_with_temperature() {
    let world = world();
    let normals = Normals::from_world(&world, Settings::default(), 23.44).unwrap();
    assert_eq!(
        normals,
        Normals::from_world(&world, Settings::default(), 23.44).unwrap()
    );
    assert_eq!(normals.monthly_day_counts.iter().sum::<usize>(), 365);
    assert_eq!(normals.monthly_east_meters_per_second.len(), 12);
    assert_eq!(
        normals.monthly_north_meters_per_second[0].len(),
        world.surface.centers.len()
    );
    assert_eq!(normals.temperature_model_version, "seasonal-temperature-1");
    let no_tilt = Normals::from_world(&world, Settings::default(), 0.).unwrap();
    for month in 1..12 {
        for region in 0..world.surface.centers.len() {
            assert!(
                (no_tilt.monthly_east_meters_per_second[month][region]
                    - no_tilt.monthly_east_meters_per_second[0][region])
                    .abs()
                    < 1e-12
            );
            assert!(
                (no_tilt.monthly_north_meters_per_second[month][region]
                    - no_tilt.monthly_north_meters_per_second[0][region])
                    .abs()
                    < 1e-12
            );
        }
    }
}

#[test]
fn invalid_parameters_reject_before_building_wind() {
    let world = world();
    for bad in [
        Settings {
            itcz_shift_fraction: f64::NAN,
            ..Settings::default()
        },
        Settings {
            midlatitude_westerly_meters_per_second: -1.,
            ..Settings::default()
        },
    ] {
        assert!(Normals::from_world(&world, bad, 23.44).is_err());
    }
    assert!(Normals::from_world(&world, Settings::default(), 90.).is_err());
}
