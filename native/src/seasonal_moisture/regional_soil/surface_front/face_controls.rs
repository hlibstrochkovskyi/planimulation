//! Frozen divergence, single-step accounting and time-truncation observations.
//! Manufactured truth supplies inputs, exterior fluxes and observers only.
use super::{
    faces::{self, Parameters},
    moments,
    reconstruction::POWER,
    reference,
};
use crate::moisture_transport::total_mass;
use serde_json::{Value, json};

const LEFT: f64 = -100.;
const SPEED: f64 = 0.1;
const PENALTIES: [f64; 3] = [0., 1., 2.];
const STEPS: [f64; 2] = [0.001, 0.0005];
fn parameters() -> Parameters {
    Parameters {
        roughness: reference::ROUGHNESS,
        cap: 100.,
    }
}
fn slope() -> f64 {
    (7. / 3.) * (SPEED * reference::ROUGHNESS).powi(2)
}
fn wave_depth(x: f64, front: f64, direction: f64) -> f64 {
    (slope() * (front - direction * x).max(0.)).powf(POWER)
}
fn wave_mean(left: f64, width: f64, front: f64, direction: f64) -> f64 {
    let normal_left = if direction > 0. { left } else { -left - width };
    reference::wave_average(normal_left, normal_left + width, front / SPEED)
}
fn rms(values: &[f64]) -> f64 {
    (total_mass(&values.iter().map(|x| x * x).collect::<Vec<_>>()) / values.len() as f64).sqrt()
}
fn maximum(values: &[f64]) -> f64 {
    values.iter().map(|x| x.abs()).fold(0., f64::max)
}

fn method(
    means: &[f64],
    width: f64,
    exact_fluxes: &[f64],
    penalty: Option<f64>,
    exact_after: Option<&[Vec<f64>]>,
) -> Result<Value, String> {
    if !(2..=1024).contains(&means.len())
        || means.iter().any(|m| !m.is_finite() || *m < 0.)
        || exact_fluxes.len() != means.len() + 1
        || exact_after.is_some_and(|after| {
            after.len() != STEPS.len()
                || after.iter().any(|row| {
                    row.len() != means.len() || row.iter().any(|h| !h.is_finite() || *h < 0.)
                })
        })
    {
        return Err("Invalid frozen method input or observer shapes.".into());
    }
    faces::rhs(exact_fluxes, width)?;
    // Reject the complete reconstructed operator before computing any face.
    // The stock baseline does not require, or silently substitute for, it.
    let fitted = if penalty.is_some() {
        let cells = moments::cells(means, width)?;
        if cells
            .iter()
            .any(|c| matches!(c, moments::Cell::Unresolved(_)))
        {
            return Err("Unresolved owner refuses the complete pressure face operator.".into());
        }
        Some(cells)
    } else {
        None
    };
    let mut interior = Vec::new();
    for i in 0..means.len() - 1 {
        interior.push(match penalty {
            Some(sigma) => faces::pressure_face(
                &fitted.as_ref().unwrap()[i],
                &fitted.as_ref().unwrap()[i + 1],
                width,
                sigma,
                parameters(),
            )?,
            None => faces::stock_face(means[i], means[i + 1], width, parameters())?,
        });
    }
    // Only the TWO exterior faces receive prescribed truth. Interior fluxes
    // never receive exact speed, gradient, profile or final mean.
    let mut fluxes = vec![exact_fluxes[0]];
    fluxes.extend(interior.iter().map(|f| f.flux));
    fluxes.push(*exact_fluxes.last().unwrap());
    let rates = faces::rhs(&fluxes, width)?;
    let exact_rates = faces::rhs(exact_fluxes, width)?;
    let flux_errors: Vec<_> = fluxes
        .iter()
        .zip(exact_fluxes)
        .map(|(a, b)| a - b)
        .collect();
    let rate_errors: Vec<_> = rates.iter().zip(&exact_rates).map(|(a, b)| a - b).collect();
    let mut telescope: Vec<_> = rates.iter().map(|r| r * width).collect();
    telescope.extend([-fluxes[0], *fluxes.last().unwrap()]);
    let dry_rates: Vec<_> = means
        .iter()
        .zip(&rates)
        .filter_map(|(m, r)| (*m == 0.).then_some(*r))
        .collect();
    let mut steps = Vec::new();
    for (k, dt) in STEPS.iter().copied().enumerate() {
        // One independently initialized Euler step, NOT a trajectory or
        // repeated advancement. Immutable inputs survive any refusal.
        let step = faces::step(means, width, &fluxes, dt)?;
        let comparison = exact_after.map(|after| {
            let errors: Vec<_> = after[k].iter().enumerate()
                .map(|(i,h)|total_mass(&[step.volume_high[i],step.volume_low[i],-h*width])/width).collect();
            let true_arrival = total_mass(&after[k].iter().zip(means).filter_map(|(h,m)|(*m==0.).then_some(h*width)).collect::<Vec<_>>());
            let actual_arrival = total_mass(&means.iter().enumerate().filter(|(_,m)|**m==0.)
                .flat_map(|(i,_)|[step.volume_high[i],step.volume_low[i]]).collect::<Vec<_>>());
            json!({"rmsDepthErrorMeters":rms(&errors),"maximumAbsoluteDepthErrorMeters":maximum(&errors),
                "depthErrorsMeters":errors,"exactArrivalInInitiallyDryOwnersSquareMeters":true_arrival,
                "computedArrivalInInitiallyDryOwnersSquareMeters":actual_arrival,
                "arrivalErrorSquareMeters":actual_arrival-true_arrival})
        });
        steps.push(json!({"seconds":dt,"step":step,"exactAfterComparison":comparison}));
    }
    Ok(
        json!({"method":if penalty.is_some() {"pressure-jump-upwind"} else {"stock-depth-upwind"},"penalty":penalty,
        "interiorFaces":interior,"fluxesSquareMetersPerSecond":fluxes,"meanDepthRatesMetersPerSecond":rates,
        "faceFluxErrorsSquareMetersPerSecond":flux_errors,"meanDepthRateErrorsMetersPerSecond":rate_errors,
        "maximumAbsoluteFaceFluxErrorSquareMetersPerSecond":maximum(&flux_errors),
        "rmsMeanDepthRateErrorMetersPerSecond":rms(&rate_errors),"maximumAbsoluteMeanDepthRateErrorMetersPerSecond":maximum(&rate_errors),
        "maximumAbsoluteInitiallyDryOwnerRateMetersPerSecond":maximum(&dry_rates),
        "signedDivergenceTelescopingResidualSquareMetersPerSecond":total_mass(&telescope),
        "steps":steps}),
    )
}

