//! Annual climate summary from existing models on the new-world recipe: seasonal temperature
//! normals and precipitation accumulated by the baseline seasonal-moisture model over two years.
//! Usage: climate_year [SEED] [YEARS]  (multiscale-geography-4, subdivision 6)
use planimulation_core::seasonal_moisture::Model;
use planimulation_core::seasonal_temperature::Normals;
use planimulation_core::{Recipe, World};
use std::time::Instant;

fn main() {
    let seed = std::env::args().nth(1).unwrap_or("first-light".into());
    let years: usize = std::env::args().nth(2).map_or(2, |v| v.parse().unwrap());
    let recipe: Recipe = serde_json::from_str(&format!(r#"{{"schemaVersion":1,"modelVersion":"multiscale-geography-4","randomVersion":"fnv1a-utf8-mulberry32-1","seed":"{seed}","subdivision":6,"radiusMeters":6371000,"plateCount":12,"maxPlateSpeedCmPerYear":8,"continentalFraction":0.38,"continentalScale":1,"reliefScale":1,"boundaryWidthKm":300,"detailAmplitudeMeters":300,"terrainPreparationPasses":4,"water":{{"mode":"coverage","fraction":0.71}}}}"#)).unwrap();
    let w = World::generate(recipe).unwrap();
    let n = w.surface.centers.len();
    let land: Vec<usize> = (0..n).filter(|&i| w.water.body_ids[i] == 0).collect();
    let normals = Normals::from_world(&w, Default::default()).unwrap();
    let model = Model::from_world(
        &w,
        Default::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut state = model.initial_state();
    let mut previous = vec![0.; n];
    let mut annual = vec![];
    for year in 0..years {
        let started = Instant::now();
        for _ in 0..365 {
            model.advance(&mut state, 86400).unwrap();
        }
        let checkpoint = state.checkpoint();
        let area: f64 = w.surface.areas.iter().sum();
        let land_area: f64 = land.iter().map(|&i| w.surface.areas[i]).sum();
        let sum = |v: &[f64], ids: &mut dyn Iterator<Item = usize>| ids.map(|i| v[i]).sum::<f64>();
        let e = &checkpoint.cumulative_evaporation_kilograms;
        let p = &checkpoint.cumulative_precipitation_kilograms;
        println!(
            "  cumulative to date, mm global / land / ocean: evaporation {:.0} / {:.0} / {:.0}, precipitation {:.0} / {:.0} / {:.0}; vapor now {:.1} mm global",
            sum(e, &mut (0..n)) / area,
            sum(e, &mut land.iter().copied()) / land_area,
            (sum(e, &mut (0..n)) - sum(e, &mut land.iter().copied())) / (area - land_area),
            sum(p, &mut (0..n)) / area,
            sum(p, &mut land.iter().copied()) / land_area,
            (sum(p, &mut (0..n)) - sum(p, &mut land.iter().copied())) / (area - land_area),
            checkpoint.vapor_kilograms.iter().sum::<f64>() / area
        );
        let total = checkpoint.cumulative_precipitation_kilograms;
        // kg over the region area → mm of liquid water.
        let mm: Vec<f64> = (0..n)
            .map(|i| (total[i] - previous[i]) / w.surface.areas[i])
            .collect();
        previous = total;
        let mut sorted: Vec<f64> = land.iter().map(|&i| mm[i]).collect();
        sorted.sort_by(f64::total_cmp);
        let q = |p: f64| sorted[((sorted.len() - 1) as f64 * p) as usize];
        let below = |limit: f64| {
            100. * sorted.iter().filter(|&&v| v < limit).count() as f64 / sorted.len() as f64
        };
        println!(
            "year {} ({:.0} s): land precipitation mm/yr p10 {:.0}, median {:.0}, p90 {:.0}, max {:.0}; land <250 mm {:.0}%, <500 mm {:.0}%, >1500 mm {:.0}%",
            year + 1,
            started.elapsed().as_secs_f64(),
            q(0.1),
            q(0.5),
            q(0.9),
            sorted[sorted.len() - 1],
            below(250.),
            below(500.),
            100. - below(1500.)
        );
        annual.push(mm);
    }
    println!(
        "latitude band: land share, mean annual temperature °C, median land precipitation mm (last year)"
    );
    let last = annual.last().unwrap();
    for band in 0..9 {
        let (lo, hi) = (-90. + band as f64 * 20., -70. + band as f64 * 20.);
        let mut values: Vec<(f64, f64)> = land
            .iter()
            .filter(|&&i| {
                let lat = w.surface.centers[i][1].asin().to_degrees();
                lat >= lo && lat < hi
            })
            .map(|&i| (normals.annual_mean_celsius[i], last[i]))
            .collect();
        if values.is_empty() {
            continue;
        }
        let t = values.iter().map(|v| v.0).sum::<f64>() / values.len() as f64;
        values.sort_by(|a, b| a.1.total_cmp(&b.1));
        println!(
            "  {lo:>4}..{hi:<4} {:>5.1}%  {t:>5.1}  {:>6.0}",
            100. * values.len() as f64 / land.len() as f64,
            values[values.len() / 2].1
        );
    }
}
