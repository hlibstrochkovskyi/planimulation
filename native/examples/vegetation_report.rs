//! Ensemble report for vegetation-2 on the new-world recipe: class shares at year 0 and
//! after the run, label changes and their most common transitions, drought marks and
//! timing. Usage: vegetation_report [YEARS] [SEED...]  (multiscale-geography-4, subdivision 6)
use planimulation_core::climate_normals::Normals;
use planimulation_core::vegetation::{Model, Settings};
use planimulation_core::{Recipe, World};
use std::collections::BTreeMap;
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

fn main() {
    let mut args = std::env::args().skip(1);
    let years: u32 = args.next().map_or(200, |y| y.parse().unwrap());
    let mut seeds: Vec<String> = args.collect();
    if seeds.is_empty() {
        seeds = vec!["first-light".into(), "5c151f46".into(), "4836187e".into()];
    }
    for seed in seeds {
        let recipe: Recipe = serde_json::from_str(&format!(r#"{{"schemaVersion":1,"modelVersion":"multiscale-geography-4","randomVersion":"fnv1a-utf8-mulberry32-1","seed":"{seed}","subdivision":6,"radiusMeters":6371000,"plateCount":12,"maxPlateSpeedCmPerYear":8,"continentalFraction":0.38,"continentalScale":1,"reliefScale":1,"boundaryWidthKm":300,"detailAmplitudeMeters":300,"terrainPreparationPasses":4,"water":{{"mode":"coverage","fraction":0.71}}}}"#)).unwrap();
        let w = World::generate(recipe).unwrap();
        let normals = Normals::from_world(&w).unwrap();
        let started = Instant::now();
        let length = std::env::var("ANOMALY_KM").map_or(1000., |v| v.parse().unwrap());
        let settings = Settings {
            anomaly_length_meters: length * 1000.,
            ..Settings::default()
        };
        let (model, mut state) = Model::new(&w, &normals, settings).unwrap();
        let setup = started.elapsed().as_secs_f64();
        let initial = state.clone();
        let mut transitions: BTreeMap<(u8, u8), u32> = BTreeMap::new();
        let mut changes = vec![0u32; state.label.len()];
        let started = Instant::now();
        for _ in 0..years {
            let before = state.label.clone();
            model.advance(&mut state, 1).unwrap();
            for i in 0..before.len() {
                if before[i] != state.label[i] {
                    *transitions.entry((before[i], state.label[i])).or_default() += 1;
                    changes[i] += 1;
                }
            }
        }
        let run = started.elapsed().as_secs_f64();
        let land: Vec<usize> = (0..state.label.len())
            .filter(|&i| w.water.body_ids[i] == 0)
            .collect();
        let area = |i: usize| w.surface.areas[i];
        let land_area: f64 = land.iter().map(|&i| area(i)).sum();
        println!("{seed}: setup {setup:.2} s, {years} years in {run:.2} s");
        for b in 1..NAMES.len() as u8 {
            let share = |labels: &[u8]| {
                100. * land
                    .iter()
                    .filter(|&&i| labels[i] == b)
                    .map(|&i| area(i))
                    .sum::<f64>()
                    / land_area
            };
            println!(
                "  {:<18} {:5.1}% → {:5.1}%",
                NAMES[b as usize],
                share(&initial.label),
                share(&state.label)
            );
        }
        let total: u32 = transitions.values().sum();
        println!(
            "  label changes per land region per year {:.4}; regions never changed {:.1}%; changed >10 times {:.1}%",
            f64::from(total) / f64::from(years) / land.len() as f64,
            100. * land.iter().filter(|&&i| changes[i] == 0).count() as f64 / land.len() as f64,
            100. * land.iter().filter(|&&i| changes[i] > 10).count() as f64 / land.len() as f64,
        );
        let mut top: Vec<_> = transitions.into_iter().collect();
        top.sort_by_key(|&(_, c)| std::cmp::Reverse(c));
        for ((a, b), c) in top.into_iter().take(6) {
            println!(
                "    {:>18} → {:<18} {:5.1}%",
                NAMES[a as usize],
                NAMES[b as usize],
                100. * f64::from(c) / f64::from(total.max(1))
            );
        }
        let s = &state.summaries;
        let (dry, wet) = s.iter().enumerate().fold((0, 0), |(d, w), (y, x)| {
            (
                if x.precipitation_factor < s[d].precipitation_factor {
                    y
                } else {
                    d
                },
                if x.precipitation_factor > s[w].precipitation_factor {
                    y
                } else {
                    w
                },
            )
        });
        println!(
            "  land precipitation factor driest year {} {:.3}, wettest year {} {:.3}; forest share {:.1}% at year 0 → min {:.1}% → {:.1}% now",
            dry + 1,
            s[dry].precipitation_factor,
            wet + 1,
            s[wet].precipitation_factor,
            100. * land
                .iter()
                .filter(|&&i| (3..=4).contains(&initial.label[i]) || initial.label[i] >= 9)
                .map(|&i| area(i))
                .sum::<f64>()
                / land_area,
            100. * s.iter().map(|x| x.forest_share).fold(1., f64::min),
            100. * s.last().unwrap().forest_share,
        );
        let productivity: Vec<f64> = s.iter().map(|x| x.productivity_petagrams).collect();
        println!(
            "  land productivity {:.1} Pg dry matter/year at year 1, range {:.1}–{:.1}",
            productivity[0],
            productivity.iter().copied().fold(f64::INFINITY, f64::min),
            productivity.iter().copied().fold(0., f64::max),
        );
        let stats = |v: &[f64]| {
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
            (mean, sd)
        };
        let factors: Vec<f64> = s.iter().map(|x| x.precipitation_factor).collect();
        let forest: Vec<f64> = s.iter().map(|x| x.forest_share).collect();
        let half = forest.len() / 2;
        println!(
            "  global land factor sd {:.3}; forest share mean {:.1}% first half, {:.1}% second half, sd {:.1} pp",
            stats(&factors).1,
            100. * stats(&forest[..half]).0,
            100. * stats(&forest[half..]).0,
            100. * stats(&forest).1,
        );
        let mut regional: Vec<f64> = land
            .iter()
            .map(|&i| state.precipitation_factor[i])
            .collect();
        regional.sort_by(f64::total_cmp);
        println!(
            "  last-year regional factor p5 {:.2} median {:.2} p95 {:.2}",
            regional[regional.len() / 20],
            regional[regional.len() / 2],
            regional[regional.len() * 19 / 20]
        );
    }
}
