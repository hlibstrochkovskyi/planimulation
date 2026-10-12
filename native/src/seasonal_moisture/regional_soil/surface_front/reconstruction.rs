//! Frozen flat-line representation controls, not a production reconstruction.
//! Exact averages and point observations are separate inputs. A known slope is
//! an algebra oracle; mean-depth powers from upstream stocks are a separate fit.
use super::{THRESHOLDS, reference};
use serde_json::{Value, json};

const POWER: f64 = 3. / 7.;
const SPEED: f64 = 0.1;
const CAP: f64 = 100.;
const RESIDUAL_LIMIT: f64 = 256. * f64::EPSILON;

fn slope() -> f64 {
    (7. / 3.) * (SPEED * reference::ROUGHNESS).powi(2)
}

/// Mean of (a+s*x)_+^POWER on a centered interval of length width.
#[cfg(test)]
fn mean_depth(a: f64, s: f64, width: f64) -> Result<f64, String> {
    if !a.is_finite() || !s.is_finite() || !width.is_finite() || width <= 0. {
        return Err("Invalid interval pressure inputs.".into());
    }
    let span = s.abs() * width;
    let high = a + 0.5 * span;
    let low = a - 0.5 * span;
    if !span.is_finite() || !high.is_finite() || !low.is_finite() {
        return Err("Nonfinite interval pressure range.".into());
    }
    mean_from_peak(high, span)
}

/// Store pressure at the wetmost endpoint so a tiny wet fraction does not
/// disappear when subtracting and adding half the much larger cell span.
fn mean_from_peak(high: f64, span: f64) -> Result<f64, String> {
    if !high.is_finite() || !span.is_finite() || span < 0. {
        return Err("Invalid peak pressure or span.".into());
    }
    let result = if high <= 0. {
        0.
    } else if span == 0. {
        high.powf(POWER)
    } else if high <= span {
        high.powf(POWER) * (high / span) / (POWER + 1.)
    } else {
        // Avoid subtracting nearly equal positive powers in a fully wet cell.
        let contrast = span / high;
        if contrast == 0. {
            high.powf(POWER)
        } else {
            high.powf(POWER) * (-((POWER + 1.) * (-contrast).ln_1p()).exp_m1())
                / ((POWER + 1.) * contrast)
        }
    };
    if !result.is_finite() || result < 0. {
        return Err("Invalid reconstructed interval mean.".into());
    }
    Ok(result)
}

/// None means zero inventory does not identify a unique latent pressure.
/// A positive fit returns wetmost endpoint pressure, not center pressure.
fn fit_peak(mean: f64, s: f64, width: f64) -> Result<Option<f64>, String> {
    if !mean.is_finite() || mean < 0. || !s.is_finite() || !width.is_finite() || width <= 0. {
        return Err("Invalid interval stock fit inputs.".into());
    }
    let span = s.abs() * width;
    if !span.is_finite() {
        return Err("Nonfinite interval pressure span.".into());
    }
    if mean == 0. {
        return Ok(None);
    }
    let high = if span == 0. {
        mean.powf(1. / POWER)
    } else if mean <= span.powf(POWER) / (POWER + 1.) {
        // A partly wet interval admits direct inversion of its exact integral.
        ((POWER + 1.) * span * mean).powf(1. / (POWER + 1.))
    } else {
        let mut low = span;
        let mut high = mean.powf(1. / POWER) + span;
        if !high.is_finite() {
            return Err("Unrepresentable interval pressure bracket.".into());
        }
        for _ in 0..96 {
            let mid = low + 0.5 * (high - low);
            if mid == low || mid == high {
                break;
            }
            if mean_from_peak(mid, span)? < mean {
                low = mid;
            } else {
                high = mid;
            }
        }
        if (mean_from_peak(low, span)? - mean).abs() <= (mean_from_peak(high, span)? - mean).abs() {
            low
        } else {
            high
        }
    };
    if !high.is_finite() || (mean_from_peak(high, span)? - mean).abs() / mean > RESIDUAL_LIMIT {
        return Err("Interval stock fit exceeds its separate residual bound.".into());
    }
    Ok(Some(high))
}

fn exact_mean(left: f64, width: f64, front: f64) -> f64 {
    reference::wave_average(left, left + width, front / SPEED)
}

fn exact_depth(normal: f64, front: f64) -> f64 {
    (slope() * (front - normal).max(0.)).powf(POWER)
}

