//! Fixed-input verification of the production paired surface operator.
//! This is not a seasonal checkpoint, terrain recipe or calibrated water model.
use super::{Mass, ResolutionBudget, surface, surface_flow};
use crate::{Surface, moisture_transport::total_mass};
use serde_json::{Value, json};

const RADIUS: f64 = 100_000.;
const DIFFUSIVITY: f64 = 10_000.;
const MEAN_DEPTH: f64 = 100.;
const AMPLITUDE: f64 = 1.;
const END_SECONDS: f64 = 36_000.;
const MAX_STEPS: usize = 16_384;
const AXES: [[f64; 3]; 2] = [[1., 0., 0.], [1., 2., 3.]];

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn normalize(axis: [f64; 3]) -> [f64; 3] {
    let norm = dot(axis, axis).sqrt();
    axis.map(|v| v / norm)
}
fn exact_depth(center: [f64; 3], axis: [f64; 3], seconds: f64, amplitude: f64) -> f64 {
    MEAN_DEPTH
        + amplitude * (-2. * DIFFUSIVITY * seconds / RADIUS.powi(2)).exp() * dot(axis, center)
}
fn layout(mesh: &Surface) -> Result<surface_flow::Layout, String> {
    surface_flow::Layout::from_fields(
        mesh,
        RADIUS,
        vec![0.; mesh.areas.len()],
        vec![true; mesh.areas.len()],
        0.,
        surface_flow::Settings {
            roughness: 0.04,
            maximum_diffusivity_square_meters_per_second: DIFFUSIVITY,
        },
    )
}
fn mass_sum(masses: &[Mass]) -> f64 {
    total_mass(
        &masses
            .iter()
            .flat_map(|m| [m.high, m.low])
            .collect::<Vec<_>>(),
    )
}

// Independently reconstruct every column's cumulative identity from directed
// face histories, including signed tails. Balanced geographic teleportation
// cannot pass merely because the global sum stays unchanged.
fn audit(
    layout: &surface_flow::Layout,
    initial: &[Mass],
    liquid: &[Mass],
    transfers: &[Mass],
) -> Result<[f64; 2], String> {
    let mut terms: Vec<Vec<f64>> = liquid
        .iter()
        .zip(initial)
        .map(|(a, b)| vec![a.high, -b.high, a.low, -b.low])
        .collect();
    for (f, face) in layout.faces.iter().enumerate() {
        for direction in 0..2 {
            let mass = transfers[2 * f + direction];
            mass.stock(f64::MAX)?;
            terms[face.regions[direction]].extend([mass.high, mass.low]);
            terms[face.regions[1 - direction]].extend([-mass.high, -mass.low]);
        }
    }
    let mut maximum: f64 = 0.;
    for terms in terms {
        let scale = terms.iter().map(|v| v.abs()).fold(1., f64::max);
        maximum = maximum.max(total_mass(&terms).abs() / scale);
    }
    let delta = total_mass(
        &liquid
            .iter()
            .zip(initial)
            .flat_map(|(a, b)| [a.high, -b.high, a.low, -b.low])
            .collect::<Vec<_>>(),
    );
    let global = delta.abs() / mass_sum(initial).max(1.);
    if !maximum.is_finite()
        || maximum > 128. * f64::EPSILON
        || !global.is_finite()
        || global > 1e-12
    {
        return Err(format!(
            "Surface verification ledger failed: local {maximum}, global {global}."
        ));
    }
    Ok([maximum, global])
}