fn wave_case(width: f64, phase: f64, direction: f64) -> Result<Value, String> {
    let n = (200. / width) as usize;
    let front = phase * width;
    let means: Vec<_> = (0..n)
        .map(|i| wave_mean(LEFT + i as f64 * width, width, front, direction))
        .collect();
    let exact_fluxes: Vec<_> = (0..=n)
        .map(|i| direction * SPEED * wave_depth(LEFT + i as f64 * width, front, direction))
        .collect();
    let after: Vec<Vec<f64>> = STEPS
        .iter()
        .map(|dt| {
            (0..n)
                .map(|i| {
                    wave_mean(
                        LEFT + i as f64 * width,
                        width,
                        front + SPEED * dt,
                        direction,
                    )
                })
                .collect()
        })
        .collect();
    let mut methods = vec![method(&means, width, &exact_fluxes, None, Some(&after))?];
    for penalty in PENALTIES {
        methods.push(method(
            &means,
            width,
            &exact_fluxes,
            Some(penalty),
            Some(&after),
        )?);
    }
    Ok(
        json!({"widthMeters":width,"phase":phase,"direction":direction,"initialMeanDepthMeters":means,
        "exactFaceFluxesSquareMetersPerSecond":exact_fluxes,"methods":methods}),
    )
}

