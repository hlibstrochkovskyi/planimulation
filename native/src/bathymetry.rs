//! `bathymetry-1`: static thermal subsidence and marine margins for geography V.
use crate::{
    Recipe, Surface,
    crust::Crust,
    mountain_belts::Key,
    tectonics::{CONVERGENT, DIVERGENT, Tectonics},
    terrain::crust_baseline,
    water::Water,
};
use std::{cmp::Reverse, collections::BinaryHeap};

pub const MODEL_VERSION: &str = "bathymetry-1";
pub const MAX_CONTRIBUTION_METERS: f64 = 12_000.;
const OCEANIC_LIMIT: f64 = 0.5;
const MAX_AGE_MYR: f64 = 180.;
const MIN_HALF_RATE_METERS_PER_MYR: f64 = 5_000.;
const MAX_HALF_RATE_METERS_PER_MYR: f64 = 70_000.;
const MARGIN_LIMIT_METERS: f64 = 1_500_000.;
const ACTIVE_LIMIT_METERS: f64 = 600_000.;
const OUTER_RISE_LIMIT_METERS: f64 = 300_000.;

pub struct Profile {
    pub contribution_meters: Vec<f64>,
    pub ridge_axis: Vec<bool>,
    pub trench_axis: Vec<bool>,
}

/// Per-region ocean-floor contribution in meters, added before dry preparation
/// and the authoritative initial water fit.
pub fn seafloor(
    surface: &Surface,
    crust: &Crust,
    tectonics: &Tectonics,
    recipe: &Recipe,
    elevation: &[f64],
) -> Result<Vec<f64>, String> {
    Ok(profile(surface, crust, tectonics, recipe, elevation)?.contribution_meters)
}

