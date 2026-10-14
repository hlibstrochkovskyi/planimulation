//! Versioned initial-condition synthesis, not geological history or erosion.
//! Smooth value-noise fields are sampled in 3D, never map coordinates.
use crate::{
    Random, Surface, V, add, cross,
    crust::{self, Crust},
    dot, unit,
};

pub const MODEL_VERSION: &str = "multiscale-geography-1";
pub const REFERENCE_SUBDIVISION: u32 = 5;
pub const WARP_AMPLITUDE: f64 = 0.32;
pub const CONTINENT_BANDS: [(f64, f64); 6] = [
    (1.7, 0.52),
    (3.5, 0.25),
    (7.1, 0.12),
    (14.3, 0.06),
    (28.7, 0.03),
    (57.5, 0.02),
];
pub const RELIEF_BANDS: [(f64, f64); 3] = [(6., 0.55), (12.5, 0.30), (25.5, 0.15)];
pub const DETAIL_BANDS: [(f64, f64); 3] = [(7.7, 0.55), (15.7, 0.30), (31.7, 0.15)];

fn mix(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846ca68b);
    value ^ (value >> 16)
}

fn lattice(seed: u32, p: [i32; 3]) -> f64 {
    let h = mix(seed ^ (p[0] as u32).wrapping_mul(0x9e3779b9));
    let h = mix(h ^ (p[1] as u32).wrapping_mul(0x85ebca6b));
    let h = mix(h ^ (p[2] as u32).wrapping_mul(0xc2b2ae35));
    f64::from(h) / f64::from(u32::MAX) * 2. - 1.
}

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6. - 15.) + 10.)
}

fn smooth_value(seed: u32, p: V) -> f64 {
    let origin = p.map(|v| v.floor() as i32);
    let blend = std::array::from_fn::<_, 3, _>(|i| fade(p[i] - f64::from(origin[i])));
    let mut value = 0.;
    for x in 0..=1 {
        for y in 0..=1 {
            for z in 0..=1 {
                let corner = [x, y, z];
                let weight = (0..3)
                    .map(|i| {
                        if corner[i] == 0 {
                            1. - blend[i]
                        } else {
                            blend[i]
                        }
                    })
                    .product::<f64>();
                value += weight * lattice(seed, std::array::from_fn(|i| origin[i] + corner[i]));
            }
        }
    }
    value
}

struct Noise {
    seed: u32,
    axes: [V; 3],
    offset: V,
}
impl Noise {
    fn new(seed: &str, channel: &str) -> Self {
        let mut rng = Random::stream(seed, channel);
        let key = rng.next_u32();
        let y = rng.next() * 2. - 1.;
        let phi = rng.next() * std::f64::consts::TAU;
        let r = (1. - y * y).sqrt();
        let w = [r * phi.cos(), y, r * phi.sin()];
        let anchor = if w[2].abs() < 0.9 {
            [0., 0., 1.]
        } else {
            [0., 1., 0.]
        };
        let u = unit(cross(w, anchor));
        let v = cross(w, u);
        let roll = rng.next() * std::f64::consts::TAU;
        let a = std::array::from_fn(|i| u[i] * roll.cos() + v[i] * roll.sin());
        let b = cross(w, a);
        Self {
            seed: key,
            axes: [a, b, w],
            offset: std::array::from_fn(|_| rng.next() * 32. - 16.),
        }
    }
    fn sample(&self, p: V, frequency: f64) -> f64 {
        smooth_value(
            self.seed,
            std::array::from_fn(|i| dot(self.axes[i], p) * frequency + self.offset[i]),
        )
    }
}

struct Band {
    frequency: f64,
    weight: f64,
    noise: Noise,
}
fn bands(seed: &str, channel: &str, spectrum: &[(f64, f64)]) -> Vec<Band> {
    spectrum
        .iter()
        .enumerate()
        .map(|(i, &(frequency, weight))| Band {
            frequency,
            weight,
            noise: Noise::new(seed, &format!("{MODEL_VERSION}.{channel}.{i}")),
        })
        .collect()
}
fn fbm(bands: &[Band], p: V, scale: f64) -> f64 {
    let sum: f64 = bands
        .iter()
        .map(|b| b.weight * b.noise.sample(p, b.frequency / scale))
        .sum();
    sum / bands.iter().map(|b| b.weight).sum::<f64>()
}
fn valid_direction(p: V) -> Result<(), String> {
    if p.iter().any(|v| !v.is_finite()) || (dot(p, p) - 1.).abs() > 1e-12 {
        return Err("Geography sampling requires a finite unit direction.".into());
    }
    Ok(())
}

