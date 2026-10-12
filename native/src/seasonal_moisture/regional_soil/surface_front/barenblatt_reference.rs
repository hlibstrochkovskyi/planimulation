//! Independent integrated cubic-pressure reference, not a selected operator.
//! Analytic pressure and slope are never supplied to interior reconstruction.
use crate::moisture_transport::total_mass;
use serde_json::{Value, json};

const POWER: f64 = 3. / 7.;
const SIMILARITY: f64 = 3. / 8.;
const ROUGHNESS: f64 = 0.04;
const CAP: f64 = 100.;
const AGE: f64 = 10.;
const INITIAL_FRONT: f64 = 10.;
const LEFT: f64 = 2.;
const RIGHT: f64 = 14.;
const END: f64 = 1.;
const K: f64 = 7. * ROUGHNESS * ROUGHNESS / 64.;
// z^9 alone needs more than 4096 panels to meet the observed doubling target.
// Increase bounded reference work rather than weakening the agreement target.
const MAX_PANELS: usize = 16384;
const AGREEMENT_FACTOR: f64 = 64. * f64::EPSILON;
const INDEPENDENT_MEAN_LIMIT: f64 = 2e-14;

// Independently evaluated incomplete-beta integrals at 80/100 decimal digits.
// Rounded fixtures never initialize stocks or alter the quadrature.
const INDEPENDENT_FIXTURES: [(f64, f64, f64, f64); 12] = [
    (0., 2., 3., 0.06537230960763588),
    (0., 9., 10., 0.026816590816971077),
    (0., 10., 11., 0.),
    (0.25, 9.5, 10., 0.023535552771155337),
    (0.25, 10., 10.5, 0.0018210155660216624),
    (0.5, 9.75, 10., 0.02269754462076316),
    (0.5, 10., 10.25, 0.009_551_912_032_441_04),
    (0.5, 10.25, 10.5, 0.),
    (1., 10., 11., 0.006112423918614549),
    (1., 10., 10.5, 0.012224847837229098),
    (1., 10.25, 10.5, 0.00467934937155234),
    (1., 10.5, 10.75, 0.),
];

fn age(time: f64) -> Result<f64, String> {
    if !time.is_finite() || !(0. ..=END).contains(&time) {
        return Err("Unsupported bounded Barenblatt reference time.".into());
    }
    Ok(AGE + time)
}
fn coordinate(x: f64) -> Result<(), String> {
    if !x.is_finite() || !(LEFT..=RIGHT).contains(&x) {
        return Err("Unsupported positive Barenblatt reference coordinate.".into());
    }
    Ok(())
}
pub(super) fn front(time: f64) -> Result<f64, String> {
    Ok(INITIAL_FRONT * (age(time)? / AGE).powf(SIMILARITY))
}
fn pressure(x: f64, time: f64) -> Result<f64, String> {
    coordinate(x)?;
    let tau = age(time)?;
    let r = front(time)?;
    if x >= r {
        return Ok(0.);
    }
    // Factored cubic avoids cancellation of two almost equal cubes.
    let v = (K / tau.powi(2)) * (r - x) * (r * r + r * x + x * x);
    if !v.is_finite() || v <= 0. {
        return Err("Unrepresentable positive analytic Barenblatt pressure.".into());
    }
    Ok(v)
}
pub(super) fn depth(x: f64, time: f64) -> Result<f64, String> {
    let v = pressure(x, time)?;
    let h = v.powf(POWER);
    if !h.is_finite() || v > 0. && h == 0. {
        return Err("Unrepresentable analytic Barenblatt depth.".into());
    }
    Ok(h)
}
pub(super) fn flux(x: f64, time: f64) -> Result<f64, String> {
    let q = SIMILARITY * x * depth(x, time)? / age(time)?;
    if !q.is_finite() {
        return Err("Unrepresentable analytic Barenblatt flux.".into());
    }
    Ok(q)
}
pub(super) fn contour(threshold: f64, time: f64) -> Result<Option<f64>, String> {
    let tau = age(time)?;
    let r = front(time)?;
    if !threshold.is_finite() || threshold < 0. {
        return Err("Invalid analytic Barenblatt contour threshold.".into());
    }
    if threshold == 0. {
        return Ok(Some(r));
    }
    // A deeper threshold cannot have a root in this monotone domain.
    if threshold > depth(LEFT, time)? {
        return Ok(None);
    }
    let target = threshold.powf(1. / POWER);
    if target == 0. || !target.is_finite() {
        return Err("Unrepresentable positive analytic contour pressure.".into());
    }
    let cube = r.powi(3) - target * tau.powi(2) / K;
    let x = cube.cbrt();
    if !x.is_finite() || x >= r {
        return Err("Unrepresentable analytic Barenblatt contour.".into());
    }
    Ok((LEFT..=RIGHT).contains(&x).then_some(x))
}

