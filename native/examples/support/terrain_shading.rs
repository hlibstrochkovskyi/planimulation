//! Display-only slope lighting from the actual regional bed/water surface.
use planimulation_core::Surface;
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(v: [f64; 3]) -> [f64; 3] {
    let length = dot(v, v).sqrt();
    v.map(|x| x / length)
}
fn basis(p: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let e = cross(p, [0., 1., 0.]);
    let east = if dot(e, e) > 1e-20 {
        unit(e)
    } else {
        [0., 0., 1.]
    };
    (east, unit(cross(east, p)))
}
pub fn gradients(s: &Surface, height: &[f64]) -> Result<Vec<[f64; 2]>, String> {
    let n = s.centers.len();
    if height.len() != n
        || s.offsets.len() != n + 1
        || height.iter().any(|v| !v.is_finite())
        || s.neighbors.len() != s.distances.len()
    {
        return Err("Invalid actual-surface shading fields.".into());
    }
    let mut result = vec![[0.; 2]; n];
    for (i, &p) in s.centers.iter().enumerate() {
        if p.iter().any(|v| !v.is_finite()) || (dot(p, p) - 1.).abs() > 1e-12 {
            return Err("Invalid shading unit direction.".into());
        }
        let (east, north) = basis(p);
        let (mut xx, mut xy, mut yy, mut bx, mut by) = (0., 0., 0., 0., 0.);
        let range = s.offsets[i] as usize..s.offsets[i + 1] as usize;
        if range.start > range.end || range.end > s.neighbors.len() {
            return Err("Invalid shading adjacency.".into());
        }
        for k in range {
            let j = s.neighbors[k] as usize;
            let distance = s.distances[k];
            if j >= n || !distance.is_finite() || distance <= 0. {
                return Err("Invalid shading neighbor/distance.".into());
            }
            let q = s.centers[j];
            let tangent = std::array::from_fn(|axis| q[axis] - dot(p, q) * p[axis]);
            if dot(tangent, tangent) == 0. {
                return Err("Degenerate shading directions.".into());
            }
            let direction = unit(tangent);
            let x = dot(direction, east);
            let y = dot(direction, north);
            let slope = (height[j] - height[i]) / distance;
            xx += x * x;
            xy += x * y;
            yy += y * y;
            bx += x * slope;
            by += y * slope;
        }
        let determinant = xx * yy - xy * xy;
        if determinant > 1e-12 {
            result[i] = [
                (yy * bx - xy * by) / determinant,
                (xx * by - xy * bx) / determinant,
            ];
        }
        if result[i].iter().any(|v| !v.is_finite()) {
            return Err("Actual-surface gradient overflowed.".into());
        }
    }
    Ok(result)
}
pub fn lighting(
    s: &Surface,
    bed: &[f64],
    water_level: f64,
    exaggeration: f64,
) -> Result<Vec<f64>, String> {
    if bed.iter().any(|v| !v.is_finite())
        || !water_level.is_finite()
        || !exaggeration.is_finite()
        || !(0. ..=100.).contains(&exaggeration)
    {
        return Err("Invalid display-only shading settings.".into());
    }
    let surface: Vec<_> = bed.iter().map(|&h| h.max(water_level)).collect();
    let slopes = gradients(s, &surface)?;
    let sun = [-0.5, 0.5, std::f64::consts::FRAC_1_SQRT_2];
    slopes
        .into_iter()
        .map(|[east, north]| {
            let normal = unit([-east * exaggeration, -north * exaggeration, 1.]);
            let light = (0.45 + 0.55 * dot(normal, sun).max(0.)) / (0.45 + 0.55 * sun[2]);
            if !light.is_finite() {
                return Err("Display lighting overflowed.".into());
            }
            Ok(light)
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constant_surface_is_flat_and_lighting_does_not_change_fields() {
        let s = Surface::build(2, 1000.);
        let h = vec![3.; s.centers.len()];
        assert!(gradients(&s, &h).unwrap().iter().all(|g| *g == [0., 0.]));
        assert!(lighting(&s, &h, 0., 20.).unwrap().iter().all(|&v| v == 1.));
        assert!(
            lighting(&s, &vec![-10.; h.len()], 0., 20.)
                .unwrap()
                .iter()
                .all(|&v| v == 1.)
        );
        assert_eq!(h, vec![3.; s.centers.len()]);
    }
    #[test]
    fn spherical_height_reconstructs_tangent_derivatives_under_refinement() {
        let axis = unit([1., 2., 3.]);
        let mut previous = f64::INFINITY;
        // Irregular neighbor stencils retain a first-order curvature error;
        // measure refinement rather than assuming an exact planar derivative.
        for level in [1, 2, 3, 4, 5, 6] {
            let s = Surface::build(level, 1000.);
            let h: Vec<_> = s
                .centers
                .iter()
                .map(|&p| 1000. * 0.01 * dot(axis, p))
                .collect();
            let slopes = gradients(&s, &h).unwrap();
            let rms = (s
                .centers
                .iter()
                .zip(slopes)
                .map(|(&p, g)| {
                    let (east, north) = basis(p);
                    (g[0] - 0.01 * dot(axis, east)).powi(2)
                        + (g[1] - 0.01 * dot(axis, north)).powi(2)
                })
                .sum::<f64>()
                / h.len() as f64)
                .sqrt();
            assert!(rms < previous, "{rms} >= {previous}");
            previous = rms;
        }
        assert!(previous < 1e-5, "Final RMS derivative error: {previous}");
    }
}
