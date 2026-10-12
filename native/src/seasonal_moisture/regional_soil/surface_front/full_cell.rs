//! Bounded full-cell propagation gate on the unchanged planar transaction.
use super::paired_evolution::{self as engine, Config, Method, State};
use crate::moisture_transport::total_mass;
use serde_json::{Value, json};

const LEFT: f64 = -32.;
const END: f64 = 11.;
const TIMES: [f64; 3] = [0.005, 0.0025, 0.00125];
const LIMIT: f64 = 256. * f64::EPSILON;

fn shape(config: Config) -> Result<(usize, usize), String> {
    // A separate fixed work contract, not an expansion of the short laboratory.
    if ![1., 0.5].contains(&config.width)
        || !TIMES.contains(&config.step_seconds)
        || config.initial_front != 0.
        || ![-1., 1.].contains(&config.direction)
    {
        return Err("Invalid bounded full-cell configuration.".into());
    }
    let cells = (-2. * LEFT / config.width) as usize;
    let steps = (END / config.step_seconds) as usize;
    if steps > 8800 || cells * steps > 1_126_400 {
        return Err("Full-cell work budget exceeded.".into());
    }
    Ok((cells, steps))
}

/// Whole owners in the normal-coordinate region [face,32]. No front selects them.
fn region_indices(config: Config, cells: usize, face: f64) -> Vec<usize> {
    (0..cells)
        .filter(|&i| {
            let a = LEFT + i as f64 * config.width;
            let normal_left = if config.direction > 0. {
                a
            } else {
                -a - config.width
            };
            normal_left >= face
        })
        .collect()
}
fn region_volume(state: &State, indices: &[usize]) -> f64 {
    total_mass(
        &indices
            .iter()
            .flat_map(|&i| [state.stocks[i].high, state.stocks[i].low])
            .collect::<Vec<_>>(),
    )
}
fn observe(state: &State, initial: &State, config: Config) -> Result<Value, String> {
    let time = state.accepted as f64 * config.step_seconds;
    let mut value = engine::observe_at(state, initial, config, LEFT)?;
    let errors: Vec<_> = state
        .stocks
        .iter()
        .enumerate()
        .map(|(i, s)| {
            total_mass(&[s.high - config.mean_at(LEFT, i, time) * config.width, s.low])
                / config.width
        })
        .collect();
    value["componentAwareMeanError"] = norm(&errors, config.width)?;
    let indices = region_indices(config, state.stocks.len(), 1.);
    let actual = region_volume(state, &indices);
    let exact = total_mass(
        &indices
            .iter()
            .map(|&i| config.mean_at(LEFT, i, time) * config.width)
            .collect::<Vec<_>>(),
    );
    value["commonFaceArrival"] = json!({"normalFaceMeters":1.,"actualVolumeSquareMeters":actual,
        "exactVolumeSquareMeters":exact,"signedVolumeErrorSquareMeters":actual-exact});
    Ok(value)
}
fn norm(values: &[f64], width: f64) -> Result<Value, String> {
    if values.is_empty()
        || values.iter().any(|v| !v.is_finite())
        || !width.is_finite()
        || width <= 0.
    {
        return Err("Invalid full-cell error observation.".into());
    }
    let rms = (total_mass(&values.iter().map(|v| v * v).collect::<Vec<_>>()) / values.len() as f64)
        .sqrt();
    let l1 = width * total_mass(&values.iter().map(|v| v.abs()).collect::<Vec<_>>());
    let signed = width * total_mass(values);
    if !rms.is_finite() || !l1.is_finite() || !signed.is_finite() {
        return Err("Full-cell norm overflow.".into());
    }
    Ok(
        json!({"rmsMeters":rms,"maximumMeters":values.iter().map(|v|v.abs()).fold(0.,f64::max),
        "l1VolumeSquareMeters":l1,"signedVolumeSquareMeters":signed}),
    )
}
fn ratio(a: f64, b: f64) -> Option<f64> {
    let r = a / b;
    (a.is_finite() && b > 0. && b.is_finite() && r.is_finite()).then_some(r)
}

