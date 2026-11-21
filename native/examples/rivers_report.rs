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
        "{seed} | P {:.1} | AET {:.1} | runoff {:.1} | land storage {:.1} mm/year | ocean {:.0} km³/year | land runoff to ocean {:.0} km³/year ({:.1}%) | endorheic land {:.3}% | {:.1} ms",
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
        "  partition: runoff/P {:.1}%, AET/P {:.1}%",
        100. * t.land_runoff / t.land_precipitation,
        100. * t.land_aet / t.land_precipitation,
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
    for cap in [0.05, 0.10, 0.20] {
        let alternate;
        let frame = if cap == rivers.settings.dry_lake_area_fraction {
            &rivers
        } else {
            alternate = Rivers::with_dry_lake_area_fraction(&world, &climate, cap).unwrap();
            &alternate
        };
        println!(
            "  {:.0}% cap | {:.3}% endorheic land | {} closed | {:.0} km³/year ocean",
            cap * 100.,
            frame.totals.endorheic_area / t.land_area * 100.,
            frame.lakes.iter().filter(|l| l.spill == 0.).count(),
            km3(frame.totals.ocean_discharge),
        );
    }
    report_budyko(&world, &climate, &rivers);
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

// Weight ratios and curves by each region's precipitation volume. This keeps
// the aggregate in each aridity band consistent with its annual water budget.
fn report_budyko(world: &World, climate: &Normals, rivers: &Rivers) {
    let bounds = [0.5, 0.8, 1.2, 2., 3., f64::INFINITY];
    let mut bands = [[0.; 7]; 6];
    let mut outside = 0.;
    for (i, &area) in world.surface.areas.iter().enumerate() {
        if world.water.body_ids[i] != 0 {
            continue;
        }
        let p = climate.annual_precipitation_mm[i];
        if p <= 0. || rivers.storage_mm[i] > 1e-6 {
            outside += area;
            continue;
        }
        let phi = rivers.pet_mm[i] / p;
        let band = bounds.iter().position(|&upper| phi < upper).unwrap();
        let volume = p * area;
        let fu = |omega: f64| 1. + phi - (1. + phi.powf(omega)).powf(1. / omega);
        for (sum, value) in bands[band].iter_mut().zip([
            area,
            volume,
            area * rivers.pet_mm[i],
            area * rivers.aet_mm[i],
            volume * fu(2.),
            volume * fu(2.6),
            volume * fu(4.),
        ]) {
            *sum += value;
        }
    }
    let mut aggregate = [0.; 7];
    for band in &bands {
        for (total, value) in aggregate.iter_mut().zip(band) {
            *total += value;
        }
    }
    println!("  Budyko: area %, PET/P, AET/P, Fu(2), Fu(2.6), Fu(4)");
    let labels = ["<0.5", "0.5–0.8", "0.8–1.2", "1.2–2", "2–3", "≥3", "all"];
    for (label, [area, p, pet, aet, lo, mid, hi]) in
        labels.into_iter().zip(bands.into_iter().chain([aggregate]))
    {
        if p > 0. {
            println!(
                "  {label} | {:.1}% | {:.3} | {:.3} | {:.3} | {:.3} | {:.3}",
                100. * area / rivers.totals.land_area,
                pet / p,
                aet / p,
                lo / p,
                mid / p,
                hi / p,
            );
        }
    }
    println!(
        "  Budyko excluded {:.1}% of land",
        100. * outside / rivers.totals.land_area
    );
}
