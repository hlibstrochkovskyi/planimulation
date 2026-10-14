//! Separately named affine-head/supported-minmod/penalty-one hypothesis.
//! Accepted integrated stocks remain owners; this is not a production selector.
use super::hypsometry::{self, Bed, Column};
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock as Stock};
use serde::Serialize;

const LIMIT: f64 = 256. * f64::EPSILON;
const ROUGHNESS: f64 = 0.04;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Face {
    pub(super) flux_square_meters_per_second: f64,
    pub(super) gradient: f64,
    pub(super) donor_depth_above_crest_meters: f64,
    pub(super) capped_branch_selected: bool,
}

#[derive(Clone, Copy, Debug)]
struct Head {
    bed: Bed,
    /// Head intercept = actual bed minimum + anchor + offset. Keep anchor and
    /// positive storage offset separate until each physical difference forms.
    anchor: f64,
    offset: f64,
    slope: f64,
    wet: bool,
}

fn minmod(left: f64, right: f64) -> f64 {
    if left == 0. || right == 0. || left.is_sign_negative() != right.is_sign_negative() {
        0.
    } else {
        left.signum() * left.abs().min(right.abs())
    }
}

fn difference(left: Head, right: Head) -> Result<f64, String> {
    let datum = right.bed.minimum() - left.bed.minimum();
    if !datum.is_finite() {
        return Err("Unrepresentable affine-head relative datum.".into());
    }
    let result = total_mass(&[
        datum,
        right.anchor,
        right.offset,
        -left.anchor,
        -left.offset,
    ]);
    if !result.is_finite() {
        return Err("Unrepresentable affine-head difference.".into());
    }
    Ok(result)
}

/// Independent trapezoid/triangle integration of two unclipped depth traces.
fn trace_mean(left: f64, right: f64) -> Result<f64, String> {
    if !left.is_finite() || !right.is_finite() {
        return Err("Nonfinite affine-head integrated trace.".into());
    }
    if left <= 0. && right <= 0. {
        return Ok(0.);
    }
    let result = if left >= 0. && right >= 0. {
        // Sum first when representable: independently halving two smallest
        // subnormals would erase an exactly representable positive mean.
        let sum = left + right;
        if sum.is_finite() {
            0.5 * sum
        } else {
            total_mass(&[0.5 * left, 0.5 * right])
        }
    } else {
        let positive = left.max(right);
        let negative = left.min(right);
        let span = total_mass(&[positive, -negative]);
        if !span.is_finite() || span <= 0. {
            return Err("Unrepresentable affine-head clipped trace span.".into());
        }
        (0.5 * positive) * (positive / span)
    };
    if !result.is_finite() || result <= 0. {
        return Err("Unrepresentable positive affine-head integrated mean.".into());
    }
    Ok(result)
}

fn fit(mean: f64, bed: Bed, slope: f64, width: f64) -> Result<Head, String> {
    bed.span()?;
    if !mean.is_finite() || mean < 0. || !slope.is_finite() {
        return Err("Invalid affine-head fit parameters.".into());
    }
    if mean == 0. {
        // Dry ordering has only the minimum-bed surrogate and no fitted slope.
        return Ok(Head {
            bed,
            anchor: 0.,
            offset: 0.,
            slope: 0.,
            wet: false,
        });
    }
    let half_change = slope * (0.5 * width);
    if !half_change.is_finite() || slope != 0. && half_change == 0. {
        return Err("Unrepresentable affine-head half-cell variation.".into());
    }
    let minimum = bed.minimum();
    // Subtract head variation from the bed, then invert exactly the same
    // accepted mean. Positive-part geometry never overwrites owned stock.
    let effective = Bed {
        left: total_mass(&[bed.left - minimum, half_change]),
        right: total_mass(&[bed.right - minimum, -half_change]),
    };
    let Column::Wet { offset, .. } = hypsometry::reconstruct(mean, effective)? else {
        return Err("Positive affine-head fit unexpectedly reconstructed dry.".into());
    };
    let anchor = effective.minimum();
    let left_depth = total_mass(&[offset, anchor - effective.left]);
    let right_depth = total_mass(&[offset, anchor - effective.right]);
    let reintegrated = trace_mean(left_depth, right_depth)?;
    let residual = (reintegrated - mean).abs() / mean;
    if !residual.is_finite() || residual > LIMIT {
        return Err(format!(
            "Affine-head reintegration exceeds the separate moment limit: mean={mean:.17e}, reconstructed={reintegrated:.17e}, left={left_depth:.17e}, right={right_depth:.17e}, slope={slope:.17e}, effectiveMinimum={:.17e}, offset={offset:.17e}, residual={residual:.17e}.",
            effective.minimum()
        ));
    }
    Ok(Head {
        bed,
        anchor,
        offset,
        slope,
        wet: true,
    })
}

