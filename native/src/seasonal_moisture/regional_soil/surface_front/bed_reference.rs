//! Independent source-free Manning traveling wave on a downhill affine bed.
//! Reference geometry never enters the evolved interior operator.

const MAX_U: f64 = 0.9;
const MAX_TERMS: usize = 256;

#[derive(Clone, Copy)]
pub(super) struct Reference {
    speed: f64,
    depth_scale: f64,
    length_scale: f64,
}

/// Positive series for integral u^power/(1-u^4) du on [a,b].
/// Differences are evaluated before summation, not by subtracting primitives.
fn integral(a: f64, b: f64, power: i32) -> Result<f64, String> {
    if !a.is_finite() || !b.is_finite() || a < 0. || b < a || b > MAX_U {
        return Err("Invalid bounded traveling-wave integral interval.".into());
    }
    if a == b {
        return Ok(0.);
    }
    let ratio = b.powi(4);
    let mut sum = 0.;
    let mut correction = 0.;
    for k in 0..MAX_TERMS {
        let exponent = power + 1 + 4 * k as i32;
        let upper = b.powi(exponent);
        let difference = if a == 0. {
            upper
        } else {
            // log1p retains close positive interval endpoints.
            upper * -((exponent as f64) * ((a - b) / b).ln_1p()).exp_m1()
        };
        let term = difference / exponent as f64;
        if !term.is_finite() || term < 0. {
            return Err("Unrepresentable positive traveling-wave integral.".into());
        }
        let adjusted = term - correction;
        let next = sum + adjusted;
        correction = (next - sum) - adjusted;
        sum = next;
        // Each subsequent positive interval integral is <= b^4 times
        // its predecessor. This bounds the remaining geometric tail.
        let tail = term * ratio / (1. - ratio);
        if tail <= 2. * f64::EPSILON * sum && sum > 0. {
            return Ok(sum);
        }
    }
    Err("Traveling-wave positive series exceeded its reference work bound.".into())
}

