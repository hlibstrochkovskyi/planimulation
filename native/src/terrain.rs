//! Static, explainable elevation relative to a reference sphere, not sea level.
use crate::{
    Random, Recipe, Surface, V, add, cross, crust::Crust, dot, norm, tectonics::Tectonics, unit,
};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

pub fn crust_baseline(thickness: f64, density: f64) -> f64 {
    // Illustrative unloaded column buoyancy; not a solved mantle/water equilibrium.
    -4500. + thickness * (1. - density / 3300.) - 7000. * (1. - 3000. / 3300.)
}

/// Convergence uplift, divergence ridge, divergence rift magnitudes, in meters.
/// Shear alone has no vertical effect in this initial model.
pub fn boundary_amplitudes(opening: f64, continentality: f64) -> [f64; 3] {
    let closing = (-opening).max(0.);
    let spreading = opening.max(0.);
    [
        6000. * (0.25 + 0.75 * continentality) * closing / (0.04 + closing),
        2500. * (1. - continentality) * spreading / (0.04 + spreading),
        1500. * continentality * spreading / (0.04 + spreading),
    ]
}

#[derive(Clone, Copy)]
struct Visit {
    value: f64,
    cell: usize,
}
impl PartialEq for Visit {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Visit {}
impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Visit {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value
            .total_cmp(&other.value)
            .then_with(|| other.cell.cmp(&self.cell))
    }
}

/// Strongest exponentially attenuated source over graph shortest paths.
/// Duplicate sources cannot amplify relief. Width and edge lengths are in meters.
pub fn spread(surface: &Surface, sources: &[f64], width: f64) -> Vec<f64> {
    assert!(width.is_finite() && width > 0. && sources.len() == surface.centers.len());
    assert!(sources.iter().all(|v| v.is_finite() && *v >= 0.));
    let mut values = sources.to_vec();
    let mut queue = BinaryHeap::new();
    for (cell, &value) in sources.iter().enumerate() {
        if value > 0. {
            queue.push(Visit { cell, value });
        }
    }
    while let Some(Visit { value, cell }) = queue.pop() {
        if value < values[cell] {
            continue;
        }
        for k in surface.offsets[cell]..surface.offsets[cell + 1] {
            let neighbor = surface.neighbors[k as usize] as usize;
            let next = value * (-surface.distances[k as usize] / width).exp();
            if next > values[neighbor] {
                values[neighbor] = next;
                queue.push(Visit {
                    cell: neighbor,
                    value: next,
                });
            }
        }
    }
    values
}

pub struct Detail(Vec<(V, f64, f64)>);
impl Detail {
    pub fn new(seed: &str) -> Self {
        let mut rng = Random::stream(seed, "terrain.detail");
        let mut modes = Vec::new();
        for (frequency, weight) in [(18., 0.6), (36., 0.3), (72., 0.1)] {
            for _ in 0..8 {
                let y = rng.next() * 2. - 1.;
                let phi = rng.next() * std::f64::consts::TAU;
                let r = (1. - y * y).sqrt();
                modes.push((
                    [
                        r * phi.cos() * frequency,
                        y * frequency,
                        r * phi.sin() * frequency,
                    ],
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

#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    pub baseline: Vec<f64>,
    pub convergence: Vec<f64>,
    pub divergence: Vec<f64>,
    pub detail: Vec<f64>,
    pub elevation: Vec<f64>,
}
impl Terrain {
    pub fn build(surface: &Surface, crust: &Crust, tectonics: &Tectonics, recipe: &Recipe) -> Self {
        let n = surface.centers.len();
        let width = recipe.boundary_width_km * 1000.;
        let mut sources = [vec![0.0_f64; n], vec![0.0_f64; n], vec![0.0_f64; n]];
        for (segment, cells) in tectonics
            .boundary_cells
            .as_chunks::<2>()
            .0
            .iter()
            .enumerate()
        {
            let [a, b] = cells.map(|id| id as usize);
            let c = (crust.continentality[a] + crust.continentality[b]) * 0.5;
            let amplitudes = boundary_amplitudes(tectonics.boundary_motion[segment * 2], c);
            let p = unit(add(
                tectonics.boundary_directions[segment * 2],
                tectonics.boundary_directions[segment * 2 + 1],
            ));
            for cell in [a, b] {
                let center = surface.centers[cell];
                let distance = norm(cross(center, p)).atan2(dot(center, p)) * recipe.radius_meters;
                for j in 0..3 {
                    sources[j][cell] =
                        sources[j][cell].max(amplitudes[j] * (-distance / width).exp());
                }
            }
        }
        let closing = spread(surface, &sources[0], width);
        let ridge = spread(surface, &sources[1], width);
        let rift = spread(surface, &sources[2], width);
        let field = Detail::new(&recipe.seed);
        let mut result = Self {
            baseline: Vec::with_capacity(n),
            convergence: Vec::with_capacity(n),
            divergence: Vec::with_capacity(n),
            detail: Vec::with_capacity(n),
            elevation: Vec::with_capacity(n),
        };
        for i in 0..n {
            let base = crust_baseline(
                crust.thickness_meters[i],
                crust.density_kg_per_cubic_meter[i],
            );
            let convergence = closing[i] * recipe.relief_scale;
            let divergence = (ridge[i] - rift[i]) * recipe.relief_scale;
            let detail = field.sample(surface.centers[i]) * recipe.detail_amplitude_meters;
            result.baseline.push(base);
            result.convergence.push(convergence);
            result.divergence.push(divergence);
            result.detail.push(detail);
            result
                .elevation
                .push(base + convergence + divergence + detail);
        }
        result
    }
}
