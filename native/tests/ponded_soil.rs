use planimulation_core::surface_water::ponded_soil::{Mass, Settings, State, advance};

fn mass(high: f64) -> Mass {
    Mass { high, low: 0. }
}
fn run(
    before: State,
    seconds: f64,
    demand: f64,
    settings: Settings,
) -> planimulation_core::surface_water::ponded_soil::Step {
    advance(before, 1., 20., demand, seconds, settings).unwrap()
}
fn nearly(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 64. * f64::EPSILON * a.abs().max(b.abs()).max(1e-300),
        "{a} != {b}"
    );
}

#[test]
fn funded_infiltration_matches_analytic_soil_capacity_response() {
    let settings = Settings {
        soil_retained_fraction: 1.,
        ..Default::default()
    };
    let before = State {
        liquid: mass(1000.),
        soil: mass(30.),
        ..Default::default()
    };
    let step = run(before, 3600., 0., settings);
    let expected = 120. * (1. - (-3600. / settings.infiltration_response_seconds).exp());
    nearly(step.transfers.infiltration_kilograms, expected);
    nearly(step.state.soil.high, 30. + expected);
    nearly(step.state.liquid.high, 1000. - expected);
    assert_eq!(step.state.drainage, mass(0.));
    let mut split = before;
    for _ in 0..4 {
        split = run(split, 900., 0., settings).state;
    }
    nearly(split.soil.high, step.state.soil.high);
}

#[test]
fn film_limit_keeps_soil_drainage_active_and_changes_no_whole_cell_switch() {
    let settings = Settings::default();
    let dry = run(
        State {
            soil: mass(120.),
            ..Default::default()
        },
        900.,
        0.,
        settings,
    );
    assert!(dry.transfers.soil_drainage_kilograms > 0.);
    for film in [1e-3, 1e-9, 1e-15, 1e-30] {
        let step = run(
            State {
                soil: mass(120.),
                liquid: mass(film),
                ..Default::default()
            },
            900.,
            0.,
            settings,
        );
        assert_eq!(step.transfers.infiltration_kilograms, film);
        assert_eq!(step.state.liquid, mass(0.));
        assert!(step.transfers.soil_drainage_kilograms > 0.);
        let drainage_fraction = -(-900. / settings.soil_drainage_response_seconds).exp_m1();
        nearly(
            step.transfers.soil_drainage_kilograms,
            dry.transfers.soil_drainage_kilograms + film * drainage_fraction,
        );
    }
}

#[test]
fn evaporation_has_one_finite_demand_and_an_owned_recipient() {
    let step = run(
        State {
            liquid: mass(2.),
            soil: mass(75.),
            vapor: mass(7.),
            ..Default::default()
        },
        900.,
        10.,
        Settings::default(),
    );
    assert_eq!(step.transfers.liquid_evaporation_kilograms, 2.);
    assert_eq!(step.transfers.soil_evaporation_kilograms, 4.);
    assert_eq!(step.state.vapor, mass(13.));
    assert_eq!(step.state.liquid, mass(0.));
    assert_eq!(step.transfers.infiltration_kilograms, 0.);
    let dry = run(
        State {
            soil: mass(75.),
            ..Default::default()
        },
        900.,
        10.,
        Settings::default(),
    );
    for film in [1e-3, 1e-9, 1e-15] {
        let step = run(
            State {
                liquid: mass(film),
                soil: mass(75.),
                ..Default::default()
            },
            900.,
            10.,
            Settings::default(),
        );
        nearly(step.state.vapor.high, dry.state.vapor.high + film * 0.5);
    }
}

#[test]
fn saturation_retention_cold_and_zero_interval_have_explicit_controls() {
    let settings = Settings {
        soil_retained_fraction: 1.,
        ..Default::default()
    };
    let full = State {
        liquid: mass(100.),
        soil: mass(150.),
        ..Default::default()
    };
    assert_eq!(run(full, 900., 0., settings).state, full);
    let dry = State {
        soil: mass(90.),
        ..Default::default()
    };
    assert_eq!(run(dry, 900., 0., Settings::default()).state, dry);
    for temperature in [-100., -5., 0.] {
        assert_eq!(
            advance(full, 1., temperature, 10., 900., Settings::default())
                .unwrap()
                .state,
            full
        );
    }
    assert_eq!(run(full, 0., 10., Settings::default()).state, full);
}