pub struct Field {
    scale: f64,
    warp: [Noise; 3],
    continents: Vec<Band>,
    ridges: Vec<Band>,
    detail: Vec<Band>,
}
#[derive(Clone, Copy, Debug)]
pub struct ReliefSample {
    /// Positive multipliers never amplify the existing bounded envelopes.
    pub convergence_factor: f64,
    pub divergence_factor: f64,
    /// Signed dimensionless detail; multiply by the recipe's meter amplitude.
    pub detail: f64,
}
impl Field {
    pub fn new(seed: &str, scale: f64) -> Result<Self, String> {
        if seed.trim().is_empty()
            || seed.encode_utf16().count() > 128
            || !scale.is_finite()
            || !(0.5..=2.).contains(&scale)
        {
            return Err("Invalid multiscale geography seed or continental scale.".into());
        }
        Ok(Self {
            scale,
            warp: std::array::from_fn(|i| Noise::new(seed, &format!("{MODEL_VERSION}.warp.{i}"))),
            continents: bands(seed, "continents", &CONTINENT_BANDS),
            ridges: bands(seed, "ridges", &RELIEF_BANDS),
            detail: bands(seed, "detail", &DETAIL_BANDS),
        })
    }
    pub fn continental_potential(&self, p: V) -> Result<f64, String> {
        valid_direction(p)?;
        let w = std::array::from_fn(|i| self.warp[i].sample(p, 2.2 / self.scale));
        // Tangent warp cannot cancel p: squared length is 1 + amplitude²|q|².
        let tangent = std::array::from_fn(|i| w[i] - dot(w, p) * p[i]);
        let warped = unit(add(p, tangent.map(|v| v * WARP_AMPLITUDE)));
        Ok(fbm(&self.continents, warped, self.scale))
    }
    pub fn relief(&self, p: V, continentality: f64) -> Result<ReliefSample, String> {
        valid_direction(p)?;
        if !continentality.is_finite() || !(0. ..=1.).contains(&continentality) {
            return Err("Invalid geography continentality.".into());
        }
        let ridged = self
            .ridges
            .iter()
            .map(|b| {
                let n = b.noise.sample(p, b.frequency);
                b.weight * (1. - n.abs()).powi(2)
            })
            .sum::<f64>()
            / self.ridges.iter().map(|b| b.weight).sum::<f64>();
        Ok(ReliefSample {
            convergence_factor: 0.35 + 0.65 * ridged,
            divergence_factor: 0.65 + 0.35 * ridged,
            detail: fbm(&self.detail, p, 1.) * (0.3 + 0.7 * continentality),
        })
    }
}

