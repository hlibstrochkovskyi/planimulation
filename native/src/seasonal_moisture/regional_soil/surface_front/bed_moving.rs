//! Independent tilted-bed traveling-front trial; no model selection or fallback.
//! Exact values enter initial owners, exterior exchange and observers only.
use super::{
    bed_reference::Reference, faces, head_reconstruction, hypsometry::Bed, paired_evolution,
    topography,
};
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock as Stock};
use serde_json::{Value, json};

const LEFT: f64 = -3.5;
const LENGTH: f64 = 4.;
const END: f64 = 2.;
const SLOPE: f64 = 0.001;
const SPEED: f64 = 0.1;
const ROUGHNESS: f64 = 0.04;
const CAP: f64 = 100.;
const LIMIT: f64 = 256. * f64::EPSILON;

#[derive(Clone, Copy)]
enum Method {
    HydrostaticBaseline,
    AffineHeadSupportedMinmodPenaltyOne,
}
impl Method {
    fn name(self) -> &'static str {
        match self {
            Self::HydrostaticBaseline => "HydrostaticBaseline",
            Self::AffineHeadSupportedMinmodPenaltyOne => "AffineHeadSupportedMinmodPenaltyOne",
        }
    }
}

#[derive(Clone, Copy)]
struct Config {
    width: f64,
    dt: f64,
    direction: f64,
    datum: f64,
    phase: f64,
    method: Method,
}
impl Config {
    fn shape(self) -> Result<(usize, usize), String> {
        let cells = LENGTH / self.width;
        let steps = END / self.dt;
        if !self.width.is_finite()
            || self.width <= 0.
            || !self.dt.is_finite()
            || self.dt <= 0.
            || !self.datum.is_finite()
            || !self.phase.is_finite()
            || self.phase.abs() > 0.125
            || ![-1., 1.].contains(&self.direction)
            || !cells.is_finite()
            || cells.fract() != 0.
            || !(2. ..=128.).contains(&cells)
            || !steps.is_finite()
            || steps.fract() != 0.
            || !(1. ..=4000.).contains(&steps)
            || cells * steps > 128_000.
        {
            return Err("Invalid bounded moving-bed geometry or clock.".into());
        }
        Ok((cells as usize, steps as usize))
    }
    fn left(self) -> f64 {
        if self.direction > 0. {
            LEFT
        } else {
            -LEFT - LENGTH
        }
    }
    fn interval(self, i: usize) -> (f64, f64) {
        let a = self.left() + i as f64 * self.width;
        if self.direction > 0. {
            (a, a + self.width)
        } else {
            (-a - self.width, -a)
        }
    }
    fn beds(self, n: usize) -> Vec<Bed> {
        (0..n)
            .map(|i| {
                let a = self.left() + i as f64 * self.width;
                Bed {
                    left: self.datum - SLOPE * self.direction * a,
                    right: self.datum - SLOPE * self.direction * (a + self.width),
                }
            })
            .collect()
    }
    fn means(self, reference: &Reference, n: usize, time: f64) -> Result<Vec<f64>, String> {
        (0..n)
            .map(|i| {
                let (a, b) = self.interval(i);
                reference.mean(a - self.phase, b - self.phase, time)
            })
            .collect()
    }
}

/// Mean-to-point interpolation is an observer, not a stock reconstruction.
/// Its own exact-mean error is retained separately; no errors are subtracted.
fn contour_positions(means: &[f64], config: Config, threshold: f64) -> Vec<f64> {
    means
        .windows(2)
        .enumerate()
        .filter_map(|(i, h)| {
            if h[0] == h[1]
                || !((h[0] <= threshold && threshold < h[1])
                    || (h[1] <= threshold && threshold < h[0]))
            {
                return None;
            }
            Some(
                config.left()
                    + (i as f64 + 0.5 + (threshold - h[0]) / (h[1] - h[0])) * config.width,
            )
        })
        .collect()
}