struct Integral {
    volume: f64,
    panels: usize,
    successive_difference: f64,
}
/// R-x=z^7 removes the singular support slope: h*|dx| is proportional to z^9.
fn transformed(r: f64, tau: f64, z: f64) -> f64 {
    let x = r - z.powi(7);
    7. * (K / tau.powi(2)).powf(POWER) * z.powi(9) * (r * r + r * x + x * x).powf(POWER)
}
fn simpson(r: f64, tau: f64, high: f64, span: f64, panels: usize) -> Result<f64, String> {
    // Consume the independently formed span; do not subtract rounded endpoints.
    let step = span / panels as f64;
    let low = high - span;
    let terms = (0..=panels)
        .map(|i| {
            let z = if i == panels {
                high
            } else {
                low + i as f64 * step
            };
            let weight = if i == 0 || i == panels {
                1.
            } else if i % 2 == 0 {
                2.
            } else {
                4.
            };
            weight * transformed(r, tau, z)
        })
        .collect::<Vec<_>>();
    let volume = total_mass(&terms) * step / 3.;
    if !volume.is_finite() || volume <= 0. {
        return Err("Unrepresentable positive transformed Barenblatt integral.".into());
    }
    Ok(volume)
}
fn integral(a: f64, b: f64, time: f64) -> Result<Integral, String> {
    coordinate(a)?;
    coordinate(b)?;
    let tau = age(time)?;
    if a >= b {
        return Err("Invalid ordered Barenblatt integration interval.".into());
    }
    let r = front(time)?;
    if a >= r {
        return Ok(Integral {
            volume: 0.,
            panels: 0,
            successive_difference: 0.,
        });
    }
    let high = (r - a).powf(1. / 7.);
    let span = if b >= r {
        high
    } else {
        let fraction = (b - a) / (r - a);
        if fraction <= 0.5 {
            // Narrow wet intervals lose significant digits in high-low.
            -high * ((-fraction).ln_1p() / 7.).exp_m1()
        } else {
            // Widely separated endpoints are safe and retain tiny wet tails.
            high - (r - b).powf(1. / 7.)
        }
    };
    if !span.is_finite() || span <= 0. {
        return Err("Unrepresentable positive Barenblatt transformed interval.".into());
    }
    let mut panels = 64;
    let mut previous = simpson(r, tau, high, span, panels)?;
    while panels < MAX_PANELS {
        panels *= 2;
        let volume = simpson(r, tau, high, span, panels)?;
        let difference = (volume - previous).abs();
        if difference <= AGREEMENT_FACTOR * volume {
            return Ok(Integral {
                volume,
                panels,
                successive_difference: difference,
            });
        }
        previous = volume;
    }
    Err("Barenblatt quadrature did not reach its bounded observed agreement criterion.".into())
}
pub(super) fn mean(a: f64, b: f64, time: f64) -> Result<f64, String> {
    let volume = integral(a, b, time)?.volume;
    let h = volume / (b - a);
    if !h.is_finite() || h < 0. || volume > 0. && h == 0. {
        return Err("Unrepresentable integrated Barenblatt mean.".into());
    }
    Ok(h)
}
fn beta(x: f64, time: f64) -> Result<f64, String> {
    Ok(pressure(x, time)? * age(time)? / (8. * POWER * K * x))
}