struct Run {
    result: Value,
    liquid: Vec<Mass>,
    error_rms: f64,
}
fn initial_flux_diagnostic(
    mesh: &Surface,
    layout: &surface_flow::Layout,
    initial: &[Mass],
    axis: [f64; 3],
    amplitude: f64,
) -> Result<Value, String> {
    let geometry = crate::moisture_transport::Geometry::from_surface(mesh, RADIUS)?;
    let mut liquid = initial.to_vec();
    let mut transfers = vec![Mass::default(); 2 * layout.faces.len()];
    let mut resolution = Some(ResolutionBudget::default());
    let seconds = 1.;
    surface::advance(
        layout,
        surface::Stocks {
            by_region: &vec![None; liquid.len()],
            liquid: &mut liquid,
            bodies: &mut [],
            transfers: &mut transfers,
            resolution: &mut resolution,
        },
        seconds,
    )?;
    audit(layout, initial, &liquid, &transfers)?;
    let mut differences = Vec::new();
    let mut reference = Vec::new();
    let mut max_angle: f64 = 0.;
    for (f, boundary) in geometry.boundaries().iter().enumerate() {
        if boundary.regions != layout.faces[f].regions {
            return Err("Surface diagnostic graph order does not match.".into());
        }
        // For h=H+A axis.s, grad_s(h)=A/R*(axis-(axis.s)s).
        // A great-circle segment's conormal is constant and perpendicular to s,
        // so its integrated outward flux is exactly -D*A/R*axis.n*arc_length.
        let exact = -1000. * DIFFUSIVITY * amplitude / RADIUS
            * total_mass(
                &boundary
                    .segments
                    .iter()
                    .map(|s| dot(axis, s.outward_normal) * s.arc_length_meters)
                    .collect::<Vec<_>>(),
            );
        let forward = transfers[2 * f];
        let reverse = transfers[2 * f + 1];
        let actual =
            total_mass(&[forward.high, -reverse.high, forward.low, -reverse.low]) / seconds;
        differences.push((actual - exact).abs());
        reference.push(exact.abs());
        let [a, b] = boundary.regions;
        let connector = std::array::from_fn(|i| mesh.centers[b][i] - mesh.centers[a][i]);
        for s in &boundary.segments {
            let projected =
                std::array::from_fn(|i| connector[i] - dot(connector, s.midpoint) * s.midpoint[i]);
            let direction = normalize(projected);
            max_angle = max_angle.max(
                dot(direction, s.outward_normal)
                    .clamp(-1., 1.)
                    .acos()
                    .to_degrees(),
            );
        }
    }
    let reference_l1 = total_mass(&reference);
    Ok(
        json!({"seconds":seconds,"analyticReference":"Exact integrated l=1 continuum flux over each existing great-circle dual segment",
        "relativeFaceFluxL1Error":if reference_l1 > 0. {Some(total_mass(&differences)/reference_l1)} else {None},
        "maximumAbsoluteFaceFluxErrorKilogramsPerSecond":differences.into_iter().fold(0.,f64::max),
        "maximumSegmentNonOrthogonalityDegrees":max_angle,"resolution":resolution}),
    )
}
fn run(level: u32, axis: [f64; 3], steps: usize, amplitude: f64) -> Result<Run, String> {
    if !(1..=5).contains(&level)
        || steps == 0
        || steps > MAX_STEPS
        || ![0., AMPLITUDE].contains(&amplitude)
    {
        return Err("Unsupported bounded surface verification control.".into());
    }
    let mesh = Surface::build(level, RADIUS);
    let layout = layout(&mesh)?;
    let dt = END_SECONDS / steps as f64;
    if dt > layout.stable_seconds {
        return Err("Surface verification must use one stable substep per call.".into());
    }
    let axis = normalize(axis);
    if axis.iter().any(|v| !v.is_finite()) {
        return Err("Invalid surface verification axis.".into());
    }
    let initial: Vec<_> = mesh
        .centers
        .iter()
        .zip(&mesh.areas)
        .map(|(&s, &area)| Mass {
            high: 1000. * area * exact_depth(s, axis, 0., amplitude),
            low: 0.,
        })
        .collect();
    let flux_diagnostic = initial_flux_diagnostic(&mesh, &layout, &initial, axis, amplitude)?;
    let mut liquid = initial.clone();
    let mut transfers = vec![Mass::default(); 2 * layout.faces.len()];
    let by_region = vec![None; liquid.len()];
    let mut resolution = Some(ResolutionBudget::default());
    let min_distance = layout
        .faces
        .iter()
        .map(|f| f.distance_meters)
        .fold(f64::MAX, f64::min);
    // At every step we check h in [H-A,H+A]. On a flat bed this certifies
    // drive <= 2A and over_crest >= H-A, so every nonzero face is capped.
    let minimum_uncapped_mobility = (MEAN_DEPTH - AMPLITUDE).powf(5. / 3.)
        / (layout.settings.roughness * (2. * AMPLITUDE / min_distance).sqrt());
    if minimum_uncapped_mobility <= DIFFUSIVITY {
        return Err("Analytic control is not certified in the capped linear regime.".into());
    }
    let mut max_audit = [0_f64; 2];
    let mut minimum_depth: f64 = MEAN_DEPTH;
    let mut maximum_depth: f64 = MEAN_DEPTH;
    for step in 0..steps {
        let substeps = surface::advance(
            &layout,
            surface::Stocks {
                by_region: &by_region,
                liquid: &mut liquid,
                bodies: &mut [],
                transfers: &mut transfers,
                resolution: &mut resolution,
            },
            dt,
        )?;
        if substeps != 1 {
            return Err("Unexpected subdivision in fixed-step surface verification.".into());
        }
        for (&m, &area) in liquid.iter().zip(&mesh.areas) {
            m.stock(f64::MAX)?;
            let depth = m.high / (1000. * area);
            minimum_depth = minimum_depth.min(depth);
            maximum_depth = maximum_depth.max(depth);
            if !(MEAN_DEPTH - AMPLITUDE..=MEAN_DEPTH + AMPLITUDE).contains(&depth) {
                return Err("Surface verification violated its certified depth bounds.".into());
            }
        }
        if (step + 1) % 32 == 0 || step + 1 == steps {
            let residuals = audit(&layout, &initial, &liquid, &transfers)?;
            for (m, r) in max_audit.iter_mut().zip(residuals) {
                *m = m.max(r);
            }
        }
    }
    let resolution = resolution.unwrap();
    resolution.validate()?;
    let total_area = total_mass(&mesh.areas);
    let errors: Vec<_> = liquid
        .iter()
        .zip(&mesh.areas)
        .zip(&mesh.centers)
        .map(|((&m, &area), &center)| {
            total_mass(&[
                m.high / (1000. * area),
                -exact_depth(center, axis, END_SECONDS, amplitude),
                m.low / (1000. * area),
            ])
        })
        .collect();
    let mean_abs = total_mass(
        &errors
            .iter()
            .zip(&mesh.areas)
            .map(|(e, a)| e.abs() * a)
            .collect::<Vec<_>>(),
    ) / total_area;
    let error_rms = (total_mass(
        &errors
            .iter()
            .zip(&mesh.areas)
            .map(|(e, a)| e * e * a)
            .collect::<Vec<_>>(),
    ) / total_area)
        .sqrt();
    let max_error = errors.iter().map(|v| v.abs()).fold(0., f64::max);
    let analytic_change_rms = amplitude
        * (1. - (-2. * DIFFUSIVITY * END_SECONDS / RADIUS.powi(2)).exp())
        * (total_mass(
            &mesh
                .centers
                .iter()
                .zip(&mesh.areas)
                .map(|(&s, area)| dot(axis, s).powi(2) * area)
                .collect::<Vec<_>>(),
        ) / total_area)
            .sqrt();
    let result = json!({
        "subdivision":level,"regions":liquid.len(),"axis":axis,"amplitudeMeters":amplitude,
        "endSeconds":END_SECONDS,"substeps":steps,"stepSeconds":dt,
        "stabilityBoundSeconds":layout.stable_seconds,
        "minimumUncappedMobilitySquareMetersPerSecond":minimum_uncapped_mobility,
        "minimumDepthMeters":minimum_depth,"maximumDepthMeters":maximum_depth,
        "initialKilograms":mass_sum(&initial),"finalKilograms":mass_sum(&liquid),
        "maximumLocalRelativeResidual":max_audit[0],"maximumGlobalRelativeResidual":max_audit[1],
        "areaWeightedMeanAbsoluteErrorMeters":mean_abs,"areaWeightedRmsErrorMeters":error_rms,
        "maximumErrorMeters":max_error,"resolution":resolution,
        "rmsErrorOverAnalyticChange":if analytic_change_rms > 0. {Some(error_rms/analytic_change_rms)} else {None},
        "activeDirectedFaces":transfers.iter().filter(|m| m.high > 0.).count(),
        "grossTransferredKilograms":mass_sum(&transfers),
        "initialFaceFluxDiagnostic":flux_diagnostic,
        "finalLiquidKilograms":liquid,"directedTransferKilograms":transfers,
        "errorAtCentersMeters":errors,
    });
    Ok(Run {
        result,
        liquid,
        error_rms,
    })
}

