//! Compare real dry-incision fields; not a new desktop generator or climate run.
use planimulation_core::{
    Recipe, Surface, World,
    drainage::Drainage,
    landscape::{self, Settings},
    moisture_transport::total_mass,
    water::{Water, WaterSettings},
};
use serde_json::json;
use std::{fmt::Write as _, io::Write, path::Path, time::Instant};

fn recipe(seed: String, subdivision: u32) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: "continuous-plates-1".into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed,
        subdivision,
        radius_meters: 6_371_000.,
        plate_count: 12,
        max_plate_speed_cm_per_year: 8.,
        continental_fraction: 0.38,
        continental_scale: 1.,
        relief_scale: 1.,
        boundary_width_km: 300.,
        detail_amplitude_meters: 300.,
        terrain_preparation_passes: Some(0),
        water: WaterSettings::Coverage { fraction: 0.71 },
    }
}

fn distances(surface: &Surface, receivers: &[u32]) -> Result<Vec<f64>, String> {
    receivers
        .iter()
        .enumerate()
        .map(|(i, &r)| {
            if r as usize == i {
                return Ok(0.);
            }
            let index = (surface.offsets[i] as usize..surface.offsets[i + 1] as usize)
                .find(|&k| surface.neighbors[k] == r)
                .ok_or("Incision receiver is not an actual neighboring region.")?;
            Ok(surface.distances[index])
        })
        .collect()
}

fn color(height: f64, depth: f64) -> String {
    let (a, b, t) = if depth > 0. {
        (
            [31., 117., 157.],
            [12., 40., 76.],
            (depth / 5000.).clamp(0., 1.),
        )
    } else {
        (
            [81., 119., 73.],
            [224., 211., 176.],
            (height / 4500.).clamp(0., 1.),
        )
    };
    format!(
        "#{:02x}{:02x}{:02x}",
        (a[0] + (b[0] - a[0]) * t).round() as u8,
        (a[1] + (b[1] - a[1]) * t).round() as u8,
        (a[2] + (b[2] - a[2]) * t).round() as u8
    )
}

fn atlas(
    surface: &Surface,
    before: &[f64],
    after: &[f64],
    old_water: &Water,
    new_water: &Water,
) -> String {
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1600\" height=\"520\" viewBox=\"0 0 1600 520\"><rect width=\"1600\" height=\"520\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style><text x=\"24\" y=\"30\" font-size=\"20\">Physical terrain comparison — experimental dry incision, not biomes</text><defs><clipPath id=\"clip\"><rect width=\"760\" height=\"380\"/></clipPath></defs>",
    );
    for (panel, (field, water, title)) in [
        (
            before,
            old_water,
            "Baseline: continuous plates + existing terrain",
        ),
        (
            after,
            new_water,
            "Candidate: same terrain + stream-power incision",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let x0 = 24 + panel * 792;
        write!(svg,"<text x=\"{x0}\" y=\"68\" font-size=\"14\">{title}</text><g transform=\"translate({x0},84)\" clip-path=\"url(#clip)\"><rect width=\"760\" height=\"380\" fill=\"#0c284c\"/>").unwrap();
        for (i, &height) in field.iter().enumerate() {
            let c = surface.centers[i];
            let longitude = c[2].atan2(c[0]);
            let fill = color(height - water.level_meters, water.depth_meters[i]);
            let points: Vec<_> = surface.boundaries
                [surface.boundary_offsets[i] as usize..surface.boundary_offsets[i + 1] as usize]
                .iter()
                .map(|v| {
                    let mut lon = v[2].atan2(v[0]);
                    while lon - longitude > std::f64::consts::PI {
                        lon -= std::f64::consts::TAU;
                    }
                    while lon - longitude < -std::f64::consts::PI {
                        lon += std::f64::consts::TAU;
                    }
                    (
                        (lon / std::f64::consts::TAU + 0.5) * 760.,
                        (0.5 - v[1].clamp(-1., 1.).asin() / std::f64::consts::PI) * 380.,
                    )
                })
                .collect();
            for shift in [-760., 0., 760.] {
                if points.iter().all(|(x, _)| x + shift < 0.)
                    || points.iter().all(|(x, _)| x + shift > 760.)
                {
                    continue;
                }
                write!(
                    svg,
                    "<polygon fill=\"{fill}\" stroke=\"{fill}\" stroke-width=\"0.12\" points=\""
                )
                .unwrap();
                for (x, y) in &points {
                    write!(svg, "{:.3},{:.3} ", x + shift, y).unwrap();
                }
                svg.push_str("\"/>");
            }
        }
        svg.push_str("</g>");
    }
    svg.push_str("<text x=\"24\" y=\"498\" font-size=\"13\">Same initial field and fixed water volume. Removed bed material is accounted in a separate reservoir; deposition is not modeled.</text></svg>\n");
    svg
}

