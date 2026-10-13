//! Separately scoped first-order affine-bed head baseline, not flat pressure.
use super::{
    faces,
    hypsometry::{self, Bed, Column},
    moments, paired_evolution,
};
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock as Stock};
use serde::Serialize;
use serde_json::{Value, json};

const LIMIT: f64 = 256. * f64::EPSILON;
const MAX_STEPS: usize = 4000;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct Face {
    flux_square_meters_per_second: f64,
    gradient: f64,
    donor_depth_above_crest_meters: f64,
    donor: Option<&'static str>,
    capped_branch_selected: bool,
}

fn ordering(column: &Column) -> (Bed, f64, bool) {
    match *column {
        Column::Dry { bed } => (bed, 0., false),
        Column::Wet { bed, offset } => (bed, offset, true),
    }
}

fn face(left: &Column, right: &Column, width: f64, cap: f64) -> Result<Face, String> {
    if !width.is_finite() || width <= 0. || !cap.is_finite() || cap <= 0. {
        return Err("Invalid affine-bed face width or cap.".into());
    }
    let (lb, lo, lw) = ordering(left);
    let (rb, ro, rw) = ordering(right);
    lb.span()?;
    rb.span()?;
    if !lo.is_finite() || !ro.is_finite() || lo < 0. || ro < 0. || lw && lo == 0. || rw && ro == 0.
    {
        return Err("Invalid affine-bed face head offset.".into());
    }
    // Dry has no inferred head. Its minimum-bed ordering surrogate is the
    // positive-storage limit of this explicitly declared local closure.
    let bed_difference = rb.minimum() - lb.minimum();
    if !bed_difference.is_finite() {
        return Err("Unrepresentable affine-bed relative datum.".into());
    }
    let difference = total_mass(&[bed_difference, ro, -lo]);
    let gradient = difference / width;
    if !gradient.is_finite() {
        return Err("Unrepresentable affine-bed relative head gradient.".into());
    }
    let crest = lb.right.max(rb.left);
    let (bed, offset, wet, donor) = if gradient < 0. {
        (lb, lo, lw, Some("left"))
    } else if gradient > 0. {
        (rb, ro, rw, Some("right"))
    } else {
        (lb, 0., false, None)
    };
    let raw_depth = if wet {
        let crest_offset = crest - bed.minimum();
        if !crest_offset.is_finite() {
            return Err("Unrepresentable affine-bed relative crest.".into());
        }
        total_mass(&[offset, -crest_offset])
    } else {
        0.
    };
    if !raw_depth.is_finite() {
        return Err("Unrepresentable affine-bed crest depth.".into());
    }
    let depth = raw_depth.max(0.);
    let (flux, capped) = if gradient == 0. || depth == 0. {
        (0., false)
    } else {
        let a = cap * gradient.abs();
        let b = depth.powf(5. / 3.) * gradient.abs().sqrt() / 0.04;
        if !a.is_finite() || !b.is_finite() || a <= 0. || b <= 0. {
            return Err("Unrepresentable positive affine-bed flux branch.".into());
        }
        (-gradient.signum() * a.min(b), a <= b)
    };
    Ok(Face {
        flux_square_meters_per_second: flux,
        gradient,
        donor_depth_above_crest_meters: depth,
        donor,
        capped_branch_selected: capped,
    })
}

fn columns(stocks: &[Stock], beds: &[Bed], width: f64) -> Result<Vec<Column>, String> {
    if !(2..=128).contains(&beds.len())
        || stocks.len() != beds.len()
        || !width.is_finite()
        || width <= 0.
    {
        return Err("Invalid bounded affine-bed owner geometry.".into());
    }
    stocks
        .iter()
        .zip(beds)
        .map(|(s, &bed)| {
            Stock::new(s.high, s.low, f64::MAX)?;
            let mean = total_mass(&[s.high, s.low]) / width;
            if s.high > 0. && mean == 0. {
                return Err("Positive affine-bed owned mean underflow.".into());
            }
            hypsometry::reconstruct(mean, bed)
        })
        .collect()
}

