//! Generate retained native physical patches and their construction comparison.
//! No Python runtime, display-generated heights or seasonal water advancement.
use planimulation_core::{
    Recipe, Surface, drainage_landforms, fixed_geological_field, physical_seed_field,
    terrain_patch_geometry::{self, Geometry},
    terrain_patch_landscape::{self, SpecificSettings},
};
use serde_json::json;
use std::{fmt::Write as _, io::Write, path::Path, time::Instant};

fn reference(seed: &str) -> Result<Recipe, String> {
    serde_json::from_value(json!({
        "schemaVersion":1,"modelVersion":"multiscale-geography-2",
        "randomVersion":"fnv1a-utf8-mulberry32-1","seed":seed,"subdivision":6,
        "radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,
        "continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,
        "boundaryWidthKm":300.,"detailAmplitudeMeters":300.,
        "terrainPreparationPasses":0,"water":{"mode":"coverage","fraction":0.71}
    }))
    .map_err(|e| e.to_string())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| e.to_string())
}

fn shade(bed: &[f64], n: usize, spacing: f64, i: usize) -> f64 {
    let x = i % n;
    let y = i / n;
    let left = x.saturating_sub(1);
    let right = (x + 1).min(n - 1);
    let bottom = y.saturating_sub(1);
    let top = (y + 1).min(n - 1);
    let gx = (bed[y * n + right] - bed[y * n + left]) / ((right - left) as f64 * spacing);
    let gy = (bed[top * n + x] - bed[bottom * n + x]) / ((top - bottom) as f64 * spacing);
    0.25 + 0.75 * ((gx - gy + 1.) / (3. * (gx * gx + gy * gy + 1.)).sqrt()).max(0.)
}