fn create(path: &str, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err(
            "Usage: landscape_preview NEW_REPORT.json NEW_ATLAS.svg SEED SUBDIVISION PASSES".into(),
        );
    }
    if args[0] == args[1] || Path::new(&args[0]).exists() || Path::new(&args[1]).exists() {
        return Err(
            "Use two different new output paths; existing outputs are never overwritten.".into(),
        );
    }
    let level: u32 = args[3]
        .parse()
        .map_err(|_| "Invalid landscape subdivision.")?;
    let passes: usize = args[4]
        .parse()
        .map_err(|_| "Invalid landscape pass count.")?;
    if !(3..=6).contains(&level) || !(1..=32).contains(&passes) {
        return Err("Preview supports subdivisions 3–6 and 1–32 geological passes.".into());
    }
    let started = Instant::now();
    let world = World::generate(recipe(args[2].clone(), level))?;
    let generated_seconds = started.elapsed().as_secs_f64();
    let mut field = world.terrain.elevation.clone();
    let settings = Settings {
        erodibility_per_year: 1e-5,
        step_years: 10_000.,
        base_level_meters: Some(world.water.level_meters),
    };
    let mut material = Vec::new();
    let mut history = Vec::new();
    let erosion_started = Instant::now();
    for pass in 0..passes {
        // Initial wet terminals stay fixed during this dry preparation experiment.
        let drainage = Drainage::build(&world.surface, &field, &world.water.body_ids);
        let result = landscape::incise(
            &field,
            &world.surface.areas,
            &drainage.receivers,
            &distances(&world.surface, &drainage.receivers)?,
            settings,
        )?;
        history.push(json!({"pass":pass+1,"changedNodes":result.changed_nodes,
            "unresolvedPositiveIncisionNodes":result.unresolved_positive_incision_nodes,
            "removedMaterialCubicMeters":result.removed_material_cubic_meters}));
        material.push(result.removed_material_cubic_meters);
        field = result.elevation_meters;
    }
    let erosion_seconds = erosion_started.elapsed().as_secs_f64();
    let candidate_water = Water::generate(
        &world.surface,
        &field,
        &WaterSettings::Volume {
            volume_cubic_meters: world.water.resolved_volume_cubic_meters,
        },
    )?;
    let removed = total_mass(&material);
    let independent = total_mass(
        &world
            .terrain
            .elevation
            .iter()
            .zip(&field)
            .zip(&world.surface.areas)
            .map(|((a, b), area)| (a - b) * area)
            .collect::<Vec<_>>(),
    );
    let residual = (removed - independent).abs() / removed.max(1.);
    if !residual.is_finite() || residual > 1e-12 {
        return Err("Landscape material ledger failed.".into());
    }
    let area = total_mass(&world.surface.areas);
    let losses: Vec<_> = world
        .terrain
        .elevation
        .iter()
        .zip(&field)
        .map(|(a, b)| a - b)
        .collect();
    let dry_area = total_mass(
        &world
            .surface
            .areas
            .iter()
            .zip(&world.water.depth_meters)
            .filter_map(|(a, d)| (*d == 0.).then_some(*a))
            .collect::<Vec<_>>(),
    );
    let changed_dry_area = total_mass(
        &world
            .surface
            .areas
            .iter()
            .zip(&losses)
            .filter_map(|(a, loss)| (*loss > 0.).then_some(*a))
            .collect::<Vec<_>>(),
    );
    let final_drainage = Drainage::build(&world.surface, &field, &world.water.body_ids);
    let dry_terminals = |graph: &Drainage| {
        graph
            .receivers
            .iter()
            .enumerate()
            .filter(|(i, r)| **r as usize == *i && world.water.body_ids[*i] == 0)
            .count()
    };
    let coverage = |w: &Water| {
        total_mass(
            &world
                .surface
                .areas
                .iter()
                .zip(&w.depth_meters)
                .filter_map(|(a, d)| (*d > 0.).then_some(*a))
                .collect::<Vec<_>>(),
        ) / area
    };
    let report = json!({"reportVersion":"landscape-preview-1","incisionModelVersion":landscape::MODEL_VERSION,
        "baselineRecipe":world.recipe,"settings":settings,"passes":passes,"regions":field.len(),
        "timingSeconds":{"initialGeneration":generated_seconds,"dryIncisionAndDrainage":erosion_seconds},
        "removedMaterialReservoirCubicMeters":removed,"independentBedLossCubicMeters":independent,
        "relativeMaterialLedgerResidual":residual,"history":history,
        "diagnostics":{"maximumIncisionMeters":losses.iter().copied().fold(0.,f64::max),
            "initialDryAreaMeanIncisionMeters":removed/dry_area,
            "incisedFractionOfInitialDryArea":changed_dry_area/dry_area,
            "initialClosedDryTerminals":dry_terminals(&world.drainage),
            "candidateClosedDryTerminalsUsingInitialWetMask":dry_terminals(&final_drainage)},
        "baselineWaterLevelMeters":world.water.level_meters,"candidateWaterLevelMeters":candidate_water.level_meters,
        "initialWaterVolumeCubicMeters":world.water.resolved_volume_cubic_meters,
        "candidateWaterVolumeCubicMeters":candidate_water.resolved_volume_cubic_meters,
        "baselineCoverage":coverage(&world.water),"candidateCoverage":coverage(&candidate_water),
        "baselineElevationMeters":world.terrain.elevation,"candidateElevationMeters":field,
        "scope":"Dry geological preparation with prescribed effective runoff, fixed initial wet terminals and off-grid material reservoir. No deposition, new continental field, climate, selected desktop model or higher-resolution product support."});
    let svg = atlas(
        &world.surface,
        &world.terrain.elevation,
        &field,
        &world.water,
        &candidate_water,
    );
    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    create(&args[0], &bytes)?;
    // The two diagnostic files are not a transactional simulation checkpoint.
    create(&args[1], svg.as_bytes())?;
    println!(
        "Landscape comparison: {} regions; {:.3} s generation, {:.3} s incision; material residual {:.3e}.",
        field.len(),
        generated_seconds,
        erosion_seconds,
        residual
    );
    Ok(())
}