fn original_flux(v: f64, s: f64) -> f64 {
    if v <= 0. || s == 0. {
        return 0.;
    }
    let h = v.powf(POWER);
    let grad_h = POWER * v.powf(POWER - 1.) * s;
    let beta = (h.powf(5. / 3.) / (reference::ROUGHNESS * grad_h.abs().sqrt())).min(CAP);
    -beta * grad_h
}

fn pressure_flux(v: f64, s: f64) -> f64 {
    if v <= 0. || s == 0. {
        return 0.;
    }
    let k = POWER.sqrt() / reference::ROUGHNESS;
    -v.powf(POWER) * (POWER * CAP / v).min(k / s.abs().sqrt()) * s
}

fn reconstruction(
    mean: f64,
    magnitude: f64,
    width: f64,
    center: f64,
    direction: f64,
    threshold: f64,
    exact_contour: f64,
) -> Result<Value, String> {
    let signed_slope = -direction * magnitude;
    let Some(peak) = fit_peak(mean, signed_slope, width)? else {
        return Ok(json!({"status":"zero-stock-underdetermined","crossingMeters":null}));
    };
    let target = threshold.powf(1. / POWER);
    let wet_endpoint = direction * (center - 0.5 * width);
    let crossing = wet_endpoint + (target - peak) / signed_slope;
    let support = wet_endpoint - peak / signed_slope;
    let at_truth = peak + signed_slope * (exact_contour - wet_endpoint);
    let actual = pressure_flux(at_truth, signed_slope);
    let original = original_flux(at_truth, signed_slope);
    let q_exact = direction * SPEED * threshold;
    let mean_recovered = mean_from_peak(peak, magnitude * width)?;
    let normal_crossing = direction * crossing;
    let inside = normal_crossing >= center - 0.5 * width && normal_crossing <= center + 0.5 * width;
    Ok(
        json!({"status":"positive-stock-fit","pressureAtCenter":peak-0.5*magnitude*width,
        "pressureAtWetEndpoint":peak,"wetEndpointMeters":wet_endpoint,
        "pressureSlope":signed_slope,"slopeMagnitudeOverExact":magnitude/slope(),
        "reconstructedWetFraction":(peak/(magnitude*width)).clamp(0.,1.),
        "recoveredMeanDepthMeters":mean_recovered,"relativeLocalStockResidual":(mean_recovered-mean).abs()/mean,
        "affineExtensionSupportMeters":support,"affineExtensionCrossingMeters":crossing,
        "crossingMeters":inside.then_some(crossing),"crossingInsideSelectedCell":inside,
        "normalSignedAffineCrossingErrorMeters":direction*(crossing-exact_contour),
        "uncappedPlanarSpeedMetersPerSecond":POWER.sqrt()/reference::ROUGHNESS*magnitude.sqrt(),
        "atExactContour":{"depthMeters":at_truth.max(0.).powf(POWER),
            "mobilityCapActive":at_truth>0. && POWER*CAP/at_truth<POWER.sqrt()/reference::ROUGHNESS/magnitude.sqrt(),
            "pressureFluxSquareMetersPerSecond":actual,"originalLawFluxSquareMetersPerSecond":original,
            "exactFluxSquareMetersPerSecond":q_exact,"normalSignedFluxErrorSquareMetersPerSecond":direction*(actual-q_exact)}}),
    )
}