/// Generate the contribution and diagnostic region masks for the sampled axes.
pub fn profile(
    surface: &Surface,
    crust: &Crust,
    tectonics: &Tectonics,
    recipe: &Recipe,
    elevation: &[f64],
) -> Result<Profile, String> {
    let n = surface.centers.len();
    if elevation.len() != n
        || crust.continentality.len() != n
        || crust.thickness_meters.len() != n
        || crust.density_kg_per_cubic_meter.len() != n
        || tectonics.owners.len() != n
        || tectonics.boundary_cells.len() != 2 * tectonics.boundary_types.len()
        || tectonics.boundary_motion.len() != tectonics.boundary_cells.len()
        || tectonics.boundary_cells.iter().any(|&id| id as usize >= n)
    {
        return Err("Invalid bathymetry input.".into());
    }
    let water = Water::generate(surface, elevation, &recipe.water)?;
    let wet = |i: usize| water.body_ids[i] != 0;
    let oceanic = |i: usize| crust.continentality[i] < OCEANIC_LIMIT;
    let same_oceanic_plate =
        |i: usize, j: usize| tectonics.owners[i] == tectonics.owners[j] && oceanic(j);
    let contacts = tectonics
        .boundary_cells
        .as_chunks::<2>()
        .0
        .iter()
        .zip(tectonics.boundary_motion.as_chunks::<2>().0)
        .zip(&tectonics.boundary_types)
        .map(|((&[a, b], &[opening, _]), &kind)| (a as usize, b as usize, opening, kind));
    let mut spreading = vec![(0., 0); n];
    for (a, b, opening, kind) in contacts.clone() {
        if kind == DIVERGENT && oceanic(a) && oceanic(b) {
            for i in [a, b] {
                spreading[i].0 += 0.5e6 * opening;
                spreading[i].1 += 1;
            }
        }
    }
    let ridges = nearest(
        surface,
        spreading
            .iter()
            .enumerate()
            .filter(|(_, rate)| rate.1 > 0)
            .map(|(i, _)| (i, 0.)),
        MAX_AGE_MYR * MAX_HALF_RATE_METERS_PER_MYR,
        same_oceanic_plate,
    );
    let age: Vec<_> = ridges
        .iter()
        .map(|&(distance, source)| {
            let (sum, count) = spreading[source];
            let half_rate = (sum / f64::from(count.max(1)))
                .clamp(MIN_HALF_RATE_METERS_PER_MYR, MAX_HALF_RATE_METERS_PER_MYR);
            (distance / half_rate).min(MAX_AGE_MYR)
        })
        .collect();

    let mut trench_depth = vec![0_f64; n];
    for (a, b, opening, kind) in contacts {
        if kind != CONVERGENT {
            continue;
        }
        let slab = |i: usize| Slab {
            oceanic: oceanic(i),
            density: crust.density_kg_per_cubic_meter[i],
            age: age[i],
            plate: tectonics.owners[i],
        };
        if let Some(i) = incoming(slab(a), slab(b)).map(|side| [a, b][side])
            && wet(i)
        {
            let closing = -opening;
            let depth = 7_600. + 1_500. * closing / (0.04 + closing) + 750. * age[i] / MAX_AGE_MYR;
            trench_depth[i] = trench_depth[i].max(depth);
        }
    }
    let trench_sources = || {
        trench_depth
            .iter()
            .enumerate()
            .filter(|&(_, depth)| *depth > 0.)
            .map(|(i, _)| (i, 0.))
    };
    let trench = nearest(
        surface,
        trench_sources(),
        OUTER_RISE_LIMIT_METERS,
        same_oceanic_plate,
    );
    let active = nearest(surface, trench_sources(), ACTIVE_LIMIT_METERS, |_, _| true);

    let contacts = |i: usize| {
        let c = crust.continentality[i];
        (surface.offsets[i] as usize..surface.offsets[i + 1] as usize).filter_map(move |k| {
            let j = surface.neighbors[k] as usize;
            let other = crust.continentality[j];
            (!oceanic(i) && oceanic(j))
                .then(|| (i, surface.distances[k] * (c - OCEANIC_LIMIT) / (c - other)))
        })
    };
    let continental_margin = nearest(
        surface,
        (0..n).flat_map(contacts),
        MARGIN_LIMIT_METERS,
        |_, j| !oceanic(j),
    );
    let shore_contacts = |i: usize| {
        let depth = water.depth_meters[i];
        (surface.offsets[i] as usize..surface.offsets[i + 1] as usize).filter_map(move |k| {
            let j = surface.neighbors[k] as usize;
            (wet(i) && !wet(j) && !oceanic(j)).then(|| {
                let rise = elevation[j] - water.level_meters;
                (i, surface.distances[k] * depth / (depth + rise))
            })
        })
    };
    let coast = nearest(
        surface,
        (0..n).flat_map(shore_contacts),
        MARGIN_LIMIT_METERS,
        |_, j| wet(j),
    );

    let contribution_meters = (0..n)
        .map(|i| {
            let margin_weight = if oceanic(i) {
                1.
            } else {
                1. - smooth((continental_margin[i].0 - 900_000.) / 600_000.)
            };
            if !wet(i) || margin_weight == 0. {
                return 0.;
            }
            let residual = (elevation[i]
                - crust_baseline(
                    crust.thickness_meters[i],
                    crust.density_kg_per_cubic_meter[i],
                ))
            .clamp(-250., 250.);
            let abyss = thermal_depth(age[i]) - 0.6 * residual;
            let (shore_distance, shore_source) = coast[i];
            let activity = 1. - smooth((active[shore_source].0 - 100_000.) / 500_000.);
            let mut depth = margin_depth(shore_distance, activity, abyss);
            let (trench_distance, trench_source) = trench[i];
            if trench_distance.is_finite() {
                let cut = 1. - smooth(trench_distance / 115_000.);
                let rise = smooth(trench_distance / 150_000.)
                    * (1. - smooth((trench_distance - 150_000.) / 150_000.));
                depth += cut * (trench_depth[trench_source] - depth) - 300. * rise;
            }
            let target = water.level_meters - depth.max(1.);
            ((target - elevation[i]) * margin_weight)
                .clamp(-MAX_CONTRIBUTION_METERS, MAX_CONTRIBUTION_METERS)
        })
        .collect();
    Ok(Profile {
        contribution_meters,
        ridge_axis: ridges.iter().map(|&(distance, _)| distance == 0.).collect(),
        trench_axis: trench_depth.iter().map(|&depth| depth > 0.).collect(),
    })
}

