//! Bounded generation-only performance probe; no desktop/world checkpoint output.
use planimulation_core::{
    Recipe, Surface,
    crust::Crust,
    drainage::Drainage,
    hash,
    landscape::{self, Settings},
    moisture_transport::total_mass,
    tectonics::Tectonics,
    terrain::Terrain,
    water::{Water, WaterSettings},
};
use serde_json::json;
use std::{io::Write, path::Path, time::Instant};

fn peak_rss_bytes() -> Option<u64> {
    // Linux process high-water RSS includes allocator and temporary geometry.
    // This is cumulative, not the isolated allocation cost of the current stage.
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find(|line| line.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()?
        .checked_mul(1024)
}

fn storage<T>(items: &Vec<T>) -> usize {
    items.capacity() * std::mem::size_of::<T>()
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: landscape_scale NEW_REPORT.json SEED SUBDIVISION(6..8)".into());
    }
    if Path::new(&args[0]).exists() {
        return Err("Scale report must use a new output path.".into());
    }
    let level: u32 = args[2].parse().map_err(|_| "Invalid scale subdivision.")?;
    if !(6..=8).contains(&level) {
        return Err("Scale probe supports subdivisions 6–8 only.".into());
    }
    // This remains a valid reference parameter recipe. Sampling resolution is
    // separate experiment metadata, never mislabeled as a supported Recipe.
    let parameters = Recipe {
        schema_version: 1,
        model_version: "continuous-plates-1".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: args[1].clone(),
        subdivision: 6,
        radius_meters: 6371000.,
        plate_count: 12,
        max_plate_speed_cm_per_year: 8.,
        continental_fraction: 0.38,
        continental_scale: 1.,
        relief_scale: 1.,
        boundary_width_km: 300.,
        detail_amplitude_meters: 300.,
        terrain_preparation_passes: Some(0),
        water: WaterSettings::Coverage { fraction: 0.71 },
    };
    parameters.validate()?;
    let started = Instant::now();
    let mut stages = Vec::new();
    let mut previous = started;
    let mut mark = |name: &str| {
        let now = Instant::now();
        stages.push(
            json!({"name":name,"seconds":now.duration_since(previous).as_secs_f64(),
            "cumulativePeakRssBytes":peak_rss_bytes()}),
        );
        previous = now;
    };
    let surface = Surface::build_generation_probe(level, parameters.radius_meters)?;
    mark("surface");
    let crust = Crust::build(
        &surface,
        &parameters.seed,
        parameters.continental_fraction,
        parameters.continental_scale,
    );
    mark("crust");
    let tectonics = Tectonics::build_with_continuous_roots(
        &surface,
        &parameters.seed,
        parameters.plate_count,
        parameters.max_plate_speed_cm_per_year,
        parameters.radius_meters,
    )?;
    mark("tectonics");
    let terrain = Terrain::build(&surface, &crust, &tectonics, &parameters);
    mark("terrain");
    let water = Water::generate(&surface, &terrain.elevation, &parameters.water)?;
    mark("initialWater");
    let drainage = Drainage::build(&surface, &terrain.elevation, &water.body_ids);
    mark("drainage");
    let distances: Vec<f64> = drainage
        .receivers
        .iter()
        .enumerate()
        .map(|(i, &r)| {
            if r as usize == i {
                return 0.;
            }
            let edge = (surface.offsets[i] as usize..surface.offsets[i + 1] as usize)
                .find(|&k| surface.neighbors[k] == r)
                .expect("Drainage receiver must be adjacent.");
            surface.distances[edge]
        })
        .collect();
    let settings = Settings {
        erodibility_per_year: 1e-5,
        step_years: 10000.,
        base_level_meters: Some(water.level_meters),
    };
    let incision = landscape::incise(
        &terrain.elevation,
        &surface.areas,
        &drainage.receivers,
        &distances,
        settings,
    )?;
    mark("oneIncisionPassIncludingLinkExtraction");
    let total_seconds = started.elapsed().as_secs_f64();
    let generation_peak = peak_rss_bytes();
    let n = surface.centers.len();
    let sphere_area = 4. * std::f64::consts::PI * parameters.radius_meters.powi(2);
    let relative_area_error = (total_mass(&surface.areas) - sphere_area).abs() / sphere_area;
    if n != 10 * 4_usize.pow(level) + 2
        || !relative_area_error.is_finite()
        || relative_area_error > 1e-12
        || surface.neighbors.len() != 6 * n - 12
        || surface.boundaries.len() != 12 * n - 24
    {
        return Err("Generation probe geometry count or area invariant failed.".into());
    }
    // Retained Vec capacity bytes are not whole-process memory or serialized size.
    let surface_storage = storage(&surface.centers)
        + storage(&surface.faces)
        + storage(&surface.offsets)
        + storage(&surface.neighbors)
        + storage(&surface.distances)
        + storage(&surface.areas)
        + storage(&surface.boundary_offsets)
        + storage(&surface.boundaries);
    let crust_storage = storage(&crust.potential)
        + storage(&crust.continentality)
        + storage(&crust.thickness_meters)
        + storage(&crust.density_kg_per_cubic_meter);
    let terrain_storage = storage(&terrain.baseline)
        + storage(&terrain.convergence)
        + storage(&terrain.divergence)
        + storage(&terrain.detail)
        + storage(&terrain.preparation)
        + storage(&terrain.elevation);
    let tectonic_storage = storage(&tectonics.owners)
        + storage(&tectonics.seeds)
        + storage(&tectonics.angular_velocities)
        + storage(&tectonics.boundary_cells)
        + storage(&tectonics.boundary_directions)
        + storage(&tectonics.boundary_motion)
        + storage(&tectonics.boundary_types);
    let water_storage = storage(&water.depth_meters) + storage(&water.body_ids);
    let drainage_storage = storage(&drainage.receivers)
        + storage(&drainage.outlets)
        + storage(&drainage.flat_steps)
        + storage(&drainage.contributing_area);
    let incision_storage = storage(&incision.elevation_meters)
        + storage(&incision.removed_thickness_meters)
        + storage(&incision.contributing_area_square_meters)
        + storage(&distances);
    // Byte-order specified diagnostic fingerprint, not a cryptographic identity.
    let mut physical = Vec::with_capacity(n * 16);
    for field in [&terrain.elevation, &incision.elevation_meters] {
        for value in field {
            physical.extend_from_slice(&value.to_le_bytes());
        }
    }
    let fingerprint = format!("{:08x}", hash(&physical));
    let report = json!({"reportVersion":"landscape-scale-1","samplingSubdivision":level,"regions":n,
        "referenceParameters":parameters,"incisionModelVersion":landscape::MODEL_VERSION,"incisionSettings":settings,
        "stageMeasurements":stages,"totalGenerationAndOnePassSeconds":total_seconds,
        "generationPeakRssBytes":generation_peak,"reportingPeakRssBytes":peak_rss_bytes(),
        "retainedVecCapacityBytes":{"surface":surface_storage,"crust":crust_storage,"terrain":terrain_storage,
            "tectonics":tectonic_storage,"water":water_storage,"drainage":drainage_storage,"incisionAndLinks":incision_storage},
        "heightFieldBytes":n*8,"relativeSphereAreaError":relative_area_error,
        "elevationRangeMeters":{"minimum":terrain.elevation.iter().copied().fold(f64::INFINITY,f64::min),
            "maximum":terrain.elevation.iter().copied().fold(f64::NEG_INFINITY,f64::max)},
        "removedMaterialReservoirCubicMeters":incision.removed_material_cubic_meters,
        "changedNodes":incision.changed_nodes,"physicalFieldFnv1aLittleEndian32":fingerprint,
        "scope":"Generation-only existing structural fields, one dry incision pass. No basins, atmosphere, dynamic water, renderer, wire, checkpoint or supported higher-resolution recipe. Linux VmHWM is cumulative; timings and memory are not reproducible physical fields."});
    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[0])
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    println!(
        "Scale: {n} nodes, {total_seconds:.3} s, generation peak RSS {generation_peak:?} bytes, report {} bytes, fields {fingerprint}.",
        bytes.len()
    );
    Ok(())
}
