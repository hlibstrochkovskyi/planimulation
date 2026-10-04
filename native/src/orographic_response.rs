//! A kinematic upslope proxy and a bounded supersaturation relaxation rate.
//! No additional cooling, atmospheric vertical solver, or cloud-water store.
use crate::{Surface, cross, dot, norm, unit};
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "orographic-response-1";

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub uplift_response_height_meters: f64,
    pub strength: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            uplift_response_height_meters: 1000.,
            strength: 1.,
        }
    }
}
impl Settings {
    pub fn validate(self) -> Result<(), String> {
        if !self.uplift_response_height_meters.is_finite()
            || !(100. ..=5000.).contains(&self.uplift_response_height_meters)
            || !self.strength.is_finite()
            || !(0. ..=2.).contains(&self.strength)
        {
            return Err("Invalid orographic-response settings.".into());
        }
        Ok(())
    }
    /// Project choice: accelerate removal of existing supersaturation only.
    /// The temperature model already accounts for the altitude dependence of
    /// capacity; do not cool a column a second time in this response.
    pub fn additional_rate_per_second(self, uplift_meters_per_second: f64) -> Result<f64, String> {
        self.validate()?;
        if !uplift_meters_per_second.is_finite() {
            return Err("Invalid terrain-following uplift.".into());
        }
        let rate =
            uplift_meters_per_second.max(0.) / self.uplift_response_height_meters * self.strength;
        if !rate.is_finite() {
            return Err("Orographic response rate overflow.".into());
        }
        Ok(rate)
    }
}

/// Inverse-distance-squared weighted least squares in a spherical log-map.
/// Returned 3D vectors are dimensionless tangent terrain gradients. No map
/// longitude, visual interpolation, or region traversal ordering is used.
pub fn terrain_gradients(
    surface: &Surface,
    radius_meters: f64,
    heights: &[f64],
) -> Result<Vec<[f64; 3]>, String> {
    let n = surface.centers.len();
    if n < 4
        || !radius_meters.is_finite()
        || radius_meters <= 0.
        || heights.len() != n
        || heights.iter().any(|v| !v.is_finite())
        || surface.offsets.len() != n + 1
        || surface.offsets.first() != Some(&0)
        || surface.offsets.last().copied().map(|v| v as usize) != Some(surface.neighbors.len())
        || surface.offsets.windows(2).any(|w| w[1] < w[0])
        || surface
            .centers
            .iter()
            .any(|&p| p.iter().any(|v| !v.is_finite()) || (norm(p) - 1.).abs() > 1e-12)
    {
        return Err("Invalid orographic terrain geometry.".into());
    }
    let mut gradients = Vec::with_capacity(n);
    for (i, &point) in surface.centers.iter().enumerate() {
        let neighbors =
            &surface.neighbors[surface.offsets[i] as usize..surface.offsets[i + 1] as usize];
        if neighbors.len() < 3 || neighbors.windows(2).any(|w| w[0] >= w[1]) {
            return Err("Invalid orographic neighbor stencil.".into());
        }
        let mut axis = [0.; 3];
        let k = (0..3)
            .min_by(|&a, &b| point[a].abs().total_cmp(&point[b].abs()))
            .unwrap();
        axis[k] = 1.;
        let east = unit(cross(point, axis));
        let north = cross(point, east);
        let (mut xx, mut xy, mut yy, mut bx, mut by) = (0., 0., 0., 0., 0.);
        for &neighbor in neighbors {
            let j = neighbor as usize;
            if j >= n || j == i {
                return Err("Invalid orographic neighbor ID.".into());
            }
            let reciprocal =
                &surface.neighbors[surface.offsets[j] as usize..surface.offsets[j + 1] as usize];
            if reciprocal.binary_search(&(i as u32)).is_err() {
                return Err("Orographic neighbors must be reciprocal.".into());
            }
            let other = surface.centers[j];
            let cosine = dot(point, other);
            let tangent = std::array::from_fn(|k| other[k] - cosine * point[k]);
            let sine = norm(tangent);
            if sine <= 1e-12 {
                return Err("Degenerate orographic neighbor edge.".into());
            }
            let direction = tangent.map(|v| v / sine);
            let distance = radius_meters * sine.atan2(cosine);
            let slope = (heights[j] - heights[i]) / distance;
            if !distance.is_finite() || distance <= 0. || !slope.is_finite() {
                return Err("Orographic neighbor distance or slope overflow.".into());
            }
            let x = dot(direction, east);
            let y = dot(direction, north);
            xx += x * x;
            xy += x * y;
            yy += y * y;
            bx += x * slope;
            by += y * slope;
        }
        let determinant = xx * yy - xy * xy;
        if !determinant.is_finite()
            || determinant <= 1e-12 * (xx + yy).powi(2)
            || !bx.is_finite()
            || !by.is_finite()
        {
            return Err("Singular or overflowing orographic gradient fit.".into());
        }
        let x = (bx * yy - by * xy) / determinant;
        let y = (by * xx - bx * xy) / determinant;
        let gradient: [f64; 3] = std::array::from_fn(|k| east[k] * x + north[k] * y);
        if gradient.iter().any(|v| !v.is_finite()) || !norm(gradient).is_finite() {
            return Err("Orographic gradient overflow.".into());
        }
        gradients.push(gradient);
    }
    Ok(gradients)
}

/// w = U · grad(h) is a terrain-following proxy, not simulated vertical wind.
pub fn uplift(point: [f64; 3], gradient: [f64; 3], velocity: [f64; 3]) -> Result<f64, String> {
    if [point, gradient, velocity]
        .iter()
        .flatten()
        .any(|v| !v.is_finite())
        || (norm(point) - 1.).abs() > 1e-12
        || !norm(gradient).is_finite()
        || !norm(velocity).is_finite()
        || dot(point, velocity).abs() > 1e-10 * norm(velocity).max(1.)
        || dot(point, gradient).abs() > 1e-10 * norm(gradient).max(1.)
    {
        return Err("Orographic uplift requires finite tangent vectors.".into());
    }
    let result = dot(gradient, velocity);
    if !result.is_finite() {
        return Err("Orographic uplift overflow.".into());
    }
    Ok(result)
}

/// Exact exponential relaxation of the excess; the removed mass has a recipient
/// outside this helper. No subsaturated donor is depleted and no stock is clamped.
pub fn deposition(
    vapor: f64,
    capacity: f64,
    base_rate: f64,
    extra_rate: f64,
    seconds: f64,
) -> Result<f64, String> {
    if [vapor, capacity, base_rate, extra_rate, seconds]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.)
    {
        return Err("Invalid orographic deposition input.".into());
    }
    let rate = base_rate + extra_rate;
    let exponent = rate * seconds;
    if !rate.is_finite() || !exponent.is_finite() {
        return Err("Orographic deposition overflow.".into());
    }
    Ok((vapor - capacity).max(0.) * (-(-exponent).exp_m1()))
}