impl Reference {
    pub(super) fn new(slope: f64, speed: f64, roughness: f64) -> Result<Self, String> {
        if [slope, speed, roughness]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.)
        {
            return Err("Invalid downhill traveling-wave parameters.".into());
        }
        let depth_scale = (roughness.powi(2) * speed.powi(2) / slope).powf(0.75);
        let length_scale = depth_scale / slope;
        if !depth_scale.is_finite()
            || depth_scale <= 0.
            || !length_scale.is_finite()
            || length_scale <= 0.
        {
            return Err("Unrepresentable downhill traveling-wave scales.".into());
        }
        Ok(Self {
            speed,
            depth_scale,
            length_scale,
        })
    }

    fn distance(self, u: f64) -> Result<f64, String> {
        let value = 3. * self.length_scale * integral(0., u, 6)?;
        if !value.is_finite() || value <= 0. {
            return Err("Unrepresentable positive traveling-wave distance.".into());
        }
        Ok(value)
    }

    fn parameter(self, x: f64, time: f64) -> Result<f64, String> {
        if !x.is_finite() || !time.is_finite() || time < 0. {
            return Err("Invalid traveling-wave coordinate or time.".into());
        }
        let behind = self.speed * time - x;
        if !behind.is_finite() {
            return Err("Unrepresentable traveling-wave relative coordinate.".into());
        }
        if behind <= 0. {
            return Ok(0.);
        }
        if behind > self.distance(MAX_U)? {
            return Err("Traveling-wave point exceeds the declared reference branch.".into());
        }
        let (mut lo, mut hi) = (0., MAX_U);
        for _ in 0..128 {
            let mid = lo + (hi - lo) * 0.5;
            if mid == lo || mid == hi {
                let low_error = if lo == 0. {
                    behind
                } else {
                    (self.distance(lo)? - behind).abs()
                };
                let high_error = (self.distance(hi)? - behind).abs();
                if low_error.min(high_error) > 64. * f64::EPSILON * behind {
                    return Err(
                        "Traveling-wave inverse exceeds its reference residual bound.".into(),
                    );
                }
                return Ok(if low_error <= high_error { lo } else { hi });
            }
            if self.distance(mid)? < behind {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Err("Traveling-wave inverse exceeded its reference work bound.".into())
    }

    pub(super) fn depth(self, x: f64, time: f64) -> Result<f64, String> {
        let u = self.parameter(x, time)?;
        let depth = self.depth_scale * u.powi(3);
        if !depth.is_finite() || u > 0. && depth <= 0. {
            return Err("Unrepresentable traveling-wave positive depth.".into());
        }
        Ok(depth)
    }

    pub(super) fn mean(self, left: f64, right: f64, time: f64) -> Result<f64, String> {
        if !left.is_finite() || !right.is_finite() || right <= left {
            return Err("Invalid traveling-wave owner interval.".into());
        }
        let upper = self.parameter(left, time)?;
        let lower = self.parameter(right, time)?;
        if upper == lower {
            if upper == 0. {
                return Ok(0.);
            }
            return Err("Positive traveling-wave owner interval lost in inversion.".into());
        }
        let mean =
            3. * self.depth_scale * self.length_scale * integral(lower, upper, 9)? / (right - left);
        if !mean.is_finite() || mean <= 0. {
            return Err("Unrepresentable traveling-wave positive mean.".into());
        }
        Ok(mean)
    }

    pub(super) fn contour(self, depth: f64, time: f64) -> Result<f64, String> {
        if !depth.is_finite() || depth < 0. || !time.is_finite() || time < 0. {
            return Err("Invalid traveling-wave contour.".into());
        }
        let distance = if depth == 0. {
            0.
        } else {
            self.distance((depth / self.depth_scale).cbrt())?
        };
        let x = self.speed * time - distance;
        if !x.is_finite() {
            return Err("Unrepresentable traveling-wave contour coordinate.".into());
        }
        Ok(x)
    }

    /// Exact interval boundary amount: integral c*h(x,t) dt = integral h dz.
    pub(super) fn boundary_volume(self, x: f64, start: f64, end: f64) -> Result<f64, String> {
        if !start.is_finite() || !end.is_finite() || start < 0. || end <= start {
            return Err("Invalid traveling-wave boundary interval.".into());
        }
        let lower = self.parameter(x, start)?;
        let upper = self.parameter(x, end)?;
        if upper == lower {
            if upper == 0. {
                return Ok(0.);
            }
            return Err("Positive traveling-wave time interval lost in inversion.".into());
        }
        let amount = 3. * self.depth_scale * self.length_scale * integral(lower, upper, 9)?;
        if !amount.is_finite() || amount <= 0. {
            return Err("Unrepresentable traveling-wave boundary amount.".into());
        }
        Ok(amount)
    }

    /// Reference-only short-interval conditioning, never an evolved history.
    pub(super) fn conditioning_report(self) -> Result<serde_json::Value, String> {
        let probes = [0.004, 0.001, 1e-6, 1e-8, 1e-10]
            .into_iter()
            .map(|dt| {
                let end = 1. + dt;
                Ok(serde_json::json!({"xMeters":-3.5,"startSeconds":1.,
                    "endSeconds":end,"requestedIntervalSeconds":dt,
                    "referenceBoundaryVolumeSquareMeters":self.boundary_volume(-3.5,1.,end)?}))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(serde_json::json!({"probes":probes,
            "contract":"Rounded inverses can amplify relative short-interval amount error. These are reference-only observations, not qualified timestep controls or physical transactions."}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_branch_invalid_inputs_and_positive_series_interval_are_explicit() {
        let r = Reference::new(0.001, 0.1, 0.04).unwrap();
        assert_eq!(r.depth(0., 0.).unwrap(), 0.);
        assert_eq!(r.mean(0., 0.5, 0.).unwrap(), 0.);
        assert_eq!(r.boundary_volume(1., 0., 2.).unwrap(), 0.);
        assert_eq!(r.contour(0., 2.).unwrap(), 0.2);
        assert!(Reference::new(0., 0.1, 0.04).is_err());
        assert!(r.depth(f64::NAN, 0.).is_err());
        assert!(r.mean(1., 0., 0.).is_err());
        assert!(r.depth(-1e6, 0.).is_err());
        assert!(r.contour(r.depth_scale, 0.).is_err());
        assert!(integral(0.2, 0.1, 6).is_err());
        assert!(integral(0., f64::NAN, 6).is_err());
    }

    #[test]
    fn exact_mean_partition_boundary_and_original_manning_law_agree() {
        let r = Reference::new(0.001, 0.1, 0.04).unwrap();
        let whole = r.mean(-3.5, 0.5, 2.).unwrap() * 4.;
        let parts: f64 = (0..32)
            .map(|i| {
                let x = -3.5 + i as f64 * 0.125;
                r.mean(x, x + 0.125, 2.).unwrap() * 0.125
            })
            .sum();
        assert!((whole - parts).abs() < 128. * f64::EPSILON * whole);
        let initial = r.mean(-3.5, 0.5, 0.).unwrap() * 4.;
        let supplied = r.boundary_volume(-3.5, 0., 2.).unwrap();
        assert!((whole - initial - supplied).abs() < 256. * f64::EPSILON * whole);
        for x in [-3., -1., -0.01] {
            let h = r.depth(x, 0.).unwrap();
            let gradient = -0.04_f64.powi(2) * 0.1_f64.powi(2) / h.powf(4. / 3.);
            let q = h.powf(5. / 3.) * (-gradient).sqrt() / 0.04;
            assert!((q - 0.1 * h).abs() < 16. * f64::EPSILON * q);
            let dx = 1e-5;
            let numerical =
                (r.depth(x + dx, 0.).unwrap() - r.depth(x - dx, 0.).unwrap()) / (2. * dx);
            assert!((numerical - (0.001 + gradient)).abs() < 1e-7);
        }
    }

    #[test]
    fn independent_eighty_digit_rational_quadrature_fixtures_agree() {
        // mpmath 1.3.0 at 80 digits, direct rational-integrand quadrature
        // and monotone root; fixtures never initialize an evolved operator.
        let r = Reference::new(0.001, 0.1, 0.04).unwrap();
        for (x, t, expected) in [
            (-3.5, 0., 0.019498896495784356),
            (-1., 0., 0.012030679037955335),
            (-0.1, 0., 0.004654608393435879),
            (0., 2., 0.006223452296559491),
            (0.125, 2., 0.004123159076564457),
        ] {
            let actual = r.depth(x, t).unwrap();
            assert!((actual - expected).abs() < 64. * f64::EPSILON * expected);
        }
        for (a, b, t, expected) in [
            (-3.5, -3., 0., 0.018967850797741656),
            (-0.125, 0., 0., 0.0035943899879409863),
            (0., 0.125, 2., 0.005273172845584223),
            (0.125, 0.25, 2., 0.001737422194938716),
            (-3.5, -3.375, 2., 0.019776368087604324),
        ] {
            let actual = r.mean(a, b, t).unwrap();
            assert!((actual - expected).abs() < 256. * f64::EPSILON * expected);
        }
        let expected = 0.003940195281307745;
        assert!(
            (r.boundary_volume(-3.5, 0., 2.).unwrap() - expected).abs()
                < 256. * f64::EPSILON * expected
        );
    }
}
