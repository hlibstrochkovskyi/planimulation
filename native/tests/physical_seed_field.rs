use planimulation_core::{
    Surface,
    physical_seed_field::{Field, Settings},
};
fn settings() -> Settings {
    Settings {
        radius_meters: 6371000.,
        amplitude_meters: 1.,
        length_scale: 1.,
    }
}
#[test]
fn bounded_values_repeat_and_query_order_does_not_consume_random_state() {
    let a = Field::new("first-light", settings()).unwrap();
    let b = Field::new("first-light", settings()).unwrap();
    assert_eq!(
        serde_json::to_vec(a.identity()).unwrap(),
        serde_json::to_vec(b.identity()).unwrap()
    );
    let points = Surface::build(3, 1.).centers;
    let values: Vec<_> = points.iter().map(|&p| a.sample(p).unwrap()).collect();
    assert!(values.iter().all(|v| v.abs() <= 1. + 1e-12));
    assert!(values.iter().any(|v| v.abs() > 0.1));
    for (i, &p) in points.iter().enumerate().rev() {
        assert_eq!(a.sample(p).unwrap(), values[i]);
        assert_eq!(b.sample(p).unwrap(), values[i]);
    }
}
#[test]
fn equal_radius_to_physical_length_ratio_preserves_the_same_point_field() {
    let a = Field::new("seed", settings()).unwrap();
    let b = Field::new(
        "seed",
        Settings {
            radius_meters: 12742000.,
            length_scale: 2.,
            ..settings()
        },
    )
    .unwrap();
    let c = Field::new(
        "seed",
        Settings {
            amplitude_meters: 5.,
            ..settings()
        },
    )
    .unwrap();
    for p in Surface::build(2, 1.).centers {
        let value = a.sample(p).unwrap();
        assert_eq!(b.sample(p).unwrap(), value);
        assert_eq!(c.sample(p).unwrap(), 5. * value);
    }
}
#[test]
fn close_seam_and_pole_directions_are_continuous_without_coordinate_wraps() {
    let f = Field::new("seed", settings()).unwrap();
    for (a, b) in [
        ([-1., 0., 1e-12], [-1., 0., -1e-12]),
        ([1e-12, 1., 0.], [-1e-12, 1., 0.]),
        ([0., -1., 1e-12], [0., -1., -1e-12]),
    ] {
        assert!((f.sample(a).unwrap() - f.sample(b).unwrap()).abs() < 1e-6);
    }
}
#[test]
fn invalid_settings_directions_and_blank_seeds_refuse_even_at_zero_amplitude() {
    for bad in [
        Settings {
            radius_meters: 0.,
            ..settings()
        },
        Settings {
            amplitude_meters: -1.,
            ..settings()
        },
        Settings {
            length_scale: 0.,
            ..settings()
        },
        Settings {
            length_scale: f64::NAN,
            ..settings()
        },
    ] {
        assert!(Field::new("seed", bad).is_err());
    }
    for seed in [" ".to_string(), "a".repeat(129), "🌍".repeat(65)] {
        assert!(Field::new(&seed, settings()).is_err());
    }
    let f = Field::new(
        "seed",
        Settings {
            amplitude_meters: 0.,
            ..settings()
        },
    )
    .unwrap();
    assert_eq!(f.sample([1., 0., 0.]).unwrap(), 0.);
    for p in [[0.; 3], [f64::NAN, 0., 0.], [2., 0., 0.]] {
        assert!(f.sample(p).is_err());
    }
}
