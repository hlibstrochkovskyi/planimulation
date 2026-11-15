//! Version-3 initial-condition synthesis: shaped convergent mountain belts and
//! sediment-filled shallow dry depressions. Procedural heuristics, not plate
//! history, isostasy or fluvial erosion. Reuses version-2 crust and plates.
use crate::{
    Recipe, Surface, V, add, cross, crust::Crust, dot, geography::Noise, norm,
    tectonics::Tectonics, unit,
};
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

pub const MODEL_VERSION: &str = "multiscale-geography-3";
/// Peak convergence uplift before modulation, as in the version-1 boundary response.
pub const MAX_UPLIFT_METERS: f64 = 6000.;
/// Angular amplitude of the tangent warp applied to belt distance lookups.
pub const WARP_RADIANS: f64 = 0.045;
/// Closed dry depressions shallower than this are filled to their spill level.
pub const MAX_FILL_DEPTH_METERS: f64 = 200.;

#[derive(Clone, Copy, PartialEq)]
struct Key(f64, usize);
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0).then(self.1.cmp(&other.1))
    }
}

/// Closing-boundary amplitude from the segment's mean continentality, as in
/// version 1. Both sides share it, so a belt has no cliff at the boundary.
pub fn segment_amplitude(opening: f64, continentality: f64) -> f64 {
    let closing = (-opening).max(0.);
    MAX_UPLIFT_METERS * (0.25 + 0.75 * continentality) * closing / (0.04 + closing)
}

/// Graph distance (meters) to the nearest convergent source and that source's amplitude.
fn nearest_sources(
    surface: &Surface,
    crust: &Crust,
    tectonics: &Tectonics,
    radius: f64,
) -> (Vec<f64>, Vec<f64>) {
    let n = surface.centers.len();
    let (mut distance, mut amplitude) = (vec![f64::INFINITY; n], vec![0.; n]);
    for (segment, cells) in tectonics
        .boundary_cells
        .as_chunks::<2>()
        .0
        .iter()
        .enumerate()
    {
        let opening = tectonics.boundary_motion[segment * 2];
        if opening >= 0. {
            continue;
        }
        let p = unit(add(
            tectonics.boundary_directions[segment * 2],
            tectonics.boundary_directions[segment * 2 + 1],
        ));
        let [first, second] = cells.map(|id| id as usize);
        let a = segment_amplitude(
            opening,
            (crust.continentality[first] + crust.continentality[second]) * 0.5,
        );
        for cell in [first, second] {
            let c = surface.centers[cell];
            let d = norm(cross(c, p)).atan2(dot(c, p)) * radius;
            if d < distance[cell] || (d == distance[cell] && a > amplitude[cell]) {
                distance[cell] = d;
                amplitude[cell] = a;
            }
        }
    }
    let mut queue: BinaryHeap<Reverse<Key>> = (0..n)
        .filter(|&i| distance[i].is_finite())
        .map(|i| Reverse(Key(distance[i], i)))
        .collect();
    while let Some(Reverse(Key(d, cell))) = queue.pop() {
        if d > distance[cell] {
            continue;
        }
        for k in surface.offsets[cell] as usize..surface.offsets[cell + 1] as usize {
            let j = surface.neighbors[k] as usize;
            let next = d + surface.distances[k];
            if next < distance[j] {
                distance[j] = next;
                amplitude[j] = amplitude[cell];
                queue.push(Reverse(Key(next, j)));
            }
        }
    }
    (distance, amplitude)
}

/// Walk to the region whose center is nearest a direction, starting near it.
fn locate(surface: &Surface, mut cell: usize, q: V) -> usize {
    loop {
        let mut best = cell;
        for k in surface.offsets[cell] as usize..surface.offsets[cell + 1] as usize {
            let j = surface.neighbors[k] as usize;
            if dot(surface.centers[j], q) > dot(surface.centers[best], q) {
                best = j;
            }
        }
        if best == cell {
            return cell;
        }
        cell = best;
    }
}

fn unit_noise(noise: &Noise, p: V, frequency: f64) -> f64 {
    (noise.sample(p, frequency) * 0.5 + 0.5).clamp(0., 1.)
}

