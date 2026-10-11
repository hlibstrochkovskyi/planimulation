//! Independent zonal reference and evolving standalone surface verification.
//! No seasonal selection, checkpoint migration or desktop promotion.
mod zonal;
use super::{
    Mass, ResolutionBudget, surface, surface_centered, surface_cotan, surface_flow,
    surface_gradient, surface_incident, surface_verification,
};
use crate::{Surface, dot, moisture_transport::total_mass, norm};
use serde_json::{Value, json};

const RADIUS: f64 = 100_000.;
const ROUGHNESS: f64 = 0.04;
const MEAN: f64 = 1.;
const AMPLITUDE: f64 = 0.1;
const CAP: f64 = 1e6;
const END_SECONDS: f64 = 1800.;
const AXES: [[f64; 3]; 2] = [[1., 0., 0.], [1., 2., 3.]];

struct Reference {
    depth: Vec<f64>,
    coarse_spatial: Vec<f64>,
    coarse_temporal: Vec<f64>,
    result: Value,
}

fn references(degree: u32) -> Result<Reference, String> {
    let mut controls = Vec::new();
    let mut profiles = Vec::new();
    for (bands, steps) in [
        (512, 1800),
        (1024, 450),
        (1024, 900),
        (1024, 1800),
        (2048, 1800),
        (2048, 3600),
        (4096, 3600),
        (4096, 7200),
    ] {
        let r = zonal::solve(bands, steps, END_SECONDS, degree, CAP)?;
        controls.push(
            json!({"bands":bands,"steps":steps,"stepSeconds":END_SECONDS/steps as f64,
            "diagnostics":r.diagnostics,"finalBandDepthMeters":r.depth}),
        );
        profiles.push(r.depth);
    }
    let points: Vec<_> = (0..=4096).map(|i| -1. + 2. * i as f64 / 4096.).collect();
    let difference = |a: usize, b: usize| -> Result<Value, String> {
        let d: Vec<_> = points
            .iter()
            .map(|&p| Ok(zonal::sample(&profiles[a], p)? - zonal::sample(&profiles[b], p)?))
            .collect::<Result<_, String>>()?;
        Ok(json!({"firstControl":a,"secondControl":b,
            "sampledRmsDifferenceMeters":(total_mass(&d.iter().map(|d|d*d).collect::<Vec<_>>())/d.len() as f64).sqrt(),
            "maximumSampledDifferenceMeters":d.iter().map(|d|d.abs()).fold(0.,f64::max)}))
    };
    let comparisons = [(0, 3), (3, 4), (1, 2), (2, 3), (4, 5), (5, 6), (6, 7)]
        .map(|(a, b)| difference(a, b))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Reference {
        coarse_spatial: profiles[5].clone(),
        coarse_temporal: profiles[6].clone(),
        depth: profiles.pop().unwrap(),
        result: json!({"degree":degree,"controls":controls,"comparisons":comparisons,
        "selectedControl":7,"comparisonSamples":points.len(),
        "sampling":"Linear interpolation/extrapolation of band-center averages; uncertainty retained by reference refinement, not fitted to the native output"}),
    })
}

fn physical_layout(mesh: &Surface) -> Result<surface_flow::Layout, String> {
    surface_flow::Layout::from_fields(
        mesh,
        RADIUS,
        vec![0.; mesh.areas.len()],
        vec![true; mesh.areas.len()],
        0.,
        surface_flow::Settings {
            roughness: ROUGHNESS,
            maximum_diffusivity_square_meters_per_second: CAP,
        },
    )
}

struct NativeRun {
    liquid: Vec<Mass>,
    result: Value,
    rms: f64,
}