fn observe(
    state: &paired_evolution::State,
    initial: &paired_evolution::State,
    config: Config,
    reference: &Reference,
) -> Result<Value, String> {
    let time = state.accepted as f64 * config.dt;
    let means = paired_evolution::means(state, config.width)?;
    let exact = config.means(reference, means.len(), time)?;
    let differences: Vec<_> = state
        .stocks
        .iter()
        .zip(&exact)
        .map(|(stock, truth)| {
            total_mass(&[stock.high - truth * config.width, stock.low]) / config.width
        })
        .collect();
    let rms = (total_mass(&differences.iter().map(|e| e * e).collect::<Vec<_>>())
        / means.len() as f64)
        .sqrt();
    let dry: Vec<_> = initial
        .stocks
        .iter()
        .enumerate()
        .filter_map(|(i, s)| (s.high == 0. && s.low == 0.).then_some(i))
        .collect();
    let arrival = total_mass(
        &dry.iter()
            .flat_map(|&i| [state.stocks[i].high, state.stocks[i].low])
            .collect::<Vec<_>>(),
    );
    let exact_arrival = total_mass(
        &dry.iter()
            .map(|&i| exact[i] * config.width)
            .collect::<Vec<_>>(),
    );
    let contours = [0.001, 0.01, 0.015]
        .into_iter()
        .map(|threshold| {
            let truth = config.direction * (config.phase + reference.contour(threshold, time)?);
            let observed = contour_positions(&means, config, threshold);
            let exact_mean_observer = contour_positions(&exact, config, threshold);
            let combined_error = (observed.len() == 1).then(|| observed[0] - truth);
            let observer_error =
                (exact_mean_observer.len() == 1).then(|| exact_mean_observer[0] - truth);
            Ok(
                json!({"thresholdMeters":threshold,"exactPositionMeters":truth,
            "positionsMeters":observed,"signedUniquePositionErrorMeters":combined_error,
            "exactMeanObserverPositionsMeters":exact_mean_observer,
            "signedUniqueExactMeanObserverErrorMeters":observer_error}),
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(
        json!({"acceptedSteps":state.accepted,"timeSeconds":time,"rmsMeanErrorMeters":rms,
        "maximumMeanErrorMeters":differences.iter().map(|v|v.abs()).fold(0.,f64::max),
        "actualMeansMeters":means,"exactMeansMeters":exact,"contours":contours,
        "initiallyDryArrivalSquareMeters":arrival,"exactInitiallyDryArrivalSquareMeters":exact_arrival,
        "signedInitiallyDryArrivalErrorSquareMeters":arrival-exact_arrival,
        "exactSupportPositionMeters":config.direction*(config.phase+SPEED*time),
        "wetOwnerIndices":state.stocks.iter().enumerate().filter_map(|(i,s)|(s.high>0.).then_some(i)).collect::<Vec<_>>(),
        "supportObserverContract":"Positive owned cells may include numerical tails; no exact support inferred from hydrostatic columns or mean interpolation.",
        "cumulativeRelativeInventoryResidual":paired_evolution::inventory_residual(state,initial)}),
    )
}

fn advance(
    state: &paired_evolution::State,
    initial: &paired_evolution::State,
    beds: &[Bed],
    config: Config,
    boundary: [f64; 2],
) -> Result<(paired_evolution::State, f64, f64, usize, f64), String> {
    let (mut q, capped) = match config.method {
        Method::HydrostaticBaseline => {
            let (q, f) = topography::fluxes(&state.stocks, beds, config.width, CAP)?;
            let capped = f
                .iter()
                .filter(|f| f.flux_square_meters_per_second != 0. && f.capped_branch_selected)
                .count();
            (q, capped)
        }
        Method::AffineHeadSupportedMinmodPenaltyOne => {
            let (q, f) = head_reconstruction::fluxes(&state.stocks, beds, config.width, CAP)?;
            let capped = f
                .iter()
                .filter(|f| f.flux_square_meters_per_second != 0. && f.capped_branch_selected)
                .count();
            (q, capped)
        }
    };
    q[0] = boundary[0];
    let last = q.len() - 1;
    q[last] = boundary[1];
    let maximum = q.iter().map(|v| v.abs()).fold(0., f64::max);
    let step = faces::step_owned(&state.stocks, config.width, &q, config.dt)?;
    let stocks = step
        .volume_high
        .iter()
        .zip(&step.volume_low)
        .map(|(&high, &low)| Stock::new(high, low, f64::MAX))
        .collect::<Result<Vec<_>, _>>()?;
    let mut candidate = paired_evolution::State {
        stocks,
        accepted: state.accepted + 1,
        ..state.clone()
    };
    for amount in [q[0] * config.dt, -q[last] * config.dt] {
        if amount >= 0. {
            candidate.input.credit(amount)?;
        } else {
            candidate.output.credit(-amount)?;
        }
    }
    let residual = paired_evolution::inventory_residual(&candidate, initial);
    if !residual.is_finite() || residual > LIMIT {
        return Err("Moving-bed cumulative ledger exceeds its unchanged arithmetic bound.".into());
    }
    Ok((
        candidate,
        step.maximum_local_relative_ledger_residual,
        step.relative_inventory_residual,
        capped,
        maximum,
    ))
}

fn trial(config: Config) -> Result<Value, String> {
    let (n, steps) = config.shape()?;
    let reference = Reference::new(SLOPE, SPEED, ROUGHNESS)?;
    let initial =
        paired_evolution::initialize_means(&config.means(&reference, n, 0.)?, config.width)?;
    let beds = config.beds(n);
    let mut state = initial.clone();
    let mut local: f64 = 0.;
    let mut global: f64 = 0.;
    let mut cumulative: f64 = 0.;
    let mut maximum_flux: f64 = 0.;
    let mut capped = 0;
    let mut refusal = None;
    let mut history = vec![observe(&state, &initial, config, &reference)?];
    for _ in 0..steps {
        let time = state.accepted as f64 * config.dt;
        let end = (state.accepted + 1) as f64 * config.dt;
        let boundary = [config.left(), config.left() + LENGTH].map(|x| {
            reference
                .boundary_volume(config.direction * x - config.phase, time, end)
                .map(|amount| config.direction * amount / config.dt)
        });
        let boundary = [boundary[0].clone()?, boundary[1].clone()?];
        match advance(&state, &initial, &beds, config, boundary) {
            Ok((next, l, g, c, q)) => {
                local = local.max(l);
                global = global.max(g);
                capped += c;
                maximum_flux = maximum_flux.max(q);
                cumulative = cumulative.max(paired_evolution::inventory_residual(&next, &initial));
                state = next;
                if state.accepted == steps / 2 {
                    history.push(observe(&state, &initial, config, &reference)?);
                }
            }
            Err(reason) => {
                refusal = Some(
                    json!({"attemptedStep":state.accepted+1,"startTimeSeconds":time,
                "candidateCommitted":false,"reason":reason}),
                );
                break;
            }
        }
    }
    history.push(observe(&state, &initial, config, &reference)?);
    let accepted_time = state.accepted as f64 * config.dt;
    let boundary_amounts = [config.left(), config.left() + LENGTH].map(|x| {
        if accepted_time == 0. {
            return Ok(0.);
        }
        reference
            .boundary_volume(config.direction * x - config.phase, 0., accepted_time)
            .map(|amount| config.direction * amount)
    });
    let reference_boundary_input = total_mass(&[
        boundary_amounts[0].clone()?.max(0.),
        (-boundary_amounts[1].clone()?).max(0.),
    ]);
    let reference_boundary_output = total_mass(&[
        (-boundary_amounts[0].clone()?).max(0.),
        boundary_amounts[1].clone()?.max(0.),
    ]);
    let boundary_input_error =
        total_mass(&[state.input.high - reference_boundary_input, state.input.low]);
    let boundary_output_error = total_mass(&[
        state.output.high - reference_boundary_output,
        state.output.low,
    ]);
    Ok(
        json!({"widthMeters":config.width,"stepSeconds":config.dt,"direction":config.direction,
        "datumMeters":config.datum,"physicalDomainMeters":[config.left(),config.left()+LENGTH],
        "initialFrontPhaseMeters":config.phase,
        "operator":config.method.name(),
        "ownerCount":n,"beds":beds,"requestedSteps":steps,"acceptedSteps":state.accepted,
        "completed":state.accepted==steps,"refusal":refusal,"history":history,
        "cappedInteriorFaceEvaluations":capped,"maximumCommittedAbsoluteFluxSquareMetersPerSecond":maximum_flux,
        "maximumLocalRelativeLedgerResidual":local,"maximumStepRelativeInventoryResidual":global,
        "maximumCumulativeRelativeInventoryResidual":cumulative,
        "initialOwnerComponents":initial.stocks.iter().map(|s|[s.high,s.low]).collect::<Vec<_>>(),
        "finalOwnerComponents":state.stocks.iter().map(|s|[s.high,s.low]).collect::<Vec<_>>(),
        "boundaryInputOwnerComponents":[state.input.high,state.input.low],
        "boundaryOutputOwnerComponents":[state.output.high,state.output.low],
        "referenceBoundaryInputSquareMeters":reference_boundary_input,
        "referenceBoundaryOutputSquareMeters":reference_boundary_output,
        "signedBoundaryInputQuadratureAndRoundtripErrorSquareMeters":boundary_input_error,
        "signedBoundaryOutputQuadratureAndRoundtripErrorSquareMeters":boundary_output_error,
        "boundaryAmountContract":"Reference exact interval amounts are divided by dt for the unchanged face transaction; actual q*dt exchanges own the counters. Independently integrated whole-interval boundary discrepancies are retained."}),
    )
}

fn final_owners(trial: &Value) -> Result<Vec<Stock>, String> {
    trial["finalOwnerComponents"]
        .as_array()
        .ok_or("Missing moving-bed owners.")?
        .iter()
        .map(|pair| {
            let high = pair[0].as_f64().ok_or("Invalid moving-bed high owner.")?;
            let low = pair[1].as_f64().ok_or("Invalid moving-bed low owner.")?;
            Stock::new(high, low, f64::MAX)
        })
        .collect()
}

/// Conservative restriction is an observation, never a physical owner rewrite.
fn compare_fields(first: &Value, second: &Value, restriction: usize) -> Result<Value, String> {
    if first["completed"] != true || second["completed"] != true {
        return Ok(
            json!({"comparable":false,"reason":"At least one trajectory refused before common final time."}),
        );
    }
    let a = final_owners(first)?;
    let b = final_owners(second)?;
    if ![1, 2].contains(&restriction) || b.len() != a.len() * restriction {
        return Err("Invalid moving-bed conservative comparison shape.".into());
    }
    let width = first["widthMeters"]
        .as_f64()
        .ok_or("Missing comparison width.")?;
    let observed: Vec<_> = a
        .iter()
        .map(|s| total_mass(&[s.high, s.low]) / width)
        .collect();
    let restricted: Vec<_> = b
        .chunks_exact(restriction)
        .map(|group| {
            total_mass(
                &group
                    .iter()
                    .flat_map(|s| [s.high, s.low])
                    .collect::<Vec<_>>(),
            ) / width
        })
        .collect();
    let difference: Vec<_> = a
        .iter()
        .zip(b.chunks_exact(restriction))
        .map(|(coarse, group)| {
            if restriction == 1 {
                total_mass(&[coarse.high - group[0].high, coarse.low, -group[0].low]) / width
            } else {
                let (large, small) = if group[0].high >= group[1].high {
                    (&group[0], &group[1])
                } else {
                    (&group[1], &group[0])
                };
                total_mass(&[
                    coarse.high - large.high,
                    -small.high,
                    coarse.low,
                    -large.low,
                    -small.low,
                ]) / width
            }
        })
        .collect();
    let rms = (total_mass(&difference.iter().map(|d| d * d).collect::<Vec<_>>())
        / difference.len() as f64)
        .sqrt();
    Ok(json!({"comparable":true,"restrictionFactor":restriction,
        "firstMeansMeters":observed,"secondConservativelyRestrictedMeansMeters":restricted,
        "signedMeanDifferencesMeters":difference,"rmsMeanDifferenceMeters":rms,
        "maximumMeanDifferenceMeters":difference.iter().map(|d|d.abs()).fold(0.,f64::max)}))
}

fn cohort(method: Method) -> Result<Value, String> {
    let reference = Reference::new(SLOPE, SPEED, ROUGHNESS)?;
    let maximum_exact_depth = reference.depth(LEFT - 0.03125, END)?;
    let exact_coefficient = SPEED * maximum_exact_depth.powf(7. / 3.) / (ROUGHNESS * SPEED).powi(2);
    if !exact_coefficient.is_finite() || exact_coefficient >= CAP {
        return Err(
            "Moving-bed exact reference does not retain its uncapped constitutive branch.".into(),
        );
    }
    let mut trials = Vec::new();
    for width in [0.5, 0.25, 0.125] {
        for dt in [0.004, 0.002, 0.001] {
            trials.push(trial(Config {
                width,
                dt,
                direction: 1.,
                datum: 0.,
                phase: 0.,
                method,
            })?);
        }
    }
    trials.push(trial(Config {
        width: 0.125,
        dt: 0.001,
        direction: -1.,
        datum: 0.,
        phase: 0.,
        method,
    })?);
    trials.push(trial(Config {
        width: 0.125,
        dt: 0.001,
        direction: 1.,
        datum: 1024.,
        phase: 0.,
        method,
    })?);
    trials.push(trial(Config {
        width: 0.125,
        dt: 0.001,
        direction: 1.,
        datum: 0.,
        phase: 0.03125,
        method,
    })?);
    for dt in [0.002, 0.001] {
        trials.push(trial(Config {
            width: 0.0625,
            dt,
            direction: 1.,
            datum: 0.,
            phase: 0.,
            method,
        })?);
    }
    let temporal = (0..3)
        .map(|mesh| {
            let index = mesh * 3;
            Ok(json!({"trialIndices":[index,index+1,index+2],
            "firstHalving":compare_fields(&trials[index],&trials[index+1],1)?,
            "secondHalving":compare_fields(&trials[index+1],&trials[index+2],1)?}))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let spatial = [(2, 5), (5, 8), (8, 13)]
        .into_iter()
        .map(|(coarse, fine)| {
            Ok(json!({"trialIndices":[coarse,fine],"sameStepSeconds":0.001,
            "comparison":compare_fields(&trials[coarse],&trials[fine],2)?}))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let normal = final_owners(&trials[8])?;
    let reflected = final_owners(&trials[9])?;
    let reflection_bits = normal
        .iter()
        .zip(reflected.iter().rev())
        .all(|(a, b)| a.high.to_bits() == b.high.to_bits() && a.low.to_bits() == b.low.to_bits());
    let datum = compare_fields(&trials[8], &trials[10], 1)?;
    let reflected_observation = json!({"completed":trials[9]["completed"],"widthMeters":0.125,
        "finalOwnerComponents":reflected.iter().rev().map(|s|[s.high,s.low]).collect::<Vec<_>>()});
    let reflection_field = compare_fields(&trials[8], &reflected_observation, 1)?;
    Ok(
        json!({"reportVersion":"surface-bed-moving-cohort-1","operator":method.name(),
        "slope":SLOPE,"speedMetersPerSecond":SPEED,"roughness":ROUGHNESS,"capSquareMetersPerSecond":CAP,
        "maximumExactReferenceDepthMeters":maximum_exact_depth,
        "maximumExactReferenceConstitutiveCoefficientSquareMetersPerSecond":exact_coefficient,
        "endSeconds":END,"reference":"Source-free traveling Manning wave on bed b=-S*x; q=c*h and H'=S-n²c² H^(-4/3)",
        "referenceUseContract":"Exact means only initialize owners and observe errors; exact time-integrated exterior flux only enters boundary counters. Interior faces use actual owned stocks and bed endpoints.",
        "observer":"Linear interpolation of cell means at centers, with separately retained exact-mean observer controls; not pressure/head reconstruction",
        "limitations":["One planar downhill affine-bed front only, not arbitrary terrain or spherical accuracy",
            "Hydrostatic one-head baseline does not resolve intracell moving profiles",
            "No support-front claim from numerical positive tails",
            "No production, seasonal, checkpoint or desktop selection changes"],"trials":trials,
        "temporalFieldComparisons":temporal,"spatialFieldComparisons":spatial,
        "extraFinestTemporalComparison":{"trialIndices":[12,13],"comparison":compare_fields(&trials[12],&trials[13],1)?},
        "reflectedOwnerComponentsBitIdentical":reflection_bits,
        "reflectedFieldComparison":reflection_field,
        "datumFieldComparison":datum,
        "fieldComparisonContract":"Same accepted final time, actual compensated owner components, conservative integrated-stock restriction. These differences separate cadence and mesh effects; scalar errors alone do not qualify front accuracy."}),
    )
}

pub(super) fn report() -> Result<Value, String> {
    let baseline = cohort(Method::HydrostaticBaseline)?;
    let candidate = cohort(Method::AffineHeadSupportedMinmodPenaltyOne)?;
    let baseline_trials = baseline["trials"]
        .as_array()
        .ok_or("Missing baseline cohort.")?;
    let candidate_trials = candidate["trials"]
        .as_array()
        .ok_or("Missing affine-head cohort.")?;
    if baseline_trials.len() != candidate_trials.len() {
        return Err("Mismatched moving-bed named cohorts.".into());
    }
    let operator_comparisons = baseline_trials
        .iter()
        .zip(candidate_trials)
        .enumerate()
        .map(|(i, (a, b))| {
            if a["initialOwnerComponents"] != b["initialOwnerComponents"]
                || a["beds"] != b["beds"]
                || a["stepSeconds"] != b["stepSeconds"]
            {
                return Err("Moving-bed operator comparison changed fixed inputs.".into());
            }
            Ok(
                json!({"pairedTrialIndex":i,"initialOwnersBitIdentical":true,
            "comparison":compare_fields(a,b,1)?}),
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    let refused = trial(Config {
        width: 0.125,
        dt: 2.,
        direction: 1.,
        datum: 0.,
        phase: 0.,
        method: Method::HydrostaticBaseline,
    })?;
    Ok(
        json!({"reportVersion":"surface-bed-moving-report-1","baseline":baseline,"candidate":candidate,
        "deliberateBaselineOverdraw":refused,
        "operatorFieldComparisons":operator_comparisons,
        "comparisonContract":"Named independent candidates receive identical initial stock means, terrain, cadence and exterior reference exchange. Neither is fallback or production selection."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_bed_work_bounds_and_observer_gaps_are_explicit() {
        let ordinary = Config {
            width: 0.125,
            dt: 0.001,
            direction: 1.,
            datum: 0.,
            phase: 0.,
            method: Method::HydrostaticBaseline,
        };
        assert_eq!(ordinary.shape().unwrap(), (32, 2000));
        for invalid in [
            Config { dt: 0., ..ordinary },
            Config {
                width: 0.01,
                ..ordinary
            },
            Config {
                direction: 0.,
                ..ordinary
            },
            Config {
                datum: f64::NAN,
                ..ordinary
            },
        ] {
            assert!(invalid.shape().is_err());
        }
        assert!(contour_positions(&[0., 0.], ordinary, 0.001).is_empty());
        assert_eq!(
            contour_positions(&[0.02, 0., 0.02], ordinary, 0.01).len(),
            2
        );
    }
    #[test]
    fn moving_bed_field_comparison_retains_sub_ulp_owned_difference() {
        let high = 2_f64.powi(54);
        let first =
            json!({"completed":true,"widthMeters":1.,"finalOwnerComponents":[[high,1.],[high,0.]]});
        let second =
            json!({"completed":true,"widthMeters":1.,"finalOwnerComponents":[[high,0.],[high,0.]]});
        let compared = compare_fields(&first, &second, 1).unwrap();
        assert_eq!(
            compared["firstMeansMeters"],
            compared["secondConservativelyRestrictedMeansMeters"]
        );
        assert_eq!(compared["signedMeanDifferencesMeters"], json!([1., 0.]));
        let restricted = json!({"completed":true,"widthMeters":0.5,
            "finalOwnerComponents":[[high/2.,0.],[high/2.,0.],[high/2.,0.],[high/2.,0.]]});
        assert_eq!(
            compare_fields(&first, &restricted, 2).unwrap()["signedMeanDifferencesMeters"],
            json!([1., 0.])
        );
        let invalid = json!({"completed":true,"widthMeters":1./3.,
            "finalOwnerComponents":[[1.,0.],[1.,0.],[1.,0.],[1.,0.],[1.,0.],[1.,0.]]});
        assert!(compare_fields(&first, &invalid, 3).is_err());
    }
    #[test]
    fn named_moving_bed_cohorts_retain_fixed_forcing_and_bounded_refinement() {
        let result = report().unwrap();
        for name in ["baseline", "candidate"] {
            let cohort = &result[name];
            for trial in cohort["trials"].as_array().unwrap() {
                assert_eq!(trial["completed"], true, "{trial}");
                assert_eq!(trial["cappedInteriorFaceEvaluations"], 0);
                assert!(
                    trial["maximumCumulativeRelativeInventoryResidual"]
                        .as_f64()
                        .unwrap()
                        < LIMIT
                );
            }
            let errors: [f64; 4] = [2, 5, 8, 13].map(|i| {
                cohort["trials"][i]["history"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap()["rmsMeanErrorMeters"]
                    .as_f64()
                    .unwrap()
            });
            assert!(errors.windows(2).all(|pair| pair[0] > pair[1]));
            for temporal in cohort["temporalFieldComparisons"].as_array().unwrap() {
                let a = temporal["firstHalving"]["rmsMeanDifferenceMeters"]
                    .as_f64()
                    .unwrap();
                let b = temporal["secondHalving"]["rmsMeanDifferenceMeters"]
                    .as_f64()
                    .unwrap();
                assert!(b / a > 0.45 && b / a < 0.55);
            }
        }
        for i in 0..14 {
            let baseline = &result["baseline"]["trials"][i];
            let candidate = &result["candidate"]["trials"][i];
            assert_eq!(
                baseline["initialOwnerComponents"],
                candidate["initialOwnerComponents"]
            );
            assert_eq!(
                baseline["boundaryInputOwnerComponents"],
                candidate["boundaryInputOwnerComponents"]
            );
        }
        let fine_error = |name: &str| {
            result[name]["trials"][8]["history"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()["rmsMeanErrorMeters"]
                .as_f64()
                .unwrap()
        };
        assert!(fine_error("candidate") < 0.5 * fine_error("baseline"));
        assert_eq!(result["deliberateBaselineOverdraw"]["acceptedSteps"], 0);
    }
    #[test]
    fn tilted_front_trial_is_conserved_uncapped_and_moves_owned_water() {
        let result = trial(Config {
            width: 0.125,
            dt: 0.001,
            direction: 1.,
            datum: 0.,
            phase: 0.,
            method: Method::HydrostaticBaseline,
        })
        .unwrap();
        assert_eq!(result["completed"], true, "{result}");
        assert_eq!(result["cappedInteriorFaceEvaluations"], 0);
        assert!(
            result["maximumCumulativeRelativeInventoryResidual"]
                .as_f64()
                .unwrap()
                < LIMIT
        );
        assert!(
            result["history"].as_array().unwrap().last().unwrap()["initiallyDryArrivalSquareMeters"]
                .as_f64()
                .unwrap() > 0.
        );
    }
    #[test]
    fn moving_bed_first_refusal_retains_zero_clock_and_original_owners() {
        let refused = trial(Config {
            width: 0.125,
            dt: 2.,
            direction: 1.,
            datum: 0.,
            phase: 0.,
            method: Method::HydrostaticBaseline,
        })
        .unwrap();
        assert_eq!(refused["completed"], false);
        assert_eq!(refused["acceptedSteps"], 0);
        assert_eq!(
            refused["initialOwnerComponents"],
            refused["finalOwnerComponents"]
        );
        assert_eq!(refused["referenceBoundaryInputSquareMeters"], 0.);
        assert_eq!(refused["referenceBoundaryOutputSquareMeters"], 0.);
    }
}
