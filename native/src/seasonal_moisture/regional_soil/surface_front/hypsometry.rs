//! Affine-bed storage inverse for a separately scoped constant-head baseline.
//! This does not extend or replace the clipped-affine pressure candidate.
use serde::Serialize;

const RESIDUAL_LIMIT: f64 = 256. * f64::EPSILON;

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Bed {
    pub left: f64,
    pub right: f64,
}
impl Bed {
    /// Callers must validate the bed before using its datum-relative minimum.
    pub(super) fn minimum(self) -> f64 {
        self.left.min(self.right)
    }
    pub(super) fn span(self) -> Result<f64, String> {
        if !self.left.is_finite() || !self.right.is_finite() {
            return Err("Nonfinite affine-bed endpoint.".into());
        }
        let span = (self.right - self.left).abs();
        if !span.is_finite() {
            return Err("Unrepresentable affine-bed span.".into());
        }
        Ok(span)
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub(super) enum Column {
    /// A dry stock does not identify a unique hydraulic head below the bed.
    Dry { bed: Bed },
    /// Head offset above bed.minimum(), never a rounded absolute water level.
    Wet { bed: Bed, offset: f64 },
}

/// Independently integrate the affine bed's clipped depth over a unit interval.
/// Positive offset below span forms a triangle; full exposure forms a trapezoid.
fn integrated_mean(offset: f64, span: f64) -> Result<f64, String> {
    if !offset.is_finite() || offset < 0. || !span.is_finite() || span < 0. {
        return Err("Invalid affine-bed integration parameters.".into());
    }
    if offset == 0. {
        return Ok(0.);
    }
    let mean = if span == 0. {
        offset
    } else if offset < span {
        // Do not square a potentially huge or tiny offset before dividing.
        (0.5 * offset) * (offset / span)
    } else {
        let half_span = 0.5 * span;
        if half_span == 0. {
            return Err("Unrepresentable positive affine-bed half-span.".into());
        }
        offset - half_span
    };
    if !mean.is_finite() || mean <= 0. {
        return Err("Unrepresentable positive integrated affine-bed mean.".into());
    }
    Ok(mean)
}

pub(super) fn reconstruct(mean: f64, bed: Bed) -> Result<Column, String> {
    let span = bed.span()?;
    if !mean.is_finite() || mean < 0. {
        return Err("Invalid affine-bed owned mean.".into());
    }
    if mean == 0. {
        return Ok(Column::Dry { bed });
    }
    let offset = if span == 0. {
        mean
    } else {
        let half_span = 0.5 * span;
        if half_span == 0. {
            return Err("Unrepresentable positive affine-bed half-span.".into());
        }
        if mean < half_span {
            let product = span * mean;
            let radicand = 2. * product;
            if product >= f64::MIN_POSITIVE && radicand.is_finite() {
                radicand.sqrt()
            } else {
                // Equivalent scaled evaluation when the product is unrepresentable.
                span.sqrt() * (mean.sqrt() * std::f64::consts::SQRT_2)
            }
        } else {
            mean + half_span
        }
    };
    if !offset.is_finite() || offset <= 0. {
        return Err("Unrepresentable positive affine-bed head offset.".into());
    }
    let reconstructed = integrated_mean(offset, span)?;
    let residual = (reconstructed - mean).abs() / mean;
    if !residual.is_finite() || residual > RESIDUAL_LIMIT {
        return Err("Affine-bed inverse exceeds its separate moment residual bound.".into());
    }
    Ok(Column::Wet { bed, offset })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offset(column: Column) -> f64 {
        match column {
            Column::Wet { offset, .. } => offset,
            Column::Dry { .. } => panic!("Expected a positive affine-bed stock."),
        }
    }
    fn trace(column: Column, right: bool) -> f64 {
        match column {
            Column::Dry { .. } => 0.,
            Column::Wet { bed, offset } => {
                let endpoint = if right { bed.right } else { bed.left };
                (offset - (endpoint - bed.minimum())).max(0.)
            }
        }
    }

    #[test]
    fn independent_dyadic_triangle_trapezoid_and_flat_fixtures_match() {
        let ramp = Bed {
            left: 0.,
            right: 4.,
        };
        // Independently computed triangle areas H^2/8 and trapezoid mean H-2.
        for (mean, expected) in [(0.125, 1.), (0.5, 2.), (1.125, 3.), (2., 4.), (3., 5.)] {
            let column = reconstruct(mean, ramp).unwrap();
            assert_eq!(offset(column), expected);
            assert_eq!(integrated_mean(expected, 4.).unwrap(), mean);
        }
        let flat = Bed {
            left: 7.,
            right: 7.,
        };
        let c = reconstruct(0.125, flat).unwrap();
        assert_eq!(offset(c), 0.125);
        assert_eq!(trace(c, false), 0.125);
        assert_eq!(trace(c, true), 0.125);
        assert!(matches!(reconstruct(0., ramp).unwrap(), Column::Dry { .. }));
        assert!(matches!(reconstruct(0., flat).unwrap(), Column::Dry { .. }));
    }

    #[test]
    fn exact_shoreline_clipping_never_fabricates_a_dry_latent_head() {
        let bed = Bed {
            left: 0.,
            right: 2.,
        };
        let partly_wet = reconstruct(0.25, bed).unwrap();
        assert_eq!(offset(partly_wet), 1.);
        assert_eq!(trace(partly_wet, false), 1.);
        assert_eq!(trace(partly_wet, true), 0.);
        let shoreline = reconstruct(1., bed).unwrap();
        assert_eq!(offset(shoreline), 2.);
        assert_eq!(trace(shoreline, true), 0.);
        let dry = reconstruct(0., bed).unwrap();
        assert!(matches!(dry, Column::Dry { .. }));
        assert_eq!(trace(dry, false), 0.);
        assert_eq!(trace(dry, true), 0.);
    }

    #[test]
    fn reflected_and_datum_shifted_beds_keep_relative_offsets_and_traces() {
        for mean in [0.125, 0.5, 2., 3.] {
            let bed = Bed {
                left: -2.,
                right: 2.,
            };
            let c = reconstruct(mean, bed).unwrap();
            let reflected = reconstruct(
                mean,
                Bed {
                    left: bed.right,
                    right: bed.left,
                },
            )
            .unwrap();
            let datum = 2_f64.powi(40);
            let shifted = reconstruct(
                mean,
                Bed {
                    left: bed.left + datum,
                    right: bed.right + datum,
                },
            )
            .unwrap();
            assert_eq!(offset(c).to_bits(), offset(reflected).to_bits());
            assert_eq!(offset(c).to_bits(), offset(shifted).to_bits());
            assert_eq!(trace(c, false).to_bits(), trace(reflected, true).to_bits());
            assert_eq!(trace(c, true).to_bits(), trace(reflected, false).to_bits());
            assert_eq!(trace(c, false).to_bits(), trace(shifted, false).to_bits());
            assert_eq!(trace(c, true).to_bits(), trace(shifted, true).to_bits());
        }
    }

    #[test]
    fn independent_nested_affine_restrictions_recover_the_same_wet_head() {
        // Bed 0..2 split into 0..1 and 1..2; each child has half the physical width.
        // The fixture means come from independently drawn trapezoids/triangles.
        for (parent_mean, low_mean, high_mean, head) in [
            (1., 1.5, 0.5, 2.),
            (0.25, 0.5, 0., 1.),
            (0.0625, 0.125, 0., 0.5),
        ] {
            assert_eq!(parent_mean, 0.5 * low_mean + 0.5 * high_mean);
            let parent = reconstruct(
                parent_mean,
                Bed {
                    left: 0.,
                    right: 2.,
                },
            )
            .unwrap();
            let low = reconstruct(
                low_mean,
                Bed {
                    left: 0.,
                    right: 1.,
                },
            )
            .unwrap();
            let high = reconstruct(
                high_mean,
                Bed {
                    left: 1.,
                    right: 2.,
                },
            )
            .unwrap();
            assert_eq!(offset(parent), head);
            assert_eq!(offset(low), head);
            if high_mean == 0. {
                assert!(matches!(high, Column::Dry { .. }));
            } else {
                assert_eq!(offset(high) + 1., head);
            }
        }
    }

    #[test]
    fn tiny_means_spans_and_scaled_products_are_retained_or_explicitly_refused() {
        let tiny = f64::from_bits(1);
        let flat = Bed {
            left: 0.,
            right: 0.,
        };
        assert_eq!(
            offset(reconstruct(tiny, flat).unwrap()).to_bits(),
            tiny.to_bits()
        );
        let c = reconstruct(
            tiny,
            Bed {
                left: 0.,
                right: 1.,
            },
        )
        .unwrap();
        assert!(offset(c) > 0.);
        assert_eq!(
            integrated_mean(offset(c), 1.).unwrap().to_bits(),
            tiny.to_bits()
        );
        for (span, mean) in [(1e-200, 1e-300), (1e300, 1e-300), (f64::MAX, 1.)] {
            let c = reconstruct(
                mean,
                Bed {
                    left: 0.,
                    right: span,
                },
            )
            .unwrap();
            assert!(
                (integrated_mean(offset(c), span).unwrap() - mean).abs() / mean <= RESIDUAL_LIMIT
            );
        }
        let c = reconstruct(
            tiny,
            Bed {
                left: 0.,
                right: 2. * tiny,
            },
        )
        .unwrap();
        assert_eq!(offset(c).to_bits(), (2. * tiny).to_bits());
        assert!(
            reconstruct(
                tiny,
                Bed {
                    left: 0.,
                    right: tiny
                }
            )
            .is_err()
        );
        assert!(
            reconstruct(
                f64::MAX,
                Bed {
                    left: 0.,
                    right: 1.
                }
            )
            .is_ok()
        );
        assert!(
            reconstruct(
                f64::MAX,
                Bed {
                    left: 0.,
                    right: f64::MAX
                }
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_beds_means_and_unrepresentable_spans_refuse() {
        let flat = Bed {
            left: 0.,
            right: 0.,
        };
        for mean in [f64::NAN, f64::INFINITY, -1.] {
            assert!(reconstruct(mean, flat).is_err());
        }
        for bed in [
            Bed {
                left: f64::NAN,
                right: 0.,
            },
            Bed {
                left: 0.,
                right: f64::INFINITY,
            },
            Bed {
                left: -f64::MAX,
                right: f64::MAX,
            },
        ] {
            assert!(bed.span().is_err());
            assert!(reconstruct(0., bed).is_err());
            assert!(reconstruct(1., bed).is_err());
        }
        assert!(integrated_mean(-1., 1.).is_err());
        assert!(integrated_mean(1., -1.).is_err());
        assert!(integrated_mean(f64::NAN, 1.).is_err());
    }
}
