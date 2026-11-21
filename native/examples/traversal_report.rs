//! Measurements report for traversal-1 on the default new-world recipe
//! (`multiscale-geography-4`, subdivision 6, 40,962 regions): speeds by terrain class,
//! widest-landmass continental crossing times (summer and winter), frozen lake/river
//! counts, and Dijkstra timings.
//! Usage: cargo run --release --example traversal_report -- [SEED...]
use planimulation_core::climate_normals::{Biome, Normals};
use planimulation_core::traversal::{
    FLAG_FROZEN_LAKE_WINTER, FLAG_FROZEN_RIVER_WINTER, Model, Season, Settings,
};
use planimulation_core::vegetation::Model as VegetationModel;
use planimulation_core::{Recipe, World};
use std::time::Instant;

const BIOME_NAMES: [&str; 11] = [
    "water",
    "ice",
    "tundra",
    "boreal forest",
    "temperate forest",
    "temperate grassland",
    "cold desert",
    "hot desert",
    "savanna",
    "tropical seasonal",
    "tropical rainforest",
];

fn main() {
    let mut seeds: Vec<String> = std::env::args().skip(1).collect();
    if seeds.is_empty() {
        seeds = vec!["first-light".into(), "5c151f46".into(), "4836187e".into()];
    }
    let settings = Settings::default();
    for seed in seeds {
        let recipe: Recipe = serde_json::from_str(&format!(
            r#"{{"schemaVersion":1,"modelVersion":"multiscale-geography-4","randomVersion":"fnv1a-utf8-mulberry32-1","seed":"{seed}","subdivision":6,"radiusMeters":6371000,"plateCount":12,"maxPlateSpeedCmPerYear":8,"continentalFraction":0.38,"continentalScale":1,"reliefScale":1,"boundaryWidthKm":300,"detailAmplitudeMeters":300,"terrainPreparationPasses":4,"water":{{"mode":"coverage","fraction":0.71}}}}"#
        ))
        .unwrap();
        let w = World::generate(recipe).unwrap();
        let normals = Normals::from_world(&w).unwrap();
        let (_, veg_state) = VegetationModel::new(&w, &normals, Default::default()).unwrap();

        let t0 = Instant::now();
        let model = Model::new(&w, &normals, Some(&veg_state), settings).unwrap();
        let setup_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let n = model.region_count();
        // Find largest connected dry landmass (summer passable component).
        let mut comp = vec![usize::MAX; n];
        let mut best_comp = Vec::new();
        for start in 0..n {
            if !model.passable_summer[start] || comp[start] != usize::MAX {
                continue;
            }
            let mut stack = vec![start];
            let mut members = Vec::new();
            comp[start] = start;
            while let Some(u) = stack.pop() {
                members.push(u);
                for k in w.surface.offsets[u] as usize..w.surface.offsets[u + 1] as usize {
                    let v = w.surface.neighbors[k] as usize;
                    if model.passable_summer[v] && comp[v] == usize::MAX {
                        comp[v] = start;
                        stack.push(v);
                    }
                }
            }
            if members.len() > best_comp.len() {
                best_comp = members;
            }
        }

        // Widest great-circle span of the largest landmass (exact over boundary/coastline
        // or sampled if huge, plus exact check on all land pairs via 2-sweep + full boundary).
        let boundary: Vec<usize> = best_comp
            .iter()
            .copied()
            .filter(|&u| {
                (w.surface.offsets[u] as usize..w.surface.offsets[u + 1] as usize)
                    .any(|k| !model.passable_summer[w.surface.neighbors[k] as usize])
            })
            .collect();
        let candidates = if boundary.len() >= 2 {
            &boundary
        } else {
            &best_comp
        };
        let mut best_dot = 2.0f64;
        let (mut end_a, mut end_b) = (best_comp[0], best_comp[0]);
        for (idx, &a) in candidates.iter().enumerate() {
            let ca = w.surface.centers[a];
            for &b in &candidates[idx + 1..] {
                let cb = w.surface.centers[b];
                let d = ca[0] * cb[0] + ca[1] * cb[1] + ca[2] * cb[2];
                if d < best_dot {
                    best_dot = d;
                    end_a = a;
                    end_b = b;
                }
            }
        }
        let t_d0 = Instant::now();
        let summer_from_a = model.travel_time(Season::Summer, end_a).unwrap();
        let dijkstra_ms = t_d0.elapsed().as_secs_f64() * 1000.0;
        let t_dw = Instant::now();
        let winter_from_a = model.travel_time(Season::Winter, end_a).unwrap();
        let winter_dijkstra_ms = t_dw.elapsed().as_secs_f64() * 1000.0;

        let chord_rad = (w.surface.centers[end_a][0] * w.surface.centers[end_b][0]
            + w.surface.centers[end_a][1] * w.surface.centers[end_b][1]
            + w.surface.centers[end_a][2] * w.surface.centers[end_b][2])
            .clamp(-1.0, 1.0)
            .acos();
        let great_circle_km = chord_rad * w.recipe.radius_meters / 1000.0;
        let summer_days = summer_from_a.days[end_b];
        let summer_km = summer_from_a.distance_meters[end_b] / 1000.0;
        let winter_days = winter_from_a.days[end_b];
        let winter_km = winter_from_a.distance_meters[end_b] / 1000.0;
        let area_km2: f64 = best_comp.iter().map(|&i| w.surface.areas[i] / 1e6).sum();
        let frozen_lakes = model
            .flags
            .iter()
            .filter(|&&f| (f & FLAG_FROZEN_LAKE_WINTER) != 0)
            .count();
        let frozen_rivers = model
            .flags
            .iter()
            .filter(|&&f| (f & FLAG_FROZEN_RIVER_WINTER) != 0)
            .count();

        println!(
            "=== {seed} === setup {setup_ms:.1} ms | Dijkstra summer {dijkstra_ms:.1} ms, winter {winter_dijkstra_ms:.1} ms"
        );
        println!(
            "  largest landmass: {} regions ({:.2} M km²) | frozen winter lakes: {frozen_lakes}, frozen river regions: {frozen_rivers}",
            best_comp.len(),
            area_km2 / 1e6
        );
        println!("  end-to-end ({end_a} -> {end_b}, great-circle {great_circle_km:.0} km):");
        println!(
            "    summer: {summer_days:.1} days, route {summer_km:.0} km ({:.1} km/day, {:.2} km/h)",
            summer_km / summer_days,
            summer_km / (summer_days * settings.hours_per_day)
        );
        println!(
            "    winter: {winter_days:.1} days, route {winter_km:.0} km ({:.1} km/day, {:.2} km/h)",
            winter_km / winter_days,
            winter_km / (winter_days * settings.hours_per_day)
        );

        // Speeds by terrain/biome class (using vegetation-2 year-0 labels).
        println!("  speeds by class (median h/100 km -> km/day, summer / winter):");
        for b in 1..=Biome::TropicalRainforest as u8 {
            let mut s_vals: Vec<f64> = (0..n)
                .filter(|&i| model.passable_summer[i] && veg_state.label[i] == b)
                .map(|i| model.hours_per_100km_summer[i])
                .collect();
            let mut w_vals: Vec<f64> = (0..n)
                .filter(|&i| model.passable_summer[i] && veg_state.label[i] == b)
                .map(|i| model.hours_per_100km_winter[i])
                .collect();
            if s_vals.is_empty() {
                continue;
            }
            s_vals.sort_by(f64::total_cmp);
            w_vals.sort_by(f64::total_cmp);
            let sm = s_vals[s_vals.len() / 2];
            let wm = w_vals[w_vals.len() / 2];
            println!(
                "    {:<20} (n={:4})  summer {:5.1} h/100km ({:4.1} km/d) | winter {:5.1} h/100km ({:4.1} km/d)",
                BIOME_NAMES[b as usize],
                s_vals.len(),
                sm,
                100.0 * settings.hours_per_day / sm,
                wm,
                100.0 * settings.hours_per_day / wm
            );
        }
        // Relief buckets (flat <100 m, hills 100-600 m, mountains >600 m).
        for (label, low, high) in [
            ("flat (<100 m relief)", 0.0, 100.0),
            ("hills (100-600 m)", 100.0, 600.0),
            ("mountains (>600 m)", 600.0, f64::INFINITY),
        ] {
            let mut s_vals: Vec<f64> = (0..n)
                .filter(|&i| {
                    model.passable_summer[i]
                        && model.local_relief_meters[i] >= low
                        && model.local_relief_meters[i] < high
                })
                .map(|i| model.hours_per_100km_summer[i])
                .collect();
            if s_vals.is_empty() {
                continue;
            }
            s_vals.sort_by(f64::total_cmp);
            let sm = s_vals[s_vals.len() / 2];
            println!(
                "    {:<20} (n={:4})  summer {:5.1} h/100km ({:4.1} km/d)",
                label,
                s_vals.len(),
                sm,
                100.0 * settings.hours_per_day / sm
            );
        }
    }
}
