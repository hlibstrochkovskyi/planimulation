//! Biome shares and land precipitation for climate-normals-1 on the new-world recipe.
//! Usage: climate_normals_report [SEED]  (multiscale-geography-4, subdivision 6)
use planimulation_core::climate_normals::Normals;
use planimulation_core::{Recipe, World};
use std::time::Instant;

const NAMES: [&str; 11] = [
    "water",
    "ice",
    "tundra",
    "boreal forest",
    "temperate forest",
    "temperate grassland",
    "cold desert",
    "hot desert",
    "savanna",
    "tropical seasonal forest",
    "tropical rainforest",
];

fn main() {
    let seed = std::env::args().nth(1).unwrap_or("first-light".into());
    let recipe: Recipe = serde_json::from_str(&format!(r#"{{"schemaVersion":1,"modelVersion":"multiscale-geography-4","randomVersion":"fnv1a-utf8-mulberry32-1","seed":"{seed}","subdivision":6,"radiusMeters":6371000,"plateCount":12,"maxPlateSpeedCmPerYear":8,"continentalFraction":0.38,"continentalScale":1,"reliefScale":1,"boundaryWidthKm":300,"detailAmplitudeMeters":300,"terrainPreparationPasses":4,"water":{{"mode":"coverage","fraction":0.71}}}}"#)).unwrap();
    let w = World::generate(recipe).unwrap();
    let started = Instant::now();
    let c = Normals::from_world(&w).unwrap();
    let elapsed = started.elapsed().as_secs_f64();
    let s = &w.surface;
    let land: Vec<usize> = (0..s.centers.len())
        .filter(|&i| w.water.body_ids[i] == 0)
        .collect();
    let land_area: f64 = land.iter().map(|&i| s.areas[i]).sum();
    let mut p: Vec<f64> = land.iter().map(|&i| c.annual_precipitation_mm[i]).collect();
    p.sort_by(f64::total_cmp);
    let mean = land
        .iter()
        .map(|&i| c.annual_precipitation_mm[i] * s.areas[i])
        .sum::<f64>()
        / land_area;
    let all = (0..s.centers.len())
        .map(|i| c.annual_precipitation_mm[i] * s.areas[i])
        .sum::<f64>()
        / s.areas.iter().sum::<f64>();
    println!(
        "{seed}: {elapsed:.2} s; precipitation mm/yr global mean {all:.0}, land mean {mean:.0}, land p10 {:.0} median {:.0} p90 {:.0}",
        p[p.len() / 10],
        p[p.len() / 2],
        p[p.len() * 9 / 10]
    );
    let mut t: Vec<f64> = land.iter().map(|&i| c.annual_mean_celsius[i]).collect();
    t.sort_by(f64::total_cmp);
    println!(
        "  land annual mean °C p5 {:.1} median {:.1} p95 {:.1}",
        t[t.len() / 20],
        t[t.len() / 2],
        t[t.len() * 19 / 20]
    );
    for (b, name) in NAMES.iter().enumerate().skip(1) {
        let a: f64 = land
            .iter()
            .filter(|&&i| c.biome[i] as usize == b)
            .map(|&i| s.areas[i])
            .sum();
        println!("  {name:<26} {:>5.1}%", 100. * a / land_area);
    }
}
