//! Version-5 landscape diagnostics: sediment budget, land hypsometry, relief,
//! dry basins, and main-stem geometry of the largest rivers.
//! Usage: eroded_landscape_report [SEED] [SUBDIVISION] [MODEL_VERSION]
use planimulation_core::{Recipe, World, water::Water};

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().unwrap_or("first-light".into());
    let subdivision: u32 = args.next().map_or(6, |v| v.parse().unwrap());
    let model = args.next().unwrap_or("multiscale-geography-5".into());
    let recipe: Recipe = serde_json::from_str(&format!(r#"{{"schemaVersion":1,"modelVersion":"{model}","randomVersion":"fnv1a-utf8-mulberry32-1","seed":"{seed}","subdivision":{subdivision},"radiusMeters":6371000,"plateCount":12,"maxPlateSpeedCmPerYear":8,"continentalFraction":0.38,"continentalScale":1,"reliefScale":1,"boundaryWidthKm":300,"detailAmplitudeMeters":300,"terrainPreparationPasses":4,"water":{{"mode":"coverage","fraction":0.71}}}}"#)).unwrap();
    let start = std::time::Instant::now();
    let w = World::generate(recipe).unwrap();
    let elapsed = start.elapsed().as_secs_f64();
    let (s, t) = (&w.surface, &w.terrain);
    let n = t.elevation.len();
    let wet = &w.water.body_ids;
    let level = w.water.level_meters;
    let range = |v: &[f64]| {
        v.iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| {
                (a.min(x), b.max(x))
            })
    };
    println!("{model} s{subdivision} {seed}: generated in {elapsed:.2} s, level {level:.1} m");
    for (name, v) in [
        ("preparation", &t.preparation),
        ("lowland", &t.lowland),
        ("seafloor", &t.seafloor),
        ("erosion", &t.erosion),
        ("sediment", &t.sediment),
    ] {
        if v.is_empty() {
            continue;
        }
        let (a, b) = range(v);
        println!("  {name:12} {a:10.1} .. {b:10.1} m");
    }
    if let Some(b) = &w.sediment_budget {
        let deposited = b.deposited();
        let share = |v: f64| {
            if deposited > 0. {
                100. * v / deposited
            } else {
                0.
            }
        };
        println!(
            "  sediment (km³): supplied {:.0}, deposited {:.0}, marine bypass {:.0}, basin remainder {:.0}; closing error {:.3e} m³ ({:.3e} relative)",
            b.supplied / 1e9,
            deposited / 1e9,
            b.marine_unplaced / 1e9,
            b.basin_unplaced / 1e9,
            b.closing_error(),
            b.closing_error() / b.supplied.max(1.)
        );
        println!(
            "  deposited shares: plains {:.2}%, fans {:.2}%, delta fronts {:.2}%, shelf wedges {:.2}%, basins {:.2}%; dry basins filled to spill {}",
            share(b.plains),
            share(b.fans),
            share(b.deltas),
            share(b.shelf),
            share(b.basins),
            b.dry_basins_filled
        );
        println!(
            "  mapped deposits: {} deltas ({:.0} km²), alluvial plains {:.0} km², fans {:.0} km²",
            b.delta_count,
            b.delta_area / 1e6,
            b.plain_area / 1e6,
            b.fan_area / 1e6
        );
        for (name, site) in [("delta mouth", b.largest_delta), ("fan", b.largest_fan)] {
            if let Some((volume, i)) = site {
                let [x, y, z] = s.centers[i];
                println!(
                    "  largest {name}: region {i}, lat {:.3}°, lon {:.3}°, {:.0} km³",
                    y.asin().to_degrees(),
                    z.atan2(x).to_degrees(),
                    volume / 1e9
                );
            }
        }
        let pre_sediment: Vec<_> = t
            .elevation
            .iter()
            .zip(&t.sediment)
            .map(|(h, d)| h - d)
            .collect();
        let prior = Water::generate(s, &pre_sediment, &w.recipe.water).unwrap();
        let coverage = |water: &Water| -> f64 {
            water
                .body_ids
                .iter()
                .zip(&s.areas)
                .filter(|(id, _)| **id != 0)
                .map(|(_, &a)| a)
                .sum::<f64>()
                / s.areas.iter().sum::<f64>()
                * 100.
        };
        let shelf_area = |water: &Water| -> f64 {
            water
                .depth_meters
                .iter()
                .zip(&water.body_ids)
                .zip(&s.areas)
                .filter(|&((&depth, &id), _)| {
                    id == water.main_ocean_id && depth > 0. && depth <= 200.
                })
                .map(|(_, &area)| area)
                .sum::<f64>()
                / 1e6
        };
        let changed = wet
            .iter()
            .zip(&prior.body_ids)
            .filter(|&(a, b)| (*a == 0) != (*b == 0))
            .count();
        println!(
            "  water coverage {:.5}% -> {:.5}%, shoreline switches {changed}, marine 0–200 m area {:.0} -> {:.0} km²",
            coverage(&prior),
            coverage(&w.water),
            shelf_area(&prior),
            shelf_area(&w.water)
        );
    }
    let mut heights: Vec<f64> = (0..n)
        .filter(|&i| wet[i] == 0)
        .map(|i| t.elevation[i] - level)
        .collect();
    heights.sort_by(f64::total_cmp);
    let q = |p: f64| heights[((heights.len() - 1) as f64 * p) as usize];
    let below = heights.iter().filter(|&&h| h < 200.).count() as f64 / heights.len() as f64;
    println!(
        "  land above water p10/50/90/99/max {:.0} {:.0} {:.0} {:.0} {:.0}; below 200 m {:.1}%",
        q(0.1),
        q(0.5),
        q(0.9),
        q(0.99),
        q(1.),
        below * 100.
    );
    // Local relief (largest dry-neighbor height difference) by regional height class.
    for (lo, hi) in [
        (0., 200.),
        (200., 500.),
        (500., 1000.),
        (1000., 2000.),
        (2000., 1e9),
    ] {
        let mut relief: Vec<f64> = (0..n)
            .filter(|&i| wet[i] == 0 && (lo..hi).contains(&(t.elevation[i] - level)))
            .map(|i| {
                (s.offsets[i] as usize..s.offsets[i + 1] as usize)
                    .map(|k| s.neighbors[k] as usize)
                    .filter(|&j| wet[j] == 0)
                    .map(|j| (t.elevation[i] - t.elevation[j]).abs())
                    .fold(0., f64::max)
            })
            .collect();
        if relief.is_empty() {
            continue;
        }
        relief.sort_by(f64::total_cmp);
        println!(
            "  local relief {lo:>5.0}–{hi:<5.0} m: median {:>5.0} m, p90 {:>5.0} m ({} regions)",
            relief[relief.len() / 2],
            relief[relief.len() * 9 / 10],
            relief.len()
        );
    }
    let r = &w.drainage.receivers;
    let direct = (0..n)
        .filter(|&i| wet[i] == 0 && wet[r[i] as usize] != 0)
        .count();
    let dry = (0..n).filter(|&i| wet[i] == 0).count();
    let sinks = (0..n)
        .filter(|&i| wet[i] == 0 && r[i] as usize == i)
        .count();
    println!(
        "  dry regions {dry}, draining straight into water {:.1}%, dry sinks {sinks}",
        100. * direct as f64 / dry as f64
    );
    // Main stems of the eight largest land catchments by contributing area.
    let area = &w.drainage.contributing_area;
    let mut mouths: Vec<usize> = (0..n)
        .filter(|&i| wet[i] == 0 && wet[r[i] as usize] != 0)
        .collect();
    mouths.sort_by(|&a, &b| area[b].total_cmp(&area[a]));
    let mut donors = vec![Vec::new(); n];
    for i in 0..n {
        if wet[i] == 0 && r[i] as usize != i {
            donors[r[i] as usize].push(i);
        }
    }
    let unit = |i: usize| s.centers[i];
    let angle = |a: [f64; 3], b: [f64; 3]| {
        (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
            .clamp(-1., 1.)
            .acos()
    };
    for &m in mouths.iter().take(8) {
        let mut path = vec![m];
        let mut i = m;
        while let Some(&j) = donors[i]
            .iter()
            .max_by(|&&a, &&b| area[a].total_cmp(&area[b]))
        {
            if area[j] < 2e10 {
                break;
            }
            path.push(j);
            i = j;
        }
        let length: f64 = path
            .windows(2)
            .map(|p| angle(unit(p[0]), unit(p[1])))
            .sum::<f64>()
            * 6371.;
        let straight = angle(unit(m), unit(i)) * 6371.;
        let mut bends = 0;
        for p in path.windows(3) {
            let d = |a: usize, b: usize| {
                [
                    unit(b)[0] - unit(a)[0],
                    unit(b)[1] - unit(a)[1],
                    unit(b)[2] - unit(a)[2],
                ]
            };
            let (u, v) = (d(p[0], p[1]), d(p[1], p[2]));
            let c = (u[0] * v[0] + u[1] * v[1] + u[2] * v[2])
                / ((u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt()
                    * (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt());
            if c < 20f64.to_radians().cos() {
                bends += 1;
            }
        }
        println!(
            "  basin {:>9.0} km²: main stem {:>5.0} km, {:>3} hops, sinuosity {:.2}, bends >20° {}",
            area[m] / 1e6,
            length,
            path.len() - 1,
            length / straight.max(1.),
            bends
        );
    }
}
