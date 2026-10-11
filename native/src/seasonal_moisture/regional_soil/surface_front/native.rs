//! Existing paired stages on fixed continuous dry-cap inputs, not new physics.
use super::super::{
    Mass, ResolutionBudget, surface, surface_centered, surface_flow, surface_gradient,
    surface_incident, surface_verification,
};
use super::{AXES, CAP, END_SECONDS, RADIUS, THRESHOLDS, reference, reference_front};
use crate::{Surface, dot, moisture_transport::total_mass, norm};
use serde_json::{Value, json};

fn rms(values: &[f64], areas: &[f64]) -> f64 {
    (total_mass(
        &values
            .iter()
            .zip(areas)
            .map(|(h, a)| h * h * a)
            .collect::<Vec<_>>(),
    ) / total_mass(areas))
    .sqrt()
}

struct NativeRun {
    liquid: Vec<Mass>,
    result: Value,
    error: f64,
}

fn run(
    level: u32,
    axis: [f64; 3],
    steps: usize,
    references: &[Vec<f64>],
    stage: surface::VerificationStage,
) -> Result<NativeRun, String> {
    let length = norm(axis);
    if !(3..=5).contains(&level)
        || steps == 0
        || steps > 65536
        || references.len() != 3
        || !length.is_finite()
        || length == 0.
    {
        return Err("Unsupported bounded native front controls.".into());
    }
    let axis = axis.map(|v| v / length);
    let mesh = Surface::build(level, RADIUS);
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
    let centered = if stage == surface::VerificationStage::FlatCenteredDepth {
        Some(surface_centered::Prepared::new(&prepared)?)
    } else {
        None
    };
    let layout = prepared.layout();
    let incident = if stage == surface::VerificationStage::FlatIncidentMobility {
        Some(surface_incident::Prepared::new(&prepared)?)
    } else {
        None
    };
    let dt = END_SECONDS / steps as f64;
    if dt > layout.stable_seconds {
        return Err("Native dry-front caller must use one stable substep.".into());
    }
    let p: Vec<_> = mesh.centers.iter().map(|&s| dot(axis, s)).collect();
    let initial: Vec<_> = p
        .iter()
        .zip(&mesh.areas)
        .map(|(p, a)| Mass {
            high: 1000. * a * p.max(0.).powf(3. / 7.),
            low: 0.,
        })
        .collect();
    let mut liquid = initial.clone();
    let mut transfers = vec![Mass::default(); 2 * layout.faces.len()];
    let by_region = vec![None; liquid.len()];
    let mut resolution = Some(ResolutionBudget::default());
    let mut maximum_audit = [0_f64; 2];
    for step in 0..steps {
        let stocks = surface::Stocks {
            by_region: &by_region,
            liquid: &mut liquid,
            bodies: &mut [],
            transfers: &mut transfers,
            resolution: &mut resolution,
        };
        let count = match stage {
            surface::VerificationStage::LegacyDirectional => surface::advance(layout, stocks, dt)?,
            surface::VerificationStage::FullGradient => {
                surface::advance_full_gradient(&prepared, stocks, dt)?
            }
            surface::VerificationStage::FlatCenteredDepth => {
                centered.as_ref().unwrap().advance(stocks, dt)?
            }
            surface::VerificationStage::FlatIncidentMobility => {
                incident.as_ref().unwrap().advance(stocks, dt)?
            }
        };
        if count != 1 {
            return Err("Unexpected native dry-front substep split.".into());
        }
        for (m, a) in liquid.iter().zip(&mesh.areas) {
            m.stock(f64::MAX)?;
            if m.high / (1000. * a) > 1. + 64. * f64::EPSILON {
                return Err("Flat dry-front depth bound violated.".into());
            }
        }
        if (step + 1) % 64 == 0 || step + 1 == steps {
            let audit = surface_verification::audit(layout, &initial, &liquid, &transfers)?;
            for (maximum, value) in maximum_audit.iter_mut().zip(audit) {
                *maximum = maximum.max(value);
            }
        }
    }
    let depth: Vec<_> = liquid
        .iter()
        .zip(&mesh.areas)
        .map(|(m, a)| total_mass(&[m.high, m.low]) / (1000. * a))
        .collect();
    let sampled: Vec<Vec<f64>> = references
        .iter()
        .map(|r| p.iter().map(|&p| reference::sample(r, p)).collect())
        .collect::<Result<_, _>>()?;
    let truth = sampled.last().unwrap();
    let errors: Vec<_> = depth.iter().zip(truth).map(|(a, b)| a - b).collect();
    let error = rms(&errors, &mesh.areas);
    let spatial: Vec<_> = sampled[0]
        .iter()
        .zip(&sampled[1])
        .map(|(a, b)| a - b)
        .collect();
    let temporal: Vec<_> = sampled[1].iter().zip(truth).map(|(a, b)| a - b).collect();
    let mut contours = Vec::new();
    for threshold in THRESHOLDS {
        let reference_p = reference_front(references.last().unwrap(), threshold)?;
        let crossings: Vec<_> = layout
            .faces
            .iter()
            .filter_map(|f| {
                let [a, b] = f.regions;
                if (depth[a] < threshold) == (depth[b] < threshold) {
                    return None;
                }
                Some(p[a] + (p[b] - p[a]) * (threshold - depth[a]) / (depth[b] - depth[a]))
            })
            .collect();
        if crossings.is_empty() {
            return Err("Missing native dry-front contour.".into());
        }
        let reference_theta = reference_p.acos();
        let errors: Vec<_> = crossings
            .iter()
            .map(|p| (p.clamp(-1., 1.).acos() - reference_theta) * RADIUS)
            .collect();
        let native_wet_area = total_mass(
            &depth
                .iter()
                .zip(&mesh.areas)
                .filter_map(|(h, a)| (*h >= threshold).then_some(*a))
                .collect::<Vec<_>>(),
        );
        contours.push(json!({"thresholdMeters":threshold,"referenceP":reference_p,
            "referenceAdvanceMeters":RADIUS*(reference_theta-threshold.powf(7./3.).acos()),
            "crossingP":crossings,"minimumSignedCrossingErrorMeters":errors.iter().copied().fold(f64::INFINITY,f64::min),
            "maximumSignedCrossingErrorMeters":errors.iter().copied().fold(f64::NEG_INFINITY,f64::max),
            "maximumAbsoluteCrossingErrorMeters":errors.iter().map(|e|e.abs()).fold(0.,f64::max),
            "nativeThresholdWetAreaSquareMeters":native_wet_area,
            "referenceThresholdWetAreaSquareMeters":2.*std::f64::consts::PI*RADIUS.powi(2)*(1.-reference_p)}));
    }
    let continuum_initial_mass = 1000. * 2. * std::f64::consts::PI * RADIUS.powi(2) * 7. / 10.;
    let initial_mass = total_mass(
        &initial
            .iter()
            .flat_map(|m| [m.high, m.low])
            .collect::<Vec<_>>(),
    );
    let initially_dry_mass = total_mass(
        &liquid
            .iter()
            .zip(&p)
            .filter(|(_, p)| **p <= 0.)
            .flat_map(|(m, _)| [m.high, m.low])
            .collect::<Vec<_>>(),
    );
    let reference_dry_mass = 1000.
        * 2.
        * std::f64::consts::PI
        * RADIUS.powi(2)
        * (2. / references.last().unwrap().len() as f64)
        * total_mass(&references.last().unwrap()[..references.last().unwrap().len() / 2]);
    let resolution = resolution.unwrap();
    resolution.validate()?;
    let result = json!({"subdivision":level,"regions":liquid.len(),"axis":axis,"steps":steps,"stepSeconds":dt,
        "legacyDirectionalMobility":stage == surface::VerificationStage::LegacyDirectional,"stabilityBoundSeconds":layout.stable_seconds,
        "areaWeightedRmsErrorMeters":error,"maximumErrorMeters":errors.iter().map(|e|e.abs()).fold(0.,f64::max),
        "referenceSpatialDifferenceRmsMeters":rms(&spatial,&mesh.areas),"referenceTemporalDifferenceRmsMeters":rms(&temporal,&mesh.areas),
        "referenceSpatialDifferenceOverNativeRmsError":rms(&spatial,&mesh.areas)/error,
        "referenceTemporalDifferenceOverNativeRmsError":rms(&temporal,&mesh.areas)/error,
        "initialPointInventoryKilograms":initial_mass,"continuumInitialInventoryKilograms":continuum_initial_mass,
        "relativeInitialQuadratureDiscrepancy":(initial_mass-continuum_initial_mass)/continuum_initial_mass,
        "initiallyDryHalfInventoryKilograms":initially_dry_mass,"referenceDryHalfInventoryKilograms":reference_dry_mass,
        "maximumLocalRelativeResidual":maximum_audit[0],"maximumGlobalRelativeResidual":maximum_audit[1],
        "resolution":resolution,"contours":contours,"initialLiquid":initial,"finalLiquid":liquid,
        "directedTransfers":transfers,"referenceAtCentersMeters":truth,"errorAtCentersMeters":errors});
    Ok(NativeRun {
        liquid,
        result,
        error,
    })
}

