use planimulation_core::climate_normals::Normals;
use planimulation_core::vegetation::{Model, RegionYear, Settings, State};
use planimulation_core::{Recipe, World};
use std::time::Instant;

fn fnv_update(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= u64::from(b);
        *h = h.wrapping_mul(0x100000001b3);
    }
}

fn hash_f64_slice(h: &mut u64, s: &[f64]) {
    for &x in s {
        fnv_update(h, &x.to_bits().to_le_bytes());
    }
}

fn hash_state(state: &State) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    fnv_update(&mut h, &state.year.to_le_bytes());
    fnv_update(&mut h, &state.random.to_le_bytes());
    hash_f64_slice(&mut h, &state.amplitudes);
    hash_f64_slice(&mut h, &state.amplitude_history);
    hash_f64_slice(&mut h, &state.woody);
    hash_f64_slice(&mut h, &state.herb);
    fnv_update(&mut h, &state.label);
    hash_f64_slice(&mut h, &state.precipitation_factor);
    hash_f64_slice(&mut h, &state.root_moisture);
    hash_f64_slice(&mut h, &state.established_cover);
    hash_f64_slice(&mut h, &state.woody_history);
    hash_f64_slice(&mut h, &state.woody_change);
    for s in &state.summaries {
        fnv_update(&mut h, &s.precipitation_factor.to_bits().to_le_bytes());
        fnv_update(&mut h, &s.woody_cover.to_bits().to_le_bytes());
        fnv_update(&mut h, &s.total_cover.to_bits().to_le_bytes());
        fnv_update(&mut h, &s.forest_share.to_bits().to_le_bytes());
        fnv_update(&mut h, &s.label_changes.to_le_bytes());
        fnv_update(&mut h, &s.productivity_petagrams.to_bits().to_le_bytes());
    }
    h
}

fn hash_series(series: &[RegionYear]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for r in series {
        fnv_update(&mut h, &r.precipitation_factor.to_bits().to_le_bytes());
        fnv_update(&mut h, &r.root_moisture.to_bits().to_le_bytes());
        fnv_update(&mut h, &r.established_cover.to_bits().to_le_bytes());
        fnv_update(&mut h, &r.woody.to_bits().to_le_bytes());
        fnv_update(&mut h, &r.herb.to_bits().to_le_bytes());
        fnv_update(&mut h, &r.woody_capacity.to_bits().to_le_bytes());
        fnv_update(&mut h, &r.cover_capacity.to_bits().to_le_bytes());
        fnv_update(&mut h, &r.productivity.to_bits().to_le_bytes());
        fnv_update(&mut h, &[r.label]);
    }
    h
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let seed = std::env::args().nth(1).unwrap_or_else(|| "first-light".into());
    let repeats: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(5);
    let threads: Option<usize> = std::env::args().nth(3).and_then(|s| s.parse().ok());
    let recipe: Recipe = serde_json::from_str(&format!(
        r#"{{"schemaVersion":1,"modelVersion":"multiscale-geography-4","randomVersion":"fnv1a-utf8-mulberry32-1","seed":"{seed}","subdivision":6,"radiusMeters":6371000,"plateCount":12,"maxPlateSpeedCmPerYear":8,"continentalFraction":0.38,"continentalScale":1,"reliefScale":1,"boundaryWidthKm":300,"detailAmplitudeMeters":300,"terrainPreparationPasses":4,"water":{{"mode":"coverage","fraction":0.71}}}}"#
    ))
    .unwrap();
    let w = World::generate(recipe).unwrap();
    let normals = Normals::from_world(&w).unwrap();
    let land_region = (0..w.surface.centers.len())
        .find(|&i| w.water.body_ids[i] == 0 && normals.biome[i] >= 3)
        .unwrap();
    let water_region = (0..w.surface.centers.len())
        .find(|&i| w.water.body_ids[i] != 0)
        .unwrap();
    println!("seed={seed} regions={} land_region={land_region} water_region={water_region} repeats={repeats} threads={:?}", w.surface.centers.len(), threads);

    let mut setup_times = Vec::new();
    let mut advance_times = Vec::new();
    let mut series_times = Vec::new();
    let mut h0 = 0;
    let mut h1000 = 0;
    let mut h_land = 0;
    let mut h_water = 0;
    for r in 0..repeats {
        let t0 = Instant::now();
        let (model, mut state) = match threads {
            Some(t) => Model::new_with_threads(&w, &normals, Settings::default(), t).unwrap(),
            None => Model::new(&w, &normals, Settings::default()).unwrap(),
        };
        let dt_setup = t0.elapsed().as_secs_f64();
        if r == 0 {
            h0 = hash_state(&state);
        }
        let t1 = Instant::now();
        model.advance(&mut state, 1000).unwrap();
        let dt_adv = t1.elapsed().as_secs_f64();
        if r == 0 {
            h1000 = hash_state(&state);
        }
        let t2 = Instant::now();
        let s_land = model.region_series(&state, land_region).unwrap();
        let dt_ser = t2.elapsed().as_secs_f64();
        if r == 0 {
            h_land = hash_series(&s_land);
            let s_water = model.region_series(&state, water_region).unwrap();
            h_water = hash_series(&s_water);
        }
        setup_times.push(dt_setup);
        advance_times.push(dt_adv);
        series_times.push(dt_ser);
        println!(
            "  run {}: setup={:.4}s advance1000={:.4}s series1000={:.4}s",
            r + 1,
            dt_setup,
            dt_adv,
            dt_ser
        );
    }
    println!(
        "MEDIAN: setup={:.4}s advance1000={:.4}s series1000={:.4}s",
        median(setup_times),
        median(advance_times),
        median(series_times)
    );
    println!("HASHES: year0={h0:016x} year1000={h1000:016x} series_land={h_land:016x} series_water={h_water:016x}");
}
