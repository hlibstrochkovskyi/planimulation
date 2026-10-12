//! Bounded planar rebuilding trial, not a native/seasonal model selection.
//! Exact wave values enter initial means, exterior forcing and observers only.
use super::{faces, moments, reconstruction::POWER, reference};
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock as Stock};
use serde::Serialize;
use serde_json::{Value, json};

const LEFT: f64 = -100.;
const SPEED: f64 = 0.1;
const END: f64 = 0.1;
const LIMIT: f64 = 256. * f64::EPSILON;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    width: f64,
    step_seconds: f64,
    initial_front: f64,
    direction: f64,
}
impl Config {
    fn shape(self) -> Result<(usize, usize), String> {
        let cells = 200. / self.width;
        let steps = END / self.step_seconds;
        if !self.width.is_finite()
            || self.width <= 0.
            || !self.step_seconds.is_finite()
            || self.step_seconds <= 0.
            || !self.initial_front.is_finite()
            || self.initial_front.abs() > 1.
            || ![-1., 1.].contains(&self.direction)
            || !cells.is_finite()
            || cells.fract() != 0.
            || !(2. ..=1024.).contains(&cells)
            || !steps.is_finite()
            || steps.fract() != 0.
            || !(1. ..=2000.).contains(&steps)
        {
            return Err("Invalid bounded planar evolution configuration.".into());
        }
        Ok((cells as usize, steps as usize))
    }
    fn front(self, time: f64) -> f64 {
        self.initial_front + SPEED * time
    }
    fn mean(self, i: usize, time: f64) -> f64 {
        let a = LEFT + i as f64 * self.width;
        let normal = if self.direction > 0. {
            a
        } else {
            -a - self.width
        };
        reference::wave_average(normal, normal + self.width, self.front(time) / SPEED)
    }
    fn boundary(self, x: f64, time: f64) -> f64 {
        let pressure = slope() * (self.front(time) - self.direction * x).max(0.);
        self.direction * SPEED * pressure.powf(POWER)
    }
}
fn slope() -> f64 {
    (7. / 3.) * (SPEED * reference::ROUGHNESS).powi(2)
}
fn zero() -> Stock {
    Stock { high: 0., low: 0. }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum Method {
    PressurePenaltyOne,
    StockBaseline,
}

#[derive(Clone, Debug)]
struct State {
    stocks: Vec<Stock>,
    input: Stock,
    output: Stock,
    accepted: usize,
}
fn initialize(config: Config) -> Result<State, String> {
    let (n, _) = config.shape()?;
    let stocks = (0..n)
        .map(|i| {
            let mean = config.mean(i, 0.);
            let volume = mean * config.width;
            if mean > 0. && volume == 0. {
                return Err("Initial planar volume underflow.".into());
            }
            Stock::new(volume, 0., f64::MAX)
        })
        .collect::<Result<_, _>>()?;
    Ok(State {
        stocks,
        input: zero(),
        output: zero(),
        accepted: 0,
    })
}
/// Project owned components for inference, never replace the physical stock.
fn means(state: &State, width: f64) -> Result<Vec<f64>, String> {
    state
        .stocks
        .iter()
        .map(|s| {
            Stock::new(s.high, s.low, f64::MAX)?;
            let h = total_mass(&[s.high, s.low]) / width;
            if !h.is_finite() || h < 0. || s.high > 0. && h == 0. {
                return Err("Unrepresentable planar stock observation.".into());
            }
            Ok(h)
        })
        .collect()
}
fn fitted(means: &[f64], width: f64) -> Result<Vec<moments::Cell>, String> {
    let cells = moments::cells(means, width)?;
    for (i, c) in cells.iter().enumerate() {
        if let moments::Cell::Unresolved(reason) = c {
            return Err(format!("Reconstruction refused at cell {i}: {reason}"));
        }
    }
    Ok(cells)
}
fn fluxes(
    means: &[f64],
    width: f64,
    method: Method,
    boundary: [f64; 2],
) -> Result<Vec<f64>, String> {
    let parameters = faces::Parameters {
        roughness: reference::ROUGHNESS,
        cap: 100.,
    };
    let cells = match method {
        Method::PressurePenaltyOne => Some(fitted(means, width)?),
        Method::StockBaseline => None,
    };
    let mut result = vec![boundary[0]];
    for i in 0..means.len() - 1 {
        let face = match &cells {
            Some(c) => faces::pressure_face(&c[i], &c[i + 1], width, 1., parameters),
            None => faces::stock_face(means[i], means[i + 1], width, parameters),
        }
        .map_err(|e| format!("Interior face {} refused: {e}", i + 1))?;
        result.push(face.flux);
    }
    result.push(boundary[1]);
    Ok(result)
}
fn inventory_residual(state: &State, initial: &State) -> f64 {
    let mut terms: Vec<_> = state
        .stocks
        .iter()
        .zip(&initial.stocks)
        .flat_map(|(a, b)| [a.high, a.low, -b.high, -b.low])
        .collect();
    terms.extend([
        -state.input.high,
        -state.input.low,
        state.output.high,
        state.output.low,
    ]);
    let initial_total = total_mass(
        &initial
            .stocks
            .iter()
            .flat_map(|s| [s.high, s.low])
            .collect::<Vec<_>>(),
    );
    let scale = initial_total
        .max(state.input.high + state.input.low)
        .max(state.output.high + state.output.low);
    let residual = total_mass(&terms).abs();
    if scale == 0. {
        residual
    } else {
        residual / scale
    }
}

/// Boundary counters and all owners are committed together, after ledger checks.
fn advance(
    state: &State,
    initial: &State,
    config: Config,
    method: Method,
) -> Result<(State, f64, f64), String> {
    let time = state.accepted as f64 * config.step_seconds;
    let observed = means(state, config.width)?;
    let q = fluxes(
        &observed,
        config.width,
        method,
        [config.boundary(LEFT, time), config.boundary(-LEFT, time)],
    )?;
    let step = faces::step_owned(&state.stocks, config.width, &q, config.step_seconds)?;
    let stocks = step
        .volume_high
        .iter()
        .zip(&step.volume_low)
        .map(|(&high, &low)| Stock::new(high, low, f64::MAX))
        .collect::<Result<_, _>>()?;
    let mut proposed = State {
        stocks,
        accepted: state.accepted + 1,
        ..state.clone()
    };
    // Add the actual two exterior amounts separately, not their rounded sum.
    for amount in [
        q[0] * config.step_seconds,
        -q[q.len() - 1] * config.step_seconds,
    ] {
        if amount >= 0. {
            proposed.input.credit(amount)?;
        } else {
            proposed.output.credit(-amount)?;
        }
    }
    let residual = inventory_residual(&proposed, initial);
    if !residual.is_finite() || residual > LIMIT {
        return Err("Cumulative planar ledger exceeds its separate arithmetic bound.".into());
    }
    Ok((
        proposed,
        step.maximum_local_relative_ledger_residual,
        residual,
    ))
}

fn observe(state: &State, initial: &State, config: Config) -> Result<Value, String> {
    let time = state.accepted as f64 * config.step_seconds;
    let observed = means(state, config.width)?;
    let exact: Vec<_> = (0..observed.len()).map(|i| config.mean(i, time)).collect();
    let errors: Vec<_> = observed.iter().zip(&exact).map(|(a, b)| a - b).collect();
    let rms = (total_mass(&errors.iter().map(|e| e * e).collect::<Vec<_>>()) / errors.len() as f64)
        .sqrt();
    let maximum = errors.iter().map(|e| e.abs()).fold(0., f64::max);
    let dry_indices: Vec<_> = initial
        .stocks
        .iter()
        .enumerate()
        .filter_map(|(i, s)| (s.high == 0.).then_some(i))
        .collect();
    let arrival = total_mass(
        &dry_indices
            .iter()
            .flat_map(|&i| [state.stocks[i].high, state.stocks[i].low])
            .collect::<Vec<_>>(),
    );
    let exact_arrival = total_mass(
        &dry_indices
            .iter()
            .map(|&i| exact[i] * config.width)
            .collect::<Vec<_>>(),
    );
    // Discover all roots from actual data first. Truth never chooses a stencil.
    let cells = moments::cells(&observed, config.width)?;
    let unresolved: Vec<_> = cells
        .iter()
        .enumerate()
        .filter_map(|(i, cell)| {
            if let moments::Cell::Unresolved(reason) = cell {
                Some(json!({"cell":i,"reason":reason}))
            } else {
                None
            }
        })
        .collect();
    let contours: Vec<_> = [0., 0.001, 0.01, 0.05].iter().map(|&threshold: &f64| {
        let positions: Vec<_> = cells.iter().enumerate().filter_map(|(i, c)| {
            c.root(threshold, config.width).map(|x| LEFT + i as f64 * config.width + x)
        }).collect();
        let truth = config.direction * (config.front(time) - threshold.powf(1. / POWER) / slope());
        let error = if unresolved.is_empty() && positions.len() == 1 { Some(positions[0] - truth) } else { None };
        json!({"thresholdMeters":threshold,"positionsMeters":positions,"exactPositionMeters":truth,
            "signedUniquePositionErrorMeters":error,"completeReconstruction":unresolved.is_empty()})
    }).collect();
    Ok(
        json!({"acceptedSteps":state.accepted,"timeSeconds":time,"rmsMeanErrorMeters":rms,
        "maximumMeanErrorMeters":maximum,"initiallyDryArrivalSquareMeters":arrival,
        "exactInitiallyDryArrivalSquareMeters":exact_arrival,"signedDryArrivalErrorSquareMeters":arrival-exact_arrival,
        "cumulativeRelativeInventoryResidual":inventory_residual(state, initial),
        "contours":contours,"unresolvedObservationCells":unresolved}),
    )
}

fn trial(config: Config, method: Method) -> Result<Value, String> {
    let (_, steps) = config.shape()?;
    let initial = initialize(config)?;
    let mut state = initial.clone();
    let mut history = vec![observe(&state, &initial, config)?];
    let mut refusal = None;
    let mut local: f64 = 0.;
    let mut cumulative: f64 = 0.;
    let mut first_dry_arrival = None;
    let next_face = (config.initial_front / config.width).ceil() * config.width;
    let exact_arrival_time = (next_face - config.initial_front) / SPEED;
    let observation_steps = [
        1,
        (0.049 / config.step_seconds).round() as usize,
        steps / 2,
        (0.051 / config.step_seconds).round() as usize,
        steps,
    ];
    while state.accepted < steps {
        match advance(&state, &initial, config, method) {
            Ok((next, l, g)) => {
                state = next;
                local = local.max(l);
                cumulative = cumulative.max(g);
            }
            Err(reason) => {
                refusal = Some(
                    json!({"attemptedStep":state.accepted+1,"startTimeSeconds":state.accepted as f64*config.step_seconds,
                    "reason":reason,"candidateCommitted":false}),
                );
                break;
            }
        }
        if first_dry_arrival.is_none()
            && state
                .stocks
                .iter()
                .zip(&initial.stocks)
                .any(|(a, b)| b.high == 0. && a.high > 0.)
        {
            first_dry_arrival = Some(state.accepted as f64 * config.step_seconds);
        }
        if observation_steps.contains(&state.accepted) {
            history.push(observe(&state, &initial, config)?);
        }
    }
    let final_observation = observe(&state, &initial, config)?;
    if history.last().unwrap()["acceptedSteps"] != final_observation["acceptedSteps"] {
        history.push(final_observation.clone());
    }
    Ok(
        json!({"config":config,"method":method,"requestedEndSeconds":END,"requestedSteps":steps,
        "completed":state.accepted==steps,"refusal":refusal,"finalObservation":final_observation,"history":history,
        "maximumLocalRelativeLedgerResidual":local,"maximumCumulativeRelativeInventoryResidual":cumulative,
        "nextInitiallyDryFaceNormalMeters":next_face,"exactDryArrivalOnsetSeconds":exact_arrival_time,
        "firstRepresentableDryArrivalSeconds":first_dry_arrival,
        "initialVolumeHigh":initial.stocks.iter().map(|s|s.high).collect::<Vec<_>>(),
        "finalVolumeHigh":state.stocks.iter().map(|s|s.high).collect::<Vec<_>>(),
        "finalVolumeLow":state.stocks.iter().map(|s|s.low).collect::<Vec<_>>(),
        "boundaryInput":{"high":state.input.high,"low":state.input.low},
        "boundaryOutput":{"high":state.output.high,"low":state.output.low}}),
    )
}

fn refinement(cases: &[Value], field: &str, coarse: f64, fine: f64) -> Vec<Value> {
    cases.iter().enumerate().filter(|(_, c)| c["config"][field] == coarse).map(|(i, a)| {
        let mut matched_config = a["config"].clone();
        matched_config[field] = json!(fine);
        let (j, b) = cases.iter().enumerate().find(|(_, b)| b["config"] == matched_config && b["method"] == a["method"]).unwrap();
        let comparable = a["completed"] == true && b["completed"] == true;
        let ratio = |key: &str| {
            let x = a["finalObservation"][key].as_f64().unwrap().abs();
            let y = b["finalObservation"][key].as_f64().unwrap().abs();
            (comparable && y > 0.).then(|| x/y)
        };
        let contours: Vec<_> = a["finalObservation"]["contours"].as_array().unwrap().iter()
            .zip(b["finalObservation"]["contours"].as_array().unwrap()).map(|(x,y)| {
                let errors = x["signedUniquePositionErrorMeters"].as_f64().zip(y["signedUniquePositionErrorMeters"].as_f64());
                let ratio = errors.and_then(|(x,y)| (comparable && y!=0.).then(|| x.abs()/y.abs()));
                json!({"thresholdMeters":x["thresholdMeters"],"absoluteErrorCoarseOverFine":ratio})
            }).collect();
        json!({"coarseCase":i,"fineCase":j,"completedAtSameEndTime":comparable,
            "rmsMeanErrorCoarseOverFine":ratio("rmsMeanErrorMeters"),
            "absoluteDryArrivalErrorCoarseOverFine":ratio("signedDryArrivalErrorSquareMeters"),"contours":contours})
    }).collect()
}

pub(super) fn report() -> Result<Value, String> {
    let mut cases = Vec::new();
    for width in [1., 0.5] {
        // Fixed physical fronts, not fixed cell phases, permit matched refinement.
        for initial_front in [0., 0.4, 0.995] {
            for direction in [1., -1.] {
                for step_seconds in [0.001, 0.0005] {
                    for method in [Method::PressurePenaltyOne, Method::StockBaseline] {
                        cases.push(trial(
                            Config {
                                width,
                                initial_front,
                                direction,
                                step_seconds,
                            },
                            method,
                        )?);
                    }
                }
            }
        }
    }
    let temporal = refinement(&cases, "stepSeconds", 0.001, 0.0005);
    let spatial = refinement(&cases, "width", 1., 0.5);
    Ok(
        json!({"reportVersion":"surface-paired-evolution-report-1","scope":"Bounded flat-line rebuilding trial; no spherical, seasonal or desktop selection",
        "domainMeters":[LEFT,-LEFT],"roughness":reference::ROUGHNESS,"capSquareMetersPerSecond":100.,
        "speedMetersPerSecond":SPEED,"endSeconds":END,"pressurePenalty":1.,
        "cohortContract":"Original fronts 0/.4 retained; .995 is a separate delayed crossing at the common face 1 m",
        "stateContract":"Persist normalized high/low stocks; rounded mean projections do not replace them",
        "boundaryContract":"Exact instantaneous exterior flux at each old time; explicit Euler, not exact time-integrated forcing",
        "refusalContract":"No film, slope floor, support snapping, resized request or stock fallback; retain last accepted state",
        "observationContract":"All cells retained; roots discovered before truth; missing/nonunique/incomplete roots have null error",
        "ledgerRelativeArithmeticLimit":LIMIT,"cases":cases,"temporalRefinement":temporal,"spatialRefinement":spatial}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(direction: f64) -> Config {
        Config {
            width: 1.,
            step_seconds: 0.001,
            initial_front: 0.4,
            direction,
        }
    }
    fn bits(s: &State) -> Vec<u64> {
        s.stocks
            .iter()
            .flat_map(|s| [s.high.to_bits(), s.low.to_bits()])
            .chain([
                s.input.high.to_bits(),
                s.input.low.to_bits(),
                s.output.high.to_bits(),
                s.output.low.to_bits(),
                s.accepted as u64,
            ])
            .collect()
    }
    #[test]
    fn owned_rebuilding_is_repeatable_and_in_memory_replay_is_exact() {
        let c = config(1.);
        let initial = initialize(c).unwrap();
        let first = advance(&initial, &initial, c, Method::PressurePenaltyOne)
            .unwrap()
            .0;
        let checkpoint = first.clone();
        let a = advance(&first, &initial, c, Method::PressurePenaltyOne)
            .unwrap()
            .0;
        let b = advance(&checkpoint, &initial, c, Method::PressurePenaltyOne)
            .unwrap()
            .0;
        assert_eq!(bits(&a), bits(&b));
        assert!(a.stocks.iter().any(|s| s.low != 0.));
        assert_eq!(
            trial(c, Method::PressurePenaltyOne).unwrap(),
            trial(c, Method::PressurePenaltyOne).unwrap()
        );
        assert!(Config { width: 0., ..c }.shape().is_err());
        assert!(
            Config {
                step_seconds: 0.00001,
                ..c
            }
            .shape()
            .is_err()
        );
    }
    #[test]
    fn failed_rebuilding_preserves_owners_counters_and_time() {
        let c = config(1.);
        let mut initial = initialize(c).unwrap();
        initial.stocks[3] = Stock::new(0.5, 0., f64::MAX).unwrap();
        let before = bits(&initial);
        assert!(
            advance(&initial, &initial, c, Method::PressurePenaltyOne)
                .unwrap_err()
                .contains("stencil")
        );
        assert_eq!(bits(&initial), before);
        let physical_initial = initialize(c).unwrap();
        let mut saturated_counter = physical_initial.clone();
        saturated_counter.input = Stock::new(f64::MAX, 0., f64::MAX).unwrap();
        let before = bits(&saturated_counter);
        assert!(
            advance(
                &saturated_counter,
                &physical_initial,
                c,
                Method::PressurePenaltyOne
            )
            .unwrap_err()
            .contains("stock")
        );
        assert_eq!(bits(&saturated_counter), before);
        let mut counter = Stock::new(2_f64.powi(54), 0., f64::MAX).unwrap();
        for _ in 0..7 {
            counter.credit(1.).unwrap();
        }
        assert_eq!(
            counter.high as i128 + counter.low as i128,
            (1_i128 << 54) + 7
        );
    }
    #[test]
    fn reflected_bounded_trajectory_keeps_paired_stocks_and_boundary_signs() {
        let a = trial(config(1.), Method::PressurePenaltyOne).unwrap();
        let b = trial(config(-1.), Method::PressurePenaltyOne).unwrap();
        assert_eq!(
            a["finalObservation"]["acceptedSteps"],
            b["finalObservation"]["acceptedSteps"]
        );
        for key in ["finalVolumeHigh", "finalVolumeLow"] {
            let av = a[key].as_array().unwrap();
            let bv = b[key].as_array().unwrap();
            for (x, y) in av.iter().zip(bv.iter().rev()) {
                assert_eq!(x.as_f64().unwrap().to_bits(), y.as_f64().unwrap().to_bits());
            }
        }
        assert_eq!(a["boundaryInput"], b["boundaryInput"]);
        assert_eq!(a["boundaryOutput"], b["boundaryOutput"]);
    }
    #[test]
    fn bounded_cohort_accounts_for_every_accepted_owner_and_retains_refusals() {
        let r = report().unwrap();
        assert_eq!(r["cases"].as_array().unwrap().len(), 48);
        for c in r["cases"].as_array().unwrap() {
            if c["method"] == "pressurePenaltyOne" {
                assert_eq!(c["completed"], true);
                assert_eq!(c["finalObservation"]["timeSeconds"], END);
                assert!(
                    c["finalObservation"]["unresolvedObservationCells"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                );
                assert!(
                    c["finalObservation"]["rmsMeanErrorMeters"]
                        .as_f64()
                        .unwrap()
                        < 2e-8
                );
                for contour in c["finalObservation"]["contours"].as_array().unwrap() {
                    assert_eq!(contour["positionsMeters"].as_array().unwrap().len(), 1);
                    // Separate follow-up crossing control; do not relax the
                    // original 0/.4-front regression bounds to accommodate it.
                    let bound = if c["config"]["initialFront"] == 0.995 {
                        1e-4
                    } else {
                        5e-5
                    };
                    assert!(
                        contour["signedUniquePositionErrorMeters"]
                            .as_f64()
                            .unwrap()
                            .abs()
                            < bound
                    );
                }
                if c["config"]["initialFront"] == 0.995 {
                    let dt = c["config"]["stepSeconds"].as_f64().unwrap();
                    let arrival = c["firstRepresentableDryArrivalSeconds"].as_f64().unwrap();
                    assert!((arrival - 0.05 - 2. * dt).abs() < 1e-14);
                    for observation in c["history"].as_array().unwrap() {
                        if observation["timeSeconds"].as_f64().unwrap() <= 0.05 {
                            assert_eq!(observation["initiallyDryArrivalSquareMeters"], 0.);
                        }
                    }
                    assert!(
                        c["finalObservation"]["initiallyDryArrivalSquareMeters"]
                            .as_f64()
                            .unwrap()
                            > 0.
                    );
                }
            } else {
                assert_eq!(c["completed"], false);
                assert_eq!(c["finalObservation"]["acceptedSteps"], 5);
                assert_eq!(c["refusal"]["attemptedStep"], 6);
                assert!(
                    c["refusal"]["reason"]
                        .as_str()
                        .unwrap()
                        .contains("Unrepresentable positive stock flux branch")
                );
            }
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
            if !c["completed"].as_bool().unwrap() {
                assert_eq!(c["refusal"]["candidateCommitted"], false);
            }
        }
        for comparison in r["temporalRefinement"].as_array().unwrap() {
            if comparison["completedAtSameEndTime"] == true {
                let ratio = comparison["rmsMeanErrorCoarseOverFine"].as_f64().unwrap();
                assert!((1.9..2.1).contains(&ratio));
            } else {
                assert!(comparison["rmsMeanErrorCoarseOverFine"].is_null());
            }
        }
    }
}