fn fluxes(
    stocks: &[Stock],
    beds: &[Bed],
    width: f64,
    cap: f64,
) -> Result<(Vec<f64>, Vec<Face>), String> {
    let fitted = columns(stocks, beds, width)?;
    let observed = fitted
        .windows(2)
        .map(|c| face(&c[0], &c[1], width, cap))
        .collect::<Result<Vec<_>, _>>()?;
    let mut q = vec![0.];
    q.extend(observed.iter().map(|f| f.flux_square_meters_per_second));
    q.push(0.);
    Ok((q, observed))
}

fn owners(step: &faces::Step) -> Result<Vec<Stock>, String> {
    step.volume_high
        .iter()
        .zip(&step.volume_low)
        .map(|(&h, &l)| Stock::new(h, l, f64::MAX))
        .collect()
}

fn bits(stocks: &[Stock]) -> Vec<[u64; 2]> {
    stocks
        .iter()
        .map(|s| [s.high.to_bits(), s.low.to_bits()])
        .collect()
}

fn trajectory(
    name: &str,
    means: &[f64],
    beds: &[Bed],
    dt: f64,
    steps: usize,
    cap: f64,
) -> Result<Value, String> {
    if !(2..=128).contains(&beds.len())
        || steps == 0
        || steps > MAX_STEPS
        || steps * beds.len() > 192_000
        || !dt.is_finite()
        || dt <= 0.
        || !(steps as f64 * dt).is_finite()
        || !cap.is_finite()
        || cap <= 0.
        || means.len() != beds.len()
    {
        return Err("Invalid bounded affine-bed trajectory work.".into());
    }
    let initial = paired_evolution::initialize_means(means, 1.)?.stocks;
    let mut state = initial.clone();
    let mut maximum_flux: f64 = 0.;
    let mut maximum_local: f64 = 0.;
    let mut maximum_global: f64 = 0.;
    let mut cumulative_residual: f64 = 0.;
    let mut every_active_face_capped = true;
    let mut refusal = None;
    let mut accepted = 0;
    let mut first_faces = Vec::new();
    for i in 0..steps {
        let proposed = (|| {
            let (q, f) = fluxes(&state, beds, 1., cap)?;
            if i == 0 {
                first_faces = f.clone();
            }
            maximum_flux = maximum_flux.max(q.iter().map(|v| v.abs()).fold(0., f64::max));
            every_active_face_capped &= f
                .iter()
                .filter(|f| f.flux_square_meters_per_second != 0.)
                .all(|f| f.capped_branch_selected);
            faces::step_owned(&state, 1., &q, dt)
        })();
        match proposed {
            Err(reason) => {
                refusal = Some(
                    json!({"attemptedStep":accepted+1,"startTimeSeconds":accepted as f64*dt,"candidateCommitted":false,"reason":reason}),
                );
                break;
            }
            Ok(step) => {
                let candidate = owners(&step)?;
                let residual = total_mass(
                    &candidate
                        .iter()
                        .zip(&initial)
                        .flat_map(|(s, b)| [s.high - b.high, s.low, -b.low])
                        .collect::<Vec<_>>(),
                )
                .abs();
                let total = total_mass(
                    &initial
                        .iter()
                        .flat_map(|s| [s.high, s.low])
                        .collect::<Vec<_>>(),
                );
                let relative = if total == 0. {
                    residual
                } else {
                    residual / total
                };
                if !relative.is_finite() || relative > LIMIT {
                    refusal = Some(
                        json!({"attemptedStep":accepted+1,"startTimeSeconds":accepted as f64*dt,"candidateCommitted":false,"reason":"Affine-bed cumulative ledger bound exceeded."}),
                    );
                    break;
                }
                maximum_local = maximum_local.max(step.maximum_local_relative_ledger_residual);
                maximum_global = maximum_global.max(step.relative_inventory_residual);
                cumulative_residual = cumulative_residual.max(relative);
                state = candidate;
                accepted += 1;
            }
        }
    }
    Ok(
        json!({"name":name,"beds":beds,"initialMeansMeters":means,"stepSeconds":dt,
        "capSquareMetersPerSecond":cap,"roughness":0.04,"requestedSteps":steps,"acceptedSteps":accepted,
        "completed":accepted==steps,"acceptedTimeSeconds":accepted as f64*dt,"refusal":refusal,
        "initialOwnerComponents":initial.iter().map(|s|[s.high,s.low]).collect::<Vec<_>>(),
        "finalOwnerComponents":state.iter().map(|s|[s.high,s.low]).collect::<Vec<_>>(),
        "ownersBitIdentical":bits(&initial)==bits(&state),"firstFaces":first_faces,
        "initialColumns":columns(&initial,beds,1.)?,
        "computedFaceDiagnosticContract":"Computed faces include a refused attempt, if any; only final owners/acceptedSteps denote committed state.",
        "maximumComputedAbsoluteFluxSquareMetersPerSecond":maximum_flux,"everyActiveFaceCapped":every_active_face_capped,
        "maximumLocalRelativeLedgerResidual":maximum_local,"maximumStepRelativeInventoryResidual":maximum_global,
        "maximumCumulativeRelativeInventoryResidual":cumulative_residual,
        "boundaryInputSquareMeters":0.,"boundaryOutputSquareMeters":0.}),
    )
}

