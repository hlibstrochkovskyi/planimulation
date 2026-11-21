//! Report for wild-food-1 on the new-world recipe: forager capacity overall and by
//! vegetation class, plant and game shares, lean-season penalty, game response over the
//! run and timing. Usage: wild_food_report [YEARS] [SEED...]  (multiscale-geography-4, subdivision 6)
use planimulation_core::climate_normals::Normals;
use planimulation_core::vegetation::Model as Vegetation;
use planimulation_core::wild_food::{Model, Settings};
use planimulation_core::{Recipe, World};
use std::time::Instant;

const NAMES: [&str; 11] = [
    "water",
    "ice",
    "tundra",
    "boreal",
    "temperate forest",
    "grassland",
    "cold desert",
    "hot desert",
    "savanna",
    "tropical seasonal",
    "rainforest",
];

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let years: u32 = args.next().map_or(300, |y| y.parse().unwrap());
    let mut seeds: Vec<String> = args.collect();
    if seeds.is_empty() {
        seeds = vec!["first-light".into(), "5c151f46".into(), "4836187e".into()];
    }
    for seed in seeds {
        let recipe: Recipe = serde_json::from_str(&format!(r#"{{"schemaVersion":1,"modelVersion":"multiscale-geography-4","randomVersion":"fnv1a-utf8-mulberry32-1","seed":"{seed}","subdivision":6,"radiusMeters":6371000,"plateCount":12,"maxPlateSpeedCmPerYear":8,"continentalFraction":0.38,"continentalScale":1,"reliefScale":1,"boundaryWidthKm":300,"detailAmplitudeMeters":300,"terrainPreparationPasses":4,"water":{{"mode":"coverage","fraction":0.71}}}}"#)).unwrap();
        let w = World::generate(recipe).unwrap();
        let normals = Normals::from_world(&w).unwrap();
        let (vegetation, mut v) = Vegetation::new(&w, &normals, Default::default()).unwrap();
        let started = Instant::now();
        let (model, mut state) =
            Model::new(&w, &normals, &vegetation, &v, Settings::default()).unwrap();
        let setup = started.elapsed().as_secs_f64();
        let mut food_seconds = 0.;
        for _ in 0..years {
            vegetation.advance(&mut v, 1).unwrap();
            let started = Instant::now();
            model.step(&vegetation, &v, &mut state).unwrap();
            food_seconds += started.elapsed().as_secs_f64();
        }
        let regions = model.regions(&vegetation, &v, &state);
        let land: Vec<usize> = (0..regions.len())
            .filter(|&i| w.water.body_ids[i] == 0)
            .collect();
        let area = |set: &[usize]| set.iter().map(|&i| w.surface.areas[i]).sum::<f64>() / 1e6;
        let ice_free: Vec<usize> = land.iter().copied().filter(|&i| v.label[i] != 1).collect();
        let people: f64 = land
            .iter()
            .map(|&i| regions[i].capacity * w.surface.areas[i] / 1e6)
            .sum();
        let stored: f64 = land
            .iter()
            .map(|&i| regions[i].stored_capacity * w.surface.areas[i] / 1e6)
            .sum();
        let summaries = &state.summaries;
        let first = summaries[0];
        let last = *summaries.last().unwrap();
        let (low, high) = summaries.iter().fold((f64::INFINITY, 0f64), |(a, b), s| {
            (a.min(s.capacity_people), b.max(s.capacity_people))
        });
        let game_low = summaries
            .iter()
            .map(|s| s.game_ratio)
            .fold(f64::INFINITY, f64::min);
        println!("== {seed}: {years} years, {} land regions", land.len());
        println!(
            "capacity: {:.2} M people ({:.2} M with perfect storage); {:.4}/km² over {:.1} M km² ice-free land",
            people / 1e6,
            stored / 1e6,
            people / area(&ice_free),
            area(&ice_free) / 1e6
        );
        println!(
            "capacity over the run: year 0 {:.2} M, min {:.2} M, max {:.2} M, end {:.2} M",
            first.capacity_people / 1e6,
            low / 1e6,
            high / 1e6,
            last.capacity_people / 1e6
        );
        println!(
            "food: plant {:.1}% of energy; game ratio end {:.3}, minimum {:.3}",
            100. * last.plant_kcal / (last.plant_kcal + last.game_kcal),
            last.game_ratio,
            game_low
        );
        println!("class: regions, median people/100 km², median lean/stored ratio, plant share");
        for (label, name) in NAMES.iter().enumerate().skip(1) {
            let set: Vec<usize> = land
                .iter()
                .copied()
                .filter(|&i| v.label[i] as usize == label)
                .collect();
            if set.is_empty() {
                continue;
            }
            let density = median(set.iter().map(|&i| 100. * regions[i].capacity).collect());
            let ratio = median(
                set.iter()
                    .filter(|&&i| regions[i].stored_capacity > 0.)
                    .map(|&i| regions[i].capacity / regions[i].stored_capacity)
                    .collect(),
            );
            let plant = median(
                set.iter()
                    .filter(|&&i| regions[i].plant_kcal + regions[i].game_kcal > 0.)
                    .map(|&i| {
                        regions[i].plant_kcal / (regions[i].plant_kcal + regions[i].game_kcal)
                    })
                    .collect(),
            );
            println!(
                "  {name:>17}: {:5} {:8.3} {:6.3} {:6.3}",
                set.len(),
                density,
                ratio,
                plant
            );
        }
        println!(
            "timing: setup {:.3} s, food step {:.2} ms/year",
            setup,
            1e3 * food_seconds / f64::from(years)
        );
    }
}