fn run(
    level: u32,
    axis: [f64; 3],
    degree: u32,
    steps: usize,
    reference: &Reference,
    stage: surface::VerificationStage,
) -> Result<NativeRun, String> {
    if !(2..=4).contains(&level) || !(1..=2).contains(&degree) || steps == 0 || steps > 65536 {
        return Err("Unsupported bounded surface evolution controls.".into());
    }
    let length = norm(axis);
    if !length.is_finite() || length == 0. {
        return Err("Invalid surface evolution axis.".into());
    }
    let axis = axis.map(|v| v / length);
    let mesh = Surface::build(level, RADIUS);
    let prepared = surface_gradient::Prepared::new(&mesh, physical_layout(&mesh)?, RADIUS)?;
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
        return Err("Evolution control must use one stable substep.".into());
    }
    let initial: Vec<_> = mesh
        .centers
        .iter()
        .zip(&mesh.areas)
        .map(|(&s, &a)| {
            let p = dot(axis, s);
            let profile = if degree == 1 {
                p
            } else {
                (3. * p * p - 1.) / 2.
            };
            Mass {
                high: 1000. * a * (MEAN + AMPLITUDE * profile),
                low: 0.,
            }
        })
        .collect();
    let mut liquid = initial.clone();
    let mut transfers = vec![Mass::default(); 2 * layout.faces.len()];
    let mut resolution = Some(ResolutionBudget::default());
    let by_region = vec![None; liquid.len()];
    let mut maximum_audit = [0_f64; 2];
    let mut minimum_depth = MEAN;
    let mut maximum_depth = MEAN;
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
            return Err("Unexpected surface evolution substep split.".into());
        }
        for (m, a) in liquid.iter().zip(&mesh.areas) {
            m.stock(f64::MAX)?;
            let h = m.high / (1000. * a);
            minimum_depth = minimum_depth.min(h);
            maximum_depth = maximum_depth.max(h);
            if !(MEAN - AMPLITUDE - 64. * f64::EPSILON..=MEAN + AMPLITUDE + 64. * f64::EPSILON)
                .contains(&h)
            {
                return Err("Wet evolution control violates its certified depth range.".into());
            }
        }
        if (step + 1) % 64 == 0 || step + 1 == steps {
            let residuals = surface_verification::audit(layout, &initial, &liquid, &transfers)?;
            for (maximum, r) in maximum_audit.iter_mut().zip(residuals) {
                *maximum = maximum.max(r);
            }
        }
    }
    let truth: Vec<_> = mesh
        .centers
        .iter()
        .map(|&s| zonal::sample(&reference.depth, dot(axis, s)))
        .collect::<Result<_, _>>()?;
    let errors: Vec<_> = liquid
        .iter()
        .zip(&mesh.areas)
        .zip(&truth)
        .map(|((m, a), h)| total_mass(&[m.high / (1000. * a), m.low / (1000. * a), -h]))
        .collect();
    let rms = weighted_rms(&errors, &mesh.areas);
    let reference_spatial: Vec<_> = mesh
        .centers
        .iter()
        .map(|&s| {
            Ok(zonal::sample(&reference.coarse_spatial, dot(axis, s))?
                - zonal::sample(&reference.coarse_temporal, dot(axis, s))?)
        })
        .collect::<Result<_, String>>()?;
    let reference_temporal: Vec<_> = mesh
        .centers
        .iter()
        .zip(&truth)
        .map(|(&s, h)| Ok(zonal::sample(&reference.coarse_temporal, dot(axis, s))? - h))
        .collect::<Result<_, String>>()?;
    let spatial_uncertainty = weighted_rms(&reference_spatial, &mesh.areas);
    let temporal_uncertainty = weighted_rms(&reference_temporal, &mesh.areas);
    let change: Vec<_> = initial
        .iter()
        .zip(&mesh.areas)
        .zip(&truth)
        .map(|((m, a), h)| m.high / (1000. * a) - h)
        .collect();
    let change_rms = weighted_rms(&change, &mesh.areas);
    let resolution = resolution.unwrap();
    resolution.validate()?;
    let result = json!({"subdivision":level,"regions":liquid.len(),"degree":degree,"axis":axis,
        "operator":match stage {
            surface::VerificationStage::LegacyDirectional => surface_cotan::MODEL_VERSION,
            surface::VerificationStage::FullGradient => surface_gradient::MODEL_VERSION,
            surface::VerificationStage::FlatCenteredDepth => surface_centered::MODEL_VERSION,
            surface::VerificationStage::FlatIncidentMobility => surface_incident::MODEL_VERSION,
        },
        "endSeconds":END_SECONDS,"steps":steps,"stepSeconds":dt,"stabilityBoundSeconds":layout.stable_seconds,
        "minimumDepthMeters":minimum_depth,"maximumDepthMeters":maximum_depth,
        "maximumLocalRelativeResidual":maximum_audit[0],"maximumGlobalRelativeResidual":maximum_audit[1],
        "referenceChangeRmsMeters":change_rms,"areaWeightedRmsErrorMeters":rms,
        "rmsErrorOverReferenceChange":rms/change_rms,
        "referenceLastSpatialDifferenceRmsMeters":spatial_uncertainty,
        "referenceLastTemporalDifferenceRmsMeters":temporal_uncertainty,
        "referenceSpatialDifferenceOverNativeRmsError":spatial_uncertainty/rms,
        "referenceTemporalDifferenceOverNativeRmsError":temporal_uncertainty/rms,
        "maximumErrorMeters":errors.iter().map(|e|e.abs()).fold(0.,f64::max),
        "resolution":resolution,"initialLiquid":initial,"finalLiquid":liquid,"directedTransfers":transfers,
        "referenceAtCentersMeters":truth,"errorAtCentersMeters":errors});
    Ok(NativeRun {
        liquid,
        result,
        rms,
    })
}

