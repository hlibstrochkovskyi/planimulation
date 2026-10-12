//! Curved-pressure spatial/temporal gate on the unchanged owned transaction.
use super::{barenblatt_reference as truth, faces, moments, paired_evolution as engine};
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock as Stock};
use engine::{Method, State};
use serde::Serialize;
use serde_json::{Value, json};

const END: f64 = 1.;
const TIMES: [f64; 3] = [0.001, 0.0005, 0.00025];
const LIMIT: f64 = 256. * f64::EPSILON;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    width: f64,
    step_seconds: f64,
    direction: f64,
}
impl Config {
    fn shape(self) -> Result<(usize, usize), String> {
        if ![1., 0.5, 0.25].contains(&self.width)
            || !TIMES.contains(&self.step_seconds)
            || ![-1., 1.].contains(&self.direction)
        {
            return Err("Invalid bounded non-affine configuration.".into());
        }
        let cells = (12. / self.width) as usize;
        let steps = (END / self.step_seconds) as usize;
        if cells > 48 || steps > 4000 || cells * steps > 192_000 {
            return Err("Non-affine work limit exceeded.".into());
        }
        Ok((cells, steps))
    }
    fn left(self) -> f64 {
        if self.direction > 0. { 2. } else { -14. }
    }
    fn interval(self, i: usize) -> [f64; 2] {
        let a = self.left() + i as f64 * self.width;
        if self.direction > 0. {
            [a, a + self.width]
        } else {
            [-a - self.width, -a]
        }
    }
    fn mean(self, i: usize, time: f64) -> Result<f64, String> {
        let [a, b] = self.interval(i);
        truth::mean(a, b, time)
    }
    fn boundary(self, time: f64) -> Result<[f64; 2], String> {
        Ok([
            self.direction * truth::flux(self.direction * self.left(), time)?,
            self.direction * truth::flux(self.direction * (self.left() + 12.), time)?,
        ])
    }
    fn indices(self, low: f64, high: f64) -> Vec<usize> {
        (0..(12. / self.width) as usize)
            .filter(|&i| {
                let [a, b] = self.interval(i);
                a >= low && b <= high
            })
            .collect()
    }
}
struct Run {
    config: Config,
    state: State,
    value: Value,
}
fn norm(values: &[f64], width: f64) -> Result<Value, String> {
    if values.is_empty()
        || values.iter().any(|v| !v.is_finite())
        || !width.is_finite()
        || width <= 0.
    {
        return Err("Invalid non-affine norm input.".into());
    }
    let rms = (total_mass(&values.iter().map(|v| v * v).collect::<Vec<_>>()) / values.len() as f64)
        .sqrt();
    let l1 = width * total_mass(&values.iter().map(|v| v.abs()).collect::<Vec<_>>());
    let signed = width * total_mass(values);
    if !rms.is_finite() || !l1.is_finite() || !signed.is_finite() {
        return Err("Non-affine norm overflow.".into());
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
fn volume(state: &State, indices: &[usize]) -> f64 {
    total_mass(
        &indices
            .iter()
            .flat_map(|&i| [state.stocks[i].high, state.stocks[i].low])
            .collect::<Vec<_>>(),
    )
}
fn errors(config: Config, state: &State, time: f64) -> Result<Vec<f64>, String> {
    if state.stocks.len() != config.shape()?.0 {
        return Err("Invalid non-affine observed shape.".into());
    }
    state
        .stocks
        .iter()
        .enumerate()
        .map(|(i, s)| {
            Stock::new(s.high, s.low, f64::MAX)?;
            Ok(total_mass(&[s.high - config.mean(i, time)? * config.width, s.low]) / config.width)
        })
        .collect()
}
fn statistics(config: Config, values: &[f64]) -> Result<Value, String> {
    if values.len() != config.shape()?.0 {
        return Err("Non-affine statistics require every owner.".into());
    }
    let interior: Vec<_> = config.indices(3., 9.).iter().map(|&i| values[i]).collect();
    Ok(
        json!({"fullDomain":norm(values,config.width)?,"fixedWetInterior":norm(&interior,config.width)?}),
    )
}
fn root_observations(
    config: Config,
    means: &[f64],
    time: f64,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    if means.len() != config.shape()?.0 {
        return Err("Root observer requires every non-affine owner.".into());
    }
    // Complete actual reconstruction first: truth does not choose roots/stencils.
    let cells = moments::cells(means, config.width)?;
    let unresolved: Vec<_> = cells
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match c {
            moments::Cell::Unresolved(reason) => Some(json!({"cell":i,"reason":reason})),
            _ => None,
        })
        .collect();
    let discovered: Vec<_> = [0., 0.001, 0.01, 0.05]
        .into_iter()
        .map(|threshold| {
            let positions: Vec<_> = cells
                .iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    c.root(threshold, config.width)
                        .map(|x| config.left() + i as f64 * config.width + x)
                })
                .collect();
            (threshold, positions)
        })
        .collect();
    let contours = discovered.into_iter().map(|(threshold,positions)| {
        let exact = truth::contour(threshold,time)?.map(|x|config.direction*x);
        let error = if unresolved.is_empty() && positions.len()==1 {exact.map(|x|positions[0]-x)} else {None};
        Ok(json!({"thresholdMeters":threshold,"positionsMeters":positions,"exactPositionMeters":exact,
            "completeReconstruction":unresolved.is_empty(),"signedUniquePositionErrorMeters":error}))
    }).collect::<Result<Vec<_>,String>>()?;
    Ok((contours, unresolved))
}
fn observe(config: Config, state: &State, initial: &State) -> Result<Value, String> {
    let time = state.accepted as f64 * config.step_seconds;
    let means = engine::means(state, config.width)?;
    let (contours, unresolved) = root_observations(config, &means, time)?;
    let dry = config.indices(10., 14.);
    let actual = volume(state, &dry);
    let exact = total_mass(
        &dry.iter()
            .map(|&i| config.mean(i, time).map(|v| v * config.width))
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(
        json!({"acceptedSteps":state.accepted,"timeSeconds":time,"meanError":statistics(config,&errors(config,state,time)?)?,
        "commonInitiallyDryArrival":{"normalFaceMeters":10.,"actualVolumeSquareMeters":actual,
            "referenceVolumeSquareMeters":exact,"signedVolumeErrorSquareMeters":actual-exact},
        "contours":contours,"unresolvedObservationCells":unresolved,
        "cumulativeRelativeInventoryResidual":engine::inventory_residual(state,initial)}),
    )
}
fn trial(config: Config) -> Result<Run, String> {
    let (cells, steps) = config.shape()?;
    let means = (0..cells)
        .map(|i| config.mean(i, 0.))
        .collect::<Result<Vec<_>, _>>()?;
    let initial = engine::initialize_means(&means, config.width)?;
    let mut state = initial.clone();
    let mut history = vec![observe(config, &state, &initial)?];
    let dry = config.indices(10., 14.);
    let mut first_arrival = None;
    let mut refusal = None;
    let mut local: f64 = 0.;
    let mut cumulative: f64 = 0.;
    while state.accepted < steps {
        let time = state.accepted as f64 * config.step_seconds;
        match engine::advance_for(
            &state,
            &initial,
            config.width,
            config.step_seconds,
            Method::PressurePenaltyOne,
            config.boundary(time)?,
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
        if first_arrival.is_none() && dry.iter().any(|&i| state.stocks[i].high > 0.) {
            let at = state.accepted as f64 * config.step_seconds;
            first_arrival = Some(
                json!({"timeSeconds":at,"actualVolumeSquareMeters":volume(&state,&dry),
                "referenceVolumeSquareMeters":total_mass(&dry.iter().map(|&i|config.mean(i,at).map(|v|v*config.width)).collect::<Result<Vec<_>,_>>()?)}),
            );
        }
        if state.accepted == 1
            || [steps / 4, steps / 2, 3 * steps / 4, steps].contains(&state.accepted)
        {
            history.push(observe(config, &state, &initial)?);
        }
    }
    let final_observation = observe(config, &state, &initial)?;
    if history.last().unwrap()["acceptedSteps"] != final_observation["acceptedSteps"] {
        history.push(final_observation.clone());
    }
    let value = json!({"config":config,"domainMeters":[config.left(),config.left()+12.],
        "requestedEndSeconds":END,"requestedSteps":steps,"completed":state.accepted==steps,"refusal":refusal,
        "history":history,"finalObservation":final_observation,"firstRepresentableDryArrival":first_arrival,
        "maximumLocalRelativeLedgerResidual":local,"maximumCumulativeRelativeInventoryResidual":cumulative,
        "initialVolumeHigh":initial.stocks.iter().map(|s|s.high).collect::<Vec<_>>(),
        "finalVolumeHigh":state.stocks.iter().map(|s|s.high).collect::<Vec<_>>(),
        "finalVolumeLow":state.stocks.iter().map(|s|s.low).collect::<Vec<_>>(),
        "boundaryInput":{"high":state.input.high,"low":state.input.low},
        "boundaryOutput":{"high":state.output.high,"low":state.output.low}});
    Ok(Run {
        config,
        state,
        value,
    })
}
fn same_problem(a: &Run, b: &Run) -> Result<(), String> {
    if a.config.direction != b.config.direction {
        return Err("Non-affine comparison mixes reflected problems.".into());
    }
    a.config.shape()?;
    b.config.shape()?;
    Ok(())
}
fn completed(run: &Run) -> bool {
    run.state.accepted == (END / run.config.step_seconds) as usize
}
fn difference(a: &Run, b: &Run) -> Result<Vec<f64>, String> {
    same_problem(a, b)?;
    if a.config.width != b.config.width || a.state.stocks.len() != b.state.stocks.len() {
        return Err("Non-affine temporal difference requires one grid.".into());
    }
    let width = a.config.width;
    Ok(a.state
        .stocks
        .iter()
        .zip(&b.state.stocks)
        .map(|(a, b)| total_mass(&[a.high - b.high, a.low, -b.low]) / width)
        .collect())
}
fn temporal(runs: &[Run], ids: [usize; 3]) -> Result<Value, String> {
    let [a, b, c] = ids.map(|i| &runs[i]);
    same_problem(a, b)?;
    same_problem(a, c)?;
    if a.config.width != b.config.width
        || b.config.width != c.config.width
        || a.config.step_seconds != 2. * b.config.step_seconds
        || b.config.step_seconds != 2. * c.config.step_seconds
    {
        return Err("Non-affine temporal comparison requires matched successive halves.".into());
    }
    if ![a, b, c].into_iter().all(completed) {
        return Ok(
            json!({"cases":ids,"completedAtSameEndTime":false,"fieldDifferences":null,"rmsDifferenceRatios":null}),
        );
    }
    let ab = difference(a, b)?;
    let bc = difference(b, c)?;
    let ab = statistics(a.config, &ab)?;
    let bc = statistics(a.config, &bc)?;
    let ratios = ["fullDomain", "fixedWetInterior"].map(|key| {
        (
            key,
            ratio(
                ab[key]["rmsMeters"].as_f64().unwrap(),
                bc[key]["rmsMeters"].as_f64().unwrap(),
            ),
        )
    });
    Ok(
        json!({"cases":ids,"completedAtSameEndTime":true,"fieldDifferences":[ab,bc],"rmsDifferenceRatios":ratios}),
    )
}
fn restricted(coarse: &[Stock], fine: &[Stock], width: f64) -> Result<Vec<f64>, String> {
    if fine.len() != 2 * coarse.len() || coarse.is_empty() || !width.is_finite() || width <= 0. {
        return Err("Invalid non-affine conservative restriction shape.".into());
    }
    coarse
        .iter()
        .zip(fine.as_chunks::<2>().0)
        .map(|(a, pair)| {
            for s in [a, &pair[0], &pair[1]] {
                Stock::new(s.high, s.low, f64::MAX)?;
            }
            let (large, small) = if pair[0].high >= pair[1].high {
                (&pair[0], &pair[1])
            } else {
                (&pair[1], &pair[0])
            };
            Ok(total_mass(&[
                a.high - large.high,
                -small.high,
                a.low,
                -large.low,
                -small.low,
            ]) / width)
        })
        .collect()
}
fn spatial(runs: &[Run], a: usize, b: usize) -> Result<Value, String> {
    let coarse = &runs[a];
    let fine = &runs[b];
    same_problem(coarse, fine)?;
    if coarse.config.width != 2. * fine.config.width
        || coarse.config.step_seconds != fine.config.step_seconds
    {
        return Err("Non-affine spatial comparison requires matched nested grids and time.".into());
    }
    if !completed(coarse) || !completed(fine) {
        return Ok(
            json!({"cases":[a,b],"completedAtSameEndTime":false,"restrictedDifference":null}),
        );
    }
    let field = statistics(
        coarse.config,
        &restricted(
            &coarse.state.stocks,
            &fine.state.stocks,
            coarse.config.width,
        )?,
    )?;
    let ratios: Vec<_> = ["fullDomain", "fixedWetInterior"]
        .into_iter()
        .map(|key| {
            let x = coarse.value["finalObservation"]["meanError"][key]["rmsMeters"]
                .as_f64()
                .unwrap();
            let y = fine.value["finalObservation"]["meanError"][key]["rmsMeters"]
                .as_f64()
                .unwrap();
            json!({"window":key,"coarseOverFineRmsError":ratio(x,y)})
        })
        .collect();
    Ok(
        json!({"cases":[a,b],"completedAtSameEndTime":true,"restrictedDifference":field,"errorRatios":ratios}),
    )
}
fn richardson(runs: &[Run], ids: [usize; 4]) -> Result<Value, String> {
    let [a, b, c, d] = ids.map(|i| &runs[i]);
    same_problem(a, b)?;
    same_problem(a, c)?;
    same_problem(a, d)?;
    if a.config.width != b.config.width
        || c.config.width != d.config.width
        || a.config.width != 2. * c.config.width
        || a.config.step_seconds != c.config.step_seconds
        || b.config.step_seconds != d.config.step_seconds
        || a.config.step_seconds != 2. * b.config.step_seconds
    {
        return Err("Non-affine Richardson restriction mixes grids or time levels.".into());
    }
    if ![a, b, c, d].into_iter().all(completed) {
        return Ok(
            json!({"cases":ids,"completedAtSameEndTime":false,"restrictedExtrapolatedErrorDifference":null}),
        );
    }
    let ea = errors(a.config, &a.state, END)?;
    let eb = errors(b.config, &b.state, END)?;
    let ec = errors(c.config, &c.state, END)?;
    let ed = errors(d.config, &d.state, END)?;
    let coarse: Vec<_> = ea.iter().zip(eb).map(|(a, b)| 2. * b - a).collect();
    let fine: Vec<_> = ec.iter().zip(ed).map(|(a, b)| 2. * b - a).collect();
    let difference: Vec<_> = coarse
        .iter()
        .zip(fine.as_chunks::<2>().0)
        .map(|(a, p)| a - 0.5 * p[0] - 0.5 * p[1])
        .collect();
    Ok(json!({"cases":ids,"completedAtSameEndTime":true,
        "coarseExtrapolatedError":statistics(a.config,&coarse)?,"fineExtrapolatedError":statistics(c.config,&fine)?,
        "restrictedExtrapolatedErrorDifference":statistics(a.config,&difference)?,
        "contract":"Observer-only 2*error(dt/2)-error(dt); never an owned state or solver input"}))
}
fn frozen(config: Config) -> Result<Value, String> {
    let (cells, _) = config.shape()?;
    let means = (0..cells)
        .map(|i| config.mean(i, 0.))
        .collect::<Result<Vec<_>, _>>()?;
    let inferred = match engine::fluxes(
        &means,
        config.width,
        Method::PressurePenaltyOne,
        config.boundary(0.)?,
    ) {
        Ok(flux) => flux,
        Err(reason) => {
            return Ok(
                json!({"widthMeters":config.width,"refusal":{"reason":reason,"candidateCommitted":false},
            "derivativeErrorMetersPerSecond":null,"inferredFluxesSquareMetersPerSecond":null}),
            );
        }
    };
    let actual = faces::rhs(&inferred, config.width)?;
    let exact = (0..cells)
        .map(|i| {
            let [a, b] = config.interval(i);
            Ok((truth::flux(a, 0.)? - truth::flux(b, 0.)?) / config.width)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let errors = actual
        .iter()
        .zip(exact)
        .map(|(a, b)| a - b)
        .collect::<Vec<_>>();
    let mut rates = statistics(config, &errors)?;
    for window in ["fullDomain", "fixedWetInterior"] {
        let fields = rates[window].as_object_mut().unwrap();
        for (old, new) in [
            ("rmsMeters", "rmsMetersPerSecond"),
            ("maximumMeters", "maximumMetersPerSecond"),
            ("l1VolumeSquareMeters", "l1VolumeSquareMetersPerSecond"),
            (
                "signedVolumeSquareMeters",
                "signedVolumeSquareMetersPerSecond",
            ),
        ] {
            let value = fields.remove(old).unwrap();
            fields.insert(new.into(), value);
        }
    }
    Ok(
        json!({"widthMeters":config.width,"refusal":null,"derivativeErrorMetersPerSecond":rates,
        "inferredFluxesSquareMetersPerSecond":inferred}),
    )
}
pub(super) fn report() -> Result<Value, String> {
    let reference = truth::report()?;
    let mut runs = Vec::new();
    let mut frozen_controls = Vec::new();
    for width in [1., 0.5, 0.25] {
        frozen_controls.push(frozen(Config {
            width,
            step_seconds: TIMES[0],
            direction: 1.,
        })?);
        for step_seconds in TIMES {
            runs.push(trial(Config {
                width,
                step_seconds,
                direction: 1.,
            })?);
        }
    }
    runs.push(trial(Config {
        width: 0.25,
        step_seconds: TIMES[2],
        direction: -1.,
    })?);
    let temporal = [[0, 1, 2], [3, 4, 5], [6, 7, 8]]
        .into_iter()
        .map(|ids| temporal(&runs, ids))
        .collect::<Result<Vec<_>, _>>()?;
    let spatial = [(0, 3), (1, 4), (2, 5), (3, 6), (4, 7), (5, 8)]
        .into_iter()
        .map(|(a, b)| spatial(&runs, a, b))
        .collect::<Result<Vec<_>, _>>()?;
    let richardson = [[0, 1, 3, 4], [1, 2, 4, 5], [3, 4, 6, 7], [4, 5, 7, 8]]
        .into_iter()
        .map(|ids| richardson(&runs, ids))
        .collect::<Result<Vec<_>, _>>()?;
    let mut reference_contours = Vec::new();
    for time in [0., END] {
        for width in [1., 0.5, 0.25] {
            let config = Config {
                width,
                step_seconds: TIMES[0],
                direction: 1.,
            };
            let means = (0..config.shape()?.0)
                .map(|i| config.mean(i, time))
                .collect::<Result<Vec<_>, _>>()?;
            let (contours, unresolved) = root_observations(config, &means, time)?;
            reference_contours.push(json!({"timeSeconds":time,"widthMeters":width,"contours":contours,
                "unresolvedObservationCells":unresolved,
                "contract":"Identical observer on independent exact integrated means; not evolved owners or a state/contour correction"}));
        }
    }
    Ok(
        json!({"reportVersion":"surface-non-affine-report-1","scope":"Bounded cubic-pressure laboratory, not production/spherical/seasonal selection",
        "domainNormalMeters":[2.,14.],"fixedInteriorNormalMeters":[3.,9.],"endSeconds":END,
        "requestedCohortSteps":25_000,"requestedCohortCellSteps":780_000,"maximumStepsPerTrial":4000,"maximumCellStepsPerTrial":192_000,
        "roughness":0.04,"capSquareMetersPerSecond":100.,"pressurePenalty":1.,"ledgerRelativeArithmeticLimit":LIMIT,
        "boundaryContract":"Exact instantaneous exterior flux at old time; no analytic interior pressure or slope",
        "stateContract":"Unchanged owned transaction; no floor, film, snapping, fallback or resized request",
        "reference":reference,"frozenControls":frozen_controls,"cases":runs.into_iter().map(|r|r.value).collect::<Vec<_>>(),
        "exactIntegratedMeanContourControls":reference_contours,
        "temporalComparisons":temporal,"spatialComparisons":spatial,"richardsonRestrictionComparisons":richardson}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(width: f64, dt: f64, direction: f64) -> Config {
        Config {
            width,
            step_seconds: dt,
            direction,
        }
    }
    /// Artificial normalized observations, never an evolved physical result.
    fn sample(c: Config) -> Run {
        let (n, steps) = c.shape().unwrap();
        let mut state = engine::initialize_means(&vec![1.; n], c.width).unwrap();
        state.accepted = steps;
        Run {
            config: c,
            state,
            value: json!({}),
        }
    }
    #[test]
    fn non_affine_bounds_geometry_and_initial_ownership_are_explicit() {
        for width in [1., 0.5, 0.25] {
            let c = config(width, TIMES[2], 1.);
            let (n, steps) = c.shape().unwrap();
            assert_eq!(steps, 4000);
            let r = config(width, TIMES[2], -1.);
            for i in 0..n {
                assert_eq!(c.interval(i), r.interval(n - 1 - i));
            }
            for (low, high) in [(3., 9.), (10., 14.)] {
                let a = c.indices(low, high);
                let b = r.indices(low, high);
                assert_eq!(a, b.iter().rev().map(|i| n - 1 - i).collect::<Vec<_>>());
                assert_eq!(a.len(), ((high - low) / width) as usize);
            }
            let initial = engine::initialize_means(
                &(0..n).map(|i| c.mean(i, 0.).unwrap()).collect::<Vec<_>>(),
                width,
            )
            .unwrap();
            assert!(errors(c, &initial, 0.).unwrap().iter().all(|&e| e == 0.));
            assert_eq!(volume(&initial, &c.indices(10., 14.)), 0.);
            assert!(
                frozen(c).unwrap()["derivativeErrorMetersPerSecond"]["fullDomain"]["rmsMetersPerSecond"]
                    .as_f64()
                    .unwrap()
                    > 1e-9
            );
        }
        let c = config(1., TIMES[0], 1.);
        for bad in [
            Config { width: 0.125, ..c },
            Config {
                step_seconds: 0.000125,
                ..c
            },
            Config {
                direction: f64::NAN,
                ..c
            },
        ] {
            assert!(bad.shape().is_err());
        }
        assert!(statistics(c, &[0.]).is_err());
        assert!(norm(&[f64::MAX], 1.).is_err());
        assert!(ratio(1., 0.).is_none());
    }
    #[test]
    fn component_differences_restrict_without_losing_small_owners() {
        let mut a = sample(config(0.5, TIMES[0], 1.));
        let mut b = sample(config(0.5, TIMES[1], 1.));
        b.state.stocks[0] = Stock::new(0.25, 0., f64::MAX).unwrap();
        assert_eq!(difference(&a, &b).unwrap()[0], 0.5);
        a.state.stocks[0] = Stock::new(2_f64.powi(54), 0., f64::MAX).unwrap();
        b.state.stocks[0] = Stock::new(2_f64.powi(54), 1., f64::MAX).unwrap();
        assert_eq!(difference(&a, &b).unwrap()[0], -2.);
        let coarse = [Stock::new(2_f64.powi(54), 0., f64::MAX).unwrap()];
        let fine = [coarse[0], Stock::new(1., 0., f64::MAX).unwrap()];
        assert_eq!(restricted(&coarse, &fine, 0.5).unwrap(), vec![-2.]);
        assert_eq!(
            restricted(&coarse, &[fine[1], fine[0]], 0.5).unwrap(),
            vec![-2.]
        );
        assert!(restricted(&coarse, &fine[..1], 0.5).is_err());
        let mut runs = vec![
            sample(config(0.5, TIMES[0], 1.)),
            sample(config(0.5, TIMES[1], 1.)),
            sample(config(0.5, TIMES[2], 1.)),
        ];
        assert!(temporal(&runs, [0, 1, 2]).unwrap()["rmsDifferenceRatios"][0][1].is_null());
        runs[1].state.accepted -= 1;
        assert!(temporal(&runs, [0, 1, 2]).unwrap()["fieldDifferences"].is_null());
        runs[1].config.direction = -1.;
        assert!(temporal(&runs, [0, 1, 2]).is_err());
        runs[1].config.direction = 1.;
        runs[1].config.width = 1.;
        assert!(temporal(&runs, [0, 1, 2]).is_err());
    }
    #[test]
    fn non_affine_cohort_retains_ledger_and_reflected_terminal_state() {
        let report = report().unwrap();
        let cases = report["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 10);
        assert_eq!(
            cases
                .iter()
                .map(|c| c["finalObservation"]["acceptedSteps"].as_u64().unwrap())
                .sum::<u64>(),
            25_000
        );
        for c in cases {
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
            assert_eq!(c["history"][0]["meanError"]["fullDomain"]["rmsMeters"], 0.);
            let width = c["config"]["width"].as_f64().unwrap();
            let bound = if width == 1. {
                0.00014
            } else if width == 0.5 {
                0.000055
            } else {
                0.000025
            };
            assert!(
                c["finalObservation"]["meanError"]["fullDomain"]["rmsMeters"]
                    .as_f64()
                    .unwrap()
                    < bound
            );
            assert!(
                c["finalObservation"]["unresolvedObservationCells"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            for contour in c["finalObservation"]["contours"].as_array().unwrap() {
                assert_eq!(contour["positionsMeters"].as_array().unwrap().len(), 1);
            }
            if c["completed"] == true {
                assert_eq!(c["finalObservation"]["timeSeconds"], END);
                assert!(c["refusal"].is_null());
            } else {
                assert_eq!(c["refusal"]["candidateCommitted"], false);
                assert_eq!(
                    c["refusal"]["attemptedStep"].as_u64().unwrap(),
                    c["finalObservation"]["acceptedSteps"].as_u64().unwrap() + 1
                );
            }
            for h in c["history"].as_array().unwrap() {
                let complete = h["unresolvedObservationCells"]
                    .as_array()
                    .unwrap()
                    .is_empty();
                for root in h["contours"].as_array().unwrap() {
                    if !complete || root["positionsMeters"].as_array().unwrap().len() != 1 {
                        assert!(root["signedUniquePositionErrorMeters"].is_null());
                    }
                }
            }
        }
        let a = &cases[8];
        let b = &cases[9];
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
        for comparison in report["temporalComparisons"].as_array().unwrap() {
            for pair in comparison["rmsDifferenceRatios"].as_array().unwrap() {
                assert!((1.9..2.1).contains(&pair[1].as_f64().unwrap()));
            }
        }
        for comparison in report["spatialComparisons"].as_array().unwrap() {
            for ratio in comparison["errorRatios"].as_array().unwrap() {
                assert!(ratio["coarseOverFineRmsError"].as_f64().unwrap() > 2.);
            }
        }
        assert_eq!(
            report["exactIntegratedMeanContourControls"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
    }
}
