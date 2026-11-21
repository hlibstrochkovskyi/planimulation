//! Version-5 initial-condition synthesis: `multiscale-geography-5`. Reuses
//! version 4 and adds the ocean floor, fluvial landscape relief and routed
//! sediment as explicit elevation contributions.
//!
//! Erosion replaces dry land by a steady-state stream-power landscape (uplift
//! balanced by detachment-limited incision, `n = 1`), whose regional mean height
//! is fitted to the version-4 bed. The drainage network and the valleys it cuts
//! are therefore built together, as in uplift–erosion terrain generation
//! (Cordonnier et al. 2016; Braun & Willett 2013 for the stack ordering).
use crate::{Random, Surface, mountain_belts::Key};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const MODEL_VERSION: &str = "multiscale-geography-5";
/// Finest subdivision supported by version 5 (163,842 regions).
pub const MAX_SUBDIVISION: u32 = 7;
/// Coastal-lowland kept fractions at the shore and inland. Version 5 compresses
/// less than version 4 because `bathymetry-1` raises the fitted water level.
/// The pre-sediment envelope also leaves room for alluvial aggradation.
pub const LOWLAND_FACTORS: [f64; 2] = [0.48, 0.935];
/// Drainage-area exponent `m` of the stream-power law (concavity with `n = 1`).
pub const AREA_EXPONENT: f64 = 0.5;
/// Receiver/height updates; the network settles well before this on review seeds.
pub const ITERATIONS: usize = 40;
/// Neighbor-averaging passes defining the regional height and chi envelopes.
pub const SMOOTHING_PASSES: usize = 2;
/// Largest relative slope reduction applied to a link when choosing receivers.
pub const RECEIVER_JITTER: f64 = 0.4;
/// Log-normal spread of the initial steepness, breaking grid symmetry.
pub const STEEPNESS_NOISE: f64 = 0.3;
/// Regional height above water at which the full fluvial relief is kept, meters.
pub const RELIEF_REFERENCE_METERS: f64 = 1500.;
/// Share of the fluvial relief kept on the lowest ground.
pub const MIN_RELIEF_WEIGHT: f64 = 0.35;
/// Minimum target height above water for dry land, meters.
pub const MIN_TARGET_METERS: f64 = 5.;

/// Steady-state fluvial landscape of the dry regions, heights above `level`.
pub struct Landscape {
    pub height: Vec<f64>,
    pub receivers: Vec<u32>,
    pub area: Vec<f64>,
    pub iterations: usize,
}

/// Signed fluvial adjustment in meters after the initial water fit. Negative
/// changes supply the sediment stage. Positive changes fit the regional relief.
pub fn erosion(
    surface: &Surface,
    elevation: &[f64],
    wet: &[u32],
    level: f64,
    seed: &str,
) -> Result<Vec<f64>, String> {
    let n = elevation.len();
    if n != surface.centers.len() || wet.len() != n || !level.is_finite() {
        return Err("Invalid erosion input.".into());
    }
    if wet.iter().all(|&b| b == 0) || wet.iter().all(|&b| b != 0) {
        return Ok(vec![0.; n]);
    }
    let target: Vec<f64> = (0..n)
        .map(|i| (elevation[i] - level).max(MIN_TARGET_METERS))
        .collect();
    let land = steady_landscape(surface, &target, wet, seed)?;
    // Keep the version-4 envelope and add the fluvial relief around its local
    // mean, damped on low ground: local relief grows with height on Earth.
    let envelope = smooth(surface, &target, wet, SMOOTHING_PASSES);
    let local = smooth(surface, &land.height, wet, SMOOTHING_PASSES);
    Ok((0..n)
        .map(|i| {
            if wet[i] != 0 {
                return 0.;
            }
            let weight = (envelope[i] / RELIEF_REFERENCE_METERS).clamp(MIN_RELIEF_WEIGHT, 1.);
            let height = envelope[i] + weight * (land.height[i] - local[i]);
            level + height.max(MIN_TARGET_METERS) - elevation[i]
        })
        .collect())
}