/// Shortest physical paths with a stable source tie-break. Infinite entries
/// have no reachable source within the cutoff.
fn nearest(
    surface: &Surface,
    seeds: impl Iterator<Item = (usize, f64)>,
    limit: f64,
    can_cross: impl Fn(usize, usize) -> bool,
) -> Vec<(f64, usize)> {
    let mut nearest = vec![(f64::INFINITY, 0); surface.centers.len()];
    let mut queue = BinaryHeap::new();
    for (i, distance) in seeds {
        if distance <= limit && distance < nearest[i].0 {
            nearest[i] = (distance, i);
            queue.push(Reverse((Key(distance, i), i)));
        }
    }
    while let Some(Reverse((Key(distance, source), i))) = queue.pop() {
        if nearest[i] != (distance, source) {
            continue;
        }
        for k in surface.offsets[i] as usize..surface.offsets[i + 1] as usize {
            let j = surface.neighbors[k] as usize;
            let next = distance + surface.distances[k];
            if next <= limit && (next, source) < nearest[j] && can_cross(i, j) {
                nearest[j] = (next, source);
                queue.push(Reverse((Key(next, source), j)));
            }
        }
    }
    nearest
}

fn smooth(x: f64) -> f64 {
    let t = x.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

// GDH1 (Stein & Stein, 1992), depth in meters, age in Myr.
fn thermal_depth(age: f64) -> f64 {
    if age < 20. {
        2600. + 365. * age.sqrt()
    } else {
        5651. - 2473. * (-0.0278 * age).exp()
    }
}

fn margin_depth(distance: f64, activity: f64, abyss: f64) -> f64 {
    let shelf = 210_000. - 150_000. * activity;
    let slope = 320_000. - 140_000. * activity;
    if distance <= shelf {
        return 25. + 135. * distance / shelf;
    }
    let foot = abyss.min(3_000.);
    let descent = smooth((distance - shelf) / slope);
    let rise = smooth((distance - shelf - slope) / 480_000.);
    160. + (foot - 160.) * descent + (abyss - foot) * rise
}

#[derive(Clone, Copy)]
struct Slab {
    oceanic: bool,
    density: f64,
    age: f64,
    plate: u32,
}

fn incoming(a: Slab, b: Slab) -> Option<usize> {
    match (a.oceanic, b.oceanic) {
        (false, false) => None,
        (true, false) => Some(0),
        (false, true) => Some(1),
        (true, true) => {
            // Older, colder lithosphere adds negative buoyancy to the static crust column.
            let buoyancy_proxy = |s: Slab| s.density + 0.2 * s.age;
            Some(usize::from(
                buoyancy_proxy(b) > buoyancy_proxy(a)
                    || (buoyancy_proxy(b) == buoyancy_proxy(a) && b.plate < a.plate),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{World, eroded_landscape, water::WaterSettings};

    fn recipe(subdivision: u32) -> Recipe {
        Recipe {
            schema_version: 1,
            model_version: eroded_landscape::MODEL_VERSION.into(),
            random_version: "fnv1a-utf8-mulberry32-1".into(),
            seed: "first-light".into(),
            subdivision,
            radius_meters: 6_371_000.,
            plate_count: 12,
            max_plate_speed_cm_per_year: 8.,
            continental_fraction: 0.38,
            continental_scale: 1.,
            relief_scale: 1.,
            boundary_width_km: 300.,
            detail_amplitude_meters: 300.,
            terrain_preparation_passes: Some(4),
            water: WaterSettings::Coverage { fraction: 0.71 },
        }
    }

    #[test]
    fn cooling_deepens_monotonically_from_ridge_to_abyss() {
        assert_eq!(thermal_depth(0.), 2_600.);
        assert!((5_600. ..5_651.).contains(&thermal_depth(MAX_AGE_MYR)));
        let mut prior = thermal_depth(0.);
        for step in 1..=1800 {
            let depth = thermal_depth(f64::from(step) / 10.);
            assert!(depth > prior);
            prior = depth;
        }
    }

    #[test]
    fn convergent_axes_follow_the_incoming_oceanic_slab() {
        let surface = Surface::build(2, 6_371_000.);
        let mut recipe = recipe(2);
        recipe.water = WaterSettings::Coverage { fraction: 1. };
        let mut crust = Crust::build(&surface, "fixture", 0., 1.);
        let mut tectonics = Tectonics::build(&surface, "fixture", 2, 0., 6_371_000.);
        let a = 0;
        let b = surface.neighbors[0] as usize;
        tectonics.owners[a] = 0;
        tectonics.owners[b] = 1;
        tectonics.boundary_cells = vec![a as u32, b as u32];
        tectonics.boundary_motion = vec![-0.05, 0.];
        tectonics.boundary_types = vec![CONVERGENT];
        let bed = vec![-4_500.; surface.centers.len()];
        for (ca, cb, da, db, expected) in [
            (0., 0.8, 3000., 2840., Some(a)),
            (0.8, 0., 2840., 3000., Some(b)),
            (0.2, 0., 2960., 3000., Some(b)),
            (0.8, 0.9, 2840., 2820., None),
        ] {
            crust.continentality[a] = ca;
            crust.continentality[b] = cb;
            crust.density_kg_per_cubic_meter[a] = da;
            crust.density_kg_per_cubic_meter[b] = db;
            let model = profile(&surface, &crust, &tectonics, &recipe, &bed).unwrap();
            assert_eq!(
                model.trench_axis.iter().filter(|&&axis| axis).count(),
                usize::from(expected.is_some())
            );
            if let Some(i) = expected {
                assert!((7_000. ..10_000.).contains(&(1. - model.contribution_meters[i])));
            }
        }
        let young = Slab {
            oceanic: true,
            density: 3000.,
            age: 0.,
            plate: 0,
        };
        let old = Slab {
            age: MAX_AGE_MYR,
            plate: 1,
            ..young
        };
        assert_eq!(incoming(young, old), Some(1));
    }

    #[test]
    fn passive_shelf_is_wider_and_both_profiles_reach_the_same_abyss() {
        let abyss = thermal_depth(MAX_AGE_MYR);
        assert!(margin_depth(200_000., 0., abyss) < 200.);
        assert!(margin_depth(200_000., 1., abyss) > 1_000.);
        assert_eq!(margin_depth(530_000., 0., abyss), 3_000.);
        assert_eq!(margin_depth(240_000., 1., abyss), 3_000.);
        assert_eq!(margin_depth(1_200_000., 0., abyss), abyss);
        assert_eq!(margin_depth(1_200_000., 1., abyss), abyss);
    }

    #[test]
    fn generated_floors_are_deterministic_bounded_and_respect_the_reference_shore() {
        for subdivision in [0, 2, 7] {
            let world = World::generate(recipe(subdivision)).unwrap();
            let bed: Vec<_> = (0..world.surface.centers.len())
                .map(|i| {
                    world.terrain.baseline[i]
                        + world.terrain.convergence[i]
                        + world.terrain.divergence[i]
                        + world.terrain.detail[i]
                })
                .collect();
            let floor = &world.terrain.seafloor;
            assert_eq!(
                *floor,
                seafloor(
                    &world.surface,
                    &world.crust,
                    &world.tectonics,
                    &world.recipe,
                    &bed
                )
                .unwrap()
            );
            let reference = Water::generate(&world.surface, &bed, &world.recipe.water).unwrap();
            let mut found_dry_interior = false;
            for (i, &change) in floor.iter().enumerate() {
                assert!(change.abs() <= MAX_CONTRIBUTION_METERS);
                if reference.body_ids[i] == 0 {
                    assert_eq!(change, 0.);
                    found_dry_interior |= world.crust.continentality[i] == 1.;
                } else {
                    assert!(bed[i] + change < reference.level_meters);
                }
            }
            assert!(found_dry_interior);
            assert!(floor.iter().any(|&change| change.abs() > 100.));
        }
    }

    #[test]
    fn a_flooded_continental_interior_stays_unchanged() {
        let surface = Surface::build(2, 6_371_000.);
        let mut recipe = recipe(2);
        recipe.water = WaterSettings::Coverage { fraction: 1. };
        let crust = Crust::build(&surface, "fixture", 1., 1.);
        let tectonics = Tectonics::build(&surface, "fixture", 2, 0., 6_371_000.);
        let bed = vec![100.; surface.centers.len()];
        assert!(
            seafloor(&surface, &crust, &tectonics, &recipe, &bed)
                .unwrap()
                .iter()
                .all(|&change| change == 0.)
        );
    }
}