fn heads(stocks: &[Stock], beds: &[Bed], width: f64) -> Result<Vec<Head>, String> {
    if !(2..=128).contains(&beds.len())
        || stocks.len() != beds.len()
        || !width.is_finite()
        || width <= 0.
        || 0.5 * width == 0.
    {
        return Err("Invalid bounded affine-head owner geometry.".into());
    }
    let means = stocks
        .iter()
        .map(|s| {
            Stock::new(s.high, s.low, f64::MAX)?;
            let mean = total_mass(&[s.high, s.low]) / width;
            if !mean.is_finite() || mean < 0. || s.high > 0. && mean == 0. {
                return Err("Unrepresentable affine-head owned mean.".into());
            }
            Ok(mean)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let initial = means
        .iter()
        .zip(beds)
        .map(|(&mean, &bed)| fit(mean, bed, 0., width))
        .collect::<Result<Vec<_>, _>>()?;
    initial
        .iter()
        .enumerate()
        .map(|(i, h)| {
            // Wet owners use the declared minimum-bed dry ordering too: a
            // neighbor becoming wet cannot abruptly activate a finite slope.
            // The geometrically supported half-cell change is bounded by the
            // owner's zero-order head offset, so its own slope tends to zero
            // with its positive stock. This is an explicit numerical closure.
            let slope = if i > 0 && i + 1 < initial.len() && h.wet {
                let left = difference(initial[i - 1], *h)? / width;
                let right = difference(*h, initial[i + 1])? / width;
                if !left.is_finite() || !right.is_finite() {
                    return Err("Unrepresentable affine-head limiter gradient.".into());
                }
                let raw = minmod(left, right);
                let change = raw.abs() * (0.5 * width);
                if !change.is_finite() || raw != 0. && change == 0. {
                    return Err("Unrepresentable supported-minmod half-cell change.".into());
                }
                if change > h.offset {
                    let bound = h.offset / (0.5 * width);
                    if !bound.is_finite() || bound <= 0. {
                        return Err("Unrepresentable positive supported-minmod slope bound.".into());
                    }
                    raw.signum() * bound
                } else {
                    raw
                }
            } else {
                0.
            };
            fit(means[i], beds[i], slope, width)
        })
        .collect()
}

fn face(left: Head, right: Head, width: f64, cap: f64) -> Result<Face, String> {
    let left_variation = left.slope * (0.5 * width);
    let right_variation = -right.slope * (0.5 * width);
    let datum = right.bed.minimum() - left.bed.minimum();
    if !left_variation.is_finite() || !right_variation.is_finite() || !datum.is_finite() {
        return Err("Unrepresentable affine-head face trace/datum.".into());
    }
    let jump = total_mass(&[
        datum,
        right.anchor,
        right.offset,
        right_variation,
        -left.anchor,
        -left.offset,
        -left_variation,
    ]);
    // Penalty one cancels the mean slopes algebraically in fully wet owners,
    // leaving the center-head secant. The candidate improves the donor trace
    // and partial-wetting intercept, not the order of that interior gradient.
    let gradient = total_mass(&[0.5 * left.slope, 0.5 * right.slope, jump / width]);
    if !gradient.is_finite() {
        return Err("Unrepresentable affine-head face gradient.".into());
    }
    let crest = left.bed.right.max(right.bed.left);
    let (donor, variation) = if gradient < 0. {
        (left, left_variation)
    } else {
        (right, right_variation)
    };
    let raw_depth = if gradient != 0. && donor.wet {
        let crest_offset = crest - donor.bed.minimum();
        if !crest_offset.is_finite() {
            return Err("Unrepresentable affine-head crest offset.".into());
        }
        total_mass(&[donor.anchor, donor.offset, variation, -crest_offset])
    } else {
        0.
    };
    if !raw_depth.is_finite() {
        return Err("Unrepresentable affine-head donor depth.".into());
    }
    let depth = raw_depth.max(0.);
    let (flux, capped) = if gradient == 0. || depth == 0. {
        (0., false)
    } else {
        let capped = cap * gradient.abs();
        let manning = depth.powf(5. / 3.) * gradient.abs().sqrt() / ROUGHNESS;
        if !capped.is_finite() || !manning.is_finite() || capped <= 0. || manning <= 0. {
            return Err("Unrepresentable positive affine-head flux branch.".into());
        }
        (-gradient.signum() * capped.min(manning), capped <= manning)
    };
    Ok(Face {
        flux_square_meters_per_second: flux,
        gradient,
        donor_depth_above_crest_meters: depth,
        capped_branch_selected: capped,
    })
}

pub(super) fn fluxes(
    stocks: &[Stock],
    beds: &[Bed],
    width: f64,
    cap: f64,
) -> Result<(Vec<f64>, Vec<Face>), String> {
    if !cap.is_finite() || cap <= 0. {
        return Err("Invalid affine-head flux cap.".into());
    }
    let fitted = heads(stocks, beds, width)?;
    let observed = fitted
        .windows(2)
        .map(|h| face(h[0], h[1], width, cap))
        .collect::<Result<Vec<_>, _>>()?;
    let mut q = vec![0.];
    q.extend(observed.iter().map(|f| f.flux_square_meters_per_second));
    q.push(0.);
    Ok((q, observed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasonal_moisture::regional_soil::surface_front::faces;

    #[test]
    fn positive_trace_average_preserves_subnormals_without_large_sum_overflow() {
        let tiny = f64::from_bits(1);
        assert_eq!(trace_mean(tiny, tiny).unwrap().to_bits(), 1);
        assert_eq!(trace_mean(tiny, 2. * tiny).unwrap().to_bits(), 2);
        assert_eq!(trace_mean(f64::MAX, f64::MAX).unwrap(), f64::MAX);
        let owners = [Stock::new(tiny, 0., f64::MAX).unwrap(); 3];
        let beds = [Bed {
            left: 0.,
            right: 0.,
        }; 3];
        let (q, _) = fluxes(&owners, &beds, 1., 100.).unwrap();
        assert!(q.iter().all(|v| *v == 0.));
    }

    fn stocks(means: &[f64], width: f64) -> Vec<Stock> {
        means
            .iter()
            .map(|h| Stock::new(h * width, 0., f64::MAX).unwrap())
            .collect()
    }

    #[test]
    fn fitted_affine_head_reintegrates_full_partial_and_reflected_stocks() {
        for bed in [
            Bed {
                left: 0.,
                right: 2.,
            },
            Bed {
                left: 2.,
                right: 0.,
            },
        ] {
            for mean in [0.03125, 0.25, 1., 3.] {
                for slope in [-4., -0.5, 0., 0.5, 4.] {
                    let h = fit(mean, bed, slope, 1.).unwrap();
                    let dl = total_mass(&[
                        h.anchor,
                        h.offset,
                        -0.5 * slope,
                        -(bed.left - bed.minimum()),
                    ]);
                    let dr = total_mass(&[
                        h.anchor,
                        h.offset,
                        0.5 * slope,
                        -(bed.right - bed.minimum()),
                    ]);
                    assert!((trace_mean(dl, dr).unwrap() - mean).abs() / mean <= LIMIT);
                    let reflected = fit(
                        mean,
                        Bed {
                            left: bed.right,
                            right: bed.left,
                        },
                        -slope,
                        1.,
                    )
                    .unwrap();
                    assert_eq!(h.anchor.to_bits(), reflected.anchor.to_bits());
                    assert_eq!(h.offset.to_bits(), reflected.offset.to_bits());
                }
            }
        }
    }

    #[test]
    fn smooth_affine_head_uses_a_stock_consistent_donor_trace() {
        let beds = vec![
            Bed {
                left: 0.,
                right: 0.
            };
            5
        ];
        let owners = stocks(&[2., 1.75, 1.5, 1.25, 1.], 1.);
        let fitted = heads(&owners, &beds, 1.).unwrap();
        assert_eq!(fitted[0].slope, 0.);
        assert_eq!(fitted[2].slope, -0.25);
        assert_eq!(fitted[2].anchor + fitted[2].offset, 1.5);
        let (_, f) = fluxes(&owners, &beds, 1., 100.).unwrap();
        assert_eq!(f[2].gradient, -0.25);
        assert_eq!(f[2].donor_depth_above_crest_meters, 1.375);
        let expected = 1.375_f64.powf(5. / 3.) * 0.25_f64.sqrt() / ROUGHNESS;
        assert!((f[2].flux_square_meters_per_second - expected).abs() / expected <= LIMIT);
    }

    #[test]
    fn tiny_partial_mean_survives_nonzero_effective_bed_anchor() {
        let bed = Bed {
            left: 0.000125,
            right: 0.,
        };
        for mean in [2.409_183_028_632_938_2e-9, 1e-24] {
            for slope in [-0.001_006_045_441_282_122_3, -0.0015, 0.0015] {
                let h = fit(mean, bed, slope, 0.125).unwrap();
                let half = slope * 0.0625;
                let effective = Bed {
                    left: total_mass(&[bed.left, half]),
                    right: -half,
                };
                let dl = total_mass(&[h.offset, h.anchor - effective.left]);
                let dr = total_mass(&[h.offset, h.anchor - effective.right]);
                assert!((trace_mean(dl, dr).unwrap() - mean).abs() / mean <= LIMIT);
                assert!(h.offset > 0.);
                let reflected = fit(
                    mean,
                    Bed {
                        left: bed.right,
                        right: bed.left,
                    },
                    -slope,
                    0.125,
                )
                .unwrap();
                assert_eq!(h.anchor.to_bits(), reflected.anchor.to_bits());
                assert_eq!(h.offset.to_bits(), reflected.offset.to_bits());
            }
        }
    }

    #[test]
    fn extrema_plateaus_and_isolated_wet_owner_have_zero_slopes() {
        let beds = vec![
            Bed {
                left: 0.,
                right: 0.
            };
            5
        ];
        for means in [
            [1., 2., 3., 2., 1.],
            [3., 2., 1., 2., 3.],
            [1., 2., 2., 2., 3.],
            [0., 0., 1., 0., 0.],
        ] {
            let fitted = heads(&stocks(&means, 1.), &beds, 1.).unwrap();
            assert_eq!(fitted[2].slope, 0.);
            let (q, _) = fluxes(&stocks(&means, 1.), &beds, 1., 100.).unwrap();
            assert!(q.iter().all(|q| q.is_finite()));
        }
    }

    #[test]
    fn exact_rest_and_dry_ridge_keep_zero_flux_and_bit_identical_owners() {
        let segments = |endpoints: &[f64]| {
            endpoints
                .windows(2)
                .map(|p| Bed {
                    left: p[0],
                    right: p[1],
                })
                .collect::<Vec<_>>()
        };
        let cases = [
            (
                segments(&[1., 0.75, 0.25, 0., 0.25, 0.75, 1.]),
                vec![0., 0.0625, 0.375, 0.375, 0.0625, 0.],
            ),
            (
                segments(&[0., 0., 2., 2., 0., 0.]),
                vec![0.5, 0.0625, 0., 0.015625, 0.25],
            ),
            (
                segments(&[0., 0.5, 0., 0.25, 0., 0.5, 0.]),
                vec![0.75, 0.75, 0.875, 0.875, 0.75, 0.75],
            ),
            (
                [0., 0.25, 0.75, 0.5]
                    .iter()
                    .map(|&z| Bed { left: z, right: z })
                    .collect(),
                vec![1., 0.75, 0.25, 0.5],
            ),
            (segments(&[0.; 5]), vec![0.5; 4]),
            (segments(&[0., 1., 0., 2., 1.]), vec![0.; 4]),
        ];
        assert_eq!(cases.len() * 2 * 2, 24);
        for (geometry, means) in cases {
            for reflection in [false, true] {
                for datum in [0., 2_f64.powi(40)] {
                    let mut beds = geometry.clone();
                    let mut means = means.clone();
                    if reflection {
                        beds = beds
                            .into_iter()
                            .rev()
                            .map(|b| Bed {
                                left: b.right,
                                right: b.left,
                            })
                            .collect();
                        means.reverse();
                    }
                    let beds = beds
                        .into_iter()
                        .map(|b| Bed {
                            left: b.left + datum,
                            right: b.right + datum,
                        })
                        .collect::<Vec<_>>();
                    let mut owners = stocks(&means, 1.);
                    let original = owners
                        .iter()
                        .map(|s| [s.high.to_bits(), s.low.to_bits()])
                        .collect::<Vec<_>>();
                    for _ in 0..1000 {
                        let (q, _) = fluxes(&owners, &beds, 1., 100.).unwrap();
                        assert!(q.iter().all(|&q| q == 0.));
                        let step = faces::step_owned(&owners, 1., &q, 0.001).unwrap();
                        owners = step
                            .volume_high
                            .iter()
                            .zip(step.volume_low)
                            .map(|(&h, l)| Stock::new(h, l, f64::MAX).unwrap())
                            .collect();
                    }
                    assert_eq!(
                        original,
                        owners
                            .iter()
                            .map(|s| [s.high.to_bits(), s.low.to_bits()])
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
    }

    #[test]
    fn dry_minimum_bed_ordering_preserves_the_baseline_positive_storage_limit() {
        for receiver in [
            Bed {
                left: 10.,
                right: 0.,
            },
            Bed {
                left: 0.,
                right: 10.,
            },
        ] {
            let beds = [
                Bed {
                    left: 0.,
                    right: 0.,
                },
                receiver,
            ];
            let (dry, _) = fluxes(&stocks(&[11., 0.], 1.), &beds, 1., 100.).unwrap();
            let mut differences = Vec::new();
            for mean in [1e-12, 1e-24] {
                let owners = stocks(&[11., mean], 1.);
                let (candidate, _) = fluxes(&owners, &beds, 1., 100.).unwrap();
                let (baseline, _) =
                    super::super::topography::fluxes(&owners, &beds, 1., 100.).unwrap();
                assert_eq!(candidate[1].to_bits(), baseline[1].to_bits());
                differences.push((candidate[1] - dry[1]).abs());
            }
            assert!(differences[0] < 1e-3);
            assert!(differences[1] < 1e-9);
            assert!(differences[1] < differences[0]);
        }
    }

    #[test]
    fn three_and_four_owner_dry_limits_keep_supported_slopes_continuous() {
        for beds in [
            vec![
                Bed {
                    left: 0.,
                    right: 0.
                };
                3
            ],
            vec![
                Bed {
                    left: 0.,
                    right: 0.
                };
                4
            ],
            (0..4)
                .map(|i| Bed {
                    left: -0.5 * i as f64,
                    right: -0.5 * (i + 1) as f64,
                })
                .collect(),
        ] {
            let mut means = vec![12., 11., 0.];
            if beds.len() == 4 {
                means.push(0.);
            }
            let (dry, _) = fluxes(&stocks(&means, 1.), &beds, 1., 100.).unwrap();
            let mut previous = f64::INFINITY;
            for mean in [1e-8, 1e-16, 1e-24] {
                means[2] = mean;
                let owned = stocks(&means, 1.);
                let fitted = heads(&owned, &beds, 1.).unwrap();
                assert!(fitted[2].slope.abs() * 0.5 <= hypsometry_offset(mean, beds[2]));
                let (positive, _) = fluxes(&owned, &beds, 1., 100.).unwrap();
                let error = dry
                    .iter()
                    .zip(positive)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0., f64::max);
                assert!(error <= previous);
                previous = error;
                assert_eq!(owned[2].high, mean);
            }
            assert!(previous < 1e-7);
        }
    }

    fn hypsometry_offset(mean: f64, bed: Bed) -> f64 {
        match hypsometry::reconstruct(mean, bed).unwrap() {
            Column::Wet { offset, .. } => offset,
            Column::Dry { .. } => 0.,
        }
    }

    #[test]
    fn crest_thresholds_and_single_face_overdraw_remain_explicit() {
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
        for (depth, active) in [
            (0.5, false),
            (1., false),
            (f64::from_bits(1_f64.to_bits() + 1), true),
            (1.5, true),
        ] {
            let owners = stocks(&[depth, 0.], 1.);
            let (q, f) = fluxes(&owners, &beds, 1., 100.).unwrap();
            assert_eq!(q[1] > 0., active);
            assert_eq!(f[0].donor_depth_above_crest_meters > 0., active);
        }
        let owners = stocks(&[1.5, 0.], 1.);
        let (q, _) = fluxes(&owners, &beds, 1., 100.).unwrap();
        assert!(faces::step_owned(&owners, 1., &q, 1.).is_err());
        assert_eq!(owners[0].high, 1.5);
        assert_eq!(owners[1].high, 0.);
    }

    #[test]
    fn invalid_geometry_or_unrepresentable_flux_refuses_without_touching_owners() {
        let owners = stocks(&[1., 0.], 1.);
        let beds = [Bed {
            left: 0.,
            right: 0.,
        }; 2];
        for width in [0., -1., f64::INFINITY, f64::NAN] {
            assert!(fluxes(&owners, &beds, width, 100.).is_err());
        }
        for cap in [0., -1., f64::INFINITY, f64::NAN] {
            assert!(fluxes(&owners, &beds, 1., cap).is_err());
        }
        assert!(fluxes(&owners, &beds[..1], 1., 100.).is_err());
        assert!(
            fluxes(
                &owners,
                &[
                    Bed {
                        left: f64::NAN,
                        right: 0.
                    },
                    beds[1]
                ],
                1.,
                100.
            )
            .is_err()
        );
        let huge = stocks(&[f64::MAX, 0.], 1.);
        assert!(fluxes(&huge, &beds, 1., 100.).is_err());
        assert_eq!(owners[0].high, 1.);
        assert_eq!(owners[1].high, 0.);
    }
}