#[test]
fn area_scaling_and_serialized_continuation_preserve_column_behavior() {
    let original = State {
        liquid: mass(10.),
        soil: mass(120.),
        vapor: mass(5.),
        drainage: mass(1.),
    };
    let base = run(original, 900., 2., Settings::default());
    for area in [1e-6, 1024., 1e12] {
        let scaled = State {
            liquid: mass(10. * area),
            soil: mass(120. * area),
            vapor: mass(5. * area),
            drainage: mass(area),
        };
        let step = advance(scaled, area, 20., 2. * area, 900., Settings::default()).unwrap();
        nearly(step.state.liquid.high / area, base.state.liquid.high);
        nearly(step.state.soil.high / area, base.state.soil.high);
        nearly(step.state.vapor.high / area, base.state.vapor.high);
        nearly(step.state.drainage.high / area, base.state.drainage.high);
    }
    let restored = serde_json::from_str(&serde_json::to_string(&base.state).unwrap()).unwrap();
    assert_eq!(
        run(restored, 900., 2., Settings::default()),
        run(base.state, 900., 2., Settings::default())
    );
}

#[test]
fn tiny_transfers_retain_signed_low_components_and_do_not_borrow_capacity() {
    let settings = Settings {
        soil_retained_fraction: 1.,
        ..Default::default()
    };
    let step = run(
        State {
            liquid: mass(1e-15),
            soil: mass(100.),
            ..Default::default()
        },
        900.,
        0.,
        settings,
    );
    assert_eq!(step.state.soil.high, 100.);
    assert_eq!(step.state.soil.low, 1e-15);
    assert_eq!(step.state.liquid, mass(0.));
    let almost_full = State {
        liquid: mass(1.),
        soil: Mass {
            high: 150.,
            low: -1e-15,
        },
        ..Default::default()
    };
    let step = run(almost_full, 900., 0., settings);
    assert!(step.transfers.infiltration_kilograms > 0.);
    assert!(step.state.soil.low <= 0.);
    let subnormal = f64::from_bits(1);
    let step = run(
        State {
            liquid: mass(subnormal),
            soil: mass(100.),
            ..Default::default()
        },
        900.,
        0.,
        settings,
    );
    assert_eq!(step.transfers.infiltration_kilograms, subnormal);
    assert_eq!(step.state.soil.low, subnormal);
    assert_eq!(step.state.liquid, mass(0.));
}

#[test]
fn malformed_or_unrepresentable_candidates_reject_without_mutating_input() {
    let before = State {
        liquid: mass(1.),
        soil: mass(120.),
        ..Default::default()
    };
    for (area, temperature, demand, seconds) in [
        (0., 20., 0., 900.),
        (f64::INFINITY, 20., 0., 900.),
        (1., f64::NAN, 0., 900.),
        (1., 20., -1., 900.),
        (1., 20., 0., 86401.),
    ] {
        assert!(
            advance(
                before,
                area,
                temperature,
                demand,
                seconds,
                Settings::default()
            )
            .is_err()
        );
    }
    for soil in [
        Mass {
            high: 0.,
            low: 1e-30,
        },
        mass(151.),
        Mass {
            high: 120.,
            low: 1.,
        },
    ] {
        assert!(
            advance(
                State { soil, ..before },
                1.,
                20.,
                0.,
                900.,
                Settings::default()
            )
            .is_err()
        );
    }
    let settings = Settings {
        soil_retained_fraction: 1.,
        ..Default::default()
    };
    let unrepresentable = State {
        liquid: mass(1e-40),
        soil: Mass {
            high: 100.,
            low: 1e-15,
        },
        ..Default::default()
    };
    assert!(advance(unrepresentable, 1., 20., 0., 900., settings).is_err());
    assert_eq!(unrepresentable.liquid.high, 1e-40);
    assert_eq!(unrepresentable.soil.low, 1e-15);
    assert_eq!(before.soil.high, 120.);
}