fn atlas(g: &Geometry, before: &[f64], patch: &drainage_landforms::Patch) -> String {
    let n = g.settings.cells_x as usize;
    let size = 480.;
    let scale = size / n as f64;
    let mut out = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1550\" height=\"620\"><rect width=\"1550\" height=\"620\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style>",
    );
    for (panel, (title, bed, links)) in [
        ("Native prepared source", before, false),
        (
            "Constructed physical bed",
            patch.construction.elevation_meters.as_slice(),
            false,
        ),
        (
            "Rebuilt actual dry collection",
            patch.construction.elevation_meters.as_slice(),
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let origin = 24. + panel as f64 * 512.;
        let min = bed.iter().copied().fold(f64::INFINITY, f64::min);
        let max = bed.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        write!(out,"<text x=\"{origin}\" y=\"28\" font-size=\"18\">{title}</text><text x=\"{origin}\" y=\"49\" font-size=\"13\">Physical height {min:.1} to {max:.1} m</text><g transform=\"translate({origin},66)\" shape-rendering=\"crispEdges\">").unwrap();
        for i in 0..bed.len() {
            let c = (255. * shade(bed, n, g.settings.spacing_meters, i)).round() as u8;
            write!(out,"<rect x=\"{:.4}\" y=\"{:.4}\" width=\"{:.4}\" height=\"{:.4}\" fill=\"#{c:02x}{c:02x}{c:02x}\"/>",(i%n) as f64*scale,(n-1-i/n) as f64*scale,scale+0.001,scale+0.001).unwrap();
        }
        if links {
            out.push_str("<g fill=\"none\" stroke=\"#46b8d7\" stroke-width=\"0.65\" opacity=\"0.8\" shape-rendering=\"geometricPrecision\">");
            let flow = &patch.actual_dry_routing;
            for i in 0..bed.len() {
                let j = flow.receivers[i] as usize;
                if i != j && flow.contributing_area_square_meters[i] >= 1e6 {
                    write!(
                        out,
                        "<path d=\"M {:.4} {:.4} L {:.4} {:.4}\"/>",
                        ((i % n) as f64 + 0.5) * scale,
                        ((n - 1 - i / n) as f64 + 0.5) * scale,
                        ((j % n) as f64 + 0.5) * scale,
                        ((n - 1 - j / n) as f64 + 0.5) * scale
                    )
                    .unwrap();
                }
            }
            out.push_str("</g>");
        }
        out.push_str("</g>");
    }
    write!(out,"<text x=\"24\" y=\"574\" font-size=\"14\">20 km chart / {n} x {n} physical cells / same normal lighting, slopes 1x / no elevation-color normalization</text>").unwrap();
    out.push_str("<text x=\"24\" y=\"599\" font-size=\"13\">Cyan: actual dry source area above 1 km2, not discharge. Graph-edge limits are not continuous-gradient limits. Not a global World.</text></svg>");
    out
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() || args.len() > 3 {
        return Err(
            "Usage: drainage_landform_patch NEW_DIRECTORY [SEED] [CELLS_PER_SIDE(40..160)]".into(),
        );
    }
    let output = Path::new(&args[0]);
    if output.exists() {
        return Err("A new output directory is required.".into());
    }
    let seed = args.get(1).map_or("first-light", String::as_str);
    let cells = args.get(2).map_or(Ok(160_u32), |s| {
        s.parse().map_err(|_| "Invalid patch cells.")
    })?;
    if !(40..=160).contains(&cells) {
        return Err("Patch cells must be 40..160 per side.".into());
    }
    let recipe = reference(seed)?;
    recipe.validate()?;
    let field = fixed_geological_field::Field::new(recipe.clone())?;
    let noise_settings = physical_seed_field::Settings {
        radius_meters: recipe.radius_meters,
        amplitude_meters: 10.,
        length_scale: 0.25,
    };
    let noise = physical_seed_field::Field::new(seed, noise_settings)?;
    let candidates = Surface::build(3, recipe.radius_meters);
    let mut center = [1., 0., 0.];
    let mut highest = f64::NEG_INFINITY;
    for &p in &candidates.centers {
        let h = field.sample(p)?.elevation_meters;
        if h > highest {
            highest = h;
            center = p;
        }
    }
    let settings = drainage_landforms::Settings {
        channel_head_area_square_meters: 250000.,
        channel_head_slope: 0.1,
        channel_concavity: 0.5,
        maximum_graph_slope: std::f64::consts::FRAC_PI_6.tan(),
    };
    let source_settings = SpecificSettings {
        erodibility_coefficient: 340. / 20000_f64.powf(1.4),
        specific_area_exponent: 0.4,
        diffusivity_square_meters_per_year: 1.,
        step_years: 37500.,
        steps: 128,
    };
    let mut records = Vec::new();
    for (case, anchor) in [("synthetic", [1., 0., 0.]), ("generated", center)] {
        let started = Instant::now();
        let geometry = terrain_patch_geometry::build(terrain_patch_geometry::Settings {
            radius_meters: recipe.radius_meters,
            center_direction: anchor,
            orientation_radians: 0.,
            cells_x: cells,
            cells_y: cells,
            spacing_meters: 20000. / f64::from(cells),
        })?;
        let mut initial = Vec::new();
        let mut rates = Vec::new();
        for (i, &p) in geometry.cell_directions.iter().enumerate() {
            let s = field.sample(p)?;
            let fixed = geometry.fixed_external_boundary[i];
            let perturbation = noise.sample(p)?;
            initial.push(if case == "synthetic" {
                if fixed { 0. } else { 1. + perturbation }
            } else {
                s.elevation_meters + perturbation
            });
            rates.push(if fixed {
                0.
            } else if case == "synthetic" {
                0.0005
            } else {
                1e-6 * 2000. * s.convergence_meters / 6000.
            });
        }
        let source = terrain_patch_landscape::prepare_specific(
            &geometry,
            &initial,
            &rates,
            source_settings,
        )?;
        let source_seconds = started.elapsed().as_secs_f64();
        let started = Instant::now();
        let patch =
            drainage_landforms::construct_patch(&geometry, &source.elevation_meters, settings)?;
        let construction_seconds = started.elapsed().as_secs_f64();
        let image = atlas(&geometry, &source.elevation_meters, &patch);
        println!(
            "{case}: source {source_seconds:.3}s, construction {construction_seconds:.3}s, closed {:.2}% -> {:.2}%",
            source.final_routing.closed_fraction_of_active_area * 100.,
            patch.actual_dry_routing.closed_fraction_of_active_area * 100.
        );
        let report = json!({
            "reportVersion":"native-drainage-landform-patch-1","case":case,"seed":seed,
            "referenceRecipe":recipe,"sourceIdentity":field.source_identity(),
            "sourceFieldModelVersion":fixed_geological_field::MODEL_VERSION,
            "seedFieldModelVersion":physical_seed_field::MODEL_VERSION,"seedFieldSettings":noise_settings,
            "geometry":geometry,"initialBedMeters":initial,"upliftMetersPerYear":rates,
            "preparedSource":source,"landforms":patch,
            "timingsSeconds":{"sourcePreparation":source_seconds,"construction":construction_seconds},
            "scope":"Native generation-only physical patch. Channel profiles and roof change actual bed with explicit detached-material debits; not time-integrated erosion, deposition or proven outlet export. Source specific-area preparation and construction are separate stages. No water/climate, global World, checkpoint or desktop integration. Fixed perimeter is an external height constraint, not ocean."
        });
        records.push((
            case,
            serde_json::to_vec(&report).map_err(|e| e.to_string())?,
            image,
        ));
    }
    // Numerical refusal creates no output directory; completion is manifest-last.
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    for (case, report, image) in records {
        write_new(&output.join(format!("{case}.json")), &report)?;
        write_new(&output.join(format!("{case}.svg")), image.as_bytes())?;
    }
    let manifest = json!({"reportVersion":"native-drainage-landform-patch-1","seed":seed,"cellsPerSide":cells,
        "constructionModelVersion":drainage_landforms::MODEL_VERSION,
        "records":["synthetic.json","generated.json"],"atlases":["synthetic.svg","generated.svg"],
        "scope":"Bounded native quality comparison, not a World or a completed R3/R4. Both source and final beds are retained."});
    write_new(
        &output.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
}
