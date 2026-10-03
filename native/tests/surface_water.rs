use planimulation_core::surface_water::Transfers;
use planimulation_core::surface_water::{Settings, Stocks, advance};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-11, "{a} != {b}");
}

#[test]
fn cold_deposition_is_snow_and_cannot_evaporate_or_infiltrate() {
    for t in [-20., 0.] {
        let result = advance(
            Stocks::default(),
            1.,
            true,
            t,
            20.,
            100.,
            86400.,
            Settings::default(),
        )
        .unwrap();
        assert_eq!(result.stocks.snow, 20.);
        assert_eq!(result.stocks.liquid, 0.);
        assert_eq!(result.stocks.soil, 0.);
        assert_eq!(result.transfers.snowfall, 20.);
        assert_eq!(result.transfers.liquid_evaporation, 0.);
        assert_eq!(result.transfers.soil_evaporation, 0.);
        assert_eq!(result.residual_kilograms, 0.);
    }
}

#[test]
fn warming_releases_snow_at_the_declared_degree_day_rate_with_a_finite_donor() {
    let initial = Stocks {
        snow: 100.,
        ..Default::default()
    };
    let result = advance(initial, 1., false, 5., 0., 0., 86400., Settings::default()).unwrap();
    assert_eq!(result.transfers.melt, 15.);
    assert_eq!(result.stocks.snow, 85.);
    assert_eq!(result.stocks.liquid, 15.);
    assert_eq!(result.stocks.total(), initial.total());
    let scarce = advance(
        Stocks {
            snow: 2.,
            ..initial
        },
        1.,
        false,
        5.,
        0.,
        0.,
        86400.,
        Settings::default(),
    )
    .unwrap();
    assert_eq!(scarce.transfers.melt, 2.);
    assert_eq!(scarce.stocks.snow, 0.);
    let no_melt = advance(
        initial,
        1.,
        false,
        5.,
        0.,
        0.,
        86400.,
        Settings {
            melt_kilograms_per_square_meter_degree_day: 0.,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(no_melt.stocks, initial);
}

#[test]
fn rainfall_infiltrates_with_finite_soil_capacity_and_runoff_has_separate_ownership() {
    let settings = Settings {
        soil_retained_fraction: 1.,
        ..Default::default()
    };
    let result = advance(
        Stocks::default(),
        1.,
        true,
        20.,
        1000.,
        0.,
        86400.,
        settings,
    )
    .unwrap();
    assert_eq!(result.transfers.rain, 1000.);
    assert_eq!(result.stocks.soil, 150.);
    assert_eq!(result.transfers.infiltration, 150.);
    close(result.transfers.liquid_runoff, 850. * (1. - (-1_f64).exp()));
    close(result.stocks.total(), 1000.);
    assert_eq!(result.stocks.pending_runoff, result.transfers.liquid_runoff);
    let before_runoff = result.stocks.pending_runoff;
    let dry = advance(result.stocks, 1., true, 20., 0., 1e6, 86400., settings).unwrap();
    assert_eq!(dry.stocks.liquid, 0.);
    assert_eq!(dry.stocks.soil, 0.);
    assert_eq!(dry.stocks.pending_runoff, before_runoff);
    close(
        dry.transfers.liquid_evaporation + dry.transfers.soil_evaporation,
        result.stocks.liquid + result.stocks.soil,
    );
}

#[test]
fn unsaturated_infiltration_and_soil_drainage_match_their_analytic_relaxations() {
    let s = Settings::default();
    let result = advance(
        Stocks {
            liquid: 10.,
            ..Default::default()
        },
        1.,
        true,
        10.,
        0.,
        0.,
        3600.,
        s,
    )
    .unwrap();
    close(
        result.transfers.infiltration,
        10. * (1. - (-1_f64 / 6.).exp()),
    );
    assert_eq!(result.transfers.soil_drainage, 0.);
    let result = advance(
        Stocks {
            soil: 150.,
            ..Default::default()
        },
        1.,
        true,
        10.,
        0.,
        0.,
        86400.,
        s,
    )
    .unwrap();
    close(
        result.transfers.soil_drainage,
        60. * (1. - (-1_f64 / 30.).exp()),
    );
    close(result.stocks.soil, 90. + 60. * (-1_f64 / 30.).exp());
    assert_eq!(result.stocks.pending_runoff, result.transfers.soil_drainage);
    let retained = advance(
        Stocks {
            soil: 90.,
            ..Default::default()
        },
        1.,
        true,
        10.,
        0.,
        0.,
        86400.,
        s,
    )
    .unwrap();
    assert_eq!(retained.stocks.soil, 90.);
}

#[test]
fn dry_soil_limits_evaporation_and_reference_water_never_receives_land_stocks() {
    let result = advance(
        Stocks {
            soil: 30.,
            ..Default::default()
        },
        1.,
        true,
        20.,
        0.,
        10.,
        3600.,
        Settings::default(),
    )
    .unwrap();
    close(result.transfers.soil_evaporation, 2.);
    close(result.stocks.soil, 28.);
    let ocean = advance(
        Stocks::default(),
        1.,
        false,
        20.,
        1000.,
        0.,
        86400.,
        Settings::default(),
    )
    .unwrap();
    assert_eq!(ocean.stocks.liquid, 1000.);
    assert_eq!(ocean.stocks.soil, 0.);
    assert_eq!(ocean.stocks.pending_runoff, 0.);
}

#[test]
fn transfer_densities_do_not_depend_on_cell_area_and_a_synthetic_year_conserves_mass() {
    let s = Settings::default();
    let small = advance(
        Stocks {
            snow: 20.,
            soil: 80.,
            ..Default::default()
        },
        1.,
        true,
        5.,
        50.,
        4.,
        3600.,
        s,
    )
    .unwrap();
    let large = advance(
        Stocks {
            snow: 140.,
            soil: 560.,
            ..Default::default()
        },
        7.,
        true,
        5.,
        350.,
        28.,
        3600.,
        s,
    )
    .unwrap();
    for (a, b) in small
        .transfers
        .values()
        .iter()
        .zip(large.transfers.values())
    {
        close(*a * 7., b);
    }
    let mut stocks = Stocks::default();
    let mut rain = 0.;
    let mut evaporation = 0.;
    for day in 0..365 {
        let temperature = if day < 100 { -10. } else { 10. };
        let deposited = if day < 110 { 2. } else { 0. };
        let result = advance(stocks, 1., true, temperature, deposited, 1., 86400., s).unwrap();
        stocks = result.stocks;
        rain += deposited;
        evaporation += result.transfers.liquid_evaporation + result.transfers.soil_evaporation;
        close(stocks.total(), rain - evaporation);
        if day == 99 {
            assert_eq!(stocks.snow, 200.);
        }
    }
    assert_eq!(stocks.snow, 0.);
    assert!(stocks.pending_runoff > 0.);
}

#[test]
fn invalid_stocks_forcing_settings_and_overflow_reject_without_mutating_inputs() {
    let input = Stocks::default();
    for stock in [
        Stocks { snow: -1., ..input },
        Stocks {
            soil: 151.,
            ..input
        },
        Stocks {
            pending_runoff: f64::NAN,
            ..input
        },
        Stocks {
            liquid: f64::MAX,
            snow: f64::MAX,
            ..input
        },
    ] {
        assert!(advance(stock, 1., true, 10., 0., 0., 3600., Settings::default()).is_err());
    }
    assert!(
        advance(
            Stocks { soil: 1., ..input },
            1.,
            false,
            10.,
            0.,
            0.,
            3600.,
            Settings::default()
        )
        .is_err()
    );
    for (area, t, p, e, dt) in [
        (0., 10., 0., 0., 3600.),
        (1., f64::NAN, 0., 0., 3600.),
        (1., 10., -1., 0., 3600.),
        (1., 10., 0., f64::INFINITY, 3600.),
        (1., 10., 0., 0., 86401.),
    ] {
        assert!(advance(input, area, true, t, p, e, dt, Settings::default()).is_err());
    }
    assert!(
        advance(
            input,
            1.,
            true,
            10.,
            0.,
            0.,
            3600.,
            Settings {
                soil_retained_fraction: 1.1,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(
        advance(
            input,
            1e307,
            false,
            50.,
            0.,
            0.,
            86400.,
            Settings {
                soil_capacity_kilograms_per_square_meter: 1.,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(input, Stocks::default());
}

#[test]
fn compensated_transfer_ledgers_retain_small_increments_and_checkpoint_their_roundoff() {
    let base = 2_f64.powi(53);
    let mut compensated = Transfers::from_values([base; 8]);
    let mut roundoff = [0.; 8];
    let one = Transfers::from_values([1.; 8]);
    compensated.accumulate_compensated(one, &mut roundoff);
    assert!(roundoff.iter().all(|c| *c == -1.));
    let bytes = serde_json::to_vec(&(compensated, roundoff)).unwrap();
    let (mut restored, mut restored_roundoff): (Transfers, [f64; 8]) =
        serde_json::from_slice(&bytes).unwrap();
    let mut naive = Transfers::from_values([base; 8]);
    for _ in 1..1000 {
        compensated.accumulate_compensated(one, &mut roundoff);
        restored.accumulate_compensated(one, &mut restored_roundoff);
    }
    for _ in 0..1000 {
        naive.accumulate(one);
    }
    assert_eq!(compensated.values(), [base + 1000.; 8]);
    assert_eq!(naive.values(), [base; 8]);
    assert_eq!(restored, compensated);
    assert_eq!(restored_roundoff, roundoff);
}