/// Steady-state stream-power landscape whose regional envelope follows `target`
/// (meters above the water level): steepness is the ratio of the smoothed target
/// to the smoothed drainage coordinate chi, so `h ≈ k·chi` region by region.
pub fn steady_landscape(
    s: &Surface,
    target: &[f64],
    wet: &[u32],
    seed: &str,
) -> Result<Landscape, String> {
    let n = target.len();
    let mut rng = Random::stream(seed, "multiscale-geography-5/steepness");
    let noise: Vec<f64> = (0..n)
        .map(|_| {
            let (u, v) = (
                (f64::from(rng.next_u32()) + 0.5) / 4294967296.,
                f64::from(rng.next_u32()) / 4294967296.,
            );
            let gauss = (-2. * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos();
            (STEEPNESS_NOISE * gauss).exp()
        })
        .collect();
    let salt = rng.next_u32();
    let mut receivers = flood_receivers(s, target, wet);
    let mut height = vec![0.; n];
    let mut area = vec![0.; n];
    let mut order = Vec::with_capacity(n);
    let mut iterations = 0;
    let mut chi = vec![0.; n];
    let smooth_target = smooth(s, target, wet, SMOOTHING_PASSES);
    for iteration in 0..ITERATIONS {
        iterations = iteration + 1;
        stack_order(&receivers, wet, &mut order);
        if order.len() != n {
            return Err("Erosion drainage is not a forest rooted in water.".into());
        }
        area.copy_from_slice(&s.areas);
        for &i in order.iter().rev() {
            let r = receivers[i] as usize;
            if r != i {
                area[r] += area[i];
            }
        }
        // Integrated drainage coordinate chi: height per unit steepness.
        for &i in &order {
            let r = receivers[i] as usize;
            chi[i] = if wet[i] != 0 {
                0.
            } else {
                chi[r] + area[i].powf(-AREA_EXPONENT) * distance(s, i, r)
            };
        }
        // Regional steepness reproduces the smoothed version-4 height: h ≈ k·chi.
        let smooth_chi = smooth(s, &chi, wet, SMOOTHING_PASSES);
        for &i in &order {
            let r = receivers[i] as usize;
            height[i] = if wet[i] != 0 {
                0.
            } else {
                let k = smooth_target[i] / smooth_chi[i].max(f64::MIN_POSITIVE);
                height[r] + noise[i] * k * area[i].powf(-AREA_EXPONENT) * distance(s, i, r)
            };
        }
        let next = steepest_receivers(s, &height, wet, salt);
        if next == receivers && iteration > 2 {
            break;
        }
        receivers = next;
    }
    Ok(Landscape {
        height,
        receivers,
        area,
        iterations,
    })
}

fn distance(s: &Surface, i: usize, j: usize) -> f64 {
    for k in s.offsets[i] as usize..s.offsets[i + 1] as usize {
        if s.neighbors[k] as usize == j {
            return s.distances[k];
        }
    }
    unreachable!("receiver must be a neighbor")
}

/// Priority flood from all water: every dry region drains to the neighbor it was
/// reached from, so closed dry depressions route over their lowest sill.
fn flood_receivers(s: &Surface, height: &[f64], wet: &[u32]) -> Vec<u32> {
    let n = height.len();
    let mut receivers: Vec<u32> = (0..n as u32).collect();
    let mut done = vec![false; n];
    let mut queue = BinaryHeap::new();
    for i in 0..n {
        if wet[i] != 0 {
            done[i] = true;
            queue.push(Reverse(Key(f64::NEG_INFINITY, i)));
        }
    }
    while let Some(Reverse(Key(_, i))) = queue.pop() {
        for &j in &s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize] {
            let j = j as usize;
            if !done[j] {
                done[j] = true;
                receivers[j] = i as u32;
                queue.push(Reverse(Key(height[j], j)));
            }
        }
    }
    receivers
}

/// Randomized steepest-descent receivers; water is the base level (height zero).
/// Each directed link's slope is scaled by a fixed factor in
/// `[1 − RECEIVER_JITTER, 1]`, a mesh-independent form of Fairfield and
/// Leymarie's (1991) Rho8 that removes the grid's preferred flow directions.
fn steepest_receivers(s: &Surface, height: &[f64], wet: &[u32], salt: u32) -> Vec<u32> {
    (0..height.len())
        .map(|i| {
            if wet[i] != 0 {
                return i as u32;
            }
            let mut best = (0., i as u32);
            for k in s.offsets[i] as usize..s.offsets[i + 1] as usize {
                let j = s.neighbors[k] as usize;
                let h = if wet[j] != 0 { 0. } else { height[j] };
                let slope = (height[i] - h) / s.distances[k] * link_factor(salt, i, j);
                if slope > best.0 {
                    best = (slope, j as u32);
                }
            }
            best.1
        })
        .collect()
}