fn trial(config: Config) -> Result<Value, String> {
    let (cells, steps) = shape(config)?;
    let means: Vec<_> = (0..cells).map(|i| config.mean_at(LEFT, i, 0.)).collect();
    let initial = engine::initialize_means(&means, config.width)?;
    let mut state = initial.clone();
    let mut history = vec![observe(&state, &initial, config)?];
    let sample_steps: Vec<_> = [5., 9.9, 9.995, 10., 10.005, 10.1, END]
        .iter()
        .map(|t| (t / config.step_seconds).round() as usize)
        .collect();
    let faces: Vec<_> = if config.width == 0.5 {
        vec![0., 0.5, 1.]
    } else {
        vec![0., 1.]
    };
    let regions: Vec<_> = faces
        .iter()
        .map(|&face| region_indices(config, cells, face))
        .collect();
    let mut arrivals: Vec<Option<f64>> = vec![None; faces.len()];
    let mut first_volumes: Vec<Option<f64>> = vec![None; faces.len()];
    let mut local: f64 = 0.;
    let mut cumulative: f64 = 0.;
    let mut refusal = None;
    while state.accepted < steps {
        let time = state.accepted as f64 * config.step_seconds;
        match engine::advance_for(
            &state,
            &initial,
            config.width,
            config.step_seconds,
            Method::PressurePenaltyOne,
            [config.boundary(LEFT, time), config.boundary(-LEFT, time)],
        ) {
            Ok((next, l, g)) => {
                state = next;
                local = local.max(l);
                cumulative = cumulative.max(g);
            }
            Err(reason) => {
                refusal = Some(
                    json!({"attemptedStep":state.accepted+1,"startTimeSeconds":time,
                    "reason":reason,"candidateCommitted":false}),
                );
                break;
            }
        }
        for ((indices, arrival), volume) in
            regions.iter().zip(&mut arrivals).zip(&mut first_volumes)
        {
            if arrival.is_none() && indices.iter().any(|&i| state.stocks[i].high > 0.) {
                *arrival = Some(state.accepted as f64 * config.step_seconds);
                *volume = Some(region_volume(&state, indices));
            }
        }
        if state.accepted == 1 || sample_steps.contains(&state.accepted) {
            history.push(observe(&state, &initial, config)?);
        }
    }
    let final_observation = observe(&state, &initial, config)?;
    if history.last().unwrap()["acceptedSteps"] != final_observation["acceptedSteps"] {
        history.push(final_observation.clone());
    }
    let crossing: Vec<_> = faces.iter().zip(arrivals).zip(&regions).zip(first_volumes).map(|(((&face,arrival),indices),volume)| {
        let exact = face/0.1;
        json!({"normalFaceMeters":face,"exactOnsetSeconds":exact,"firstRepresentableArrivalSeconds":arrival,
            "firstRepresentableArrivalVolumeSquareMeters":volume,
            "exactVolumeAtFirstArrivalSquareMeters":arrival.map(|time|total_mass(&indices.iter()
                .map(|&i|config.mean_at(LEFT,i,time)*config.width).collect::<Vec<_>>())),
            "signedArrivalLagSeconds":arrival.map(|a|a-exact),
            "terminalVolumeSquareMeters":region_volume(&state,indices)})
    }).collect();
    Ok(
        json!({"config":config,"requestedSteps":steps,"requestedEndSeconds":END,
        "completed":state.accepted==steps,"refusal":refusal,"finalObservation":final_observation,"history":history,
        "crossings":crossing,"maximumLocalRelativeLedgerResidual":local,
        "maximumCumulativeRelativeInventoryResidual":cumulative,
        "initialVolumeHigh":initial.stocks.iter().map(|s|s.high).collect::<Vec<_>>(),
        "finalVolumeHigh":state.stocks.iter().map(|s|s.high).collect::<Vec<_>>(),
        "finalVolumeLow":state.stocks.iter().map(|s|s.low).collect::<Vec<_>>(),
        "boundaryInput":{"high":state.input.high,"low":state.input.low},
        "boundaryOutput":{"high":state.output.high,"low":state.output.low}}),
    )
}

