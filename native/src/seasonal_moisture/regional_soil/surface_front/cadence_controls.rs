//! Time/grid conditioning diagnostics on the unchanged affine-pressure wave.
//! Extrapolated observations are never physical stock or a selected method.
use super::paired_evolution::{self, Config, Method};
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock as Stock};
use serde_json::{Value, json};

const END: f64 = 0.1;
const TIMES: [f64; 4] = [0.001, 0.0005, 0.00025, 0.000125];
struct Profile {
    config: Config,
    stocks: Vec<Stock>,
    errors: Vec<f64>,
}
impl Profile {
    fn from_trial(trial: &Value) -> Result<Option<Self>, String> {
        if trial["completed"] != true {
            return Ok(None);
        }
        let number = |v: &Value| {
            v.as_f64()
                .filter(|x| x.is_finite())
                .ok_or("Invalid cadence scalar.".to_string())
        };
        let c = &trial["config"];
        let config = Config {
            width: number(&c["width"])?,
            step_seconds: number(&c["stepSeconds"])?,
            initial_front: number(&c["initialFront"])?,
            direction: number(&c["direction"])?,
        };
        let (cells, steps) = config.shape()?;
        if number(&trial["finalObservation"]["timeSeconds"])? != END
            || trial["method"] != "pressurePenaltyOne"
        {
            return Err("Unmatched cadence method or final time.".into());
        }
        let high = trial["finalVolumeHigh"]
            .as_array()
            .ok_or("Missing cadence stocks.")?;
        let low = trial["finalVolumeLow"]
            .as_array()
            .ok_or("Missing cadence stock low parts.")?;
        if high.len() != low.len()
            || high.len() != cells
            || trial["finalObservation"]["acceptedSteps"] != steps
        {
            return Err("Invalid cadence stock shape.".into());
        }
        let stocks = high
            .iter()
            .zip(low)
            .map(|(h, l)| Stock::new(number(h)?, number(l)?, f64::MAX))
            .collect::<Result<Vec<_>, String>>()?;
        // Subtract nearby high components before summing their low corrections.
        let errors = stocks
            .iter()
            .enumerate()
            .map(|(i, s)| {
                total_mass(&[s.high - config.mean(i, END) * config.width, s.low]) / config.width
            })
            .collect::<Vec<_>>();
        if errors.iter().any(|e| !e.is_finite()) {
            return Err("Unrepresentable cadence error observation.".into());
        }
        Ok(Some(Self {
            config,
            stocks,
            errors,
        }))
    }
}
fn same_problem(a: &Profile, b: &Profile) -> Result<(), String> {
    if a.config.initial_front != b.config.initial_front || a.config.direction != b.config.direction
    {
        return Err("Cadence comparison requires the same physical front and direction.".into());
    }
    Ok(())
}
fn norm(values: &[f64], width: f64) -> Result<Value, String> {
    if values.is_empty()
        || values.iter().any(|v| !v.is_finite())
        || !width.is_finite()
        || width <= 0.
    {
        return Err("Invalid cadence norm observation.".into());
    }
    let squares = values.iter().map(|v| v * v).collect::<Vec<_>>();
    let rms = (total_mass(&squares) / values.len() as f64).sqrt();
    let l1 = width * total_mass(&values.iter().map(|v| v.abs()).collect::<Vec<_>>());
    let signed = width * total_mass(values);
    if !rms.is_finite() || !l1.is_finite() || !signed.is_finite() {
        return Err("Cadence norm overflow.".into());
    }
    Ok(
        json!({"rmsMeters":rms,"maximumMeters":values.iter().map(|v|v.abs()).fold(0.,f64::max),
        "l1VolumeSquareMeters":l1,"signedVolumeSquareMeters":signed}),
    )
}
fn ratio(a: f64, b: f64) -> Option<f64> {
    let r = a / b;
    (a.is_finite() && b.is_finite() && b > 0. && r.is_finite()).then_some(r)
}
fn differences(a: &Profile, b: &Profile) -> Result<Vec<f64>, String> {
    same_problem(a, b)?;
    if a.config.width != b.config.width || a.stocks.len() != b.stocks.len() {
        return Err("Temporal difference requires one grid.".into());
    }
    let width = a.config.width;
    Ok(a.stocks
        .iter()
        .zip(&b.stocks)
        .map(|(a, b)| total_mass(&[a.high - b.high, a.low, -b.low]) / width)
        .collect())
}
// Diagnostic cancellation acts on error vectors, not scalar error magnitudes.
fn extrapolate(coarse: &[f64], fine: &[f64]) -> Result<Vec<f64>, String> {
    if coarse.len() != fine.len()
        || coarse.is_empty()
        || coarse.iter().chain(fine).any(|e| !e.is_finite())
    {
        return Err("Invalid Richardson observation vectors.".into());
    }
    let errors = coarse
        .iter()
        .zip(fine)
        .map(|(c, f)| 2. * f - c)
        .collect::<Vec<_>>();
    if errors.iter().any(|e| !e.is_finite()) {
        return Err("Richardson observation overflow.".into());
    }
    Ok(errors)
}
fn restricted_difference(coarse: &Profile, fine: &Profile) -> Result<Vec<f64>, String> {
    same_problem(coarse, fine)?;
    if coarse.config.width != 2. * fine.config.width || fine.stocks.len() != 2 * coarse.stocks.len()
    {
        return Err("Cadence restriction requires aligned nested two-to-one grids.".into());
    }
    Ok(coarse
        .stocks
        .iter()
        .zip(fine.stocks.as_chunks::<2>().0.iter())
        .map(|(c, f)| {
            // Cancel the largest high first so a tiny fine owner is not lost.
            let (large, small) = if f[0].high >= f[1].high {
                (&f[0], &f[1])
            } else {
                (&f[1], &f[0])
            };
            total_mass(&[
                c.high - large.high,
                -small.high,
                c.low,
                -large.low,
                -small.low,
            ]) / coarse.config.width
        })
        .collect())
}
fn temporal(trials: &[Value], indices: [usize; 3]) -> Result<Value, String> {
    let profiles = indices
        .map(|i| Profile::from_trial(&trials[i]))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let [Some(a), Some(b), Some(c)] = profiles.as_slice() else {
        return Ok(json!({"trials":indices,"completedAtSameEndTime":false,"comparison":null}));
    };
    if a.config.step_seconds != 2. * b.config.step_seconds
        || b.config.step_seconds != 2. * c.config.step_seconds
    {
        return Err("Richardson comparison requires consecutive temporal halvings.".into());
    }
    let ab = differences(a, b)?;
    let bc = differences(b, c)?;
    let r_ab = extrapolate(&a.errors, &b.errors)?;
    let r_bc = extrapolate(&b.errors, &c.errors)?;
    let summary = |errors: &[f64]| -> Result<Value, String> {
        let mut n = norm(errors, a.config.width)?;
        let samples = errors
            .iter()
            .enumerate()
            .map(|(i, e)| a.config.mean(i, END) + e)
            .collect::<Vec<_>>();
        n["negativeSampleCount"] = json!(samples.iter().filter(|x| **x < 0.).count());
        n["minimumSampleMeters"] = json!(samples.iter().copied().fold(f64::INFINITY, f64::min));
        Ok(n)
    };
    let ab_norm = norm(&ab, a.config.width)?;
    let bc_norm = norm(&bc, a.config.width)?;
    let ra = summary(&r_ab)?;
    let rb = summary(&r_bc)?;
    Ok(json!({"trials":indices,"completedAtSameEndTime":true,
        "coarseMinusMiddle":ab_norm,"middleMinusFine":bc_norm,
        "successiveFieldDifferenceRmsRatio":ratio(ab_norm["rmsMeters"].as_f64().unwrap(),bc_norm["rmsMeters"].as_f64().unwrap()),
        "fineTemporalErrorEstimate":bc_norm,"fineActualError":norm(&c.errors,c.config.width)?,
        "coarsePairRichardsonError":ra,"finePairRichardsonError":rb,
        "richardsonErrorRmsRatio":ratio(ra["rmsMeters"].as_f64().unwrap(),rb["rmsMeters"].as_f64().unwrap())}))
}
fn spatial(
    trials: &[Value],
    coarse_indices: [usize; 2],
    fine_indices: [usize; 2],
) -> Result<Value, String> {
    let profiles = coarse_indices
        .into_iter()
        .chain(fine_indices)
        .map(|i| Profile::from_trial(&trials[i]))
        .collect::<Result<Vec<_>, _>>()?;
    let [Some(a), Some(b), Some(c), Some(d)] = profiles.as_slice() else {
        return Ok(
            json!({"coarseTrials":coarse_indices,"fineTrials":fine_indices,"completedAtSameEndTime":false,"comparison":null}),
        );
    };
    if a.config.step_seconds != c.config.step_seconds
        || b.config.step_seconds != d.config.step_seconds
        || a.config.step_seconds != 2. * b.config.step_seconds
    {
        return Err("Spatial conditioning comparison requires matched temporal halves.".into());
    }
    same_problem(a, b)?;
    same_problem(c, d)?;
    if a.config.width != b.config.width || c.config.width != d.config.width {
        return Err("Each spatial time pair requires a single grid.".into());
    }
    let coarse_diff = restricted_difference(a, c)?;
    let fine_diff = restricted_difference(b, d)?;
    let extrapolated = extrapolate(&coarse_diff, &fine_diff)?;
    // Initialization restriction discrepancy remains visible; not subtracted.
    let initial = (0..a.stocks.len())
        .map(|i| {
            a.config.mean(i, 0.) - 0.5 * (c.config.mean(2 * i, 0.) + c.config.mean(2 * i + 1, 0.))
        })
        .collect::<Vec<_>>();
    let final_reference = (0..a.stocks.len())
        .map(|i| {
            a.config.mean(i, END)
                - 0.5 * (c.config.mean(2 * i, END) + c.config.mean(2 * i + 1, END))
        })
        .collect::<Vec<_>>();
    Ok(
        json!({"coarseTrials":coarse_indices,"fineTrials":fine_indices,"completedAtSameEndTime":true,
        "coarseTimeIntergridDifference":norm(&coarse_diff,a.config.width)?,
        "fineTimeIntergridDifference":norm(&fine_diff,a.config.width)?,
        "richardsonIntergridDifference":norm(&extrapolated,a.config.width)?,
        "initialExactMeanRestrictionDifference":norm(&initial,a.config.width)?,
        "finalExactMeanRestrictionDifference":norm(&final_reference,a.config.width)?}),
    )
}