fn original_flux(v: f64, s: f64) -> f64 {
    let depth = v.powf(POWER);
    let gradient = POWER * v.powf(POWER - 1.) * s;
    -(depth.powf(5. / 3.) / (reference::ROUGHNESS * gradient.abs().sqrt())).min(parameters().cap)
        * gradient
}
fn nonaffine(width: f64) -> Result<Value, String> {
    let left = -4.;
    let n = (8. / width) as usize;
    let panels = 4096;
    let v = |x: f64| slope() * (6. - x + 0.025 * x * x);
    let s = |x: f64| slope() * (-1. + 0.05 * x);
    let means: Vec<_> = (0..n)
        .map(|i| {
            let a = left + i as f64 * width;
            let h = |j: usize| v(a + width * j as f64 / panels as f64).powf(POWER);
            let interior = (1..panels)
                .map(|j| if j % 2 == 0 { 2. * h(j) } else { 4. * h(j) })
                .sum::<f64>();
            (h(0) + h(panels) + interior) / (3. * panels as f64)
        })
        .collect();
    let exact: Vec<_> = (0..=n)
        .map(|i| {
            let x = left + i as f64 * width;
            original_flux(v(x), s(x))
        })
        .collect();
    let mut methods = vec![method(&means, width, &exact, None, None)?];
    for penalty in PENALTIES {
        methods.push(method(&means, width, &exact, Some(penalty), None)?);
    }
    for result in &mut methods {
        let errors = result["meanDepthRateErrorsMetersPerSecond"]
            .as_array()
            .unwrap();
        let mut interior = Vec::new();
        let mut exterior = Vec::new();
        for (i, error) in errors.iter().enumerate() {
            let a = left + i as f64 * width;
            if a >= -2. && a + width <= 2. {
                interior.push(error.as_f64().unwrap());
            } else {
                exterior.push(error.as_f64().unwrap());
            }
        }
        result["fixedInteriorWindow"] = json!({"domainMeters":[-2.,2.],"cells":interior.len(),
            "rmsMeanDepthRateErrorMetersPerSecond":rms(&interior),"maximumAbsoluteMeanDepthRateErrorMetersPerSecond":maximum(&interior)});
        result["remainingCells"] = json!({"cells":exterior.len(),"rmsMeanDepthRateErrorMetersPerSecond":rms(&exterior),
            "maximumAbsoluteMeanDepthRateErrorMetersPerSecond":maximum(&exterior)});
    }
    Ok(
        json!({"widthMeters":width,"domainMeters":[-4.,4.],"meanIntegrationPanels":panels,
        "initialMeanDepthMeters":means,"exactFaceFluxesSquareMetersPerSecond":exact,"zeroFaceIndex":n/2,
        "methods":methods}),
    )
}