fn segments(endpoints: &[f64]) -> Vec<Bed> {
    endpoints
        .windows(2)
        .map(|x| Bed {
            left: x[0],
            right: x[1],
        })
        .collect()
}

fn rest_cases() -> Result<Vec<Value>, String> {
    let fixtures = [
        (
            "shoreline-bowl",
            segments(&[1., 0.75, 0.25, 0., 0.25, 0.75, 1.]),
            vec![0., 0.0625, 0.375, 0.375, 0.0625, 0.],
        ),
        (
            "disconnected-unequal-pools",
            segments(&[0., 0., 2., 2., 0., 0.]),
            vec![0.5, 0.0625, 0., 0.015625, 0.25],
        ),
        (
            "fully-wet-alternating-bed",
            segments(&[0., 0.5, 0., 0.25, 0., 0.5, 0.]),
            vec![0.75, 0.75, 0.875, 0.875, 0.75, 0.75],
        ),
        (
            "discontinuous-bed-rest",
            [0., 0.25, 0.75, 0.5]
                .iter()
                .map(|&z| Bed { left: z, right: z })
                .collect(),
            vec![1., 0.75, 0.25, 0.5],
        ),
        ("uniform-flat", segments(&[0.; 5]), vec![0.5; 4]),
        (
            "dry-variable-bed",
            segments(&[0., 1., 0., 2., 1.]),
            vec![0.; 4],
        ),
    ];
    let mut results = Vec::new();
    for (name, beds, means) in fixtures {
        for datum in [0., 2_f64.powi(40)] {
            for reflected in [false, true] {
                let mut b: Vec<_> = beds
                    .iter()
                    .map(|b| Bed {
                        left: b.left + datum,
                        right: b.right + datum,
                    })
                    .collect();
                let mut m = means.clone();
                if reflected {
                    b = b
                        .into_iter()
                        .rev()
                        .map(|b| Bed {
                            left: b.right,
                            right: b.left,
                        })
                        .collect();
                    m.reverse();
                }
                let mut run = trajectory(name, &m, &b, 0.001, 1000, 100.)?;
                run["datumMeters"] = json!(datum);
                run["reflected"] = json!(reflected);
                results.push(run);
            }
        }
    }
    Ok(results)
}

fn coverage() -> Result<Vec<Value>, String> {
    [ ("uniform",vec![1.,1.,1.]), ("peak",vec![1.,2.,1.]),
      ("trough",vec![2.,1.,2.]), ("mixed-plateau",vec![1.,1.,0.5]),
      ("isolated-wet",vec![0.,1.,0.]), ("bowl-rest-depths",vec![0.,0.0625,0.375,0.375,0.0625,0.]) ]
        .into_iter().map(|(name,means)| {
            let cells=moments::cells(&means,1.)?;
            let unresolved: Vec<_>=cells.iter().enumerate().filter_map(|(i,c)|match c {
                moments::Cell::Unresolved(reason)=>Some(json!({"cell":i,"reason":reason})),_=>None}).collect();
            let state=paired_evolution::initialize_means(&means,1.)?;
            let before=bits(&state.stocks);
            let attempted=paired_evolution::advance_for(&state,&state,1.,0.001,paired_evolution::Method::PressurePenaltyOne,[0.,0.]);
            Ok(json!({"name":name,"meansMeters":means,"unresolved":unresolved,
                "flatOperatorAttempt":match attempted {Ok(_)=>json!({"accepted":true,"reason":null}),Err(e)=>json!({"accepted":false,"reason":e})},
                "inputsUnchanged":before==bits(&state.stocks),"accuracyComparison":null}))
        }).collect()
}