fn weighted_rms(values: &[f64], areas: &[f64]) -> f64 {
    (total_mass(
        &values
            .iter()
            .zip(areas)
            .map(|(v, a)| v * v * a)
            .collect::<Vec<_>>(),
    ) / total_mass(areas))
    .sqrt()
}

fn root() -> Value {
    json!({"reportVersion":"surface-nonlinear-evolution-report-1",
        "surfaceModelVersion":surface_gradient::MODEL_VERSION,"radiusMeters":RADIUS,
        "roughness":ROUGHNESS,"meanDepthMeters":MEAN,"amplitudeMeters":AMPLITUDE,
        "capSquareMetersPerSecond":CAP,"endSeconds":END_SECONDS,
        "scope":"Unforced positive fully wet axisymmetric profiles on a closed flat sphere. Independent latitude-band reference. Not moving dry fronts, terrain/coastal hydraulics, seasonal integration or physical calibration.",
        "referenceEquation":"d_t h = R^-2 d_p[beta*(1-p^2)*d_p h], beta=min(cap,h^(5/3)/(n*sqrt(sqrt(1-p^2)*abs(d_p h)/R)))",
        "referenceMethod":"Equal-area band finite volumes; backward Euler with converged damped Picard, centered face depth, closed pole fluxes; separate reference mesh/time refinement",
        "nativeInitialization":"Continuous point samples times unchanged actual spherical areas; no global rescaling",
        "referenceInitialization":"Exact band averages of P1/P2; no native graph, conductances, histories or calibration"})
}

/// Reference-only controls are separately labelled, never a native accuracy pass.
pub fn reference_report() -> Result<Value, String> {
    let mut result = root();
    result["reportVersion"] = json!("surface-zonal-reference-report-1");
    result["references"] = json!([references(1)?.result, references(2)?.result]);
    Ok(result)
}

/// Fixed unforced evolution, cross-method reference and independent time controls.
pub fn report() -> Result<Value, String> {
    report_for(surface::VerificationStage::FullGradient)
}

/// Same fixed wet problems and references, with only interface depth changed.
pub fn centered_report() -> Result<Value, String> {
    report_for(surface::VerificationStage::FlatCenteredDepth)
}

/// Same fixed wet problems, with gradient-side mobility assembly changed.
pub fn incident_report() -> Result<Value, String> {
    report_for(surface::VerificationStage::FlatIncidentMobility)
}