pub(super) fn report() -> Result<Value, String> {
    let mut waves = Vec::new();
    for width in [4., 2., 1., 0.5] {
        for phase in [0., 1. / 1048576., 0.001, 0.05, 0.2, 0.5, 0.8, 0.95, 1.] {
            for direction in [-1., 1.] {
                waves.push(wave_case(width, phase, direction)?);
            }
        }
    }
    let nonaffine: Vec<_> = [1., 0.5, 0.25, 0.125, 0.0625]
        .into_iter()
        .map(nonaffine)
        .collect::<Result<_, _>>()?;
    let controls = [
        ("uniform",vec![0.02;8]),("dry",vec![0.;8]),
        ("tiny-partial",vec![1.,1e-30,0.]),("tiny-partial-reflected",vec![0.,1e-30,1.]),
        ("ambiguous-extremum",vec![0.,0.01,0.]),
    ].into_iter().map(|(name,means)| {
        let exact = vec![0.;means.len()+1];
        let results: Vec<_> = [None,Some(0.),Some(1.),Some(2.)].into_iter().map(|p| match method(&means,1.,&exact,p,None) {
            Ok(mut r)=>{
                // The zero-filled array prescribes CLOSED EXTERIOR faces in
                // these controls; its interior entries are not physical truth.
                for key in ["faceFluxErrorsSquareMetersPerSecond","meanDepthRateErrorsMetersPerSecond",
                    "maximumAbsoluteFaceFluxErrorSquareMetersPerSecond","rmsMeanDepthRateErrorMetersPerSecond",
                    "maximumAbsoluteMeanDepthRateErrorMetersPerSecond"] {
                    r[key]=Value::Null;
                }
                json!({"status":"accepted","result":r})
            },Err(reason)=>json!({"status":"refused","penalty":p,"reason":reason})
        }).collect();
        json!({"name":name,"meansMeters":means,"comparisonScope":"Closed exterior fluxes; no arbitrary interior truth claimed","methods":results})
    }).collect::<Vec<_>>();
    Ok(json!({"reportVersion":"surface-shared-face-report-1",
        "scope":"Frozen one-dimensional shared flux/divergence and independently initialized one-step transfers; no evolving trajectory, native/seasonal integration or production selection",
        "roughness":reference::ROUGHNESS,"capSquareMetersPerSecond":100.,"penalties":PENALTIES,"stepSeconds":STEPS,
        "pressureFaceHypothesis":"G=(sLeft+sRight)/2+sigma*(vRight-vLeft)/width; use zero slope at a dry physical trace, one upwind trace and the original capped pressure flux evaluated at G",
        "sourceDistinction":"New adaptation, not the published DG scheme: its friction denominator uses the unpenalized averaged gradient, whereas this hypothesis uses G",
        "stockBaseline":"Depth secant of cell means with larger/upwind depth, original cap; not an unbiased point reconstruction",
        "truthContract":"Integrated manufactured means, explicit TWO exterior fluxes and post-compute observers only; no interior exact gradient/speed or fitted penalty",
        "oneStepContract":"Frozen forward Euler including frozen exterior fluxes, not exact time-integrated exterior forcing; error against future wave includes boundary/time truncation",
        "accountingContract":"One computed volume per face applied with opposite signs, compensated stocks, complete old-stock outgoing budget before credits; no resizing, clipping, film or repair",
        "accountingRelativeLimit":256.*f64::EPSILON,
        "alignedFrontContract":"Zero instantaneous dry influx can coexist with positive O(dt^(10/7)) future exact arrival; frozen consistency is not a wetting or stability certificate",
        "nonAffineProfile":"v=S*(6-x+0.025*x*x); exact instantaneous flux from original depth-gradient law; no future analytic solution claimed",
        "waves":waves,"nonAffine":nonaffine,"controls":controls}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_wave_fluxes_reflect_and_keep_dry_arrival_separate_from_derivatives() {
        let a = wave_case(1., 0., 1.).unwrap();
        let b = wave_case(1., 0., -1.).unwrap();
        for (a, b) in a["methods"]
            .as_array()
            .unwrap()
            .iter()
            .zip(b["methods"].as_array().unwrap())
        {
            for (q, r) in a["fluxesSquareMetersPerSecond"]
                .as_array()
                .unwrap()
                .iter()
                .zip(
                    b["fluxesSquareMetersPerSecond"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .rev(),
                )
            {
                assert!((q.as_f64().unwrap() + r.as_f64().unwrap()).abs() < 1e-14);
            }
            assert!(
                a["signedDivergenceTelescopingResidualSquareMetersPerSecond"]
                    .as_f64()
                    .unwrap()
                    .abs()
                    < 1e-14
            );
            for (r, s) in a["meanDepthRatesMetersPerSecond"]
                .as_array()
                .unwrap()
                .iter()
                .zip(
                    b["meanDepthRatesMetersPerSecond"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .rev(),
                )
            {
                assert!((r.as_f64().unwrap() - s.as_f64().unwrap()).abs() < 1e-14);
            }
            for (left, right) in a["steps"]
                .as_array()
                .unwrap()
                .iter()
                .zip(b["steps"].as_array().unwrap())
            {
                for field in ["volumeHigh", "volumeLow"] {
                    for (r, s) in left["step"][field]
                        .as_array()
                        .unwrap()
                        .iter()
                        .zip(right["step"][field].as_array().unwrap().iter().rev())
                    {
                        assert!((r.as_f64().unwrap() - s.as_f64().unwrap()).abs() < 1e-14);
                    }
                }
                assert_eq!(
                    left["step"]["boundaryInput"],
                    right["step"]["boundaryInput"]
                );
            }
        }
        let candidate = &a["methods"][2];
        assert!(
            candidate["maximumAbsoluteMeanDepthRateErrorMetersPerSecond"]
                .as_f64()
                .unwrap()
                < 1e-8
        );
        assert!(candidate["steps"][0]["exactAfterComparison"]["exactArrivalInInitiallyDryOwnersSquareMeters"].as_f64().unwrap()>0.);
        assert!(
            candidate["steps"][0]["step"]["relativeInventoryResidual"]
                .as_f64()
                .unwrap()
                < 1e-14
        );
        let flat = method(&[0.02; 8], 1., &[0.; 9], Some(1.), None).unwrap();
        assert!(
            flat["meanDepthRatesMetersPerSecond"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r == &json!(0.))
        );
        for means in [[0.02; 8], [0.; 8]] {
            let control = method(&means, 1., &[0.; 9], Some(1.), None).unwrap();
            for step in control["steps"].as_array().unwrap() {
                assert_eq!(step["step"]["volumeHigh"], json!(means));
                assert_eq!(step["step"]["volumeLow"], json!(vec![0.; 8]));
            }
        }
    }
    #[test]
    fn shared_face_retains_tiny_endpoint_block_and_refuses_unknown_stencils() {
        let tiny = method(&[1., 1e-30, 0.], 1., &[0.; 4], Some(1.), None).unwrap();
        assert_eq!(tiny["interiorFaces"][0]["flux"], 0.);
        assert_eq!(
            tiny["interiorFaces"][0]["donorBlockedPositiveOppositeTrace"],
            true
        );
        assert!(method(&[0., 0.01, 0.], 1., &[0.; 4], Some(1.), None).is_err());
        let reflected = method(&[0., 1e-30, 1.], 1., &[0.; 4], Some(1.), None).unwrap();
        assert_eq!(reflected["interiorFaces"][1]["flux"], 0.);
        assert_eq!(
            reflected["interiorFaces"][1]["donorBlockedPositiveOppositeTrace"],
            true
        );
        assert!(method(&[], 1., &[], None, None).is_err());
        assert!(method(&[1., 1.], 1., &[0.; 4], None, None).is_err());
        assert!(method(&[1., 1.], 1., &[0.; 3], None, Some(&[vec![1.]])).is_err());
    }
    #[test]
    fn shared_complete_cohort_repeats_and_keeps_all_accepted_local_ledgers() {
        let a = report().unwrap();
        assert_eq!(a, report().unwrap());
        assert_eq!(a["waves"].as_array().unwrap().len(), 72);
        assert!(
            a["controls"][2]["methods"][2]["result"]["faceFluxErrorsSquareMetersPerSecond"]
                .is_null()
        );
        for wave in a["waves"].as_array().unwrap() {
            assert_eq!(wave["methods"].as_array().unwrap().len(), 4);
            for result in wave["methods"].as_array().unwrap() {
                assert_eq!(result["steps"].as_array().unwrap().len(), 2);
                if result["penalty"].is_number() {
                    assert!(
                        result["maximumAbsoluteMeanDepthRateErrorMetersPerSecond"]
                            .as_f64()
                            .unwrap()
                            < 1e-8
                    );
                }
                for step in result["steps"].as_array().unwrap() {
                    assert!(
                        step["step"]["maximumLocalRelativeLedgerResidual"]
                            .as_f64()
                            .unwrap()
                            < 256. * f64::EPSILON
                    );
                    assert!(
                        step["step"]["relativeInventoryResidual"].as_f64().unwrap()
                            < 256. * f64::EPSILON
                    );
                    for (high, low) in step["step"]["volumeHigh"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .zip(step["step"]["volumeLow"].as_array().unwrap())
                    {
                        crate::surface_water::CompensatedStock::new(
                            high.as_f64().unwrap(),
                            low.as_f64().unwrap(),
                            f64::MAX,
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    #[test]
    fn shared_smooth_control_separates_interior_accuracy_from_boundary_rates() {
        let mut previous: Option<(f64, f64, f64)> = None;
        for width in [1., 0.5, 0.25, 0.125, 0.0625] {
            let control = nonaffine(width).unwrap();
            let result = &control["methods"][2];
            let face = result["faceFluxErrorsSquareMetersPerSecond"]
                [control["zeroFaceIndex"].as_u64().unwrap() as usize]
                .as_f64()
                .unwrap()
                .abs();
            let interior = result["fixedInteriorWindow"]["rmsMeanDepthRateErrorMetersPerSecond"]
                .as_f64()
                .unwrap();
            let global = result["rmsMeanDepthRateErrorMetersPerSecond"]
                .as_f64()
                .unwrap();
            if let Some((old_face, old_interior, old_global)) = previous {
                assert!((3.8..4.1).contains(&(old_face / face)));
                assert!((3.8..4.1).contains(&(old_interior / interior)));
                assert!((2.6..3.).contains(&(old_global / global)));
            }
            previous = Some((face, interior, global));
        }
    }
}