fn interface_controls() -> Result<Vec<Value>, String> {
    let mut results = Vec::new();
    // Exact dyadic crest and adjacent binary64 elevations, not a tolerance band.
    for (name, mean) in [
        ("below-crest", 0.5),
        ("at-crest", 1.),
        ("one-ulp-above-crest", f64::from_bits(1_f64.to_bits() + 1)),
        ("above-crest", 1.25),
    ] {
        let beds = [
            Bed {
                left: 0.,
                right: 0.,
            },
            Bed {
                left: 1.,
                right: 1.,
            },
        ];
        let means = [mean, 0.];
        let initial = paired_evolution::initialize_means(&means, 1.)?.stocks;
        let (q, f) = fluxes(&initial, &beds, 1., 100.)?;
        let stepped = faces::step_owned(&initial, 1., &q, 0.001)?;
        results.push(
            json!({"name":name,"meansMeters":means,"beds":beds,"faces":f,
            "acceptedStep":stepped,"accuracyComparison":null}),
        );
    }
    for bed in [
        Bed {
            left: 10.,
            right: 0.,
        },
        Bed {
            left: 0.,
            right: 10.,
        },
    ] {
        for mean in [1e-12, 1e-24] {
            let donor = hypsometry::reconstruct(
                11.,
                Bed {
                    left: 0.,
                    right: 0.,
                },
            )?;
            let dry = hypsometry::reconstruct(0., bed)?;
            let tiny = hypsometry::reconstruct(mean, bed)?;
            let a = face(&donor, &dry, 1., 100.)?;
            let b = face(&donor, &tiny, 1., 100.)?;
            results.push(json!({"name":"dry-positive-storage-limit","receiverBed":bed,"tinyMeanMeters":mean,
            "dryFace":a,"positiveFace":b,"absoluteFluxDifferenceSquareMetersPerSecond":(a.flux_square_meters_per_second-b.flux_square_meters_per_second).abs(),
            "contract":"Dry uses minimum-bed ordering surrogate, not a physical inferred head or face-bed surrogate."}));
        }
    }
    Ok(results)
}

