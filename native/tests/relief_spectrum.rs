use planimulation_core::{
    Surface,
    relief_spectrum::{BANDS, Field, Settings, footprint_retention},
};
fn settings(footprint: f64) -> Settings {
    Settings {
        radius_meters: 6_371_000.,
        sampling_length_meters: footprint,
        amplitude_meters: 300.,
    }
}
fn unit(v: [f64; 3]) -> [f64; 3] {
    let length = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    v.map(|x| x / length)
}
#[test]
fn declared_physical_bands_and_footprint_filter_have_known_limits() {
    assert_eq!(BANDS.iter().map(|b| b.1).sum::<f64>(), 1.);
    assert_eq!(footprint_retention(100., 50.).unwrap(), 0.);
    assert_eq!(footprint_retention(100., 100. / 3.).unwrap(), 0.5);
    assert_eq!(footprint_retention(100., 25.).unwrap(), 1.);
    let coarse = Field::new("first-light", settings(120_000.)).unwrap();
    let fine = Field::new("first-light", settings(30_000.)).unwrap();
    assert_eq!(coarse.bands()[2].footprint_retention, 0.);
    assert!(fine.bands()[2].footprint_retention > 0.);
    assert_eq!(fine.bands()[4].footprint_retention, 0.);
    for (a, b) in coarse.bands().iter().zip(fine.bands()) {
        assert_eq!(a.angular_frequency, b.angular_frequency);
        assert_eq!(a.correlation_length_meters, b.correlation_length_meters);
        assert!(a.footprint_retention <= b.footprint_retention);
    }
}
#[test]
fn actual_samples_repeat_bound_amplitude_and_preserve_structural_controls() {
    let s = Surface::build(3, 1.);
    let f = Field::new("first-light", settings(30_000.)).unwrap();
    let same = Field::new("first-light", settings(30_000.)).unwrap();
    let other = Field::new("another", settings(30_000.)).unwrap();
    let mut different = 0;
    for &p in &s.centers {
        for c in [0., 0.38, 1.] {
            for convergence in [0., 1000., 12000.] {
                let a = f.sample(p, c, convergence).unwrap();
                assert_eq!(a, same.sample(p, c, convergence).unwrap());
                assert!(a.signed_detail_meters.abs() <= 300.);
                assert_eq!(
                    a.mountain_strength,
                    c * (convergence / (1000. + convergence))
                );
                if c == 0. || convergence == 0. {
                    assert_eq!(a.mountain_strength, 0.);
                }
            }
        }
        if f.sample(p, 1., 1000.).unwrap() != other.sample(p, 1., 1000.).unwrap() {
            different += 1;
        }
    }
    assert!(different > s.centers.len() / 2);
}
#[test]
fn same_physical_footprint_has_identical_shared_direction_values() {
    let coarse = Surface::build(2, 1000.);
    let fine = Surface::build(3, 1000.);
    let f = Field::new("shared", settings(30_000.)).unwrap();
    for (a, b) in coarse.centers.iter().zip(&fine.centers) {
        assert_eq!(a, b);
        assert_eq!(
            f.sample(*a, 1., 6000.).unwrap(),
            f.sample(*b, 1., 6000.).unwrap()
        );
    }
}
#[test]
fn no_seam_or_pole_branch_zero_amplitude_and_fully_suppressed_field_are_exact() {
    let f = Field::new("seam", settings(30_000.)).unwrap();
    for p in [[0., 1., 0.], [0., -1., 0.], [-1., 0., 0.]] {
        let near = unit([p[0] + 1e-8, p[1] + 1e-8, p[2] + 1e-8]);
        assert!(
            (f.sample(p, 1., 6000.).unwrap().signed_detail_meters
                - f.sample(near, 1., 6000.).unwrap().signed_detail_meters)
                .abs()
                < 0.01
        );
    }
    for cfg in [
        Settings {
            amplitude_meters: 0.,
            ..settings(30_000.)
        },
        settings(1_000_000.),
    ] {
        let f = Field::new("zero", cfg).unwrap();
        assert_eq!(
            f.sample([1., 0., 0.], 1., 6000.)
                .unwrap()
                .signed_detail_meters,
            0.
        );
    }
}
#[test]
fn invalid_fields_refuse_without_clamping_physical_inputs() {
    for cfg in [
        settings(0.),
        settings(f64::NAN),
        Settings {
            amplitude_meters: -1.,
            ..settings(1.)
        },
        Settings {
            radius_meters: 0.,
            ..settings(1.)
        },
        Settings {
            amplitude_meters: 1001.,
            ..settings(1.)
        },
    ] {
        assert!(Field::new("seed", cfg).is_err());
    }
    assert!(Field::new(" ", settings(1.)).is_err());
    let f = Field::new("ok", settings(1.)).unwrap();
    for (p, c, h) in [
        ([0.; 3], 1., 0.),
        ([1., 0., 0.], -1., 0.),
        ([1., 0., 0.], 1., -1.),
        ([1., 0., 0.], 1., f64::INFINITY),
    ] {
        assert!(f.sample(p, c, h).is_err());
    }
}
