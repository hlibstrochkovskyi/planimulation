//! Actual fine-bed comparisons; regional experiments, not coupled global tiles.
use planimulation_core::{
    Random, Recipe, Surface, fixed_geological_field, physical_seed_field, terrain_patch_facets,
    terrain_patch_geometry::{self, Geometry},
    terrain_patch_landscape::{self, Preparation, Settings},
};
use serde_json::{Value, json};
use std::{
    fmt::Write as _,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
const DURATION_YEARS: f64 = 4_800_000.;
trait ReviewFlow {
    fn closed_count(&self) -> usize;
    fn closed_fraction(&self) -> f64;
    fn areas(&self) -> &[f64];
    fn branches(&self, i: usize) -> [(u32, f64); 2];
}
impl ReviewFlow for terrain_patch_landscape::Routing {
    fn closed_count(&self) -> usize {
        self.closed_sink_count
    }
    fn closed_fraction(&self) -> f64 {
        self.closed_fraction_of_active_area
    }
    fn areas(&self) -> &[f64] {
        &self.contributing_area_square_meters
    }
    fn branches(&self, i: usize) -> [(u32, f64); 2] {
        [(self.receivers[i], 1.), (i as u32, 0.)]
    }
}
impl ReviewFlow for terrain_patch_facets::Routing {
    fn closed_count(&self) -> usize {
        self.closed_sink_count
    }
    fn closed_fraction(&self) -> f64 {
        self.closed_fraction_of_active_area
    }
    fn areas(&self) -> &[f64] {
        &self.contributing_area_square_meters
    }
    fn branches(&self, i: usize) -> [(u32, f64); 2] {
        std::array::from_fn(|j| {
            (
                self.descents[i].receivers[j],
                self.descents[i].area_weights[j],
            )
        })
    }
}
fn reference(seed: &str) -> Recipe {
    serde_json::from_value(json!({"schemaVersion":1,"modelVersion":"multiscale-geography-2","randomVersion":"fnv1a-utf8-mulberry32-1",
        "seed":seed,"subdivision":6,"radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,"continentalFraction":0.38,
        "continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,"detailAmplitudeMeters":300.,"terrainPreparationPasses":0,
        "water":{"mode":"coverage","fraction":0.71}})).unwrap()
}
struct Arguments {
    directory: PathBuf,
    seed: String,
    steps: usize,
    duration: f64,
    diffusion: f64,
    facets: bool,
    physical_seeds: bool,
    physical_amplitude: f64,
}
fn parse(args: &[String]) -> Result<Arguments, String> {
    if !(1..=7).contains(&args.len())
        || args[0].trim().is_empty()
        || args
            .get(5)
            .is_some_and(|s| !matches!(s.as_str(), "--facets" | "--spectrum"))
        || args.len() == 7 && args[5] != "--spectrum"
    {
        return Err(
            "Usage: fine_landscape NEW_DIRECTORY [SEED] [STEPS(1..128)] [DURATION_YEARS] [DIFFUSIVITY_M2_PER_YEAR] [--facets|--spectrum [AMPLITUDE_METERS]]".into(),
        );
    }
    let path = PathBuf::from(&args[0]);
    if std::fs::symlink_metadata(&path).is_ok()
        || path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .is_some_and(|p| !p.is_dir())
    {
        return Err("Fine landscape requires a new directory with an existing parent.".into());
    }
    let seed = args.get(1).cloned().unwrap_or("first-light".into());
    reference(&seed).validate()?;
    let steps = args
        .get(2)
        .map(|v| v.parse::<usize>())
        .transpose()
        .map_err(|_| "Invalid fine-landscape step count.")?
        .unwrap_or(64);
    if !(1..=128).contains(&steps) {
        return Err("Fine-landscape steps must be1–128.".into());
    }
    let duration = args
        .get(3)
        .map(|v| v.parse::<f64>())
        .transpose()
        .map_err(|_| "Invalid fine-landscape duration.")?
        .unwrap_or(DURATION_YEARS);
    if !duration.is_finite() || !(1. ..=50_000_000.).contains(&duration) {
        return Err("Fine-landscape duration must be1–50000000years.".into());
    }
    let diffusion = args
        .get(4)
        .map(|v| v.parse::<f64>())
        .transpose()
        .map_err(|_| "Invalid fine-landscape diffusivity.")?
        .unwrap_or(1.);
    if !diffusion.is_finite() || !(0. ..=1.).contains(&diffusion) {
        return Err("Fine-landscape diffusivity supports0–1m2/year.".into());
    }
    let amplitude = args
        .get(6)
        .map(|s| s.parse::<f64>())
        .transpose()
        .map_err(|_| "Invalid physical seed amplitude.")?
        .unwrap_or(1.);
    if !amplitude.is_finite() || !(0. ..=1000.).contains(&amplitude) {
        return Err("Physical seed amplitude must be 0–1000m.".into());
    }
    Ok(Arguments {
        directory: path,
        seed,
        steps,
        duration,
        diffusion,
        facets: args.len() >= 6,
        physical_seeds: args.get(5).is_some_and(|s| s == "--spectrum"),
        physical_amplitude: amplitude,
    })
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())
}
fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    write_new(
        path,
        &serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?,
    )
}
struct Perturbation {
    waves: Vec<([f64; 3], f64)>,
}
impl Perturbation {
    fn new(seed: &str) -> Self {
        let mut rng = Random::stream(seed, "fine-patch-seed-perturbations-1");
        let mut next = || f64::from(rng.next_u32()) / 4294967296.;
        let mut waves = Vec::new();
        for _ in 0..6 {
            let y = 2. * next() - 1.;
            let phi = next() * std::f64::consts::TAU;
            let r = (1. - y * y).sqrt();
            let scale = std::f64::consts::TAU * 6371000. / 4000.;
            waves.push((
                [r * phi.cos() * scale, y * scale, r * phi.sin() * scale],
                next() * std::f64::consts::TAU,
            ));
        }
        Self { waves }
    }
    fn sample(&self, p: [f64; 3]) -> f64 {
        self.waves
            .iter()
            .map(|(w, phase)| (w.iter().zip(p).map(|(w, p)| w * p).sum::<f64>() + phase).sin() / 6.)
            .sum()
    }
}
fn metrics<R: ReviewFlow>(g: &Geometry, h: &[f64], p: &Preparation<R>) -> Value {
    let mut sorted: Vec<_> = h
        .iter()
        .copied()
        .zip(g.reference_areas_square_meters.iter().copied())
        .collect();
    sorted.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    let area: f64 = g.reference_areas_square_meters.iter().sum();
    let mut cumulative = 0.;
    let mut quantiles = Vec::new();
    let mut index = 0;
    for q in [0.05, 0.5, 0.95] {
        while index + 1 < sorted.len() && cumulative + sorted[index].1 < q * area {
            cumulative += sorted[index].1;
            index += 1;
        }
        quantiles.push(sorted[index].0);
    }
    let mut squared = 0.;
    let mut total_weight = 0.;
    let mut maximum_slope = 0_f64;
    for e in &g.side_faces {
        let a = e.a as usize;
        let b = e.b as usize;
        let slope = (h[a] - h[b]) / e.center_distance_meters;
        maximum_slope = maximum_slope.max(slope.abs());
        squared += slope * slope * e.shared_face_length_meters;
        total_weight += e.shared_face_length_meters;
    }
    json!({"areaSquareMeters":area,"heightQuantileProbabilities":[0.05,0.5,0.95],"heightQuantilesMeters":quantiles,
        "minimumHeightMeters":sorted[0].0,"maximumHeightMeters":sorted.last().unwrap().0,
        "neighborScaleRmsSlope":(squared/total_weight).sqrt(),"neighborScaleMaximumSlope":maximum_slope,
        "closedDrySinkCount":p.final_routing.closed_count(),"closedFractionOfActiveArea":p.final_routing.closed_fraction()})
}
fn record<R: ReviewFlow>(
    g: &Geometry,
    p: &Preparation<R>,
    label: &str,
    file: &str,
    seconds: f64,
) -> Value {
    let mut record = json!({"variant":label,"settings":p.settings,"preparationModelVersion":p.model_version,
        "diffusionSubstepsPerStep":p.diffusion_substeps_per_step,"metrics":metrics(g,&p.elevation_meters,p),
        "commonSupport":common_support(g,&p.elevation_meters),
        "acceptedUpliftPrismCubicMeters":p.accepted_uplift_prism_cubic_meters,"detachedMaterialReservoirCubicMeters":p.detached_material_reservoir_cubic_meters,
        "signedHillslopeBoundaryExchangeCubicMeters":p.signed_hillslope_boundary_exchange_cubic_meters,
        "grossRequestedHillslopeTransferCubicMeters":p.gross_requested_hillslope_transfer_cubic_meters,
        "relativePrismLedgerResidual":p.relative_prism_ledger_residual,"history":p.history,"physicalFieldsFile":file,"preparationSeconds":seconds});
    if let Some(c) = &p.receiver_constraint {
        record["receiverConstraint"] = serde_json::to_value(c).unwrap();
    }
    record
}
fn shade(g: &Geometry, h: &[f64], i: usize) -> f64 {
    let nx = g.settings.cells_x as usize;
    let ny = g.settings.cells_y as usize;
    let row = i / nx;
    let col = i % nx;
    let left = if col > 0 { i - 1 } else { i };
    let right = if col + 1 < nx { i + 1 } else { i };
    let below = if row > 0 { i - nx } else { i };
    let above = if row + 1 < ny { i + nx } else { i };
    let distance = |j| {
        if j == i {
            0.
        } else {
            let start = g.offsets[i] as usize;
            g.distances_meters[start
                + g.neighbors[start..g.offsets[i + 1] as usize]
                    .binary_search(&(j as u32))
                    .unwrap()]
        }
    };
    let dx = (h[right] - h[left]) / (distance(left) + distance(right));
    let dy = (h[above] - h[below]) / (distance(below) + distance(above));
    let light = (dx - dy + 1.) / ((dx * dx + dy * dy + 1.).sqrt() * 3_f64.sqrt());
    0.3 + 0.7 * light.max(0.)
}
fn atlas(
    g: &Geometry,
    initial: &[f64],
    a: &Preparation,
    b: &Preparation,
    facets: Option<&Preparation<terrain_patch_facets::Routing>>,
) -> String {
    let nx = g.settings.cells_x as usize;
    let ny = g.settings.cells_y as usize;
    let scale = 480. / nx as f64;
    let minimum = initial.iter().copied().fold(f64::INFINITY, f64::min);
    let mut maximum = initial
        .iter()
        .chain(&a.elevation_meters)
        .chain(&b.elevation_meters)
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    type Panel<'a> = (&'a str, &'a [f64], Option<&'a dyn ReviewFlow>);
    let mut panels: Vec<Panel<'_>> = vec![
        ("Initial authoritative bed", initial, None),
        (
            "Incision only",
            a.elevation_meters.as_slice(),
            Some(&a.final_routing),
        ),
        (
            "Incision + hillslope transfer",
            b.elevation_meters.as_slice(),
            Some(&b.final_routing),
        ),
    ];
    if let Some(p) = facets {
        maximum = maximum.max(
            p.elevation_meters
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max),
        );
        panels.push((
            "Facet incision + hillslope transfer",
            &p.elevation_meters,
            Some(&p.final_routing),
        ));
    }
    let width = 16 + 508 * panels.len();
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"610\"><rect width=\"{width}\" height=\"610\" fill=\"#10191f\"/><style>text{{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}}</style>"
    );
    for (panel, (label, h, routing)) in panels.into_iter().enumerate() {
        let x = 24 + panel * 508;
        write!(svg,"<text x=\"{x}\" y=\"30\" font-size=\"16\">{label}</text><g transform=\"translate({x},50)\">").unwrap();
        svg.push_str("<g shape-rendering=\"crispEdges\">");
        for (i, &height) in h.iter().enumerate() {
            let t = if maximum == minimum {
                0.5
            } else {
                ((height - minimum) / (maximum - minimum)).clamp(0., 1.)
            };
            let light = shade(g, h, i);
            let c = ((80. + 170. * t) * light).round() as u8;
            write!(svg,"<rect x=\"{:.4}\" y=\"{:.4}\" width=\"{:.4}\" height=\"{:.4}\" fill=\"#{c:02x}{c:02x}{c:02x}\"/>",(i%nx) as f64*scale,(ny-1-i/nx) as f64*scale,scale+0.001,scale+0.001).unwrap();
        }
        svg.push_str("</g>");
        if let Some(r) = routing {
            svg.push_str(
                "<g fill=\"none\" stroke=\"#46b8d7\" stroke-width=\"0.8\" opacity=\"0.65\">",
            );
            for i in 0..h.len() {
                if r.areas()[i] < 5_000_000. {
                    continue;
                }
                for (j, weight) in r.branches(i) {
                    let j = j as usize;
                    if j == i || weight == 0. {
                        continue;
                    }
                    let opacity = if weight < 1. {
                        format!(" opacity=\"{weight:.5}\"")
                    } else {
                        String::new()
                    };
                    write!(
                        svg,
                        "<path{opacity} d=\"M {:.4} {:.4} L {:.4} {:.4}\"/>",
                        ((i % nx) as f64 + 0.5) * scale,
                        ((ny - 1 - i / nx) as f64 + 0.5) * scale,
                        ((j % nx) as f64 + 0.5) * scale,
                        ((ny - 1 - j / nx) as f64 + 0.5) * scale
                    )
                    .unwrap();
                }
            }
            svg.push_str("</g>");
        }
        svg.push_str("</g>");
    }
    write!(svg,"<text x=\"24\" y=\"558\" font-size=\"13\">50km chart extent · {:.0}m chart spacing · common elevation range {minimum:.1}–{maximum:.1}m</text>",g.settings.spacing_meters).unwrap();
    svg.push_str("<text x=\"24\" y=\"580\" font-size=\"13\">Actual bed / slope lighting1x. Cyan: static contributing-area links, not water discharge. Fixed outer collar is not ocean.</text></svg>");
    svg
}
fn raw_fields<R>(
    path: &Path,
    initial: &[f64],
    u: &[f64],
    p: &Preparation<R>,
) -> Result<(), String> {
    let mut f = std::io::BufWriter::new(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?,
    );
    for i in 0..initial.len() {
        for v in [
            initial[i],
            u[i],
            p.elevation_meters[i],
            p.accepted_uplift_displacement_meters[i],
            p.removed_incision_thickness_meters[i],
            p.signed_hillslope_height_change_meters[i],
        ] {
            f.write_all(&v.to_le_bytes()).map_err(|e| e.to_string())?;
        }
    }
    f.flush().map_err(|e| e.to_string())
}
fn profiles<A, B>(g: &Geometry, h: &[f64], a: &Preparation<A>, b: &Preparation<B>) -> Value {
    let nx = g.settings.cells_x as usize;
    let ny = g.settings.cells_y as usize;
    let sample = |i: usize, x: f64| json!({"chartDistanceMeters":x,"initialMeters":h[i],"incisionOnlyMeters":a.elevation_meters[i],"coupledMeters":b.elevation_meters[i]});
    json!({"horizontal":(0..nx).map(|x|sample((ny/2)*nx+x,(x as f64+0.5-nx as f64/2.)*g.settings.spacing_meters)).collect::<Vec<_>>(),
        "vertical":(0..ny).map(|y|sample(y*nx+nx/2,(y as f64+0.5-ny as f64/2.)*g.settings.spacing_meters)).collect::<Vec<_>>()})
}
/// Diagnostic point queries, not conservative restriction or a coupling map.
fn interpolate(g: &Geometry, h: &[f64], x: f64, y: f64) -> f64 {
    let nx = g.settings.cells_x as usize;
    let ny = g.settings.cells_y as usize;
    let sample = |p: f64, n: usize| {
        let t = (p / g.settings.spacing_meters + n as f64 / 2. - 0.5).clamp(0., (n - 1) as f64);
        let i = (t.floor() as usize).min(n - 2);
        (i, t - i as f64)
    };
    let (col, u) = sample(x, nx);
    let (row, v) = sample(y, ny);
    let low = h[row * nx + col] * (1. - u) + h[row * nx + col + 1] * u;
    let high = h[(row + 1) * nx + col] * (1. - u) + h[(row + 1) * nx + col + 1] * u;
    low * (1. - v) + high * v
}
fn common_support(g: &Geometry, h: &[f64]) -> Value {
    let coordinates: Vec<_> = (-80..=80).map(|i| f64::from(i) * 250.).collect();
    let horizontal: Vec<_> = coordinates
        .iter()
        .map(|&x| interpolate(g, h, x, 0.))
        .collect();
    let vertical: Vec<_> = coordinates
        .iter()
        .map(|&y| interpolate(g, h, 0., y))
        .collect();
    let differences: Vec<_> = horizontal
        .windows(5)
        .chain(vertical.windows(5))
        .map(|w| (w[4] - w[0]) / 1000.)
        .collect();
    json!({"interpolation":"bilinear-cell-center-diagnostic-1","chartCoordinatesMeters":coordinates,
        "horizontalHeightsMeters":horizontal,"verticalHeightsMeters":vertical,
        "roughnessDistanceMeters":1000.,"transectRmsSlopeAt1km":(differences.iter().map(|v|v*v).sum::<f64>()/differences.len() as f64).sqrt(),
        "scope":"Common central40km transects,250m query interval. Derived display/diagnostic queries only, not additional physical samples, channel/storage-preserving aggregation or a global coupling map."})
}
fn seed_summary(samples: &[(f64, f64, f64)]) -> Result<Value, String> {
    if samples.is_empty()
        || samples.iter().any(|&(a, b, area)| {
            !a.is_finite() || !b.is_finite() || !area.is_finite() || area <= 0.
        })
    {
        return Err("Invalid physical seed diagnostic samples.".into());
    }
    let area: f64 = samples.iter().map(|v| v.2).sum();
    let summary = |legacy: bool| {
        let value = |v: &(f64, f64, f64)| if legacy { v.0 } else { v.1 };
        let mean = samples.iter().map(|v| value(v) * v.2).sum::<f64>() / area;
        let variance = samples
            .iter()
            .map(|v| (value(v) - mean).powi(2) * v.2)
            .sum::<f64>()
            / area;
        json!({"areaWeightedMeanMeters":mean,"areaWeightedStandardDeviationMeters":variance.sqrt(),
            "minimumMeters":samples.iter().map(value).fold(f64::INFINITY,f64::min),
            "maximumMeters":samples.iter().map(value).fold(f64::NEG_INFINITY,f64::max)})
    };
    Ok(
        json!({"sampleCount":samples.len(),"areaSquareMeters":area,"legacy":summary(true),"physical":summary(false),
        "scope":"Recorded initial interior seed values; not matched RMS or a single-frequency intervention."}),
    )
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let Arguments {
        directory,
        seed,
        steps,
        duration,
        diffusion,
        facets,
        physical_seeds,
        physical_amplitude,
    } = parse(&args)?;
    let field = fixed_geological_field::Field::new(reference(&seed))?;
    let perturb = Perturbation::new(&seed);
    let physical_field = physical_seeds
        .then(|| {
            physical_seed_field::Field::new(
                &seed,
                physical_seed_field::Settings {
                    radius_meters: 6371000.,
                    amplitude_meters: physical_amplitude,
                    length_scale: 1.,
                },
            )
        })
        .transpose()?;
    let candidates = Surface::build(3, 6371000.);
    let mut chosen = [1., 0., 0.];
    let mut highest = f64::NEG_INFINITY;
    for &p in &candidates.centers {
        let h = field.sample(p)?.elevation_meters;
        if h > highest {
            highest = h;
            chosen = p;
        }
    }
    std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
    write_json(
        &directory.join("sources.json"),
        &json!({"sourceIdentity":field.source_identity(),"envelopeModelVersion":field.envelope_model_version(),"sources":field.sources()}),
    )?;
    let mut rows = Vec::new();
    for (case, center) in [("synthetic", [1., 0., 0.]), ("generated", chosen)] {
        for cells in [50u32, 100, 200] {
            let grid = terrain_patch_geometry::Settings {
                radius_meters: 6371000.,
                center_direction: center,
                orientation_radians: 0.,
                cells_x: cells,
                cells_y: cells,
                spacing_meters: 50000. / f64::from(cells),
            };
            let started = Instant::now();
            let g = terrain_patch_geometry::build(grid)?;
            let mut initial = Vec::new();
            let mut rates = Vec::new();
            let mut seed_samples = Vec::new();
            for (i, &p) in g.cell_directions.iter().enumerate() {
                let s = field.sample(p)?;
                let fixed = g.fixed_external_boundary[i];
                let legacy = perturb.sample(p);
                // Hold the existing external boundary heights exactly fixed
                // across initial-field controls, not just the collar location.
                let noise = if !fixed && let Some(field) = &physical_field {
                    field.sample(p)?
                } else {
                    legacy
                };
                if physical_seeds && !fixed {
                    seed_samples.push((legacy, noise, g.reference_areas_square_meters[i]));
                }
                initial.push(if case == "synthetic" {
                    if fixed { 0. } else { 1. + noise }
                } else {
                    s.elevation_meters + noise
                });
                rates.push(if fixed {
                    0.
                } else if case == "synthetic" {
                    0.0005
                } else {
                    1e-6 * 2000. * s.convergence_meters / 6000.
                });
            }
            let mut prepared = Vec::new();
            let mut variants = Vec::new();
            for (label, diffusivity) in [("incision", 0.), ("coupled", diffusion)] {
                let begin = Instant::now();
                let p = terrain_patch_landscape::prepare(
                    &g,
                    &initial,
                    &rates,
                    Settings {
                        erodibility_per_year: 1e-6,
                        diffusivity_square_meters_per_year: diffusivity,
                        step_years: duration / steps as f64,
                        steps,
                    },
                )?;
                let seconds = begin.elapsed().as_secs_f64();
                let file = format!("{case}-{cells}-{label}.f64le");
                raw_fields(&directory.join(&file), &initial, &rates, &p)?;
                let record = record(&g, &p, label, &file, seconds);
                println!(
                    "{case} {cells} {label}: closed {:.2}%, sinks{}, bedmax{:.1}m, {seconds:.3}s",
                    p.final_routing.closed_fraction_of_active_area * 100.,
                    p.final_routing.closed_sink_count,
                    p.elevation_meters
                        .iter()
                        .copied()
                        .fold(f64::NEG_INFINITY, f64::max)
                );
                variants.push(record);
                prepared.push(p);
            }
            let facet_prepared = if facets {
                let begin = Instant::now();
                let p = terrain_patch_landscape::prepare_facets(
                    &g,
                    &initial,
                    &rates,
                    Settings {
                        erodibility_per_year: 1e-6,
                        diffusivity_square_meters_per_year: diffusion,
                        step_years: duration / steps as f64,
                        steps,
                    },
                )?;
                let seconds = begin.elapsed().as_secs_f64();
                let file = format!("{case}-{cells}-facets.f64le");
                raw_fields(&directory.join(&file), &initial, &rates, &p)?;
                variants.push(record(&g, &p, "facets", &file, seconds));
                println!(
                    "{case} {cells} facets: closed {:.2}%, sinks{}, limited{}, {seconds:.3}s",
                    p.final_routing.closed_fraction_of_active_area * 100.,
                    p.final_routing.closed_sink_count,
                    p.receiver_constraint.as_ref().unwrap().limited_node_updates
                );
                Some(p)
            } else {
                None
            };
            let svgfile = format!("{case}-{cells}.svg");
            write_new(
                &directory.join(&svgfile),
                atlas(
                    &g,
                    &initial,
                    &prepared[0],
                    &prepared[1],
                    facet_prepared.as_ref(),
                )
                .as_bytes(),
            )?;
            let mut row = json!({"case":case,"geometryModelVersion":g.model_version,"geometrySettings":grid,"referenceRecipe":field.reference_recipe(),
                "geologicalFieldModelVersion":fixed_geological_field::MODEL_VERSION,"sourceIdentity":field.source_identity(),"sourceGeometryFile":"sources.json",
                "initialPerturbation":{"modelVersion":"fine-patch-seed-perturbations-1","amplitudeBoundMeters":1.,"sphericalWaveLengthMeters":4000.,"modeCount":6,"radiusMeters":6371000.},
                "selectionRule":if case=="generated" {"Highest fixed-field initial bed among subdivision3 global directions; first ID breaks ties. No water fit."}else{"Controlled uniform uplift, fixed zero boundary collar and bounded initial perturbations."},
                "variants":variants,"transects":profiles(&g,&initial,&prepared[0],&prepared[1]),"atlasFile":svgfile,"totalSeconds":started.elapsed().as_secs_f64()});
            if let Some(p) = &facet_prepared {
                row["facetTransects"] = profiles(&g, &initial, &prepared[0], p);
            }
            if let Some(field) = &physical_field {
                let legacy = row["initialPerturbation"].clone();
                row["initialPerturbation"] =
                    serde_json::to_value(field.identity()).map_err(|e| e.to_string())?;
                row["initialPerturbation"]["boundaryPolicy"] = json!(if case == "synthetic" {
                    "Fixed zero collar; spectrum on active interior."
                } else {
                    "Legacy boundary perturbation retained exactly; spectrum on active interior."
                });
                row["initialPerturbation"]["legacyBoundaryPerturbation"] = legacy;
                row["initialPerturbation"]["sampledInteriorSummary"] = seed_summary(&seed_samples)?;
            }
            write_json(&directory.join(format!("{case}-{cells}.json")), &row)?;
            rows.push(row);
        }
    }
    write_json(
        &directory.join("manifest.json"),
        &json!({"reportVersion":if physical_seeds {"fine-landscape-review-3"}else if facets {"fine-landscape-review-2"}else{"fine-landscape-review-1"},"durationYears":duration,"diffusivitySquareMetersPerYear":diffusion,"rows":rows,
        "fieldRecord":{"format":"headerless-f64-le-region-major-1","columns":["initialBedMeters","prescribedUpliftMetersPerYear","preparedBedMeters","acceptedUpliftDisplacementMeters","removedIncisionThicknessMeters","signedHillslopeHeightChangeMeters"]},
        "scope":"Bounded actual fine-bed experiments. Fixed outer cell-center collar is an external height reservoir, not ocean. Detached material is in a bookkeeping reservoir, not deposition or material proven to leave through an outlet. Paired lateral transfers and external exchange accounted; no global water, climate or sediment stock advanced. Equal outer chart extents; active collar positions vary with spacing. Native records are not World/checkpoints or camera-driven display detail. Manifest written last."}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refusals_precede_output_creation_and_perturbations_are_bounded_stateless() {
        for args in [
            vec![],
            vec!["."],
            vec!["fine-uncreated", " "],
            vec!["fine-uncreated", "seed", "0"],
            vec!["fine-uncreated", "seed", "129"],
            vec!["fine-uncreated", "seed", "64", "NaN"],
            vec!["fine-uncreated", "seed", "64", "4800000", "-1"],
            vec!["fine-uncreated", "seed", "64", "4800000", "1", "unknown"],
            vec![
                "fine-uncreated",
                "seed",
                "64",
                "4800000",
                "1",
                "--facets",
                "100",
            ],
            vec![
                "fine-uncreated",
                "seed",
                "64",
                "4800000",
                "1",
                "--spectrum",
                "NaN",
            ],
            vec![
                "fine-uncreated",
                "seed",
                "64",
                "4800000",
                "1",
                "--spectrum",
                "1001",
            ],
        ] {
            assert!(parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        let p = Perturbation::new("seed");
        let q = Perturbation::new("seed");
        for x in Surface::build(2, 1.).centers {
            let a = p.sample(x);
            assert!(a.abs() <= 1.);
            assert_eq!(a, q.sample(x));
            assert_eq!(a, p.sample(x));
        }
        assert_eq!(
            parse(&["fine-review-uncreated-unit".into()]).unwrap().steps,
            64
        );
        let args = [
            "fine-review-uncreated-unit",
            "seed",
            "32",
            "4800000",
            "0.01",
            "--facets",
        ]
        .map(str::to_owned);
        let parsed = parse(&args).unwrap();
        assert!(parsed.facets && !parsed.physical_seeds);
        let mut args = args;
        args[5] = "--spectrum".into();
        let parsed = parse(&args).unwrap();
        assert!(parsed.facets && parsed.physical_seeds);
        assert_eq!(parsed.physical_amplitude, 1.);
        let mut args = args.to_vec();
        args.push("100".into());
        assert_eq!(parse(&args).unwrap().physical_amplitude, 100.);
    }
    #[test]
    fn common_support_queries_reproduce_an_independent_affine_chart() {
        let g = terrain_patch_geometry::build(terrain_patch_geometry::Settings {
            radius_meters: 6371000.,
            center_direction: [1., 0., 0.],
            orientation_radians: 0.,
            cells_x: 50,
            cells_y: 50,
            spacing_meters: 1000.,
        })
        .unwrap();
        let h: Vec<_> = (0..2500)
            .map(|i| {
                let x = (i % 50) as f64 * 1000. - 24500.;
                let y = (i / 50) as f64 * 1000. - 24500.;
                10. + x / 1000. + y / 500.
            })
            .collect();
        for x in [-20000., -125., 0., 9000., 20000.] {
            for y in [-20000., 0., 125., 20000.] {
                assert!((interpolate(&g, &h, x, y) - (10. + x / 1000. + y / 500.)).abs() < 1e-12);
            }
        }
        let v = common_support(&g, &h);
        assert!((v["transectRmsSlopeAt1km"].as_f64().unwrap() - (2.5e-6_f64).sqrt()).abs() < 1e-14);
    }

    #[test]
    fn physical_seed_diagnostics_have_known_area_weighted_means_and_variance() {
        let s = seed_summary(&[(0., 1., 1.), (2., -1., 3.)]).unwrap();
        assert_eq!(s["legacy"]["areaWeightedMeanMeters"], 1.5);
        assert_eq!(s["physical"]["areaWeightedMeanMeters"], -0.5);
        assert!(
            (s["physical"]["areaWeightedStandardDeviationMeters"]
                .as_f64()
                .unwrap()
                - 0.75_f64.sqrt())
            .abs()
                < 1e-14
        );
        assert!(seed_summary(&[]).is_err());
        assert!(seed_summary(&[(0., f64::NAN, 1.)]).is_err());
    }
}