fn relaxation() -> Result<Vec<Value>, String> {
    let mut results = Vec::new();
    let exact_difference = 0.5 * (-0.02_f64).exp();
    let exact = [
        1.25 + exact_difference / 2.,
        1.25 - exact_difference / 2. - 0.25,
    ];
    let beds = [
        Bed {
            left: 0.,
            right: 0.,
        },
        Bed {
            left: 0.25,
            right: 0.25,
        },
    ];
    for dt in [0.001, 0.0005, 0.00025] {
        let mut run = trajectory(
            "capped-two-owner-relaxation",
            &[1.5, 0.75],
            &beds,
            dt,
            (1. / dt) as usize,
            0.01,
        )?;
        let error = if run["completed"] == true {
            Some(
                (0..2)
                    .map(|i| {
                        total_mass(&[
                            run["finalOwnerComponents"][i][0].as_f64().unwrap() - exact[i],
                            run["finalOwnerComponents"][i][1].as_f64().unwrap(),
                        ])
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };
        run["exactDiscreteOdeMeansMeters"] = json!(exact);
        run["signedMeanErrorsMeters"] = json!(error);
        run["oracleContract"] = json!(
            "Independent d'= -2*cap*d on two width-1 fully wet constant-bed owners; not a continuum PDE reference."
        );
        results.push(run);
    }
    Ok(results)
}

pub(super) fn report() -> Result<Value, String> {
    let rest = rest_cases()?;
    let roundoff_beds = segments(&[0., 0.1, 0.33, 0.17, 0.9, 0.23, 0.]);
    // Independent geometric initialization: trapezoid for a full wet interval,
    // triangle for the part crossing H=.5. These heads are not solver inputs.
    let roundoff_means: Vec<_> = roundoff_beds
        .iter()
        .map(|b| {
            let a = (0.5 - b.left).max(0.);
            let c = (0.5 - b.right).max(0.);
            if b.left < 0.5 && b.right < 0.5 {
                (a + c) / 2.
            } else {
                (a + c).powi(2) / (2. * (b.right - b.left).abs())
            }
        })
        .collect();
    Ok(json!({"reportVersion":"surface-topography-report-1",
        "scope":"Separate first-order head/storage/face-crest baseline on bounded monotone affine owners; no pressure/spherical/seasonal selection",
        "widthMeters":1.,"maximumOwners":128,"maximumSteps":MAX_STEPS,"maximumCellSteps":192000,
        "physicalLaw":"q=-sign(g)*min(cap*abs(g), depthAboveCrest^(5/3)*sqrt(abs(g))/roughness)",
        "crestContract":"Maximum adjacent interface bed traces, not center beds; no unresolved internal ridge in an affine owner",
        "dryContract":"No inferred dry physical head; minimum-bed ordering surrogate is the positive-storage limit; dry donor flux remains zero",
        "stateContract":"Original compensated closed single-face transaction; no film, epsilon, snapping, resized request or fallback",
        "restCases":rest,"interfaces":interface_controls()?,"flatPressureCoverage":coverage()?,
        "roundoffControl":trajectory("non-dyadic-rest-input",&roundoff_means,&roundoff_beds,0.001,1000,100.)?,
        "refusalControl":trajectory("complete-old-donor-overdraw",&[1.5,0.75],&[Bed{left:0.,right:0.},Bed{left:0.25,right:0.25}],1e6,1,100.)?,
        "relaxation":relaxation()?,"qualification":"Bounded storage/rest/blocking and discrete-ODE time checks only; variable-bed PDE accuracy and general reconstruction remain open"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn topographic_rest_and_barriers_preserve_owned_components() {
        for c in rest_cases().unwrap() {
            assert_eq!(c["completed"], true);
            assert_eq!(c["ownersBitIdentical"], true);
            assert_eq!(c["maximumComputedAbsoluteFluxSquareMetersPerSecond"], 0.);
        }
        let controls = interface_controls().unwrap();
        for c in &controls[..2] {
            assert_eq!(c["faces"][0]["fluxSquareMetersPerSecond"], 0.);
        }
        for c in &controls[2..4] {
            assert!(c["faces"][0]["fluxSquareMetersPerSecond"].as_f64().unwrap() > 0.);
        }
        for c in &controls[4..] {
            let bound = if c["tinyMeanMeters"] == 1e-12 {
                1e-3
            } else {
                1e-9
            };
            assert!(
                c["absoluteFluxDifferenceSquareMetersPerSecond"]
                    .as_f64()
                    .unwrap()
                    < bound
            );
        }
        let (pairs, remainder) = controls[4..].as_chunks::<2>();
        assert!(remainder.is_empty());
        for pair in pairs {
            assert!(
                pair[1]["absoluteFluxDifferenceSquareMetersPerSecond"]
                    .as_f64()
                    .unwrap()
                    < pair[0]["absoluteFluxDifferenceSquareMetersPerSecond"]
                        .as_f64()
                        .unwrap()
            );
        }
    }
    #[test]
    fn terrain_baseline_retains_coverage_refusals_and_temporal_error() {
        let r = report().unwrap();
        assert_eq!(r["restCases"].as_array().unwrap().len(), 24);
        assert_eq!(r["roundoffControl"]["completed"], true);
        assert_eq!(r["roundoffControl"]["ownersBitIdentical"], false);
        assert!(
            r["roundoffControl"]["maximumComputedAbsoluteFluxSquareMetersPerSecond"]
                .as_f64()
                .unwrap()
                > 0.
        );
        assert_eq!(r["refusalControl"]["acceptedSteps"], 0);
        assert_eq!(r["refusalControl"]["ownersBitIdentical"], true);
        assert_eq!(r["refusalControl"]["refusal"]["candidateCommitted"], false);
        assert_eq!(r["refusalControl"]["acceptedTimeSeconds"], 0.);
        assert_eq!(r["refusalControl"]["refusal"]["attemptedStep"], 1);
        assert!(
            r["refusalControl"]["maximumComputedAbsoluteFluxSquareMetersPerSecond"]
                .as_f64()
                .unwrap()
                > 0.
        );
        for c in r["restCases"]
            .as_array()
            .unwrap()
            .iter()
            .chain(r["relaxation"].as_array().unwrap())
            .chain(std::iter::once(&r["roundoffControl"]))
        {
            for key in [
                "maximumLocalRelativeLedgerResidual",
                "maximumStepRelativeInventoryResidual",
                "maximumCumulativeRelativeInventoryResidual",
            ] {
                assert!(c[key].as_f64().unwrap() <= LIMIT);
            }
        }
        for c in r["flatPressureCoverage"].as_array().unwrap().iter().skip(1) {
            assert_eq!(c["flatOperatorAttempt"]["accepted"], false);
            assert_eq!(c["inputsUnchanged"], true);
        }
        let errors: Vec<_> = r["relaxation"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                assert_eq!(c["completed"], true);
                assert_eq!(c["everyActiveFaceCapped"], true);
                c["signedMeanErrorsMeters"][0].as_f64().unwrap().abs()
            })
            .collect();
        for e in errors.windows(2) {
            assert!((1.99..2.01).contains(&(e[0] / e[1])));
        }
    }
    #[test]
    fn terrain_faces_reflect_and_invalid_transactions_do_not_mutate() {
        let a = hypsometry::reconstruct(
            1.5,
            Bed {
                left: 0.,
                right: 0.,
            },
        )
        .unwrap();
        let b = hypsometry::reconstruct(
            0.75,
            Bed {
                left: 0.25,
                right: 0.25,
            },
        )
        .unwrap();
        assert_eq!(
            face(&a, &b, 1., 100.)
                .unwrap()
                .flux_square_meters_per_second,
            -face(&b, &a, 1., 100.)
                .unwrap()
                .flux_square_meters_per_second
        );
        for width in [0., -1., f64::NAN] {
            assert!(face(&a, &b, width, 100.).is_err());
        }
        for cap in [0., -1., f64::NAN] {
            assert!(face(&a, &b, 1., cap).is_err());
        }
        for invalid in [
            Column::Dry {
                bed: Bed {
                    left: f64::NAN,
                    right: 0.,
                },
            },
            Column::Dry {
                bed: Bed {
                    left: 0.,
                    right: f64::NAN,
                },
            },
            Column::Wet {
                bed: Bed {
                    left: 0.,
                    right: 0.,
                },
                offset: f64::NAN,
            },
            Column::Wet {
                bed: Bed {
                    left: 0.,
                    right: 0.,
                },
                offset: 0.,
            },
            Column::Wet {
                bed: Bed {
                    left: -f64::MAX,
                    right: -f64::MAX,
                },
                offset: 1.,
            },
        ] {
            assert!(face(&invalid, &b, 1., 100.).is_err());
        }
        let overflow_donor = Column::Wet {
            bed: Bed {
                left: -f64::MAX / 2.,
                right: -f64::MAX / 2.,
            },
            offset: f64::MAX,
        };
        let overflow_crest = Column::Dry {
            bed: Bed {
                left: f64::MAX,
                right: 0.,
            },
        };
        assert!(face(&overflow_donor, &overflow_crest, 1., 100.).is_err());
        let initial = paired_evolution::initialize_means(&[1.5, 0.75], 1.)
            .unwrap()
            .stocks;
        let before = bits(&initial);
        let beds = [
            Bed {
                left: 0.,
                right: 0.,
            },
            Bed {
                left: 0.25,
                right: 0.25,
            },
        ];
        let (q, _) = fluxes(&initial, &beds, 1., 100.).unwrap();
        assert!(faces::step_owned(&initial, 1., &q, 1e6).is_err());
        assert_eq!(bits(&initial), before);
        assert!(columns(&initial, &beds[..1], 1.).is_err());
        assert!(trajectory("invalid", &[1.5, 0.75], &beds, 0.001, 4001, 100.).is_err());
        assert!(trajectory("invalid-clock", &[1.5, 0.75], &beds, f64::MAX, 2, 100.).is_err());
    }
}
