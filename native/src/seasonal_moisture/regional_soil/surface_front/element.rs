//! Fixed dry witness with explicit quadrature and time/spatial comparisons.
//! Wall-clock measurements belong to execution logs, not deterministic reports.
use super::super::{surface, surface_element, surface_flow, surface_gradient};
use super::{AXES, CAP, END_SECONDS, RADIUS, THRESHOLDS, native, reference};
use crate::{Surface, moisture_transport::total_mass};
use serde_json::{Value, json};

fn stock_difference(a: &native::NativeRun, b: &native::NativeRun, areas: &[f64]) -> f64 {
    let differences: Vec<_> = a
        .liquid
        .iter()
        .zip(&b.liquid)
        .zip(areas)
        .map(|((a, b), area)| total_mass(&[a.high, a.low, -b.high, -b.low]) / (1000. * area))
        .collect();
    native::rms(&differences, areas)
}

fn comparison(a: &native::NativeRun, b: &native::NativeRun, areas: &[f64]) -> Value {
    let contours: Vec<_> = THRESHOLDS
        .iter()
        .enumerate()
        .map(|(i, threshold)| {
            let ac = &a.result["contours"][i];
            let bc = &b.result["contours"][i];
            // Extrema differences are not a correspondence between crossing
            // points and must not be labelled a maximum contour displacement.
            json!({"thresholdMeters":threshold,
                "secondMinusFirstMinimumSignedErrorMeters":bc["minimumSignedCrossingErrorMeters"].as_f64().unwrap()-ac["minimumSignedCrossingErrorMeters"].as_f64().unwrap(),
                "secondMinusFirstMaximumSignedErrorMeters":bc["maximumSignedCrossingErrorMeters"].as_f64().unwrap()-ac["maximumSignedCrossingErrorMeters"].as_f64().unwrap(),
                "secondMinusFirstMaximumAbsoluteErrorMeters":bc["maximumAbsoluteCrossingErrorMeters"].as_f64().unwrap()-ac["maximumAbsoluteCrossingErrorMeters"].as_f64().unwrap()})
        })
        .collect();
    json!({"areaWeightedEndpointDifferenceRmsMeters":stock_difference(a,b,areas),
        "endpointDifferenceOverSecondReferenceRmsError":stock_difference(a,b,areas)/b.error,
        "secondOverFirstReferenceRmsError":b.error/a.error,"contourExtremaDifferences":contours})
}

