//! Frozen-state weak-work screening of the shared nonlinear surface stage.
//! Not an evolving solution, seasonal checkpoint or wetting-front qualification.
use super::{
    Mass, ResolutionBudget, surface, surface_cotan, surface_flow, surface_gradient,
    surface_verification,
};
use crate::{Surface, dot, moisture_transport::total_mass, norm};
use serde_json::{Value, json};

const RADIUS: f64 = 100_000.;
const MEAN: f64 = 1.;
const AMPLITUDE: f64 = 0.1;
const ROUGHNESS: f64 = 0.04;
const SECONDS: f64 = 0.01;
const AXES: [[f64; 3]; 2] = [[1., 0., 0.], [1., 2., 3.]];
const CAPS: [f64; 2] = [100., 1e6];

// Independent continuum weak work on a closed sphere for h=H+A(axis.s):
// integral beta(h,|grad h|) |grad h|^2 dArea
// = 2 pi A^2 integral_{-1}^{1} beta(p)(1-p^2) dp.
// Hydraulic radius is explicitly approximated by depth, as in our closure.
fn continuum_work(cap: f64, intervals: usize) -> Result<f64, String> {
    continuum_work_degree(cap, intervals, 1)
}

fn continuum_work_degree(cap: f64, intervals: usize, degree: u32) -> Result<f64, String> {
    if !cap.is_finite()
        || cap <= 0.
        || !(2..=65536).contains(&intervals)
        || !intervals.is_multiple_of(2)
        || !(1..=2).contains(&degree)
    {
        return Err("Invalid constitutive quadrature settings.".into());
    }
    let values: Vec<_> = (0..=intervals)
        .map(|i| {
            let p = -1. + 2. * i as f64 / intervals as f64;
            let profile = if degree == 1 {
                p
            } else {
                (3. * p * p - 1.) / 2.
            };
            let tangent_squared = if degree == 1 {
                (1. - p * p).max(0.)
            } else {
                (1. - p * p).max(0.) * (3. * p).powi(2)
            };
            let slope = AMPLITUDE / RADIUS * tangent_squared.sqrt();
            let beta = if slope == 0. {
                cap
            } else {
                ((MEAN + AMPLITUDE * profile).powf(5. / 3.) / (ROUGHNESS * slope.sqrt())).min(cap)
            };
            let weight = if i == 0 || i == intervals {
                1.
            } else if i % 2 == 0 {
                2.
            } else {
                4.
            };
            weight * beta * tangent_squared
        })
        .collect();
    Ok(
        2. * std::f64::consts::PI * AMPLITUDE.powi(2) * total_mass(&values) * 2.
            / (3. * intervals as f64),
    )
}

fn run(level: u32, axis: [f64; 3], cap: f64, seconds: f64) -> Result<Value, String> {
    run_selected(level, axis, cap, seconds, false, 1)
}

