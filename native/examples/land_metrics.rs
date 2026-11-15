//! Land metrics for one generated world: closed dry depressions, closed-sink drainage share
//! and land hypsometry above the initial water level.
//! Usage: land_metrics [SEED] [MODEL_VERSION]  (subdivision 6, default recipe parameters;
//! MODEL_VERSION defaults to multiscale-geography-3)
use planimulation_core::{Recipe, World};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

fn main() {
    let seed = std::env::args().nth(1).unwrap_or("first-light".into());
    let model = std::env::args()
        .nth(2)
        .unwrap_or("multiscale-geography-3".into());
    let recipe: Recipe = serde_json::from_str(&format!(r#"{{"schemaVersion":1,"modelVersion":"{model}","randomVersion":"fnv1a-utf8-mulberry32-1","seed":"{seed}","subdivision":6,"radiusMeters":6371000,"plateCount":12,"maxPlateSpeedCmPerYear":8,"continentalFraction":0.38,"continentalScale":1,"reliefScale":1,"boundaryWidthKm":300,"detailAmplitudeMeters":300,"terrainPreparationPasses":4,"water":{{"mode":"coverage","fraction":0.71}}}}"#)).unwrap();
    let w = World::generate(recipe).unwrap();
    let s = &w.surface;
    let h = &w.terrain.elevation;
    let n = h.len();
    let mut f = vec![f64::INFINITY; n];
    let mut heap = BinaryHeap::new();
    for i in 0..n {
        if w.water.body_ids[i] != 0 {
            f[i] = h[i].max(w.water.level_meters);
            heap.push(Reverse((ordered(f[i]), i)));
        }
    }
    while let Some(Reverse((_, c))) = heap.pop() {
        for k in s.offsets[c] as usize..s.offsets[c + 1] as usize {
            let j = s.neighbors[k] as usize;
            if f[j].is_finite() {
                continue;
            }
            f[j] = h[j].max(f[c]);
            heap.push(Reverse((ordered(f[j]), j)));
        }
    }
    let land: f64 = (0..n)
        .filter(|&i| w.water.body_ids[i] == 0)
        .map(|i| s.areas[i])
        .sum();
    let mut seen = vec![false; n];
    let mut comps = vec![];
    for i in 0..n {
        if seen[i] || w.water.body_ids[i] != 0 || f[i] - h[i] <= 1e-9 {
            continue;
        }
        let (mut stack, mut area, mut depth, mut cells) = (vec![i], 0., 0f64, 0);
        seen[i] = true;
        while let Some(c) = stack.pop() {
            area += s.areas[c];
            depth = depth.max(f[c] - h[c]);
            cells += 1;
            for k in s.offsets[c] as usize..s.offsets[c + 1] as usize {
                let j = s.neighbors[k] as usize;
                if !seen[j] && w.water.body_ids[j] == 0 && f[j] - h[j] > 1e-9 {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        comps.push((depth, area, cells));
    }
    let mut heights: Vec<(f64, f64)> = (0..n)
        .filter(|&i| w.water.body_ids[i] == 0)
        .map(|i| (h[i] - w.water.level_meters, s.areas[i]))
        .collect();
    heights.sort_by(|a, b| a.0.total_cmp(&b.0));
    let below = |limit: f64| {
        100. * heights
            .iter()
            .filter(|x| x.0 < limit)
            .map(|x| x.1)
            .sum::<f64>()
            / land
    };
    let quantile = |q: f64| {
        let mut acc = 0.;
        for x in &heights {
            acc += x.1;
            if acc >= q * land {
                return x.0;
            }
        }
        0.
    };
    let mean = heights.iter().map(|x| x.0 * x.1).sum::<f64>() / land;
    println!(
        "  land above sea: <200 m {:.0}%, <500 m {:.0}%, median {:.0} m, mean {:.0} m, p95 {:.0} m",
        below(200.),
        below(500.),
        quantile(0.5),
        mean,
        quantile(0.95)
    );
    comps.sort_by(|a, b| b.0.total_cmp(&a.0));
    let total: f64 = comps.iter().map(|c| c.1).sum();
    println!(
        "{seed}: {} depressions, flooded share of land {:.1}%, closed-sink share {:.1}%",
        comps.len(),
        100. * total / land,
        100. * w
            .drainage
            .contributing_area
            .iter()
            .enumerate()
            .filter(|(i, _)| w.drainage.receivers[*i] as usize == *i && w.water.body_ids[*i] == 0)
            .map(|(_, a)| a)
            .sum::<f64>()
            / land
    );
    for q in [0., 0.5, 0.9, 0.99] {
        let c = &comps[((comps.len() - 1) as f64 * q) as usize];
        println!(
            "  rank {q}: depth {:.0} m, area {:.0} km2, cells {}",
            c.0,
            c.1 / 1e6,
            c.2
        );
    }
    for d in [10., 50., 200., 500.] {
        let k = comps.iter().filter(|c| c.0 < d).count();
        println!("  depth<{d} m: {k}");
    }
}
fn ordered(v: f64) -> u64 {
    let b = v.to_bits();
    if b >> 63 == 1 { !b } else { b | 1 << 63 }
}
