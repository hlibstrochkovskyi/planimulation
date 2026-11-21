//! Area-weighted ocean hypsometry and shoreline comparisons for bathymetry-1.
use planimulation_core::{
    Recipe, World, bathymetry, coastal_lowlands, eroded_landscape, mountain_belts,
    terrain_preparation,
    water::{Water, WaterSettings},
};
use std::time::Instant;

fn reference_bed(world: &World) -> Vec<f64> {
    let t = &world.terrain;
    (0..t.elevation.len())
        .map(|i| t.baseline[i] + t.convergence[i] + t.divergence[i] + t.detail[i])
        .collect()
}

// V4 accepts subdivisions up to 6. Apply its preparation on the common
// pre-bathymetry input to obtain a measurement-only V4 control at 7.
fn v4_control(world: &World, raw: &[f64]) -> Result<(Vec<f64>, Water), String> {
    let s = &world.surface;
    let settings = &world.recipe.water;
    let t = &world.terrain;
    let mut bed =
        terrain_preparation::prepare(s, raw, world.recipe.terrain_preparation_passes.unwrap_or(0))?
            .elevation_meters;
    let original = Water::generate(s, &bed, settings)?;
    let lowering = coastal_lowlands::lowering(
        s,
        &bed,
        &t.baseline,
        &t.convergence,
        &original.body_ids,
        original.level_meters,
        [
            coastal_lowlands::COAST_FACTOR,
            coastal_lowlands::INTERIOR_FACTOR,
        ],
    )?;
    for (height, delta) in bed.iter_mut().zip(lowering) {
        *height += delta;
    }
    let fill = mountain_belts::fill_shallow_depressions(
        s,
        &bed,
        &original.body_ids,
        original.level_meters,
        mountain_belts::MAX_FILL_DEPTH_METERS,
    )?;
    for (height, delta) in bed.iter_mut().zip(fill) {
        *height += delta;
    }
    let water = Water::generate(s, &bed, settings)?;
    Ok((bed, water))
}

fn quantile(samples: &[(f64, f64)], fraction: f64) -> f64 {
    let target = fraction * samples.iter().map(|(_, area)| area).sum::<f64>();
    let mut total = 0.;
    for &(value, area) in samples {
        total += area;
        if total >= target {
            return value;
        }
    }
    f64::NAN
}

fn print_metrics(
    family: &str,
    world: &World,
    bed: &[f64],
    water: &Water,
    profile: &bathymetry::Profile,
    changed_shore: f64,
) {
    let mut ocean = Vec::new();
    let mut ridge = Vec::new();
    let mut trench = Vec::new();
    let mut land = 0.;
    let mut low_land = 0.;
    for (i, &area) in world.surface.areas.iter().enumerate() {
        let body = water.body_ids[i];
        if body == 0 {
            land += area;
            if bed[i] - water.level_meters < 200. {
                low_land += area;
            }
        } else if body == water.main_ocean_id {
            let sample = (water.depth_meters[i], area);
            ocean.push(sample);
            if profile.ridge_axis[i] && !profile.trench_axis[i] {
                ridge.push(sample);
            }
            if profile.trench_axis[i] {
                trench.push(sample);
            }
        }
    }
    for samples in [&mut ocean, &mut ridge, &mut trench] {
        samples.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    }
    let ocean_area = ocean.iter().map(|(_, area)| area).sum::<f64>();
    let percent = |predicate: fn(f64) -> bool| {
        100. * ocean
            .iter()
            .filter(|&&(d, _)| predicate(d))
            .map(|(_, a)| a)
            .sum::<f64>()
            / ocean_area
    };
    let mean_depth = ocean.iter().map(|(d, a)| d * a).sum::<f64>() / ocean_area;
    let total = world.surface.areas.iter().sum::<f64>();
    let recipe = &world.recipe;
    println!(
        "{} | {} | {} | {:.0} | {:.0} | {:.0} | {:.0} | {:.0} | {:.2} | {:.2} | {:.0} | {:.0} | {:.0} | {:.0} | {:.3} | {:.2} | {:.1} | {:.3}",
        recipe.seed,
        recipe.subdivision,
        family,
        quantile(&ocean, 0.1),
        quantile(&ocean, 0.5),
        quantile(&ocean, 0.9),
        ocean.last().map_or(f64::NAN, |sample| sample.0),
        mean_depth,
        percent(|d| d <= 200.),
        percent(|d| d > 3000.),
        quantile(&ridge, 0.1),
        quantile(&ridge, 0.5),
        quantile(&trench, 0.5),
        trench.last().map_or(f64::NAN, |sample| sample.0),
        100. * land / total,
        100. * low_land / land,
        water.level_meters,
        changed_shore
    );
}

fn main() -> Result<(), String> {
    println!(
        "Seed | Subd. | Model | D10 m | D50 m | D90 m | Max m | Mean m | Shelf % | >3 km % | Ridge D10 m | Ridge D50 m | Trench D50 m | Trench max m | Land % | Land <200 m % | Level m | Switched %"
    );
    // Area-weighted 20 arc-minute ETOPO1 Ice Surface sample. The maximum is
    // from the full-resolution NOAA curve. See docs/bathymetry.md for the method.
    println!(
        "Earth | — | ETOPO1 | 549 | 4097 | 5379 | 10911 | 3686 | 7.45 | 75.19 | — | — | — | — | 29.049 | 28.13 | 0.0 | —"
    );
    for subdivision in [6, 7] {
        for seed in ["first-light", "5c151f46", "4836187e"] {
            let recipe = Recipe {
                schema_version: 1,
                model_version: eroded_landscape::MODEL_VERSION.into(),
                random_version: "fnv1a-utf8-mulberry32-1".into(),
                seed: seed.into(),
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
            };
            let start = Instant::now();
            let world = World::generate(recipe.clone())?;
            let world_time = start.elapsed();
            let raw = reference_bed(&world);
            let start = Instant::now();
            let profile = bathymetry::profile(
                &world.surface,
                &world.crust,
                &world.tectonics,
                &recipe,
                &raw,
            )?;
            let profile_time = start.elapsed();
            let (v4_bed, v4_water) = v4_control(&world, &raw)?;
            if subdivision == 6 {
                let actual = World::generate(Recipe {
                    model_version: coastal_lowlands::MODEL_VERSION.into(),
                    ..recipe
                })?;
                assert_eq!(actual.terrain.elevation, v4_bed);
                assert_eq!(actual.water, v4_water);
            }
            let switched = world
                .surface
                .areas
                .iter()
                .enumerate()
                .filter(|&(i, _)| (world.water.body_ids[i] == 0) != (v4_water.body_ids[i] == 0))
                .map(|(_, area)| area)
                .sum::<f64>()
                / world.surface.areas.iter().sum::<f64>();
            print_metrics(
                if subdivision == 7 { "IV control" } else { "IV" },
                &world,
                &v4_bed,
                &v4_water,
                &profile,
                0.,
            );
            print_metrics(
                "V",
                &world,
                &world.terrain.elevation,
                &world.water,
                &profile,
                100. * switched,
            );
            eprintln!(
                "{seed} / {subdivision}: V generation {:.3} s, bathymetry {:.3} s",
                world_time.as_secs_f64(),
                profile_time.as_secs_f64()
            );
        }
    }
    Ok(())
}