fn locate(trials: &[Value], width: f64, dt: f64, front: f64) -> usize {
    trials
        .iter()
        .position(|t| {
            t["config"]["width"] == width
                && t["config"]["stepSeconds"] == dt
                && t["config"]["initialFront"] == front
                && t["config"]["direction"] == 1.
        })
        .unwrap()
}
pub(super) fn report() -> Result<Value, String> {
    let mut trials = Vec::new();
    for width in [1., 0.5] {
        for initial_front in [0., 0.995] {
            for step_seconds in TIMES {
                trials.push(paired_evolution::trial(
                    Config {
                        width,
                        initial_front,
                        step_seconds,
                        direction: 1.,
                    },
                    Method::PressurePenaltyOne,
                )?);
            }
        }
    }
    trials.push(paired_evolution::trial(
        Config {
            width: 0.25,
            initial_front: 0.,
            step_seconds: 0.0000625,
            direction: 1.,
        },
        Method::PressurePenaltyOne,
    )?);
    trials.push(paired_evolution::trial(
        Config {
            width: 0.5,
            initial_front: 0.995,
            step_seconds: 0.000125,
            direction: -1.,
        },
        Method::PressurePenaltyOne,
    )?);
    let mut temporal_controls = Vec::new();
    let mut spatial_controls = Vec::new();
    for front in [0., 0.995] {
        for width in [1., 0.5] {
            for k in 0..TIMES.len() - 2 {
                temporal_controls.push(temporal(
                    &trials,
                    [
                        locate(&trials, width, TIMES[k], front),
                        locate(&trials, width, TIMES[k + 1], front),
                        locate(&trials, width, TIMES[k + 2], front),
                    ],
                )?);
            }
        }
        for k in 0..TIMES.len() - 1 {
            spatial_controls.push(spatial(
                &trials,
                [
                    locate(&trials, 1., TIMES[k], front),
                    locate(&trials, 1., TIMES[k + 1], front),
                ],
                [
                    locate(&trials, 0.5, TIMES[k], front),
                    locate(&trials, 0.5, TIMES[k + 1], front),
                ],
            )?);
        }
    }
    let diagonal= [locate(&trials,1.,0.001,0.),locate(&trials,0.5,0.00025,0.),locate(&trials,0.25,0.0000625,0.)].map(|i| {
        let t=&trials[i];json!({"trial":i,"capDiffusionScale":100.*t["config"]["stepSeconds"].as_f64().unwrap()/t["config"]["width"].as_f64().unwrap().powi(2),
            "completed":t["completed"],"observation":t["finalObservation"]})
    });
    Ok(
        json!({"reportVersion":"surface-cadence-conditioning-report-1","endSeconds":END,
        "scope":"Unchanged affine-pressure planar wave; temporal/grid conditioning, not genuine non-affine spatial convergence",
        "diagnosticContract":"Richardson acts on component-aware field differences only; never stock, flux input or physical-state correction",
        "semidiscreteContract":"In exact arithmetic this wave is an exact reconstructed semidiscrete trajectory at tested widths with identifiable stencils and inactive cap; no nonzero spatial truncation order can be established from it",
        "safetyContract":"Existing donor and ledger refusal remain; cap*dt/width^2 is a scale proxy, not a reconstructed-operator stability theorem",
        "trials":trials,"temporalControls":temporal_controls,"spatialConditioningControls":spatial_controls,"coupledDiagonal":diagonal}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vector_extrapolation_cancels_only_the_leading_temporal_term() {
        let u = [0., 2., 1.];
        let a = [1., -2., 0.];
        let b = [0.5, 0., 2.];
        let error = |dt: f64| {
            a.iter()
                .zip(b)
                .map(|(a, b)| a * dt + b * dt * dt)
                .collect::<Vec<_>>()
        };
        let r = extrapolate(&error(0.2), &error(0.1)).unwrap();
        let s = extrapolate(&error(0.1), &error(0.05)).unwrap();
        for i in 0..u.len() {
            assert!((r[i] + b[i] * 0.2_f64.powi(2) / 2.).abs() < 1e-15);
            assert!((r[i] - 4. * s[i]).abs() < 1e-15);
        }
        assert!(u[0] + r[0] < 0.); // Keep a nonphysical observation; do not clamp it.
        assert!(extrapolate(&[1.], &[]).is_err());
        assert!(norm(&[f64::MAX], 1.).is_err());
        assert_eq!(ratio(1., 0.), None);
        assert_eq!(ratio(f64::INFINITY, 1.), None);
    }
    fn profile(width: f64, stocks: Vec<Stock>) -> Profile {
        Profile {
            config: Config {
                width,
                initial_front: 0.,
                direction: 1.,
                step_seconds: 0.001,
            },
            errors: vec![0.; stocks.len()],
            stocks,
        }
    }
    #[test]
    fn restriction_uses_volume_components_and_requires_matching_problems() {
        let large = 2_f64.powi(54);
        let c = profile(2., vec![Stock::new(large, 0., f64::MAX).unwrap(); 2]);
        let f = profile(
            1.,
            vec![
                Stock::new(large / 2., 1., f64::MAX).unwrap(),
                Stock::new(large / 2., 0., f64::MAX).unwrap(),
                Stock::new(large / 2., 0., f64::MAX).unwrap(),
                Stock::new(large / 2., 1., f64::MAX).unwrap(),
            ],
        );
        assert_eq!(restricted_difference(&c, &f).unwrap(), vec![-0.5, -0.5]);
        let mut wrong = profile(1., f.stocks.clone());
        wrong.config.direction = -1.;
        assert!(restricted_difference(&c, &wrong).is_err());
        let mut reversed = profile(1., f.stocks.iter().rev().copied().collect());
        reversed.config.direction = 1.;
        assert_eq!(
            restricted_difference(&c, &reversed).unwrap(),
            vec![-0.5, -0.5]
        );
        let asymmetric_coarse = profile(2., vec![Stock::new(large, 1., f64::MAX).unwrap()]);
        let asymmetric_fine = profile(
            1.,
            vec![
                Stock::new(1., 0., f64::MAX).unwrap(),
                Stock::new(large, 0., f64::MAX).unwrap(),
            ],
        );
        assert_eq!(
            restricted_difference(&asymmetric_coarse, &asymmetric_fine).unwrap(),
            vec![0.]
        );
        let asymmetric_reverse =
            profile(1., asymmetric_fine.stocks.iter().rev().copied().collect());
        assert_eq!(
            restricted_difference(&asymmetric_coarse, &asymmetric_reverse).unwrap(),
            vec![0.]
        );
        assert!(
            Profile::from_trial(&json!({"completed":false}))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            temporal(&vec![json!({"completed":false}); 3], [0, 1, 2]).unwrap()["comparison"],
            Value::Null
        );
    }
    #[test]
    fn repeated_small_pair_retains_old_report_and_refuses_mismatched_time() {
        let c = Config {
            width: 1.,
            step_seconds: 0.001,
            initial_front: 0.995,
            direction: 1.,
        };
        let t = paired_evolution::trial(c, Method::PressurePenaltyOne).unwrap();
        let p = Profile::from_trial(&t).unwrap().unwrap();
        assert_eq!(
            norm(&differences(&p, &p).unwrap(), c.width).unwrap()["rmsMeters"],
            0.
        );
        let mut invalid = t.clone();
        invalid["finalObservation"]["timeSeconds"] = json!(0.05);
        assert!(Profile::from_trial(&invalid).is_err());
        let mut mixed = t.clone();
        mixed["config"]["initialFront"] = json!(0.);
        mixed["config"]["stepSeconds"] = json!(0.0005);
        mixed["finalObservation"]["acceptedSteps"] = json!(200);
        let mut fine = t.clone();
        fine["config"]["width"] = json!(0.5);
        fine["finalVolumeHigh"] = json!(vec![0.; 400]);
        fine["finalVolumeLow"] = json!(vec![0.; 400]);
        let mut fine_mixed = fine.clone();
        fine_mixed["config"]["initialFront"] = json!(0.);
        fine_mixed["config"]["stepSeconds"] = json!(0.0005);
        fine_mixed["finalObservation"]["acceptedSteps"] = json!(200);
        assert!(
            spatial(&[t.clone(), mixed, fine, fine_mixed], [0, 1], [2, 3])
                .unwrap_err()
                .contains("same physical front")
        );
        assert_eq!(
            t,
            paired_evolution::trial(c, Method::PressurePenaltyOne).unwrap()
        );
    }
    #[test]
    fn complete_cadence_cohort_retains_refinement_and_reflected_owned_state() {
        let r = report().unwrap();
        let trials = r["trials"].as_array().unwrap();
        assert_eq!(trials.len(), 18);
        for t in trials {
            assert_eq!(t["completed"], true);
            assert_eq!(t["finalObservation"]["timeSeconds"], END);
        }
        for t in r["temporalControls"].as_array().unwrap() {
            let ratio = t["successiveFieldDifferenceRmsRatio"].as_f64().unwrap();
            assert!((1.9..2.2).contains(&ratio));
            assert!(
                t["finePairRichardsonError"]["rmsMeters"].as_f64().unwrap()
                    < t["fineActualError"]["rmsMeters"].as_f64().unwrap()
            );
        }
        for s in r["spatialConditioningControls"].as_array().unwrap() {
            assert!(
                s["richardsonIntergridDifference"]["rmsMeters"]
                    .as_f64()
                    .unwrap()
                    < s["fineTimeIntergridDifference"]["rmsMeters"]
                        .as_f64()
                        .unwrap()
            );
        }
        let diagonal = r["coupledDiagonal"].as_array().unwrap();
        for d in diagonal {
            assert_eq!(d["capDiffusionScale"], 0.1);
            assert_eq!(d["completed"], true);
        }
        for pair in diagonal.windows(2) {
            assert!(
                pair[1]["observation"]["rmsMeanErrorMeters"]
                    .as_f64()
                    .unwrap()
                    < pair[0]["observation"]["rmsMeanErrorMeters"]
                        .as_f64()
                        .unwrap()
            );
        }
        for key in ["finalVolumeHigh", "finalVolumeLow"] {
            for (a, b) in trials[15][key]
                .as_array()
                .unwrap()
                .iter()
                .zip(trials[17][key].as_array().unwrap().iter().rev())
            {
                assert_eq!(a.as_f64().unwrap().to_bits(), b.as_f64().unwrap().to_bits());
            }
        }
        assert_eq!(trials[15]["boundaryInput"], trials[17]["boundaryInput"]);
        assert_eq!(trials[15]["boundaryOutput"], trials[17]["boundaryOutput"]);
    }
}