pub(super) fn report() -> Result<Value, String> {
    let mut cases = Vec::new();
    for width in [4., 2., 1., 0.5] {
        for phase in [0., 1. / 1048576., 0.001, 0.05, 0.2, 0.5, 0.8, 0.95, 1.] {
            let front = phase * width;
            for direction in [-1., 1.] {
                let mut contours = Vec::new();
                for threshold in [0., THRESHOLDS[0], THRESHOLDS[1], THRESHOLDS[2]] {
                    let normal_truth = front - threshold.powf(1. / POWER) / slope();
                    // Selecting this cell uses the exact contour only to isolate
                    // representation error, not to discover a front in a solver.
                    let cell = if threshold == 0. {
                        0.
                    } else {
                        (normal_truth / width).floor()
                    };
                    let left = cell * width;
                    let center = left + 0.5 * width;
                    let mean = exact_mean(left, width, front);
                    let upstream_near = exact_mean(left - width, width, front);
                    let upstream_far = exact_mean(left - 2. * width, width, front);
                    let estimated =
                        (upstream_far.powf(1. / POWER) - upstream_near.powf(1. / POWER)) / width;
                    if !estimated.is_finite() || estimated <= 0. {
                        return Err("Invalid upstream mean-power slope estimate.".into());
                    }
                    let h_left = exact_depth(left, front);
                    let h_right = exact_depth(left + width, front);
                    let interpolate = |power: f64| -> Option<f64> {
                        if h_left == h_right {
                            None
                        } else {
                            Some(
                                direction
                                    * (left
                                        + width * (threshold.powf(power) - h_left.powf(power))
                                            / (h_right.powf(power) - h_left.powf(power))),
                            )
                        }
                    };
                    let truth = direction * normal_truth;
                    contours.push(json!({"thresholdMeters":threshold,"normalCellIndex":cell,
                        "normalCellBoundsMeters":[left,left+width],"exactCrossingMeters":truth,
                        "inputMeanDepthMeters":mean,"inputVolumePerUnitBreadthSquareMeters":mean*width,
                        "upstreamMeanDepthMeters":[upstream_far,upstream_near],
                        "pointEndpointDepthMeters":[h_left,h_right],
                        "pointDepthLinearCrossingMeters":interpolate(1.),
                        "pointPressureLinearCrossingMeters":interpolate(1./POWER),
                        "knownSlopeOracle":reconstruction(mean,slope(),width,center,direction,threshold,truth)?,
                        "upstreamMeanPowerSlope":reconstruction(mean,estimated,width,center,direction,threshold,truth)?}));
                }
                cases.push(
                    json!({"widthMeters":width,"phase":phase,"direction":direction,
                    "exactSupportMeters":direction*front,"contours":contours}),
                );
            }
        }
    }
    Ok(
        json!({"reportVersion":"surface-planar-reconstruction-report-1",
        "scope":"Frozen one-dimensional interval representations; no time evolution, triangles, native spherical integration or production selection",
        "speedMetersPerSecond":SPEED,"roughness":reference::ROUGHNESS,"pressureSlopeMagnitude":slope(),
        "capSquareMetersPerSecond":CAP,"stockFitRelativeResidualLimit":RESIDUAL_LIMIT,
        "stockInputs":"Exact analytic interval averages of the existing shifted planar wave; unit transverse breadth; not native point-initialized stocks",
        "pointInputs":"Exact depths at interval endpoints; separate nonconservative observer controls, not region means or the spherical adjacency observable",
        "cellSelection":"The exact contour selects its containing interval; this isolates reconstruction and is not front discovery",
        "slopeInputs":"Known exact slope is an algebra oracle. Data slope uses only two fully wet upstream interval mean-depth powers and prescribed normal orientation",
        "zeroStock":"No unique support or pressure intercept; null means underdetermined, not zero error",
        "widthSweep":"Support phase controls are self-similar; absolute scaling alone is not a general convergence certificate",
        "cases":cases}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_integrals_and_stock_inversion_cover_dry_partial_and_wet() {
        assert_eq!(mean_depth(-1., 1., 1.).unwrap(), 0.);
        assert_eq!(mean_depth(1., 0., 2.).unwrap(), 1.);
        assert_eq!(fit_peak(0., 1., 1.).unwrap(), None);
        for a in [-0.499, -0.45, 0., 0.5, 1., 10.] {
            let mean = mean_depth(a, 1., 1.).unwrap();
            let recovered = fit_peak(mean, 1., 1.).unwrap().unwrap() - 0.5;
            assert!((recovered - a).abs() < 2e-13);
            assert_eq!(
                mean_depth(a, 1., 1.).unwrap(),
                mean_depth(a, -1., 1.).unwrap()
            );
        }
        // Independent variable substitution t=u^7 removes the endpoint power
        // singularity: Simpson integrates a degree-nine polynomial here.
        let panels = 2048;
        let a = -0.2;
        let wet_length: f64 = a + 0.5;
        let f = |u: f64| 7. * wet_length.powf(POWER + 1.) * u.powi(9);
        let sum = (1..panels)
            .map(|i| {
                if i % 2 == 0 {
                    2. * f(i as f64 / panels as f64)
                } else {
                    4. * f(i as f64 / panels as f64)
                }
            })
            .sum::<f64>();
        let independent = (f(0.) + f(1.) + sum) / (3. * panels as f64);
        assert!((independent - mean_depth(a, 1., 1.).unwrap()).abs() / independent < 2e-12);
    }

    #[test]
    fn fitting_retains_local_stock_but_does_not_identify_unknown_slope() {
        let mean = mean_depth(-0.2, 1., 1.).unwrap();
        let peak = fit_peak(mean, 4., 1.).unwrap().unwrap();
        let a = peak - 2.;
        assert!((mean_from_peak(peak, 4.).unwrap() - mean).abs() / mean < RESIDUAL_LIMIT);
        assert!((-a / 4. - 0.2).abs() > 0.05);
        assert!(mean_depth(f64::NAN, 1., 1.).is_err());
        assert!(mean_depth(1., 1., 0.).is_err());
        assert!(fit_peak(-1., 1., 1.).is_err());
        assert!(fit_peak(1., f64::INFINITY, 1.).is_err());
        assert!(fit_peak(1., 1., f64::NAN).is_err());
        // Almost constant fully wet profiles must not lose their mean by
        // cancellation in a difference of nearly equal powers.
        assert!((mean_depth(1., 1e-16, 1.).unwrap() - 1.).abs() < 1e-14);
        // This wet fraction is too small to survive a center-intercept round
        // trip. Endpoint storage must retain it without inventing a wet floor.
        let high: f64 = 1e-20;
        let thin = high.powf(POWER + 1.) / (POWER + 1.);
        let fit = fit_peak(thin, 1., 1.).unwrap().unwrap();
        assert!((fit - high).abs() / high < 1e-14);
        assert!(mean_from_peak(fit, 1.).unwrap() > 0.);
        assert_eq!(mean_depth(fit - 0.5, 1., 1.).unwrap(), 0.);
        assert_eq!(fit_peak(1., 0., 1.).unwrap(), Some(1.));
        assert!(fit_peak(1., f64::MAX, 2.).is_err());
        assert!(mean_from_peak(f64::INFINITY, 1.).is_err());
    }

    #[test]
    fn capped_pressure_flux_matches_original_constitutive_law() {
        for v in [1e-12, 1e-6, 0.1, 10.] {
            for s in [-1e-3, -1e-8, 1e-8, 1e-3] {
                let original = original_flux(v, s);
                assert!((pressure_flux(v, s) - original).abs() / original.abs() < 1e-13);
            }
        }
        assert_eq!(pressure_flux(0., slope()), 0.);
        assert_eq!(pressure_flux(1., 0.), 0.);
    }

    #[test]
    fn frozen_report_repeats_and_separates_oracle_from_neighbor_data() {
        let report = report().unwrap();
        assert_eq!(report, super::report().unwrap());
        let cases = report["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 72);
        for pair in cases.as_chunks::<2>().0 {
            for (a, b) in pair[0]["contours"]
                .as_array()
                .unwrap()
                .iter()
                .zip(pair[1]["contours"].as_array().unwrap())
            {
                for name in ["knownSlopeOracle", "upstreamMeanPowerSlope"] {
                    assert_eq!(a[name]["status"], b[name]["status"]);
                    if a[name]["status"] == "positive-stock-fit" {
                        let residual = a[name]["relativeLocalStockResidual"].as_f64().unwrap();
                        assert!(residual <= RESIDUAL_LIMIT);
                        assert_eq!(
                            a[name]["normalSignedAffineCrossingErrorMeters"],
                            b[name]["normalSignedAffineCrossingErrorMeters"]
                        );
                        if name == "knownSlopeOracle" {
                            assert!(
                                a[name]["normalSignedAffineCrossingErrorMeters"]
                                    .as_f64()
                                    .unwrap()
                                    .abs()
                                    < 1e-10
                            );
                        } else {
                            assert!(a[name]["slopeMagnitudeOverExact"].as_f64().unwrap() > 1.);
                        }
                    }
                }
            }
        }
        assert_eq!(
            cases[0]["contours"][0]["knownSlopeOracle"]["status"],
            "zero-stock-underdetermined"
        );
        assert!(cases[0]["contours"][0]["pointDepthLinearCrossingMeters"].is_null());
        // Independent normalized phase calculation, not a value fitted from
        // this report. Mean-depth powers have a measurable slope bias.
        let row = cases
            .iter()
            .find(|c| c["widthMeters"] == 4. && c["phase"] == 0.8 && c["direction"] == 1.)
            .unwrap();
        let fitted = &row["contours"][0]["upstreamMeanPowerSlope"];
        assert!(
            (fitted["slopeMagnitudeOverExact"].as_f64().unwrap() - 1.0083789605756979).abs()
                < 1e-13
        );
        assert!(
            (fitted["normalSignedAffineCrossingErrorMeters"]
                .as_f64()
                .unwrap()
                + 0.008000272489785409)
                .abs()
                < 1e-13
        );
    }
}