fn run_selected(
    level: u32,
    axis: [f64; 3],
    cap: f64,
    seconds: f64,
    full_gradient: bool,
    degree: u32,
) -> Result<Value, String> {
    let length = norm(axis);
    if !(2..=5).contains(&level)
        || !length.is_finite()
        || length <= 0.
        || !CAPS.contains(&cap)
        || ![SECONDS, SECONDS / 2.].contains(&seconds)
        || !(1..=2).contains(&degree)
        || (!full_gradient && degree != 1)
    {
        return Err("Unsupported bounded constitutive control.".into());
    }
    let axis = axis.map(|v| v / length);
    let mesh = Surface::build(level, RADIUS);
    let layout = surface_cotan::prepare(
        &mesh,
        surface_flow::Layout::from_fields(
            &mesh,
            RADIUS,
            vec![0.; mesh.areas.len()],
            vec![true; mesh.areas.len()],
            0.,
            surface_flow::Settings {
                roughness: ROUGHNESS,
                maximum_diffusivity_square_meters_per_second: cap,
            },
        )?,
    )?;
    if seconds > layout.stable_seconds {
        return Err("Constitutive control must use one stable frozen substep.".into());
    }
    let initial: Vec<_> = mesh
        .centers
        .iter()
        .zip(&mesh.areas)
        .map(|(&s, &area)| {
            let p = dot(axis, s);
            let profile = if degree == 1 {
                p
            } else {
                (3. * p * p - 1.) / 2.
            };
            Mass {
                high: 1000. * area * (MEAN + AMPLITUDE * profile),
                low: 0.,
            }
        })
        .collect();
    let heads: Vec<_> = initial
        .iter()
        .zip(&mesh.areas)
        .map(|(m, area)| m.high / (1000. * area))
        .collect();
    let gradient = if full_gradient {
        Some(surface_gradient::Prepared::new(
            &mesh,
            layout.clone(),
            RADIUS,
        )?)
    } else {
        None
    };
    let mut liquid = initial.clone();
    let mut histories = vec![Mass::default(); 2 * layout.faces.len()];
    let mut resolution = Some(ResolutionBudget::default());
    let stocks = surface::Stocks {
        by_region: &vec![None; liquid.len()],
        liquid: &mut liquid,
        bodies: &mut [],
        transfers: &mut histories,
        resolution: &mut resolution,
    };
    let count = if let Some(g) = &gradient {
        surface::advance_full_gradient(g, stocks, seconds)?
    } else {
        surface::advance(&layout, stocks, seconds)?
    };
    if count != 1 {
        return Err("Unexpected constitutive substep splitting.".into());
    }
    for m in &liquid {
        m.stock(f64::MAX)?;
    }
    let audits = surface_verification::audit(&layout, &initial, &liquid, &histories)?;
    let work_terms: Vec<_> = layout
        .faces
        .iter()
        .enumerate()
        .flat_map(|(f, face)| {
            let delta = heads[face.regions[0]] - heads[face.regions[1]];
            [
                histories[2 * f].high * delta,
                histories[2 * f].low * delta,
                -histories[2 * f + 1].high * delta,
                -histories[2 * f + 1].low * delta,
            ]
        })
        .collect();
    let work = total_mass(&work_terms) / (1000. * seconds);
    let stock_terms: Vec<_> = initial
        .iter()
        .zip(&liquid)
        .zip(&heads)
        .map(|((a, b), h)| total_mass(&[a.high, -b.high, a.low, -b.low]) * h)
        .collect();
    let stock_work = total_mass(&stock_terms) / (1000. * seconds);
    let reference = if degree == 1 {
        continuum_work(cap, 32768)?
    } else {
        continuum_work_degree(cap, 32768, degree)?
    };
    let coarse_reference = if degree == 1 {
        continuum_work(cap, 16384)?
    } else {
        continuum_work_degree(cap, 16384, degree)?
    };
    let quadrature_difference = (reference - coarse_reference).abs() / reference;
    if !work.is_finite() || work <= 0. || quadrature_difference > 1e-7 {
        return Err("Invalid constitutive work or unresolved reference quadrature.".into());
    }
    if (work - stock_work).abs() / work > 128. * f64::EPSILON {
        return Err("Constitutive graph/stock weak-work identity failed.".into());
    }
    let minimum_distance = layout
        .faces
        .iter()
        .map(|f| f.distance_meters)
        .fold(f64::MAX, f64::min);
    let mut lower_mobility =
        (MEAN - AMPLITUDE).powf(5. / 3.) / (ROUGHNESS * (2. * AMPLITUDE / minimum_distance).sqrt());
    if let Some(g) = &gradient {
        let maximum_slope = g.edge_slopes(&heads)?.into_iter().fold(0., f64::max);
        lower_mobility = (MEAN - AMPLITUDE).powf(5. / 3.) / (ROUGHNESS * maximum_slope.sqrt());
    }
    let resolution = resolution.unwrap();
    resolution.validate()?;
    let mut result = json!({"subdivision":level,"regions":liquid.len(),"axis":axis,
        "capSquareMetersPerSecond":cap,"stepSeconds":seconds,"substeps":count,
        "stabilityBoundSeconds":layout.stable_seconds,
        "minimumInitialUncappedEdgeMobilityBound":lower_mobility,
        "fullyCappedEdgeRegimeCertified":lower_mobility >= cap,
        "continuumWorkMetersFourthPerSecond":reference,
        "coarseQuadratureWorkMetersFourthPerSecond":coarse_reference,
        "relativeReferenceQuadratureDifference":quadrature_difference,
        "completedAdjacencyWorkMetersFourthPerSecond":work,
        "stockReconstructedWorkMetersFourthPerSecond":stock_work,
        "relativeStockWorkIdentityDifference":(work-stock_work).abs()/work,
        "signedRelativeWorkDiscrepancy":work/reference-1.,
        "maximumLocalRelativeResidual":audits[0],"globalRelativeResidual":audits[1],
        "resolution":resolution,"initialLiquid":initial,"finalLiquid":liquid,
        "directedTransfers":histories});
    if full_gradient {
        result["degree"] = json!(degree);
        result["mobilityBoundMeaning"] = json!(
            "Minimum analytic depth^(5/3) divided by roughness times square root of maximum reconstructed initial edge slope"
        );
    }
    Ok(result)
}