fn report_for(stage: surface::VerificationStage) -> Result<Value, String> {
    let mesh = Surface::build(4, RADIUS);
    let prepared = surface_gradient::Prepared::new(&mesh, physical_layout(&mesh)?, RADIUS)?;
    let base_steps = (END_SECONDS / (0.9 * prepared.layout().stable_seconds)).ceil() as usize;
    if base_steps > 16384 {
        return Err("Surface evolution cohort exceeds its declared work bound.".into());
    }
    let mut groups = Vec::new();
    let mut spatial = Vec::new();
    let mut controls = Vec::new();
    let mut references_json = Vec::new();
    for degree in 1..=2 {
        let reference = references(degree)?;
        for axis in AXES {
            let mut previous: Option<(u32, f64)> = None;
            for level in 2..=4 {
                let mut runs = Vec::new();
                for factor in [1, 2, 4] {
                    runs.push(run(
                        level,
                        axis,
                        degree,
                        base_steps * factor,
                        &reference,
                        stage,
                    )?);
                }
                let mesh = Surface::build(level, RADIUS);
                let mut differences = Vec::new();
                for pair in runs.windows(2) {
                    let d: Vec<_> = pair[0]
                        .liquid
                        .iter()
                        .zip(&pair[1].liquid)
                        .zip(&mesh.areas)
                        .map(|((a, b), area)| {
                            total_mass(&[a.high, -b.high, a.low, -b.low]) / (1000. * area)
                        })
                        .collect();
                    differences.push(weighted_rms(&d, &mesh.areas));
                }
                let error = runs[2].rms;
                if let Some((old_level, old_error)) = previous {
                    spatial.push(json!({"degree":degree,"axis":axis,"coarseSubdivision":old_level,"fineSubdivision":level,
                        "fineOverCoarseRmsError":error/old_error,"rmsErrorDecreases":error<old_error}));
                }
                previous = Some((level, error));
                if level == 4 {
                    let control = if stage == surface::VerificationStage::FlatIncidentMobility {
                        surface::VerificationStage::FlatCenteredDepth
                    } else if stage == surface::VerificationStage::FlatCenteredDepth {
                        surface::VerificationStage::FullGradient
                    } else {
                        surface::VerificationStage::LegacyDirectional
                    };
                    controls.push(
                        run(level, axis, degree, base_steps * 4, &reference, control)?.result,
                    );
                }
                groups.push(json!({"degree":degree,"axis":axis,"subdivision":level,"timeRefinementRmsDifferencesMeters":differences,
                    "successiveTimeDifferenceRatio":differences[1]/differences[0],
                    "lastTimeDifferenceOverFineReferenceError":differences[1]/error,
                    "runs":runs.into_iter().map(|r|r.result).collect::<Vec<_>>()}));
            }
        }
        references_json.push(reference.result);
    }
    let mut result = root();
    result["baseSteps"] = json!(base_steps);
    result["references"] = json!(references_json);
    result["groups"] = json!(groups);
    result["spatialComparisons"] = json!(spatial);
    if stage == surface::VerificationStage::FlatCenteredDepth {
        result["reportVersion"] = json!("surface-flat-centered-wet-report-1");
        result["surfaceModelVersion"] = json!(surface_centered::MODEL_VERSION);
        result["interfaceDepth"] = json!(
            "Arithmetic mean of frozen neighboring depths; flat closed terrestrial cells only"
        );
        result["matchedSourceDepthControls"] = json!(controls);
    } else if stage == surface::VerificationStage::FlatIncidentMobility {
        result["reportVersion"] = json!("surface-flat-incident-wet-report-1");
        result["surfaceModelVersion"] = json!(surface_incident::MODEL_VERSION);
        result["interfaceDepth"] = json!("Unchanged arithmetic mean of frozen neighboring depths");
        result["mobilityAssembly"] = json!(
            "Positive individual half-cotangent weighted mean of separately capped incident-triangle mobilities; not triangle depth integration"
        );
        result["matchedCenteredDepthControls"] = json!(controls);
    } else {
        result["matchedLegacyControls"] = json!(controls);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_reference(degree: u32) -> Reference {
        Reference {
            depth: zonal::solve(256, 512, END_SECONDS, degree, CAP)
                .unwrap()
                .depth,
            coarse_spatial: zonal::solve(128, 256, END_SECONDS, degree, CAP)
                .unwrap()
                .depth,
            coarse_temporal: zonal::solve(256, 256, END_SECONDS, degree, CAP)
                .unwrap()
                .depth,
            result: Value::Null,
        }
    }

    #[test]
    fn incident_wet_evolution_repeats_with_identical_initialization_and_reference() {
        let reference = test_reference(2);
        let a = run(
            2,
            AXES[1],
            2,
            512,
            &reference,
            surface::VerificationStage::FlatIncidentMobility,
        )
        .unwrap();
        let b = run(
            2,
            AXES[1],
            2,
            512,
            &reference,
            surface::VerificationStage::FlatIncidentMobility,
        )
        .unwrap();
        let old = run(
            2,
            AXES[1],
            2,
            512,
            &reference,
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
        assert_eq!(a.result["operator"], surface_incident::MODEL_VERSION);
    }

    #[test]
    fn evolving_native_state_repeats_without_front_or_seasonal_promotion() {
        let reference = test_reference(2);
        let a = run(
            2,
            AXES[1],
            2,
            512,
            &reference,
            surface::VerificationStage::FullGradient,
        )
        .unwrap();
        let b = run(
            2,
            AXES[1],
            2,
            512,
            &reference,
            surface::VerificationStage::FullGradient,
        )
        .unwrap();
        assert_eq!(a.result, b.result);
        assert!(a.result["maximumGlobalRelativeResidual"].as_f64().unwrap() < 1e-12);
        assert!(a.result["referenceChangeRmsMeters"].as_f64().unwrap() > 1e-4);
        let initial = a.result["initialLiquid"].as_array().unwrap();
        let final_liquid = a.result["finalLiquid"].as_array().unwrap();
        assert_ne!(initial, final_liquid);
        assert!(a.rms.is_finite() && a.rms > 0.);
        assert!(
            a.result["referenceSpatialDifferenceOverNativeRmsError"]
                .as_f64()
                .unwrap()
                > 0.
        );
        assert!(
            a.result["referenceTemporalDifferenceOverNativeRmsError"]
                .as_f64()
                .unwrap()
                > 0.
        );
    }

    #[test]
    fn bounded_native_controls_refuse_invalid_and_unstable_inputs() {
        let reference = test_reference(1);
        for (level, axis, degree, steps) in [
            (1, AXES[0], 1, 512),
            (5, AXES[0], 1, 512),
            (2, [0.; 3], 1, 512),
            (2, AXES[0], 0, 512),
            (2, AXES[0], 1, 0),
            (2, AXES[0], 1, 65537),
            (4, AXES[0], 1, 1),
        ] {
            assert!(
                run(
                    level,
                    axis,
                    degree,
                    steps,
                    &reference,
                    surface::VerificationStage::FullGradient
                )
                .is_err()
            );
        }
    }
}