/// Convergence uplift in meters for every region, bounded by MAX_UPLIFT × relief scale.
///
/// Belt distance is looked up at a tangentially warped direction so belts meander.
/// Width and along-belt amplitude vary with low-frequency noise (massifs and
/// saddles), and ridged noise forms ranges inside each belt. Belts are symmetric
/// about the boundary; no overriding-plate asymmetry is modeled.
pub fn uplift(
    surface: &Surface,
    crust: &Crust,
    tectonics: &Tectonics,
    recipe: &Recipe,
) -> Result<Vec<f64>, String> {
    let n = surface.centers.len();
    if crust.continentality.len() != n || recipe.boundary_width_km <= 0. {
        return Err("Invalid mountain-belt inputs.".into());
    }
    let channel = |name: &str| Noise::new(&recipe.seed, &format!("{MODEL_VERSION}.belts.{name}"));
    let warp = [channel("warp.0"), channel("warp.1"), channel("warp.2")];
    let (width_noise, strength_noise) = (channel("width"), channel("strength"));
    let ranges = [
        (channel("ranges.0"), 16., 0.6),
        (channel("ranges.1"), 32., 0.4),
    ];
    let (distance, amplitude) = nearest_sources(surface, crust, tectonics, recipe.radius_meters);
    let width0 = recipe.boundary_width_km * 1000.;
    let mut result = Vec::with_capacity(n);
    for (i, &p) in surface.centers.iter().enumerate() {
        let w: V = std::array::from_fn(|k| warp[k].sample(p, 4.));
        let tangent: V = std::array::from_fn(|k| w[k] - dot(w, p) * p[k]);
        let q = unit(add(p, tangent.map(|v| v * WARP_RADIANS)));
        let source = locate(surface, i, q);
        let (d, a) = (distance[source], amplitude[source]);
        if !d.is_finite() || a <= 0. {
            result.push(0.);
            continue;
        }
        let width = width0 * (0.55 + 0.9 * unit_noise(&width_noise, p, 3.));
        let strength = 0.35 + 0.65 * unit_noise(&strength_noise, p, 5.);
        let s = (1. - d / (1.6 * width)).clamp(0., 1.);
        let profile = 0.75 * s * s * (3. - 2. * s) + 0.25 * (-d / width).exp();
        let ridged = ranges
            .iter()
            .map(|(noise, f, weight)| weight * (1. - noise.sample(q, *f).abs()).powi(2))
            .sum::<f64>();
        let value = a * strength * profile * (0.4 + 0.6 * ridged) * recipe.relief_scale;
        if !value.is_finite() || value < 0. {
            return Err("Mountain-belt uplift is not finite.".into());
        }
        result.push(value.min(MAX_UPLIFT_METERS * recipe.relief_scale));
    }
    Ok(result)
}