pub(super) fn report(mut result: Value) -> Result<Value, String> {
    let controls = result["spherical"].as_array().unwrap();
    let depths: Vec<Vec<f64>> = controls
        .iter()
        .rev()
        .take(3)
        .map(|r| {
            serde_json::from_value(r["finalBandDepthMeters"].clone()).map_err(|e| e.to_string())
        })
        .collect::<Result<_, _>>()?;
    let depths: Vec<_> = depths.into_iter().rev().collect();
    let mesh = Surface::build(5, RADIUS);
    let layout = surface_flow::Layout::from_fields(
        &mesh,
        RADIUS,
        vec![0.; mesh.areas.len()],
        vec![true; mesh.areas.len()],
        0.,
        surface_flow::Settings {
            roughness: reference::ROUGHNESS,
            maximum_diffusivity_square_meters_per_second: CAP,
        },
    )?;
    let prepared = surface_gradient::Prepared::new(&mesh, layout, RADIUS)?;
    let base_steps = (END_SECONDS / (0.9 * prepared.layout().stable_seconds)).ceil() as usize;
    if base_steps > 16384 {
        return Err("Element dry-front cohort exceeds its work bound.".into());
    }
    let mut groups = Vec::new();
    let mut spatial = Vec::new();
    let mut quadrature_controls = Vec::new();
    let mut quadrature_comparisons = Vec::new();
    let mut matched = Vec::new();
    let mut stage_comparisons = Vec::new();
    for axis in AXES {
        let mut previous: Option<(u32, f64)> = None;
        for level in 3..=5 {
            let mesh = Surface::build(level, RADIUS);
            let mut runs = Vec::new();
            for factor in [1, 2, 4] {
                runs.push(native::run_with_element(
                    level,
                    axis,
                    base_steps * factor,
                    &depths,
                    surface::VerificationStage::FullGradient,
                    Some(0),
                )?);
            }
            let differences: Vec<_> = runs
                .windows(2)
                .map(|r| stock_difference(&r[0], &r[1], &mesh.areas))
                .collect();
            if let Some((old_level, old_error)) = previous {
                spatial.push(json!({"axis":axis,"coarseSubdivision":old_level,
                    "fineSubdivision":level,"fineOverCoarseRmsError":runs[2].error/old_error}));
            }
            previous = Some((level, runs[2].error));
            if level == 3 || level == 5 {
                let q1 = native::run_with_element(
                    level,
                    axis,
                    base_steps * 4,
                    &depths,
                    surface::VerificationStage::FullGradient,
                    Some(1),
                )?;
                let mut c = comparison(&runs[2], &q1, &mesh.areas);
                c["axis"] = json!(axis);
                c["subdivision"] = json!(level);
                c["firstQuadratureSubdivisions"] = json!(0);
                c["secondQuadratureSubdivisions"] = json!(1);
                quadrature_comparisons.push(c);
                if level == 3 {
                    let q2 = native::run_with_element(
                        level,
                        axis,
                        base_steps * 4,
                        &depths,
                        surface::VerificationStage::FullGradient,
                        Some(2),
                    )?;
                    let mut c = comparison(&q1, &q2, &mesh.areas);
                    c["axis"] = json!(axis);
                    c["subdivision"] = json!(level);
                    c["firstQuadratureSubdivisions"] = json!(1);
                    c["secondQuadratureSubdivisions"] = json!(2);
                    quadrature_comparisons.push(c);
                    quadrature_controls.push(q2.result);
                }
                quadrature_controls.push(q1.result);
            }
            if level == 5 {
                let old = native::run(
                    level,
                    axis,
                    base_steps * 4,
                    &depths,
                    surface::VerificationStage::FlatIncidentMobility,
                )?;
                let mut c = comparison(&old, &runs[2], &mesh.areas);
                c["axis"] = json!(axis);
                c["subdivision"] = json!(level);
                stage_comparisons.push(c);
                matched.push(old.result);
            }
            groups.push(
                json!({"axis":axis,"subdivision":level,"quadratureSubdivisions":0,
                "timeRefinementRmsDifferencesMeters":differences,
                "successiveTimeDifferenceRatio":differences[1]/differences[0],
                "lastTimeDifferenceOverFineRmsError":differences[1]/runs[2].error,
                "runs":runs.into_iter().map(|r|r.result).collect::<Vec<_>>()}),
            );
        }
    }
    result["reportVersion"] = json!("surface-flat-element-front-report-1");
    result["surfaceModelVersion"] = json!(surface_element::MODEL_VERSION);
    result["scope"] = json!(
        "Unchanged dry-cap witness. Eighteen q=0 trajectories; q=1 at levels 3/5 and q=2 at level 3, finest caller cadence. No fine-mesh q=2 certificate, wet evolution, terrain barriers, seasonal selection or calibration."
    );
    result["mobilityAssembly"] = json!(
        "Positive cap-split triangle integration with frozen affine P1 depth and constant chordal gradient; unchanged spherical stock areas and paired transfers"
    );
    result["baseSteps"] = json!(base_steps);
    result["groups"] = json!(groups);
    result["spatialComparisons"] = json!(spatial);
    result["quadratureControls"] = json!(quadrature_controls);
    result["quadratureComparisons"] = json!(quadrature_comparisons);
    result["matchedIncidentControls"] = json!(matched);
    result["incidentComparisons"] = json!(stage_comparisons);
    result["costMeasure"] = json!(
        "Deterministic coefficient-node upper bounds, not measured evaluations. Timings remain separate; no hardware-independent speed or scaling claim."
    );
    Ok(result)
}