/// Deterministic sixteen-case sphere suite plus one uniform control.
/// No mutable seasonal session or
/// user-supplied recipe is exposed by this narrowly bounded verification API.
pub fn report() -> Result<Value, String> {
    let finest = Surface::build(5, RADIUS);
    let base_steps = (END_SECONDS / (0.5 * layout(&finest)?.stable_seconds)).ceil() as usize;
    let mut groups = Vec::new();
    let mut spatial_comparisons = Vec::new();
    let uniform = run(5, AXES[0], base_steps, 0.)?;
    for axis in AXES {
        let mut previous: Option<(u32, usize, f64)> = None;
        for level in 2..=5 {
            let coarse = run(level, axis, base_steps, AMPLITUDE)?;
            let fine = run(level, axis, 2 * base_steps, AMPLITUDE)?;
            let mesh = Surface::build(level, RADIUS);
            if let Some((old_level, old_regions, old_error)) = previous {
                spatial_comparisons.push(json!({"axis":normalize(axis),"coarseSubdivision":old_level,"fineSubdivision":level,
                    "fineOverCoarseRmsError":fine.error_rms/old_error,
                    "rmsErrorDecreases":fine.error_rms < old_error,
                    "observedOrderUsingSqrtRegionCount":(old_error/fine.error_rms).ln() / (mesh.areas.len() as f64 / old_regions as f64).sqrt().ln()}));
            }
            previous = Some((level, mesh.areas.len(), fine.error_rms));
            let difference: Vec<_> = coarse
                .liquid
                .iter()
                .zip(&fine.liquid)
                .zip(&mesh.areas)
                .map(|((a, b), area)| {
                    total_mass(&[a.high, -b.high, a.low, -b.low]) / (1000. * area)
                })
                .collect();
            let rms = (total_mass(
                &difference
                    .iter()
                    .zip(&mesh.areas)
                    .map(|(d, a)| d * d * a)
                    .collect::<Vec<_>>(),
            ) / total_mass(&mesh.areas))
            .sqrt();
            groups.push(json!({"subdivision":level,"axis":normalize(axis),
                "timeRefinementRmsDifferenceMeters":rms,
                "timeDifferenceOverFineAnalyticRmsError":if fine.error_rms > 0. { Some(rms / fine.error_rms) } else {None},
                "runs":[coarse.result,fine.result]}));
        }
    }
    Ok(json!({"reportVersion":"paired-surface-spatial-report-1",
        "surfaceFlowModelVersion":"regional-surface-flow-paired-2",
        "scope":"Isolated fully wet, flat-bed, capped linear diffusion; not seasonal dynamics, wetting-front validation or hydraulic calibration.",
        "radiusMeters":RADIUS,"diffusivitySquareMetersPerSecond":DIFFUSIVITY,
        "meanDepthMeters":MEAN_DEPTH,"amplitudeMeters":AMPLITUDE,"roughness":0.04,
        "endSeconds":END_SECONDS,"baseSubsteps":base_steps,"auditEverySubsteps":32,
        "initialization":"Continuous l=1 field sampled at region centers times physical cell area; not exact cell averages and not globally rescaled.",
        "analyticReference":"H + A exp(-2 D t / R^2) dot(axis,unitCenter)",
        "uniformControl":uniform.result,"groups":groups,"spatialComparisons":spatial_comparisons}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analytic_reference_has_correct_initial_state_decay_and_rotation() {
        let axis = normalize([1., 2., 3.]);
        assert!((exact_depth(axis, axis, 0., 1.) - 101.).abs() < 1e-13);
        assert!(
            (exact_depth(axis, axis, RADIUS.powi(2) / (2. * DIFFUSIVITY), 1.)
                - (100. + (-1_f64).exp()))
            .abs()
                < 1e-13
        );
        assert_eq!(
            exact_depth([1., 0., 0.], [0., 1., 0.], END_SECONDS, 1.),
            100.
        );
    }
    #[test]
    fn physical_layout_refuses_malformed_fields_without_fake_world_recipes() {
        let mesh = Surface::build(2, RADIUS);
        for (beds, land, level) in [
            (
                vec![0.; mesh.areas.len() - 1],
                vec![true; mesh.areas.len()],
                0.,
            ),
            (vec![0.; mesh.areas.len()], vec![], 0.),
            (
                vec![f64::NAN; mesh.areas.len()],
                vec![true; mesh.areas.len()],
                0.,
            ),
            (
                vec![0.; mesh.areas.len()],
                vec![true; mesh.areas.len()],
                f64::INFINITY,
            ),
        ] {
            assert!(
                surface_flow::Layout::from_fields(
                    &mesh,
                    RADIUS,
                    beds,
                    land,
                    level,
                    Default::default()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn uniform_field_stays_within_depth_roundoff_with_audited_microflows() {
        let r = run(2, AXES[0], 200, 0.).unwrap();
        assert_eq!(r.result["initialKilograms"], r.result["finalKilograms"]);
        assert!(r.result["maximumErrorMeters"].as_f64().unwrap() < 64. * f64::EPSILON * MEAN_DEPTH);
        for (m, area) in r.liquid.iter().zip(Surface::build(2, RADIUS).areas) {
            assert!((m.high / (1000. * area) - MEAN_DEPTH).abs() < 64. * f64::EPSILON * MEAN_DEPTH);
        }
    }
    #[test]
    fn retained_spatial_witness_does_not_masquerade_as_convergence() {
        for axis in AXES {
            let coarse = run(2, axis, 800, 1.).unwrap();
            let finer = run(3, axis, 800, 1.).unwrap();
            let temporal = run(3, axis, 1600, 1.).unwrap();
            assert!(finer.error_rms > coarse.error_rms);
            assert!(
                (finer.error_rms - temporal.error_rms).abs()
                    < 0.01 * (finer.error_rms - coarse.error_rms)
            );
            for r in [&coarse, &finer, &temporal] {
                assert!(r.result["activeDirectedFaces"].as_u64().unwrap() > 0);
                assert!(
                    r.result["resolution"]["summedDeferredRequestKilograms"]
                        .as_f64()
                        .unwrap()
                        .is_finite()
                );
                let flux = &r.result["initialFaceFluxDiagnostic"];
                assert!(flux["relativeFaceFluxL1Error"].as_f64().unwrap() > 0.01);
                assert!(
                    flux["maximumSegmentNonOrthogonalityDegrees"]
                        .as_f64()
                        .unwrap()
                        > 1.
                );
            }
        }
    }
    #[test]
    fn audit_rejects_balanced_teleportation_and_uses_signed_tails() {
        let mesh = Surface::build(2, RADIUS);
        let l = layout(&mesh).unwrap();
        let initial = vec![
            Mass {
                high: 2_f64.powi(50),
                low: 0.
            };
            mesh.areas.len()
        ];
        let mut changed = initial.clone();
        changed[0].low = 0.01;
        changed[1].low = -0.01;
        // Below audit tolerance but retained rather than rounded out of the test.
        let a = audit(
            &l,
            &initial,
            &changed,
            &vec![Mass::default(); l.faces.len() * 2],
        )
        .unwrap();
        assert!(a[0] > 0.);
        assert_eq!(a[1], 0.);
        changed[0].high += 1000.;
        changed[0].low = 0.;
        changed[1].high -= 1000.;
        changed[1].low = 0.;
        assert!(
            audit(
                &l,
                &initial,
                &changed,
                &vec![Mass::default(); l.faces.len() * 2]
            )
            .is_err()
        );
    }
    #[test]
    fn temporal_controls_repeat_exactly_and_refuse_unstable_or_unbounded_work() {
        let a = run(3, AXES[1], 400, 1.).unwrap();
        let b = run(3, AXES[1], 800, 1.).unwrap();
        assert!(a.liquid != b.liquid);
        assert_eq!(
            serde_json::to_string(&b.result).unwrap(),
            serde_json::to_string(&run(3, AXES[1], 800, 1.).unwrap().result).unwrap()
        );
        for (level, steps) in [(0, 400), (6, 400), (3, 0), (3, MAX_STEPS + 1), (5, 1)] {
            assert!(run(level, AXES[0], steps, 1.).is_err());
        }
        assert!(run(2, [0.; 3], 400, 1.).is_err());
    }
}