/// Fixed meshes/inputs; each case starts afresh and measures one frozen stage.
/// A passed ledger or small caller-step difference is not a passed PDE gate.
pub fn report() -> Result<Value, String> {
    let mut cases = Vec::new();
    for cap in CAPS {
        for axis in AXES {
            for level in 2..=5 {
                for seconds in [SECONDS, SECONDS / 2.] {
                    cases.push(run(level, axis, cap, seconds)?);
                }
            }
        }
    }
    Ok(json!({"reportVersion":"surface-constitutive-work-report-1",
        "surfaceStage":"Unchanged paired stage with cotangent weak-form conductances",
        "scope":"Frozen fully wet flat sphere; necessary weak-work consistency screen against an explicitly chosen isotropic capped diffusive-wave reference. Not an evolution, wetting front, calibrated discharge or seasonal checkpoint.",
        "radiusMeters":RADIUS,"meanDepthMeters":MEAN,"amplitudeMeters":AMPLITUDE,
        "roughness":ROUGHNESS,"quadratureIntervals":[16384,32768],
        "initialization":"Continuous depth point samples times unchanged spherical areas; no mass rescaling",
        "continuumReference":"beta=min(cap,h^(5/3)/(n*sqrt(|grad h|))); work=integral beta |grad h|^2 dArea",
        "implementedSlopeMeaning":"Absolute head difference divided by edge distance, not a reconstructed full gradient magnitude",
        "cases":cases}))
}