#[test]
fn repeated_small_evaporation_matches_an_independent_integer_ownership_oracle() {
    let initial = 1_i128 << 70;
    let mut state = State {
        liquid: mass(initial as f64),
        soil: mass(150.),
        vapor: mass(initial as f64),
        ..Default::default()
    };
    let settings = Settings {
        soil_retained_fraction: 1.,
        ..Default::default()
    };
    let mut retained_tail = false;
    for elapsed in 1..=100_000_i128 {
        let step = run(state, 1., 1., settings);
        assert_eq!(step.transfers.liquid_evaporation_kilograms, 1.);
        assert_eq!(step.transfers.soil_evaporation_kilograms, 0.);
        assert_eq!(step.transfers.infiltration_kilograms, 0.);
        state = step.state;
        assert_eq!(
            state.liquid.high as i128 + state.liquid.low as i128,
            initial - elapsed
        );
        assert_eq!(
            state.vapor.high as i128 + state.vapor.low as i128,
            initial + elapsed
        );
        assert_eq!(state.soil, mass(150.));
        assert_eq!(state.drainage, mass(0.));
        assert_eq!(step.stock_residuals_kilograms, [0.; 4]);
        retained_tail |= state.liquid.low != 0. && state.vapor.low != 0.;
    }
    assert!(retained_tail);
    let restored = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    assert_eq!(
        run(restored, 1., 1., settings),
        run(state, 1., 1., settings)
    );
}

#[test]
fn coupled_infiltration_and_drainage_refine_toward_an_independent_ode_solution() {
    let settings = Settings::default();
    let duration = 7200.;
    let i = 1. / settings.infiltration_response_seconds;
    let d = 1. / settings.soil_drainage_response_seconds;
    let retained = 150. * settings.soil_retained_fraction;
    let equilibrium = (150. * i + retained * d) / (i + d);
    let expected_soil = equilibrium + (120. - equilibrium) * (-(i + d) * duration).exp();
    let expected_drainage = d
        * ((equilibrium - retained) * duration
            + (120. - equilibrium) * -(-(i + d) * duration).exp_m1() / (i + d));
    let mut previous_errors = [f64::INFINITY; 2];
    for seconds in [900., 450., 225.] {
        let mut state = State {
            liquid: mass(1000.),
            soil: mass(120.),
            ..Default::default()
        };
        for _ in 0..(duration / seconds) as usize {
            state = run(state, seconds, 0., settings).state;
        }
        let errors = [
            (state.soil.high - expected_soil).abs(),
            (state.drainage.high - expected_drainage).abs(),
        ];
        for j in 0..2 {
            assert!(errors[j] < 0.51 * previous_errors[j]);
        }
        previous_errors = errors;
        nearly(
            state.liquid.high + state.soil.high + state.drainage.high,
            1120.,
        );
        println!(
            "ponded-soil refinement: seconds={seconds} soil_error={} drainage_error={}",
            errors[0], errors[1]
        );
    }
}

#[test]
fn finite_recipient_capacity_caps_grants_and_settings_reject_nan() {
    let step = run(
        State {
            liquid: mass(1.),
            vapor: mass(f64::MAX),
            ..Default::default()
        },
        900.,
        1.,
        Settings::default(),
    );
    assert_eq!(step.transfers.liquid_evaporation_kilograms, 0.);
    assert_eq!(step.state.vapor, mass(f64::MAX));
    assert_eq!(step.transfers.infiltration_kilograms, 1.);
    let blocked = State {
        soil: mass(120.),
        drainage: mass(f64::MAX),
        ..Default::default()
    };
    let step = run(blocked, 900., 0., Settings::default());
    assert_eq!(step.state, blocked);
    assert_eq!(step.transfers.soil_drainage_kilograms, 0.);
    for settings in [
        Settings {
            soil_capacity_kilograms_per_square_meter: f64::NAN,
            ..Default::default()
        },
        Settings {
            soil_retained_fraction: -0.1,
            ..Default::default()
        },
        Settings {
            infiltration_response_seconds: 0.,
            ..Default::default()
        },
        Settings {
            soil_drainage_response_seconds: f64::INFINITY,
            ..Default::default()
        },
    ] {
        assert!(advance(State::default(), 1., 20., 0., 900., settings).is_err());
    }
}
