//! Static initial crust, independent of plate ownership. Not land or geological history.
use crate::{Random, Surface, V, dot};

pub const TRANSITION_WIDTH: f64 = 0.12;
pub const OCEANIC_THICKNESS: f64 = 7_000.;
pub const CONTINENTAL_THICKNESS: f64 = 35_000.;
pub const OCEANIC_DENSITY: f64 = 3_000.;
pub const CONTINENTAL_DENSITY: f64 = 2_800.;

/// Smooth 3D modes sampled on unit directions, without longitude seams or pole branches.
pub struct Potential(Vec<(V, f64, f64)>);
impl Potential {
    pub fn new(seed: &str, scale: f64) -> Self {
        assert!(scale.is_finite() && (0.5..=2.).contains(&scale));
        let mut rng = Random::stream(seed, "crust.structure");
        let mut modes = Vec::new();
        for (frequency, weight) in [(3., 0.7), (6., 0.22), (12., 0.08)] {
            for _ in 0..8 {
                let y = rng.next() * 2. - 1.;
                let phi = rng.next() * std::f64::consts::TAU;
                let r = (1. - y * y).sqrt();
                let k = frequency * (0.8 + 0.4 * rng.next()) / scale;
                modes.push((
                    [r * phi.cos() * k, y * k, r * phi.sin() * k],
                    rng.next() * std::f64::consts::TAU,
                    weight / 8.,
                ));
            }
        }
        Self(modes)
    }
    pub fn sample(&self, direction: V) -> f64 {
        self.0
            .iter()
            .map(|(wave, phase, weight)| weight * (dot(*wave, direction) + phase).sin())
            .sum()
    }
}

/// Choose the nearest attainable area of a strict potential superlevel set.
/// Equal values remain together. Exact area-error ties prefer the smaller area.
pub fn fit_threshold(values: &[f64], areas: &[f64], fraction: f64) -> f64 {
    assert!(!values.is_empty() && values.len() == areas.len());
    assert!(values.iter().all(|v| v.is_finite()));
    assert!(areas.iter().all(|v| v.is_finite() && *v > 0.));
    assert!(fraction.is_finite() && (0.0..=1.).contains(&fraction));
    let mut order: Vec<_> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[b].total_cmp(&values[a]).then(a.cmp(&b)));
    let total: f64 = areas.iter().sum();
    let mut threshold = values[order[0]] + TRANSITION_WIDTH;
    let mut best_error = fraction;
    let mut sum = 0.;
    for (rank, &id) in order.iter().enumerate() {
        sum += areas[id];
        if rank + 1 < order.len() && values[id] == values[order[rank + 1]] {
            continue;
        }
        let error = (sum / total - fraction).abs();
        if error < best_error {
            best_error = error;
            threshold = if rank + 1 == order.len() {
                values[id] - TRANSITION_WIDTH
            } else {
                values[order[rank + 1]] + (values[id] - values[order[rank + 1]]) * 0.5
            };
        }
    }
    threshold
}

pub fn continentality(potential: f64, threshold: f64) -> f64 {
    let x = ((potential - threshold) / TRANSITION_WIDTH + 0.5).clamp(0., 1.);
    x * x * (3. - 2. * x)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Crust {
    pub potential: Vec<f64>,
    pub continentality: Vec<f64>,
    pub thickness_meters: Vec<f64>,
    pub density_kg_per_cubic_meter: Vec<f64>,
    pub threshold: f64,
}
impl Crust {
    pub fn build(surface: &Surface, seed: &str, fraction: f64, scale: f64) -> Self {
        let field = Potential::new(seed, scale);
        let potential: Vec<_> = surface.centers.iter().map(|&p| field.sample(p)).collect();
        let threshold = fit_threshold(&potential, &surface.areas, fraction);
        let continentality: Vec<_> = potential
            .iter()
            .map(|&p| continentality(p, threshold))
            .collect();
        let thickness_meters = continentality
            .iter()
            .map(|c| OCEANIC_THICKNESS + c * (CONTINENTAL_THICKNESS - OCEANIC_THICKNESS))
            .collect();
        let density_kg_per_cubic_meter = continentality
            .iter()
            .map(|c| OCEANIC_DENSITY + c * (CONTINENTAL_DENSITY - OCEANIC_DENSITY))
            .collect();
        Self {
            potential,
            continentality,
            thickness_meters,
            density_kg_per_cubic_meter,
            threshold,
        }
    }
}
