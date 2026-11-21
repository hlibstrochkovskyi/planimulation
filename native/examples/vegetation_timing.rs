//! Time default vegetation setup, bulk advance and warm-land history replay.
#[path = "support/natural_world.rs"]
mod natural_world;

use planimulation_core::World;
use planimulation_core::climate_normals::Normals;
use planimulation_core::vegetation::Model;
use std::time::Instant;

fn main() {
    let world = World::generate(natural_world::recipe("first-light", 6)).unwrap();
    let normals = Normals::from_world(&world).unwrap();
    let (region, component_size) = largest_woody_component(&world, &normals);
    println!("first-light, region {region}, {component_size} connected warm regions, 5 repeats");
    let mut samples = [[0.; 5]; 3];
    for run in 0..5 {
        let start = Instant::now();
        let (model, mut state) = Model::new(&world, &normals, Default::default()).unwrap();
        samples[0][run] = start.elapsed().as_secs_f64();
        let start = Instant::now();
        model.advance(&mut state, 1000).unwrap();
        samples[1][run] = start.elapsed().as_secs_f64();
        let start = Instant::now();
        std::hint::black_box(model.region_series(&state, region).unwrap());
        samples[2][run] = start.elapsed().as_secs_f64();
        println!(
            "{:.4} / {:.4} / {:.4} s",
            samples[0][run], samples[1][run], samples[2][run]
        );
    }
    for values in &mut samples {
        values.sort_by(f64::total_cmp);
    }
    println!(
        "Medians (setup / advance / replay): {:.4} / {:.4} / {:.4} s",
        samples[0][2], samples[1][2], samples[2][2]
    );
}

// Match the replay's physical dependency when choosing a representative warm-land
// target. Taking the first land cell can time a small island instead.
fn largest_woody_component(world: &World, normals: &Normals) -> (usize, usize) {
    let surface = &world.surface;
    let mut seen: Vec<_> = world
        .water
        .body_ids
        .iter()
        .zip(&normals.warmest_month_celsius)
        .map(|(&body, &warmest)| body != 0 || warmest < 10.)
        .collect();
    let mut largest = (0, 0);
    for start in 0..seen.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut pending = vec![start];
        let mut size = 0;
        while let Some(i) = pending.pop() {
            size += 1;
            for &j in
                &surface.neighbors[surface.offsets[i] as usize..surface.offsets[i + 1] as usize]
            {
                let j = j as usize;
                if !seen[j] {
                    seen[j] = true;
                    pending.push(j);
                }
            }
        }
        if size > largest.1 {
            largest = (start, size);
        }
    }
    largest
}