pub(super) fn report(result: Value) -> Result<Value, String> {
    report_for(result, surface::VerificationStage::FullGradient)
}

pub(super) fn centered_report(result: Value) -> Result<Value, String> {
    report_for(result, surface::VerificationStage::FlatCenteredDepth)
}

pub(super) fn incident_report(result: Value) -> Result<Value, String> {
    report_for(result, surface::VerificationStage::FlatIncidentMobility)
}

fn report_for(mut result: Value, stage: surface::VerificationStage) -> Result<Value, String> {
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
        return Err("Native dry-front cohort exceeds its work bound.".into());
    }
    let mut groups = Vec::new();
    let mut spatial = Vec::new();
    let mut legacy = Vec::new();
    for axis in AXES {
        let mut previous: Option<(u32, f64)> = None;
        for level in 3..=5 {
            let mut runs = Vec::new();
            for factor in [1, 2, 4] {
                runs.push(run(level, axis, base_steps * factor, &depths, stage)?);
            }
            let mesh = Surface::build(level, RADIUS);
            let time_differences: Vec<_> = runs
                .windows(2)
                .map(|r| {
                    let differences: Vec<_> = r[0]
                        .liquid
                        .iter()
                        .zip(&r[1].liquid)
                        .zip(&mesh.areas)
                        .map(|((a, b), area)| {
                            total_mass(&[a.high, a.low, -b.high, -b.low]) / (1000. * area)
                        })
                        .collect();
                    rms(&differences, &mesh.areas)
                })
                .collect();
            if let Some((old_level, old_error)) = previous {
                spatial.push(json!({"axis":axis,"coarseSubdivision":old_level,
                "fineSubdivision":level,"fineOverCoarseRmsError":runs[2].error/old_error}));
            }
            previous = Some((level, runs[2].error));
            if level == 5 {
                let control = if stage == surface::VerificationStage::FlatIncidentMobility {
                    surface::VerificationStage::FlatCenteredDepth
                } else if stage == surface::VerificationStage::FlatCenteredDepth {
                    surface::VerificationStage::FullGradient
                } else {
                    surface::VerificationStage::LegacyDirectional
                };
                legacy.push(run(level, axis, base_steps * 4, &depths, control)?.result);
            }
            groups.push(json!({"axis":axis,"subdivision":level,"timeRefinementRmsDifferencesMeters":time_differences,
                "successiveTimeDifferenceRatio":time_differences[1]/time_differences[0],
                "lastTimeDifferenceOverFineRmsError":time_differences[1]/runs[2].error,
                "runs":runs.into_iter().map(|r|r.result).collect::<Vec<_>>()}));
        }
    }
    result["reportVersion"] = json!("surface-dry-front-report-1");
    result["surfaceModelVersion"] = json!(surface_gradient::MODEL_VERSION);
    result["scope"] = json!(
        "One flat closed sphere with h=p_+^(3/7), no film, two orientations, independent band reference. Threshold contours are not exact support. No terrain barriers, coasts, seasonal selection or calibration."
    );
    result["baseSteps"] = json!(base_steps);
    result["groups"] = json!(groups);
    result["spatialComparisons"] = json!(spatial);
    if stage == surface::VerificationStage::FlatCenteredDepth {
        result["reportVersion"] = json!("surface-flat-centered-front-report-1");
        result["surfaceModelVersion"] = json!(surface_centered::MODEL_VERSION);
        result["interfaceDepth"] = json!(
            "Arithmetic mean of frozen neighboring depths; flat closed terrestrial cells only"
        );
        result["matchedSourceDepthControls"] = json!(legacy);
    } else if stage == surface::VerificationStage::FlatIncidentMobility {
        result["reportVersion"] = json!("surface-flat-incident-front-report-1");
        result["surfaceModelVersion"] = json!(surface_incident::MODEL_VERSION);
        result["interfaceDepth"] = json!("Unchanged arithmetic mean of frozen neighboring depths");
        result["mobilityAssembly"] = json!(
            "Positive individual half-cotangent weighted mean of separately capped incident-triangle mobilities; not triangle depth integration"
        );
        result["matchedCenteredDepthControls"] = json!(legacy);
    } else {
        result["matchedLegacyControls"] = json!(legacy);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incident_dry_evolution_repeats_with_identical_initialization_and_reference() {
        let references = [
            reference::spherical(128, 600).unwrap().depth,
            reference::spherical(256, 600).unwrap().depth,
            reference::spherical(256, 1200).unwrap().depth,
        ];
        let a = run(
            3,
            AXES[1],
            1200,
            &references,
            surface::VerificationStage::FlatIncidentMobility,
        )
        .unwrap();
        let b = run(
            3,
            AXES[1],
            1200,
            &references,
            surface::VerificationStage::FlatIncidentMobility,
        )
        .unwrap();
        let old = run(
            3,
            AXES[1],
            1200,
            &references,
            surface::VerificationStage::FlatCenteredDepth,
        )
        .unwrap();
        assert_eq!(a.result, b.result);
        assert_eq!(a.result["initialLiquid"], old.result["initialLiquid"]);
        assert_eq!(
            a.result["referenceAtCentersMeters"],
            old.result["referenceAtCentersMeters"]
        );
        assert_ne!(a.result["finalLiquid"], old.result["finalLiquid"]);
        assert!(
            a.result["initiallyDryHalfInventoryKilograms"]
                .as_f64()
                .unwrap()
                > 0.
        );
    }

    #[test]
    fn native_dry_front_repeats_and_has_conservative_arrivals_without_a_film() {
        let coarse = reference::spherical(128, 600).unwrap().depth;
        let fine = reference::spherical(256, 600).unwrap().depth;
        let finest = reference::spherical(256, 1200).unwrap().depth;
        let references = [coarse, fine, finest];
        let a = run(
            3,
            AXES[1],
            1200,
            &references,
            surface::VerificationStage::FullGradient,
        )
        .unwrap();
        let b = run(
            3,
            AXES[1],
            1200,
            &references,
            surface::VerificationStage::FullGradient,
        )
        .unwrap();
        assert_eq!(a.result, b.result);
        assert!(
            a.result["initiallyDryHalfInventoryKilograms"]
                .as_f64()
                .unwrap()
                > 0.
        );
        assert!(a.result["maximumGlobalRelativeResidual"].as_f64().unwrap() < 1e-12);
        assert!(
            a.result["initialLiquid"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["high"] == 0.)
        );
        for (level, axis, steps) in [
            (2, AXES[0], 1000),
            (6, AXES[0], 1000),
            (3, [0.; 3], 1000),
            (3, AXES[0], 0),
            (5, AXES[0], 1),
        ] {
            assert!(
                run(
                    level,
                    axis,
                    steps,
                    &references,
                    surface::VerificationStage::FullGradient
                )
                .is_err()
            );
        }
    }
}