/// Fixed reference quadrature keeps threshold and material properties independent
/// of requested mesh/radius. Actual coarse mesh coverage need not match the target.
pub fn build_crust(
    surface: &Surface,
    seed: &str,
    fraction: f64,
    scale: f64,
) -> Result<Crust, String> {
    if !fraction.is_finite() || !(0. ..=1.).contains(&fraction) {
        return Err("Invalid multiscale continental fraction.".into());
    }
    let field = Field::new(seed, scale)?;
    let reference = Surface::build(REFERENCE_SUBDIVISION, 1.);
    let reference_values = reference
        .centers
        .iter()
        .map(|&p| field.continental_potential(p))
        .collect::<Result<Vec<_>, _>>()?;
    let threshold = if fraction == 0. {
        1. + crust::TRANSITION_WIDTH
    } else if fraction == 1. {
        -1. - crust::TRANSITION_WIDTH
    } else {
        crust::fit_threshold(&reference_values, &reference.areas, fraction)
    };
    let potential = surface
        .centers
        .iter()
        .map(|&p| field.continental_potential(p))
        .collect::<Result<Vec<_>, _>>()?;
    let continentality: Vec<_> = potential
        .iter()
        .map(|&p| crust::continentality(p, threshold))
        .collect();
    Ok(Crust {
        potential,
        thickness_meters: continentality.iter().map(|c| 7000. + 28000. * c).collect(),
        density_kg_per_cubic_meter: continentality.iter().map(|c| 3000. - 200. * c).collect(),
        continentality,
        threshold,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn value_noise_matches_corners_and_has_smooth_joined_cells() {
        let seed = 123;
        for p in [[0, 0, 0], [-1, 2, 3], [16, -14, -5]] {
            assert_eq!(smooth_value(seed, p.map(f64::from)), lattice(seed, p));
        }
        for t in [0., 0.25, 0.5, 0.75, 1.] {
            assert!((0. ..=1.).contains(&fade(t)));
        }
        assert_eq!(fade(0.), 0.);
        assert_eq!(fade(1.), 1.);
        assert_eq!(fade(0.5), 0.5);
        let center = smooth_value(seed, [1., 0.35, 0.75]);
        for delta in [1e-3, 1e-4] {
            let left = smooth_value(seed, [1. - delta, 0.35, 0.75]);
            let right = smooth_value(seed, [1. + delta, 0.35, 0.75]);
            assert!((left - center).abs() < 20. * delta.powi(3));
            assert!((right - center).abs() < 20. * delta.powi(3));
        }
    }
    #[test]
    fn rotation_is_orthonormal_and_sampling_stays_bounded_and_reproducible() {
        let n = Noise::new("control", "rotation");
        for (i, a) in n.axes.iter().enumerate() {
            for (j, b) in n.axes.iter().enumerate() {
                assert!((dot(*a, *b) - f64::from(i == j)).abs() < 1e-14);
            }
        }
        let surface = Surface::build(3, 1.);
        for seed in ["field-a", "field-b"] {
            for scale in [0.5, 1., 2.] {
                let a = Field::new(seed, scale).unwrap();
                let b = Field::new(seed, scale).unwrap();
                for &p in &surface.centers {
                    let v = a.continental_potential(p).unwrap();
                    assert!((-1. ..=1.).contains(&v));
                    assert_eq!(v, b.continental_potential(p).unwrap());
                    let r = a.relief(p, 0.38).unwrap();
                    assert!((0.35..=1.).contains(&r.convergence_factor));
                    assert!((0.65..=1.).contains(&r.divergence_factor));
                    assert!(r.detail.abs() <= 1.);
                }
            }
        }
        assert!((crate::norm(n.axes[0]) - 1.).abs() < 1e-14);
    }
    #[test]
    fn shared_directions_and_fixed_quadrature_keep_identical_crust_across_sampling() {
        let a = Surface::build(2, 6371000.);
        let b = Surface::build(3, 12742000.);
        let coarse = build_crust(&a, "shared", 0.38, 1.).unwrap();
        let fine = build_crust(&b, "shared", 0.38, 1.).unwrap();
        assert_eq!(coarse.threshold, fine.threshold);
        assert_eq!(&coarse.potential, &fine.potential[..a.centers.len()]);
        assert_eq!(
            &coarse.continentality,
            &fine.continentality[..a.centers.len()]
        );
        assert_eq!(
            &coarse.thickness_meters,
            &fine.thickness_meters[..a.centers.len()]
        );
        for fraction in [0., 1.] {
            let c = build_crust(&a, "shared", fraction, 1.).unwrap();
            assert!(c.continentality.iter().all(|&v| v == fraction));
        }
    }
    #[test]
    fn no_map_seam_or_pole_branch_and_invalid_inputs_refuse() {
        let f = Field::new("seams", 1.).unwrap();
        for p in [[0., 1., 0.], [0., -1., 0.], [-1., 0., 0.]] {
            let nearby = unit(add(p, [0., 1e-7, 1e-7]));
            assert!(
                (f.continental_potential(p).unwrap() - f.continental_potential(nearby).unwrap())
                    .abs()
                    < 1e-4
            );
        }
        for p in [[0.; 3], [f64::NAN, 0., 1.], [2., 0., 0.]] {
            assert!(f.continental_potential(p).is_err());
        }
        assert!(f.relief([1., 0., 0.], f64::NAN).is_err());
        assert!(Field::new(" ", 1.).is_err());
        assert!(Field::new("seed", 0.49).is_err());
    }
}