/// Explicit standalone candidate. Existing witness/default reports stay unchanged.
pub fn full_gradient_report() -> Result<Value, String> {
    let mut cases = Vec::new();
    for degree in 1..=2 {
        for cap in CAPS {
            for axis in [AXES[0], AXES[1], [0., 1., 0.], [0.31, 0.72, -0.43]] {
                for level in 2..=5 {
                    for seconds in [SECONDS, SECONDS / 2.] {
                        cases.push(run_selected(level, axis, cap, seconds, true, degree)?);
                    }
                }
            }
        }
    }
    Ok(
        json!({"reportVersion":"surface-full-gradient-work-report-1",
        "surfaceModelVersion":surface_gradient::MODEL_VERSION,
        "scope":"Standalone frozen fully wet flat sphere. Not an evolving PDE solution, seasonal checkpoint, dry-front accuracy or default promotion.",
        "radiusMeters":RADIUS,"meanDepthMeters":MEAN,"amplitudeMeters":AMPLITUDE,
        "roughness":ROUGHNESS,"quadratureIntervals":[16384,32768],
        "initialization":"Continuous depth point samples times unchanged spherical areas; no mass rescaling",
        "continuumReference":"beta=min(cap,h^(5/3)/(n*sqrt(|grad h|))); work=integral beta |grad h|^2 dArea",
        "profiles":"h=H+A*P_l(axis.s), l=1,2",
        "implementedSlopeMeaning":"Mean of full chordal P1 gradient magnitudes of the two incident triangles; relative bed plus depth differences",
        "cases":cases}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_quadrature_matches_exact_capped_harmonic_work() {
        let exact = 8. * std::f64::consts::PI / 3. * CAPS[0] * AMPLITUDE.powi(2);
        for intervals in [16, 128, 16384] {
            assert!((continuum_work(CAPS[0], intervals).unwrap() / exact - 1.).abs() < 1e-14);
        }
        let a = continuum_work(CAPS[1], 16384).unwrap();
        let b = continuum_work(CAPS[1], 32768).unwrap();
        let c = continuum_work(CAPS[1], 65536).unwrap();
        assert!((b - c).abs() / c < 1e-7);
        assert!((b - c).abs() < (a - b).abs());
        for (cap, intervals) in [
            (0., 16),
            (f64::NAN, 16),
            (100., 0),
            (100., 3),
            (100., 65538),
        ] {
            assert!(continuum_work(cap, intervals).is_err());
        }
    }
    #[test]
    fn actual_frozen_stage_has_independent_stock_work_and_exact_repetition() {
        let a = run(3, AXES[1], CAPS[1], SECONDS).unwrap();
        let b = run(3, AXES[1], CAPS[1], SECONDS / 2.).unwrap();
        assert!(a["relativeStockWorkIdentityDifference"].as_f64().unwrap() < 1e-9);
        let wa = a["completedAdjacencyWorkMetersFourthPerSecond"]
            .as_f64()
            .unwrap();
        let wb = b["completedAdjacencyWorkMetersFourthPerSecond"]
            .as_f64()
            .unwrap();
        assert!((wa / wb - 1.).abs() < 1e-12);
        assert_eq!(a, run(3, AXES[1], CAPS[1], SECONDS).unwrap());
    }
    #[test]
    fn low_cap_control_refines_without_substituting_a_rate_law() {
        let mut previous = f64::MAX;
        for level in 2..=4 {
            let r = run(level, AXES[1], CAPS[0], SECONDS).unwrap();
            assert_eq!(r["fullyCappedEdgeRegimeCertified"], true);
            let error = r["signedRelativeWorkDiscrepancy"].as_f64().unwrap().abs();
            assert!(error < previous);
            previous = error;
        }
    }
    #[test]
    fn nonlinear_finest_mesh_preserves_the_unpassed_isotropic_work_gate() {
        // A regression witness, not an assertion that the physical gate passes.
        // Keep the existing closure reproducible when a new candidate is added.
        for axis in AXES {
            let r = run(5, axis, CAPS[1], SECONDS).unwrap();
            assert!(!r["fullyCappedEdgeRegimeCertified"].as_bool().unwrap());
            assert!(r["signedRelativeWorkDiscrepancy"].as_f64().unwrap() > 0.1);
            assert!(r["relativeReferenceQuadratureDifference"].as_f64().unwrap() < 1e-7);
        }
    }
    #[test]
    fn bounded_api_refuses_unsupported_inputs() {
        for (level, axis, cap, seconds) in [
            (1, AXES[0], CAPS[0], SECONDS),
            (6, AXES[0], CAPS[0], SECONDS),
            (2, [0.; 3], CAPS[0], SECONDS),
            (2, [f64::NAN; 3], CAPS[0], SECONDS),
            (2, AXES[0], 0., SECONDS),
            (2, AXES[0], CAPS[0], 0.),
        ] {
            assert!(run(level, axis, cap, seconds).is_err());
        }
    }

    #[test]
    fn full_gradient_candidate_preserves_the_completed_capped_stage_exactly() {
        let old = run(3, AXES[1], CAPS[0], SECONDS).unwrap();
        let new = run_selected(3, AXES[1], CAPS[0], SECONDS, true, 1).unwrap();
        assert_eq!(new["fullyCappedEdgeRegimeCertified"], true);
        for key in [
            "finalLiquid",
            "directedTransfers",
            "resolution",
            "completedAdjacencyWorkMetersFourthPerSecond",
        ] {
            assert_eq!(old[key], new[key]);
        }
    }

    #[test]
    fn full_gradient_candidate_repeats_and_has_independent_stock_work() {
        let a = run_selected(3, AXES[1], CAPS[1], SECONDS, true, 1).unwrap();
        assert_eq!(
            a,
            run_selected(3, AXES[1], CAPS[1], SECONDS, true, 1).unwrap()
        );
        let b = run_selected(3, AXES[1], CAPS[1], SECONDS / 2., true, 1).unwrap();
        let wa = a["completedAdjacencyWorkMetersFourthPerSecond"]
            .as_f64()
            .unwrap();
        let wb = b["completedAdjacencyWorkMetersFourthPerSecond"]
            .as_f64()
            .unwrap();
        assert!((wa / wb - 1.).abs() < 1e-12);
        assert!(a["relativeStockWorkIdentityDifference"].as_f64().unwrap() < 1e-12);
    }

    #[test]
    fn second_profile_reference_has_independent_exact_cap_and_quadrature_controls() {
        let exact = 24. * std::f64::consts::PI / 5. * CAPS[0] * AMPLITUDE.powi(2);
        assert!((continuum_work_degree(CAPS[0], 16384, 2).unwrap() / exact - 1.).abs() < 1e-14);
        let a = continuum_work_degree(CAPS[1], 16384, 2).unwrap();
        let b = continuum_work_degree(CAPS[1], 32768, 2).unwrap();
        let c = continuum_work_degree(CAPS[1], 65536, 2).unwrap();
        assert!((b - c).abs() / c < 1e-7);
        assert!((b - c).abs() < (a - b).abs());
        assert!(continuum_work_degree(CAPS[1], 16, 3).is_err());
        assert!(run_selected(2, AXES[0], CAPS[1], SECONDS, false, 2).is_err());
        assert!(run_selected(2, AXES[0], CAPS[1], SECONDS, true, 0).is_err());
    }

    #[test]
    fn full_gradient_two_profiles_improve_on_the_finest_refinement_without_fitting() {
        // Bounded weak-functional regression, not universal PDE convergence.
        // Coarse nonlinear errors can cancel; do not require every pair to fall.
        for degree in 1..=2 {
            for axis in [AXES[0], AXES[1], [0.31, 0.72, -0.43]] {
                let coarse = run_selected(4, axis, CAPS[1], SECONDS, true, degree).unwrap();
                let fine = run_selected(5, axis, CAPS[1], SECONDS, true, degree).unwrap();
                let c = coarse["signedRelativeWorkDiscrepancy"]
                    .as_f64()
                    .unwrap()
                    .abs();
                let f = fine["signedRelativeWorkDiscrepancy"]
                    .as_f64()
                    .unwrap()
                    .abs();
                assert!(f < c && f < 0.01);
                assert_eq!(fine["fullyCappedEdgeRegimeCertified"], false);
            }
        }
    }
}
