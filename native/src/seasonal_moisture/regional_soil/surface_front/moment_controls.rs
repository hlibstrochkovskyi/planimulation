//! Fixed-grid frozen stock reconstruction and one-sided face observations.
//! Truth is used only after reconstruction; no caller evolves or repairs stocks.
use super::{
    THRESHOLDS,
    moments::{self, Cell},
    reconstruction::POWER,
    reference,
};
use serde_json::{Value, json};

const LEFT: f64 = -100.;
const SPEED: f64 = 0.1;
fn exact_slope() -> f64 {
    (7. / 3.) * (SPEED * reference::ROUGHNESS).powi(2)
}

fn mean(left: f64, width: f64, direction: f64, front: f64) -> f64 {
    let normal_left = if direction > 0. { left } else { -left - width };
    reference::wave_average(normal_left, normal_left + width, front / SPEED)
}

fn observe(means: &[f64], width: f64, origin: f64) -> Result<(Vec<Cell>, Value), String> {
    let fitted = moments::cells(means, width)?;
    let mut owners = Vec::new();
    for (i, c) in fitted.iter().enumerate() {
        owners.push(match c {
            Cell::Dry=>json!({"cell":i,"status":"dry","inputMeanDepthMeters":means[i]}),
            Cell::Unresolved(reason)=>json!({"cell":i,"status":"unresolved","reason":reason,"inputMeanDepthMeters":means[i]}),
            Cell::Fit {peak,slope,neighbor,residual,pair_residual,pair_peak_difference}=>json!({"cell":i,
                "status":if *slope==0. {"constant"} else {"two-moment-fit"},
                "inputMeanDepthMeters":means[i],"inputVolumePerUnitBreadthSquareMeters":means[i]*width,
                "pressureAtWetEndpoint":peak,"pressureSlope":slope,"neighborCell":neighbor,
                "relativeLocalStockResidual":residual,"maximumRelativePairMomentResidual":pair_residual,
                "ownFitMinusPairPeakPressure":pair_peak_difference,
                "leftEndpointDepthMeters":c.depth(0.,width),"rightEndpointDepthMeters":c.depth(width,width)})
        });
    }
    let mut faces = Vec::new();
    for (i, pair) in fitted.windows(2).enumerate() {
        let hl = pair[0].depth(width, width);
        let hr = pair[1].depth(0., width);
        let ql = pair[0].flux(width, width);
        let qr = pair[1].flux(0., width);
        faces.push(json!({"leftCell":i,"coordinateMeters":origin+(i+1) as f64*width,
            "leftDepthMeters":hl,"rightDepthMeters":hr,"rightMinusLeftDepthMeters":hl.zip(hr).map(|(a,b)|b-a),
            "leftFluxSquareMetersPerSecond":ql,"rightFluxSquareMetersPerSecond":qr,
            "rightMinusLeftFluxSquareMetersPerSecond":ql.zip(qr).map(|(a,b)|b-a),
            "dryOwnerNextToPositiveFaceDepth":matches!(pair[0],Cell::Dry)&&hr.is_some_and(|h|h>0.)
                ||matches!(pair[1],Cell::Dry)&&hl.is_some_and(|h|h>0.)}));
    }
    Ok((fitted, json!({"owners":owners,"faces":faces})))
}

fn wave_case(width: f64, phase: f64, direction: f64) -> Result<Value, String> {
    let n = (200. / width) as usize;
    let front = phase * width;
    let means: Vec<_> = (0..n)
        .map(|i| mean(LEFT + i as f64 * width, width, direction, front))
        .collect();
    let (cells, mut observation) = observe(&means, width, LEFT)?;
    let mut contours = Vec::new();
    for threshold in [0., THRESHOLDS[0], THRESHOLDS[1], THRESHOLDS[2]] {
        // Scan all cells first. No exact contour coordinates choose a stencil.
        let crossings: Vec<_> = cells
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                c.root(threshold, width)
                    .map(|x| json!({"cell":i,"coordinateMeters":LEFT+i as f64*width+x}))
            })
            .collect();
        let truth = direction * (front - threshold.powf(1. / POWER) / exact_slope());
        // Pointwise truth comparison is an observer after reconstruction, not
        // an input to moment inference or root discovery.
        let mut point_errors = Vec::new();
        for (i, c) in cells.iter().enumerate() {
            let left = LEFT + i as f64 * width;
            if truth >= left && truth <= left + width {
                point_errors.push(json!({"cell":i,"depthMeters":c.depth(truth-left,width),
                    "fluxSquareMetersPerSecond":c.flux(truth-left,width),
                    "exactFluxSquareMetersPerSecond":direction*SPEED*threshold,
                    "normalSignedFluxErrorSquareMetersPerSecond":c.flux(truth-left,width).map(|q|direction*q-SPEED*threshold)}));
            }
        }
        let errors: Vec<_> = crossings
            .iter()
            .map(|x| direction * (x["coordinateMeters"].as_f64().unwrap() - truth))
            .collect();
        contours.push(json!({"thresholdMeters":threshold,"exactCoordinateMeters":truth,
            "crossings":crossings,"normalSignedCrossingErrorsMeters":errors,"atExactCoordinate":point_errors}));
    }
    let dry_contacts: Vec<_> = observation["faces"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| {
            matches!(cells[f["leftCell"].as_u64().unwrap() as usize], Cell::Dry)
                != matches!(
                    cells[f["leftCell"].as_u64().unwrap() as usize + 1],
                    Cell::Dry
                )
        })
        .cloned()
        .collect();
    observation["widthMeters"] = json!(width);
    observation["phase"] = json!(phase);
    observation["direction"] = json!(direction);
    observation["contours"] = json!(contours);
    observation["wetDryStockContacts"] = json!(dry_contacts);
    observation["scope"] =
        json!("All fixed-grid stocks reconstructed before truth comparison; no time evolution");
    Ok(observation)
}

