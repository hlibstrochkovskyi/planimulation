//! Climate-normal runoff, ocean discharge and river mouths for the new-world recipe.
//! Usage: rivers_report [SEED ...] (default: the three review seeds)
use planimulation_core::{Recipe, World, climate_normals::Normals, rivers::Rivers};
use std::time::Instant;

fn main() {
    let seeds: Vec<_> = std::env::args().skip(1).collect();
    if seeds.is_empty() {
        for seed in ["first-light", "5c151f46", "4836187e"] {
            report(seed);
        }
    } else {
        for seed in seeds {
            report(&seed);
        }
    }
}

fn report(seed: &str) {
    let recipe: Recipe = serde_json::from_value(serde_json::json!({
        "schemaVersion": 1, "modelVersion": "multiscale-geography-4",
        "randomVersion": "fnv1a-utf8-mulberry32-1", "seed": seed,
        "subdivision": 6, "radiusMeters": 6371000, "plateCount": 12, "maxPlateSpeedCmPerYear": 8,
        "continentalFraction": 0.38, "continentalScale": 1., "reliefScale": 1.,
        "boundaryWidthKm": 300., "detailAmplitudeMeters": 300., "terrainPreparationPasses": 4,
        "water": { "mode": "coverage", "fraction": 0.71 }
    }))
    .unwrap();
    let world = World::generate(recipe).unwrap();
    let climate = Normals::from_world(&world).unwrap();
    let start = Instant::now();
    let rivers = Rivers::from_world(&world, &climate).unwrap();
    let elapsed = start.elapsed();
    let t = rivers.totals;
    let km3 = |m3| m3 / 1e9;
    let depth = |m3| m3 / t.land_area * 1e3;
    println!(
        "{seed} | P {:.1} | AET {:.1} | runoff {:.1} | land storage {:.1} mm/year | ocean {:.0} km³/year | land runoff to ocean {:.0} km³/year ({:.1}%) | endorheic land {:.1}% | {:.1} ms",
        depth(t.land_precipitation),
        depth(t.land_aet),
        depth(t.land_runoff),
        depth(t.land_storage),
        km3(t.ocean_discharge),
        km3(t.land_runoff_to_ocean),
        t.land_runoff_to_ocean / t.land_runoff * 100.,
        t.endorheic_area / t.land_area * 100.,
        elapsed.as_secs_f64() * 1000.
    );
    println!(
        "  land runoff fate: {:.2}% ocean, {:.2}% closed terminals, {:.2}% evaporation in overflowing lakes",
        t.land_runoff_to_ocean / t.land_runoff * 100.,
        t.land_runoff_to_endorheic / t.land_runoff * 100.,
        t.land_runoff_evaporated_in_spilling_lakes / t.land_runoff * 100.
    );
    println!(
        "  inland terminals: {} overflowing / {} closed ({} previously wet), lake rain {:.1}, evaporation {:.1}, terminal storage {:.1} km³/year",
        rivers.lakes.iter().filter(|l| l.spill > 0.).count(),
        rivers.lakes.iter().filter(|l| l.spill == 0.).count(),
        rivers
            .lakes
            .iter()
            .filter(|l| world.water.body_ids[l.region as usize] > 0)
            .count(),
        km3(t.lake_precipitation),
        km3(t.lake_evaporation),
        km3(t.terminal_storage)
    );
    let mut mouths: Vec<_> = rivers
        .mean_discharge
        .iter()
        .copied()
        .enumerate()
        .filter(|&(i, q)| {
            world.water.main_ocean_id != 0
                && world.water.body_ids[i] == world.water.main_ocean_id
                && q > 0.
        })
        .collect();
    mouths.sort_by(|a, b| b.1.total_cmp(&a.1));
    print!("  Top ocean mouths (region: m³/s):");
    for (i, q) in mouths.into_iter().take(10) {
        print!(" {i}: {q:.0}");
    }
    println!();
}