/// Deterministic per-link factor in `[1 − RECEIVER_JITTER, 1]`.
fn link_factor(salt: u32, i: usize, j: usize) -> f64 {
    let mut v = salt ^ (i as u32).wrapping_mul(0x9e37_79b1) ^ (j as u32).wrapping_mul(0x85eb_ca77);
    v = (v ^ (v >> 16)).wrapping_mul(0x7feb_352d);
    v = (v ^ (v >> 15)).wrapping_mul(0x846c_a68b);
    v ^= v >> 16;
    1. - RECEIVER_JITTER * f64::from(v) / 4294967296.
}

/// Regions ordered from water upstream (each after its receiver).
pub(crate) fn stack_order(receivers: &[u32], wet: &[u32], order: &mut Vec<usize>) {
    let n = receivers.len();
    let mut offsets = vec![0u32; n + 1];
    for (i, &r) in receivers.iter().enumerate() {
        if r as usize != i {
            offsets[r as usize + 1] += 1;
        }
    }
    for i in 0..n {
        offsets[i + 1] += offsets[i];
    }
    let mut fill = offsets.clone();
    let mut donors = vec![0u32; offsets[n] as usize];
    for (i, &r) in receivers.iter().enumerate() {
        if r as usize != i {
            donors[fill[r as usize] as usize] = i as u32;
            fill[r as usize] += 1;
        }
    }
    order.clear();
    order.extend((0..n).filter(|&i| wet[i] != 0 || receivers[i] as usize == i));
    let mut cursor = 0;
    while cursor < order.len() {
        let i = order[cursor];
        cursor += 1;
        order.extend(
            donors[offsets[i] as usize..offsets[i + 1] as usize]
                .iter()
                .map(|&d| d as usize),
        );
    }
}

/// Repeated dry-neighborhood averaging.
fn smooth(s: &Surface, value: &[f64], wet: &[u32], passes: usize) -> Vec<f64> {
    let mut v = value.to_vec();
    for _ in 0..passes {
        v = ring_mean(s, &v, wet);
    }
    v
}