fn nonaffine() -> Result<Value, String> {
    let width = 1.;
    let n = 8;
    // Smooth positive, strictly decreasing pressure on [-4,4]. Simpson
    // integration is independent of the affine-pressure moment inverse.
    let left = -4.;
    let panels = 4096;
    let pressure = |x: f64| exact_slope() * (6. - x + 0.025 * x * x);
    let means: Vec<_> = (0..n)
        .map(|i| {
            let a = left + i as f64;
            let h = |j: usize| pressure(a + j as f64 / panels as f64).powf(POWER);
            let interior = (1..panels)
                .map(|j| if j % 2 == 0 { 2. * h(j) } else { 4. * h(j) })
                .sum::<f64>();
            (h(0) + h(panels) + interior) / (3. * panels as f64)
        })
        .collect();
    let (_, mut observed) = observe(&means, width, left)?;
    observed["scenario"] = json!("Smooth non-affine pressure: S*(6-x+0.025*x*x), interval [-4,4]");
    observed["widthMeters"] = json!(width);
    observed["meanIntegrationPanels"] = json!(panels);
    Ok(observed)
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
    let (_, uniform) = observe(&[0.02; 8], 1., 0.)?;
    let (_, dry) = observe(&[0.; 8], 1., 0.)?;
    let (_, ambiguous) = observe(&[0., 0.01, 0., 0.02, 0.01, 0.02, 0.02, 0.02], 1., 0.)?;
    // Tiny partial support can retain both integrated moments yet lose a
    // downstream endpoint in the upstream cell's pressure subtraction.
    let (_, tiny_partial) = observe(&[1., 1e-30, 0.], 1., 0.)?;
    Ok(
        json!({"reportVersion":"surface-moment-reconstruction-report-1",
        "scope":"Frozen equal-width one-dimensional cell-mean inference and face/root observations; no paired transfers, evolution, native spherical integration or model selection",
        "domainMeters":[LEFT,-LEFT],"roughness":reference::ROUGHNESS,"capSquareMetersPerSecond":100.,
        "inputContract":"Manufactured exact integrated cell means, not native point-initialized stocks",
        "inferenceInputs":"Only cell means and common width; orientation and neighboring pair selected from means; no exact slope, speed, front, point sample or truth-selected stencil",
        "momentAnsatz":"Two positive adjacent means identify clipped-affine pressure within this ansatz, not arbitrary physical profiles",
        "faceFluxScope":"Two one-sided constitutive fluxes in one global direction; not a shared conservative exchange",
        "contourScope":"Scan isolated in-cell affine roots in the waves; constant/dry level sets are non-isolated and not represented by Cell::root. Retain missing/multiple roots and exact-float boundary gaps without snapping",
        "waves":waves,"nonAffineControl":nonaffine()?,"tinyPartialControl":tiny_partial,
        "uniformControl":uniform,"dryControl":dry,"ambiguousControl":ambiguous}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_wave_cohort_recovers_positive_contours_and_preserves_boundary_gaps() {
        let a = report().unwrap();
        assert_eq!(a, report().unwrap());
        assert_eq!(a["waves"].as_array().unwrap().len(), 72);
        let mut missing_support = 0;
        for wave in a["waves"].as_array().unwrap() {
            for owner in wave["owners"].as_array().unwrap() {
                assert_ne!(owner["status"], "unresolved");
                if owner["status"] == "two-moment-fit" {
                    assert!(
                        owner["relativeLocalStockResidual"].as_f64().unwrap()
                            <= 256. * f64::EPSILON
                    );
                    assert!(
                        owner["maximumRelativePairMomentResidual"].as_f64().unwrap()
                            <= 256. * f64::EPSILON
                    );
                }
            }
            for contour in wave["contours"].as_array().unwrap() {
                let count = contour["crossings"].as_array().unwrap().len();
                let threshold = contour["thresholdMeters"].as_f64().unwrap();
                if threshold > 0. {
                    assert_eq!(count, 1);
                    assert!(!contour["atExactCoordinate"].as_array().unwrap().is_empty());
                    for point in contour["atExactCoordinate"].as_array().unwrap() {
                        assert!(point["depthMeters"].as_f64().unwrap() > 0.);
                        assert!(
                            point["normalSignedFluxErrorSquareMetersPerSecond"]
                                .as_f64()
                                .unwrap()
                                .abs()
                                < 1e-13
                        );
                    }
                } else {
                    assert!(count <= 1);
                    if count == 0 {
                        assert!(wave["phase"] == 0. || wave["phase"] == 1.);
                        missing_support += 1;
                    }
                }
                for error in contour["normalSignedCrossingErrorsMeters"]
                    .as_array()
                    .unwrap()
                {
                    assert!(error.as_f64().unwrap().abs() < 1e-11);
                }
            }
        }
        // Boundary gaps must remain visible rather than being snapped into a
        // successful support identification. The count is a same-build result.
        assert!(missing_support > 0);
        assert!(
            a["uniformControl"]["owners"]
                .as_array()
                .unwrap()
                .iter()
                .all(|x| x["status"] == "constant")
        );
        assert!(
            a["dryControl"]["owners"]
                .as_array()
                .unwrap()
                .iter()
                .all(|x| x["status"] == "dry")
        );
        assert!(
            a["ambiguousControl"]["owners"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x["status"] == "unresolved")
        );
    }
    #[test]
    fn all_cell_snapshot_repeats_without_truth_selected_stencils() {
        let a = wave_case(1., 0.8, 1.).unwrap();
        let b = wave_case(1., 0.8, 1.).unwrap();
        assert_eq!(a, b);
        assert!(
            a["owners"]
                .as_array()
                .unwrap()
                .iter()
                .all(|x| x["status"] != "unresolved")
        );
        for contour in a["contours"].as_array().unwrap() {
            assert_eq!(contour["crossings"].as_array().unwrap().len(), 1);
            assert!(
                contour["normalSignedCrossingErrorsMeters"][0]
                    .as_f64()
                    .unwrap()
                    .abs()
                    < 1e-9
            );
        }
        let reflected = wave_case(1., 0.8, -1.).unwrap();
        for (a, b) in a["owners"]
            .as_array()
            .unwrap()
            .iter()
            .zip(reflected["owners"].as_array().unwrap().iter().rev())
        {
            assert_eq!(a["status"], b["status"]);
            if let (Some(s), Some(t)) = (a["pressureSlope"].as_f64(), b["pressureSlope"].as_f64()) {
                assert!((s + t).abs() < 1e-14);
            }
        }
    }
    #[test]
    fn exact_local_moments_do_not_hide_non_affine_face_jumps() {
        let control = nonaffine().unwrap();
        let face = &control["faces"][3];
        assert!(face["rightMinusLeftDepthMeters"].as_f64().unwrap().abs() > 4e-5);
        assert!(
            face["rightMinusLeftFluxSquareMetersPerSecond"]
                .as_f64()
                .unwrap()
                .abs()
                > 6e-5
        );
        for owner in control["owners"].as_array().unwrap() {
            assert!(owner["maximumRelativePairMomentResidual"].as_f64().unwrap() < 1e-13);
            assert!(owner["relativeLocalStockResidual"].as_f64().unwrap() < 1e-13);
        }
    }
    #[test]
    fn tiny_positive_pair_moments_do_not_certify_endpoint_arithmetic() {
        let (_, control) = observe(&[1., 1e-30, 0.], 1., 0.).unwrap();
        for owner in &control["owners"].as_array().unwrap()[..2] {
            assert_eq!(owner["status"], "two-moment-fit");
            assert!(
                owner["maximumRelativePairMomentResidual"].as_f64().unwrap() <= 256. * f64::EPSILON
            );
            assert!(owner["relativeLocalStockResidual"].as_f64().unwrap() <= 256. * f64::EPSILON);
        }
        let face = &control["faces"][0];
        assert_eq!(face["leftDepthMeters"], 0.);
        assert!(face["rightDepthMeters"].as_f64().unwrap() > 1e-9);
        // The mismatch remains observable, not clipped or corrected away.
        assert!(
            face["rightMinusLeftFluxSquareMetersPerSecond"]
                .as_f64()
                .unwrap()
                > 0.
        );
        let (_, reflected) = observe(&[0., 1e-30, 1.], 1., 0.).unwrap();
        let reflected_face = &reflected["faces"][1];
        assert_eq!(reflected_face["rightDepthMeters"], 0.);
        assert_eq!(reflected_face["leftDepthMeters"], face["rightDepthMeters"]);
        assert_eq!(
            reflected_face["leftFluxSquareMetersPerSecond"]
                .as_f64()
                .unwrap(),
            -face["rightFluxSquareMetersPerSecond"].as_f64().unwrap()
        );
    }
}