fn temporal(cases: &[Value], indices: [usize; 3]) -> Result<Value, String> {
    let [a, b, c] = indices.map(|i| &cases[i]);
    if [b, c].iter().any(|v| {
        ["width", "initialFront", "direction"]
            .iter()
            .any(|key| v["config"][key] != a["config"][key])
    }) || a["config"]["stepSeconds"].as_f64()
        != b["config"]["stepSeconds"].as_f64().map(|dt| 2. * dt)
        || b["config"]["stepSeconds"].as_f64()
            != c["config"]["stepSeconds"].as_f64().map(|dt| 2. * dt)
    {
        return Err(
            "Full-cell temporal comparison requires one problem and successive time halves.".into(),
        );
    }
    let complete = [a, b, c].iter().all(|v| v["completed"] == true);
    if !complete {
        return Ok(
            json!({"cases":indices,"completedAtSameEndTime":false,"fieldDifferences":null,"rmsDifferenceRatio":null}),
        );
    }
    if [a, b, c].iter().any(|v| {
        v["finalObservation"]["timeSeconds"] != END
            || v["finalObservation"]["acceptedSteps"] != v["requestedSteps"]
            || v["finalVolumeHigh"].as_array().map(|a| a.len())
                != a["finalVolumeHigh"].as_array().map(|a| a.len())
            || v["finalVolumeLow"].as_array().map(|a| a.len())
                != a["finalVolumeHigh"].as_array().map(|a| a.len())
    }) {
        return Err(
            "Full-cell temporal comparison requires matching completed fields and end times."
                .into(),
        );
    }
    let width = a["config"]["width"].as_f64().unwrap();
    let difference = |a: &Value, b: &Value| {
        a["finalVolumeHigh"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, h)| {
                total_mass(&[
                    h.as_f64().unwrap() - b["finalVolumeHigh"][i].as_f64().unwrap(),
                    a["finalVolumeLow"][i].as_f64().unwrap(),
                    -b["finalVolumeLow"][i].as_f64().unwrap(),
                ]) / width
            })
            .collect::<Vec<_>>()
    };
    let ab = norm(&difference(a, b), width)?;
    let bc = norm(&difference(b, c), width)?;
    Ok(
        json!({"cases":indices,"completedAtSameEndTime":true,"fieldDifferences":[ab,bc],
        "rmsDifferenceRatio":ratio(ab["rmsMeters"].as_f64().unwrap(),bc["rmsMeters"].as_f64().unwrap())}),
    )
}
pub(super) fn report() -> Result<Value, String> {
    let mut cases = Vec::new();
    for width in [1., 0.5] {
        for step_seconds in TIMES {
            cases.push(trial(Config {
                width,
                step_seconds,
                initial_front: 0.,
                direction: 1.,
            })?);
        }
    }
    cases.push(trial(Config {
        width: 0.5,
        step_seconds: TIMES[2],
        initial_front: 0.,
        direction: -1.,
    })?);
    let comparisons = [[0, 1, 2], [3, 4, 5]]
        .iter()
        .map(|&indices| temporal(&cases, indices))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        json!({"reportVersion":"surface-full-cell-report-1","scope":"Bounded flat-line full-cell laboratory; no production selection",
        "domainMeters":[LEFT,-LEFT],"endSeconds":END,"speedMetersPerSecond":0.1,"roughness":0.04,
        "capSquareMetersPerSecond":100.,"pressurePenalty":1.,"maximumStepsPerTrial":8800,
        "maximumCellStepsPerTrial":1_126_400,"requestedCohortSteps":39_600,
        "boundaryContract":"Exact instantaneous exterior forcing at old time; not time-integrated flux",
        "refusalContract":"Unchanged strict transaction; no film, floor, support snapping, fallback or resized request",
        "observationContract":"Fixed whole-owner region beyond common face 1 m; all-cell roots found before truth; arrival has no detection floor",
        "ledgerRelativeArithmeticLimit":LIMIT,"cases":cases,"temporalComparisons":comparisons}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface_water::CompensatedStock as Stock;
    #[test]
    fn full_cell_contract_is_separate_and_bounded() {
        let c = Config {
            width: 0.5,
            step_seconds: TIMES[2],
            initial_front: 0.,
            direction: 1.,
        };
        assert_eq!(shape(c).unwrap(), (128, 8800));
        // The same width/dt still describe only .1 s in the legacy contract.
        assert_eq!(c.shape().unwrap(), (400, 80));
        assert!(
            Config {
                step_seconds: 0.00001,
                ..c
            }
            .shape()
            .is_err()
        );
        for bad in [
            Config { width: 0.25, ..c },
            Config {
                step_seconds: 0.0001,
                ..c
            },
            Config {
                initial_front: 0.4,
                ..c
            },
            Config {
                direction: f64::NAN,
                ..c
            },
        ] {
            assert!(shape(bad).is_err());
        }
        for width in [1., 0.5] {
            let c = Config { width, ..c };
            let cells = shape(c).unwrap().0;
            let a = region_indices(c, cells, 1.);
            let b = region_indices(
                Config {
                    direction: -1.,
                    ..c
                },
                cells,
                1.,
            );
            assert_eq!(a, b.iter().rev().map(|i| cells - 1 - i).collect::<Vec<_>>());
            assert_eq!(a.len(), (31. / width) as usize);
        }
    }
    #[test]
    fn full_cell_cohort_retains_completed_or_refused_physical_states() {
        let r = report().unwrap();
        let cases = r["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 7);
        assert_eq!(
            cases
                .iter()
                .map(|c| c["finalObservation"]["acceptedSteps"].as_u64().unwrap())
                .sum::<u64>(),
            39_600
        );
        for c in cases {
            // Outcome pinned after the first fixed cohort, not a permissive
            // assertion that could pass if every trial refused its first step.
            assert_eq!(c["completed"], true);
            assert!(c["maximumLocalRelativeLedgerResidual"].as_f64().unwrap() <= LIMIT);
            assert!(
                c["maximumCumulativeRelativeInventoryResidual"]
                    .as_f64()
                    .unwrap()
                    <= LIMIT
            );
            for (h, l) in c["finalVolumeHigh"]
                .as_array()
                .unwrap()
                .iter()
                .zip(c["finalVolumeLow"].as_array().unwrap())
            {
                Stock::new(h.as_f64().unwrap(), l.as_f64().unwrap(), f64::MAX).unwrap();
            }
            if c["completed"] == true {
                assert_eq!(c["finalObservation"]["timeSeconds"], END);
                assert!(c["refusal"].is_null());
            } else {
                let n = c["finalObservation"]["acceptedSteps"].as_u64().unwrap();
                assert_eq!(c["refusal"]["attemptedStep"], n + 1);
                assert_eq!(c["refusal"]["candidateCommitted"], false);
            }
            assert!(
                c["finalObservation"]["componentAwareMeanError"]["rmsMeters"]
                    .as_f64()
                    .unwrap()
                    < 3e-7
            );
            assert!(
                c["finalObservation"]["unresolvedObservationCells"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            for contour in c["finalObservation"]["contours"].as_array().unwrap() {
                assert_eq!(contour["positionsMeters"].as_array().unwrap().len(), 1);
                assert!(
                    contour["signedUniquePositionErrorMeters"]
                        .as_f64()
                        .unwrap()
                        .abs()
                        < 4e-4
                );
            }
            let arrival = c["crossings"].as_array().unwrap().last().unwrap();
            assert_eq!(arrival["normalFaceMeters"], 1.);
            let lag = arrival["signedArrivalLagSeconds"].as_f64().unwrap();
            assert!(lag > 0. && lag <= 0.011);
            let v = &c["finalObservation"]["commonFaceArrival"];
            let actual = v["actualVolumeSquareMeters"].as_f64().unwrap();
            let exact = v["exactVolumeSquareMeters"].as_f64().unwrap();
            assert!(actual > 0. && actual < exact && (actual - exact).abs() / exact < 0.01);
            for h in c["history"].as_array().unwrap() {
                if h["timeSeconds"].as_f64().unwrap() <= 10. {
                    assert_eq!(h["commonFaceArrival"]["actualVolumeSquareMeters"], 0.);
                }
            }
        }
        let a = &cases[5];
        let b = &cases[6];
        assert_eq!(a["completed"], b["completed"]);
        assert_eq!(
            a["finalObservation"]["acceptedSteps"],
            b["finalObservation"]["acceptedSteps"]
        );
        for key in ["finalVolumeHigh", "finalVolumeLow"] {
            for (x, y) in a[key]
                .as_array()
                .unwrap()
                .iter()
                .zip(b[key].as_array().unwrap().iter().rev())
            {
                assert_eq!(x.as_f64().unwrap().to_bits(), y.as_f64().unwrap().to_bits());
            }
        }
        assert_eq!(a["boundaryInput"], b["boundaryInput"]);
        assert_eq!(a["boundaryOutput"], b["boundaryOutput"]);
        for comparison in r["temporalComparisons"].as_array().unwrap() {
            assert_eq!(comparison["completedAtSameEndTime"], true);
            assert!((1.9..2.2).contains(&comparison["rmsDifferenceRatio"].as_f64().unwrap()));
            let ids = comparison["cases"].as_array().unwrap();
            let errors: Vec<_> = ids
                .iter()
                .map(|i| {
                    cases[i.as_u64().unwrap() as usize]
                ["finalObservation"]["componentAwareMeanError"]["rmsMeters"].as_f64().unwrap()
                })
                .collect();
            assert!(errors[0] > errors[1] && errors[1] > errors[2]);
        }
        for key in ["width", "direction", "initialFront", "stepSeconds"] {
            let mut bad = cases.clone();
            bad[1]["config"][key] = json!(123.);
            assert!(temporal(&bad, [0, 1, 2]).is_err());
        }
        let mut bad = cases.clone();
        bad[1]["finalObservation"]["timeSeconds"] = json!(10.);
        assert!(temporal(&bad, [0, 1, 2]).is_err());
        bad[1]["completed"] = json!(false);
        let comparison = temporal(&bad, [0, 1, 2]).unwrap();
        assert!(comparison["fieldDifferences"].is_null());
        assert!(comparison["rmsDifferenceRatio"].is_null());
    }
    #[test]
    fn full_cell_norm_refuses_invalid_or_unrepresentable_observations() {
        assert!(norm(&[], 1.).is_err());
        assert!(norm(&[f64::NAN], 1.).is_err());
        assert!(norm(&[1.], 0.).is_err());
        assert!(norm(&[f64::MAX], 1.).is_err());
        assert!(ratio(1., 0.).is_none());
        assert_eq!(
            norm(&[-1., 1.], 0.5).unwrap()["signedVolumeSquareMeters"],
            0.
        );
    }
}