/// Raise every dry region inside a closed depression shallower than `max_depth`
/// to that depression's spill level (priority flood from wet regions). Deeper
/// depressions stay closed basins. Returns per-region added thickness in meters;
/// it represents sediment-filled plains, not modeled transport or water.
pub fn fill_shallow_depressions(
    surface: &Surface,
    elevation: &[f64],
    wet: &[u32],
    level: f64,
    max_depth: f64,
) -> Result<Vec<f64>, String> {
    let n = elevation.len();
    if wet.len() != n
        || surface.centers.len() != n
        || !level.is_finite()
        || max_depth.is_nan()
        || max_depth < 0.
    {
        return Err("Invalid depression-filling inputs.".into());
    }
    let mut spill = vec![f64::INFINITY; n];
    let mut queue = BinaryHeap::new();
    for i in 0..n {
        if wet[i] != 0 {
            spill[i] = elevation[i].max(level);
            queue.push(Reverse(Key(spill[i], i)));
        }
    }
    while let Some(Reverse(Key(value, cell))) = queue.pop() {
        for k in surface.offsets[cell] as usize..surface.offsets[cell + 1] as usize {
            let j = surface.neighbors[k] as usize;
            if spill[j].is_finite() {
                continue;
            }
            spill[j] = elevation[j].max(value);
            queue.push(Reverse(Key(spill[j], j)));
        }
    }
    let mut fill = vec![0.; n];
    let mut seen = vec![false; n];
    let in_depression = |i: usize| wet[i] == 0 && spill[i].is_finite() && spill[i] > elevation[i];
    for start in 0..n {
        if seen[start] || !in_depression(start) {
            continue;
        }
        let (mut stack, mut members, mut depth) = (vec![start], vec![], 0_f64);
        seen[start] = true;
        while let Some(cell) = stack.pop() {
            members.push(cell);
            depth = depth.max(spill[cell] - elevation[cell]);
            for k in surface.offsets[cell] as usize..surface.offsets[cell + 1] as usize {
                let j = surface.neighbors[k] as usize;
                if !seen[j] && in_depression(j) {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        if depth < max_depth {
            for cell in members {
                fill[cell] = spill[cell] - elevation[cell];
            }
        }
    }
    Ok(fill)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{continental_uplands, tectonics, terrain};

    fn recipe(seed: &str, subdivision: u32) -> Recipe {
        serde_json::from_value(serde_json::json!({"schemaVersion":1,"modelVersion":MODEL_VERSION,
            "randomVersion":"fnv1a-utf8-mulberry32-1","seed":seed,"subdivision":subdivision,"radiusMeters":6371000.,
            "plateCount":12,"maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,"continentalScale":1.,
            "reliefScale":1.,"boundaryWidthKm":300.,"detailAmplitudeMeters":300.,"terrainPreparationPasses":4,
            "water":{"mode":"coverage","fraction":0.71}})).unwrap()
    }

    #[test]
    fn segment_amplitude_matches_the_version_one_response_and_ignores_opening() {
        for (opening, c) in [(-0.02, 0.), (-0.05, 1.), (-0.1, 0.4)] {
            assert_eq!(
                segment_amplitude(opening, c),
                terrain::boundary_amplitudes(opening, c)[0]
            );
        }
        assert_eq!(segment_amplitude(0.03, 1.), 0.);
    }

    #[test]
    fn uplift_is_bounded_reproducible_and_present_only_with_convergence() {
        let r = recipe("belts", 4);
        let surface = Surface::build(r.subdivision, r.radius_meters);
        let crust = continental_uplands::build_crust(&surface, &r.seed, 0.38, 1.).unwrap();
        let plates = tectonics::Tectonics::build_with_continuous_roots(
            &surface,
            &r.seed,
            12,
            8.,
            r.radius_meters,
        )
        .unwrap();
        let a = uplift(&surface, &crust, &plates, &r).unwrap();
        assert_eq!(a, uplift(&surface, &crust, &plates, &r).unwrap());
        assert!(a.iter().all(|v| (0. ..=MAX_UPLIFT_METERS).contains(v)));
        assert!(a.iter().any(|&v| v > 1000.));
        let mut still = plates.clone();
        still.boundary_motion.iter_mut().for_each(|v| *v = v.abs());
        assert!(
            uplift(&surface, &crust, &still, &r)
                .unwrap()
                .iter()
                .all(|&v| v == 0.)
        );
    }

    #[test]
    fn shallow_depressions_fill_to_spill_level_and_deep_ones_stay_closed() {
        let surface = Surface::build(2, 1000.);
        let n = surface.centers.len();
        let mut wet = vec![0; n];
        wet[0] = 1;
        let mut elevation = vec![100.; n];
        elevation[0] = -50.;
        let far = (0..n)
            .max_by(|&a, &b| {
                dot(surface.centers[0], surface.centers[b])
                    .total_cmp(&dot(surface.centers[0], surface.centers[a]))
            })
            .unwrap();
        elevation[far] = 60.;
        let fill = fill_shallow_depressions(&surface, &elevation, &wet, 0., 50.).unwrap();
        assert_eq!(fill[far], 40.);
        assert!(fill.iter().enumerate().all(|(i, &f)| i == far || f == 0.));
        elevation[far] = -400.;
        let kept = fill_shallow_depressions(&surface, &elevation, &wet, 0., 50.).unwrap();
        assert!(kept.iter().all(|&f| f == 0.));
        assert!(fill_shallow_depressions(&surface, &elevation, &wet[1..], 0., 50.).is_err());
    }
}