pub(super) fn report() -> Result<Value, String> {
    let mut cases = Vec::new();
    let mut restriction = Vec::new();
    let mut maximum_difference: f64 = 0.;
    let mut maximum_panels = 0;
    for time in [0., 0.25, 0.5, 1.] {
        for width in [1., 0.5, 0.25] {
            let n = ((RIGHT - LEFT) / width) as usize;
            let mut means = Vec::new();
            let mut agreements = Vec::new();
            for i in 0..n {
                let a = LEFT + i as f64 * width;
                let q = integral(a, a + width, time)?;
                means.push(q.volume / width);
                agreements.push(json!({"panels":q.panels,"successiveVolumeDifferenceSquareMeters":q.successive_difference}));
                maximum_panels = maximum_panels.max(q.panels);
                maximum_difference = maximum_difference.max(q.successive_difference);
            }
            let whole = integral(LEFT, RIGHT, time)?;
            let inventory = total_mass(&means) * width;
            cases.push(json!({"timeSeconds":time,"widthMeters":width,"meansMeters":means,
                "quadratureAgreement":agreements,"inventorySquareMeters":inventory,
                "wholeDomainIntegralSquareMeters":whole.volume,"signedPartitionDifferenceSquareMeters":inventory-whole.volume}));
            if width > 0.25 {
                let differences = (0..n)
                    .map(|i| {
                        let a = LEFT + i as f64 * width;
                        Ok(mean(a, a + width, time)?
                            - 0.5 * mean(a, a + 0.5 * width, time)?
                            - 0.5 * mean(a + 0.5 * width, a + width, time)?)
                    })
                    .collect::<Result<Vec<f64>, String>>()?;
                restriction.push(json!({"timeSeconds":time,"coarseWidthMeters":width,
                    "meanRestrictionDifferencesMeters":differences,
                    "maximumAbsoluteMeanRestrictionDifferenceMeters":differences.iter().map(|d|d.abs()).fold(0.,f64::max)}));
            }
        }
    }
    // R increases, age>=AGE and x>=LEFT. Discarding -x^3 gives a conservative bound.
    let bound = 7. * front(END)?.powi(3) / (24. * AGE * LEFT);
    if bound >= CAP {
        return Err("Analytic Barenblatt cap-inactivity certificate failed.".into());
    }
    let contours = [0.,0.25,0.5,1.].into_iter().map(|time| {
        let roots = [0.,0.001,0.01,0.05].into_iter().map(|threshold| {
            Ok(json!({"thresholdMeters":threshold,"positionMeters":contour(threshold,time)?}))
        }).collect::<Result<Vec<Value>,String>>()?;
        Ok(json!({"timeSeconds":time,"frontMeters":front(time)?,"leftBoundaryDiffusivitySquareMetersPerSecond":beta(LEFT,time)?,"contours":roots}))
    }).collect::<Result<Vec<Value>,String>>()?;
    let independent = INDEPENDENT_FIXTURES.into_iter().map(|(time,a,b,reference)| {
        let observed=mean(a,b,time)?;
        if (observed-reference).abs() > INDEPENDENT_MEAN_LIMIT {
            return Err("Barenblatt reference disagrees with its independent integrated fixture.".into());
        }
        Ok(json!({"timeSeconds":time,"intervalMeters":[a,b],"independentRoundedMeanMeters":reference,
            "observedMeanMeters":observed,"signedMeanDifferenceMeters":observed-reference}))
    }).collect::<Result<Vec<Value>,String>>()?;
    Ok(
        json!({"reportVersion":"surface-barenblatt-reference-report-1",
        "scope":"Independent bounded integrated cubic-pressure oracle; no reconstruction or evolution selection",
        "domainMeters":[LEFT,RIGHT],"endSeconds":END,"roughness":ROUGHNESS,"capSquareMetersPerSecond":CAP,
        "initialAgeSeconds":AGE,"initialFrontMeters":INITIAL_FRONT,"pressureCoefficient":K,
        "analyticDiffusivityUpperBoundSquareMetersPerSecond":bound,
        "integrationContract":"Support-clipped analytic integral with R-x=z^7 and stable narrow-interval span; bounded Simpson doubling; successive agreement is empirical, not a certified error bound",
        "maximumPanelsPerIntegral":MAX_PANELS,"observedMaximumCellPanels":maximum_panels,
        "observedMaximumSuccessiveCellVolumeDifferenceSquareMeters":maximum_difference,
        "oracleContract":"Integrated means only; analytic pressure and slope never enter interior inference",
        "independentFixtureContract":"Incomplete-beta integrals independently evaluated at 80/100 decimal digits; rounded values are validation observations only",
        "independentMaximumAbsoluteMeanDifferenceLimitMeters":INDEPENDENT_MEAN_LIMIT,
        "independentIncompleteBetaFixtures":independent,"cases":cases,"restriction":restriction,"contours":contours}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_flux_and_continuity_are_independently_satisfied() {
        for time in [0., 0.25, 0.5, 1.] {
            let tau = age(time).unwrap();
            for x in [2., 5., 9., front(time).unwrap() - 0.001] {
                let v = pressure(x, time).unwrap();
                let h = depth(x, time).unwrap();
                let vx = -3. * K * x * x / tau.powi(2);
                let hx = POWER * h * vx / v;
                let physical = -h.powf(5. / 3.) * hx.signum() * hx.abs().sqrt() / ROUGHNESS;
                let exact = flux(x, time).unwrap();
                assert!((physical - exact).abs() < 2e-16);
                assert!(beta(x, time).unwrap() < CAP);
                let vt = (-7. / 8. * v + 9. / 8. * K * x.powi(3) / tau.powi(2)) / tau;
                let ht = POWER * h * vt / v;
                let qx = SIMILARITY * (h + x * hx) / tau;
                assert!((ht + qx).abs() < 2e-12 * (ht.abs() + qx.abs()).max(1.));
            }
            let r = front(time).unwrap();
            assert_eq!(depth(r, time).unwrap(), 0.);
            assert_eq!(flux(r, time).unwrap(), 0.);
            assert_eq!(mean(r, RIGHT, time).unwrap(), 0.);
        }
        assert_eq!(front(0.).unwrap(), INITIAL_FRONT);
        assert!(7. * front(1.).unwrap().powi(3) / (24. * AGE * LEFT) < CAP);
    }
    #[test]
    fn integrated_derivative_matches_independent_exterior_flux_difference() {
        let mut maximum_errors = Vec::new();
        for dt in [0.0001, 0.00005, 0.000025] {
            let mut maximum: f64 = 0.;
            for time in [0.25, 0.5, 0.75] {
                for (a, b) in [(2., 2.25), (9., 10.), (10., 10.25), (11., 12.)] {
                    let derivative = (mean(a, b, time + dt).unwrap()
                        - mean(a, b, time - dt).unwrap())
                        / (2. * dt);
                    let rhs = (flux(a, time).unwrap() - flux(b, time).unwrap()) / (b - a);
                    maximum = maximum.max((derivative - rhs).abs());
                    if dt == 0.000025 {
                        assert!(
                            (derivative - rhs).abs() < 2e-10,
                            "{a} {b} {time}: {derivative} {rhs}"
                        );
                    }
                }
            }
            maximum_errors.push(maximum);
        }
        for pair in maximum_errors.windows(2) {
            assert!((3.7..4.3).contains(&(pair[0] / pair[1])));
        }
    }
    #[test]
    fn integrated_means_restrict_conservatively_and_contours_are_independent() {
        let r = report().unwrap();
        assert_eq!(r["cases"].as_array().unwrap().len(), 12);
        for case in r["cases"].as_array().unwrap() {
            assert!(
                case["signedPartitionDifferenceSquareMeters"]
                    .as_f64()
                    .unwrap()
                    .abs()
                    < 2e-14
            );
        }
        for pair in r["restriction"].as_array().unwrap() {
            assert!(
                pair["maximumAbsoluteMeanRestrictionDifferenceMeters"]
                    .as_f64()
                    .unwrap()
                    < 2e-14
            );
        }
        for time in [0., 0.25, 0.5, 1.] {
            for threshold in [0.001, 0.01, 0.05] {
                let x = contour(threshold, time).unwrap().unwrap();
                assert!((depth(x, time).unwrap() - threshold).abs() < 2e-13);
            }
            assert_eq!(contour(1., time).unwrap(), None);
        }
    }
    #[test]
    fn invalid_controls_refuse_before_quadrature() {
        for time in [f64::NAN, f64::INFINITY, -0.001, 1.001] {
            assert!(front(time).is_err());
            assert!(mean(2., 3., time).is_err());
            assert!(contour(0., time).is_err());
        }
        for x in [f64::NAN, f64::INFINITY, 1.999, 14.001] {
            assert!(depth(x, 0.).is_err());
            assert!(flux(x, 0.).is_err());
        }
        for (a, b) in [(3., 2.), (2., 2.), (1., 3.), (2., 15.)] {
            assert!(mean(a, b, 0.).is_err());
        }
        for threshold in [f64::NAN, f64::INFINITY, -1.] {
            assert!(contour(threshold, 0.).is_err());
        }
        assert!(contour(f64::MIN_POSITIVE, 0.).is_err());
    }

    #[test]
    fn independently_evaluated_incomplete_beta_fixtures_match_integrated_means() {
        for (time, a, b, reference) in INDEPENDENT_FIXTURES {
            let observed = mean(a, b, time).unwrap();
            assert!(
                (observed - reference).abs() < INDEPENDENT_MEAN_LIMIT,
                "{time} {a} {b}: {observed} {reference}"
            );
            if reference == 0. {
                assert_eq!(observed, 0.);
            }
        }
    }

    #[test]
    fn narrow_wet_intervals_do_not_lose_the_transformed_span() {
        for time in [0., 0.25, 0.5, 1.] {
            for a in [2., 5., 9.] {
                let b = a + 1e-12;
                let observed = mean(a, b, time).unwrap();
                let high = depth(a, time).unwrap();
                let low = depth(b, time).unwrap();
                assert!(observed >= low - 2e-15 && observed <= high + 2e-15);
            }
        }
    }
}