/// Area-weighted mean of dry values over each region and its dry neighbors.
fn ring_mean(s: &Surface, value: &[f64], wet: &[u32]) -> Vec<f64> {
    (0..value.len())
        .map(|i| {
            if wet[i] != 0 {
                return 0.;
            }
            let (mut sum, mut weight) = (value[i] * s.areas[i], s.areas[i]);
            for k in s.offsets[i] as usize..s.offsets[i + 1] as usize {
                let j = s.neighbors[k] as usize;
                if wet[j] == 0 {
                    sum += value[j] * s.areas[j];
                    weight += s.areas[j];
                }
            }
            sum / weight
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Recipe, World, coastal_lowlands};

    fn recipe(model: &str, subdivision: u32) -> Recipe {
        serde_json::from_value(serde_json::json!({"schemaVersion":1,"modelVersion":model,
            "randomVersion":"fnv1a-utf8-mulberry32-1","seed":"first-light","subdivision":subdivision,
            "radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,
            "continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,"detailAmplitudeMeters":300.,
            "terrainPreparationPasses":4,"water":{"mode":"coverage","fraction":0.71}})).unwrap()
    }

    #[test]
    fn contributions_sum_to_elevation() {
        let w = World::generate(recipe(MODEL_VERSION, 4)).unwrap();
        let t = &w.terrain;
        for i in 0..t.elevation.len() {
            let sum = t.baseline[i]
                + t.convergence[i]
                + t.divergence[i]
                + t.detail[i]
                + t.preparation[i]
                + t.lowland[i]
                + t.seafloor[i]
                + t.erosion[i]
                + t.sediment[i];
            assert!((t.elevation[i] - sum).abs() < 1e-8);
        }
        let v4 = World::generate(recipe(coastal_lowlands::MODEL_VERSION, 4)).unwrap();
        assert!(v4.terrain.seafloor.iter().all(|&v| v == 0.));
        assert!(v4.terrain.erosion.iter().all(|&v| v == 0.));
        assert!(v4.terrain.sediment.is_empty());
    }

    #[test]
    fn erosion_keeps_shorelines_and_regional_heights_and_drains_all_land_to_water() {
        let w = World::generate(recipe(coastal_lowlands::MODEL_VERSION, 5)).unwrap();
        let (s, h, wet, level) = (
            &w.surface,
            &w.terrain.elevation,
            &w.water.body_ids,
            w.water.level_meters,
        );
        let change = erosion(s, h, wet, level, "first-light").unwrap();
        assert_eq!(change, erosion(s, h, wet, level, "first-light").unwrap());
        assert_ne!(change, erosion(s, h, wet, level, "other").unwrap());
        let (mut before, mut after, mut area) = (0., 0., 0.);
        for i in 0..h.len() {
            if wet[i] != 0 {
                assert_eq!(change[i], 0.);
                continue;
            }
            assert!(h[i] + change[i] >= level + MIN_TARGET_METERS - 1e-9);
            before += (h[i] - level) * s.areas[i];
            after += (h[i] + change[i] - level) * s.areas[i];
            area += s.areas[i];
        }
        assert!(
            (after / before - 1.).abs() < 0.1,
            "mean land height {} -> {}",
            before / area,
            after / area
        );
        // The fitted network drains every dry region to water through lower receivers.
        let target: Vec<f64> = (0..h.len())
            .map(|i| (h[i] - level).max(MIN_TARGET_METERS))
            .collect();
        let land = steady_landscape(s, &target, wet, "first-light").unwrap();
        for i in 0..h.len() {
            let mut j = i;
            for _ in 0..h.len() {
                if wet[j] != 0 {
                    break;
                }
                let r = land.receivers[j] as usize;
                assert_ne!(r, j, "dry sink at {j}");
                assert!(wet[r] != 0 || land.height[r] < land.height[j]);
                j = r;
            }
            assert!(wet[j] != 0);
        }
        assert!(land.iterations < ITERATIONS, "network did not settle");
    }

    #[test]
    fn local_relief_grows_with_height() {
        let w = World::generate(recipe(MODEL_VERSION, 5)).unwrap();
        let (s, t, wet, level) = (
            &w.surface,
            &w.terrain,
            &w.water.body_ids,
            w.water.level_meters,
        );
        let relief = |lo: f64, hi: f64| {
            let mut v: Vec<f64> = (0..t.elevation.len())
                .filter(|&i| wet[i] == 0 && (lo..hi).contains(&(t.elevation[i] - level)))
                .map(|i| {
                    (s.offsets[i] as usize..s.offsets[i + 1] as usize)
                        .map(|k| s.neighbors[k] as usize)
                        .filter(|&j| wet[j] == 0)
                        .map(|j| (t.elevation[i] - t.elevation[j]).abs())
                        .fold(0., f64::max)
                })
                .collect();
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        let (low, middle, high) = (relief(0., 300.), relief(300., 1000.), relief(1000., 1e9));
        assert!(low < middle && middle < high, "{low} {middle} {high}");
    }

    #[test]
    fn no_water_means_no_erosion() {
        let surface = Surface::build(2, 1000.);
        let n = surface.centers.len();
        let h: Vec<f64> = (0..n).map(|i| i as f64).collect();
        assert!(
            erosion(&surface, &h, &vec![0; n], 0., "x")
                .unwrap()
                .iter()
                .all(|&v| v == 0.)
        );
        assert!(erosion(&surface, &h, &vec![0; n - 1], 0., "x").is_err());
    }

    #[test]
    fn only_version_5_accepts_subdivision_7() {
        assert!(recipe(MODEL_VERSION, 7).validate().is_ok());
        assert!(recipe(MODEL_VERSION, 8).validate().is_err());
        assert!(
            recipe(coastal_lowlands::MODEL_VERSION, 7)
                .validate()
                .is_err()
        );
    }
}
