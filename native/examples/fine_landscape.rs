//! Actual fine-bed comparisons; regional experiments, not coupled global tiles.
mod fine_record;
use planimulation_core::{
    Random, Recipe, Surface, fixed_geological_field, humid_patch, physical_seed_field,
    terrain_patch_facets,
    terrain_patch_geometry::{self, Geometry},
    terrain_patch_landscape::{self, PowerSettings, Preparation, Settings},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fmt::Write as _,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
const DURATION_YEARS: f64 = 4_800_000.;
trait ReviewFlow {
    fn audit_metadata(&self) -> Option<Value> {
        None
    }
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
impl ReviewFlow for humid_patch::Routing {
    fn audit_metadata(&self) -> Option<Value> {
        Some(
            json!({"modelVersion":self.model_version,"assumption":self.assumption,
            "potentialGeometry":self.potential.summary,
            "actualBedClosedFractionOfActiveArea":self.actual_bed_routing.closed_fraction_of_active_area,
            "proxyClosedFractionOfActiveArea":self.proxy_routing.closed_fraction_of_active_area,
            "proxyTerminalAreaRelativeResidual":self.proxy_routing.terminal_area_relative_residual,
            "scope":"Height/closed-basin metrics use actual bed. Cyan links use potential-lake proxy ancestry under the explicit humid assumption; connectivity is imposed, not evidence of physical water supply."}),
        )
    }
    fn closed_count(&self) -> usize {
        self.actual_bed_routing.closed_sink_count
    }
    fn closed_fraction(&self) -> f64 {
        self.actual_bed_routing.closed_fraction_of_active_area
    }
    fn areas(&self) -> &[f64] {
        &self.proxy_routing.contributing_area_square_meters
    }
    fn branches(&self, i: usize) -> [(u32, f64); 2] {
        self.proxy_routing.branches(i)
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
    humid: bool,
    reference_law: bool,
    scaled_regime: bool,
}
struct Profile {
    extent_meters: f64,
    cells: [u32; 3],
    seed_length_scale: f64,
    power_coefficient: f64,
}
fn profile(scaled: bool) -> Profile {
    if scaled {
        Profile {
            extent_meters: 20000.,
            cells: [40, 80, 160],
            seed_length_scale: 0.25,
            power_coefficient: 500_f64.powf(-1.8),
        }
    } else {
        Profile {
            extent_meters: 50000.,
            cells: [50, 100, 200],
            seed_length_scale: 1.,
            power_coefficient: 2e-4,
        }
    }
}
fn parse(args: &[String]) -> Result<Arguments, String> {
    let scaled = args.get(5).is_some_and(|s| s == "--scaled-regime");
    if !(1..=7).contains(&args.len())
        || args[0].trim().is_empty()
        || args.get(5).is_some_and(|s| {
            !matches!(
                s.as_str(),
                "--facets" | "--spectrum" | "--humid" | "--reference-law" | "--scaled-regime"
            )
        })
        || args.len() == 7
            && !matches!(
                args[5].as_str(),
                "--spectrum" | "--humid" | "--reference-law" | "--scaled-regime"
            )
    {
        return Err(
            "Usage: fine_landscape NEW_DIRECTORY [SEED] [STEPS(1..128; scaled profile:1..256)] [DURATION_YEARS] [DIFFUSIVITY_M2_PER_YEAR] [--facets|--spectrum [AMPLITUDE_METERS]|--humid [AMPLITUDE_METERS]|--reference-law [AMPLITUDE_METERS]|--scaled-regime [AMPLITUDE_METERS]]".into(),
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
        .unwrap_or(if scaled { 128 } else { 64 });
    if !(1..=if scaled { 256 } else { 128 }).contains(&steps) {
        if scaled {
            return Err("Scaled-regime steps must be 1–256.".into());
        }
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
    if scaled && diffusion != 1. {
        return Err("The scaled-regime control requires D=1 m2/year.".into());
    }
    let amplitude = args
        .get(6)
        .map(|s| s.parse::<f64>())
        .transpose()
        .map_err(|_| "Invalid physical seed amplitude.")?
        .unwrap_or(if scaled { 10. } else { 1. });
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
        physical_seeds: args.get(5).is_some_and(|s| {
            matches!(
                s.as_str(),
                "--spectrum" | "--humid" | "--reference-law" | "--scaled-regime"
            )
        }),
        physical_amplitude: amplitude,
        humid: args.get(5).is_some_and(|s| {
            matches!(
                s.as_str(),
                "--humid" | "--reference-law" | "--scaled-regime"
            )
        }),
        reference_law: args
            .get(5)
            .is_some_and(|s| matches!(s.as_str(), "--reference-law" | "--scaled-regime")),
        scaled_regime: scaled,
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
fn metrics<R: ReviewFlow, S>(g: &Geometry, h: &[f64], p: &Preparation<R, S>) -> Value {
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
fn record<R: ReviewFlow, S: Serialize>(
    g: &Geometry,
    p: &Preparation<R, S>,
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
    if let Some(info) = p.final_routing.audit_metadata() {
        record["humidRouting"] = info;
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
struct PowerPair {
    dry: Preparation<terrain_patch_facets::Routing, PowerSettings>,
    humid: Preparation<humid_patch::Routing, PowerSettings>,
    scaled: bool,
}
type Panel<'a> = (&'a str, &'a [f64], Option<&'a dyn ReviewFlow>);
fn power_panels(p: &PowerPair) -> Vec<Panel<'_>> {
    vec![
        (
            if p.scaled {
                "Scaled regime m=.4 / dry bed"
            } else {
                "Reference law m=.4 / dry bed"
            },
            &p.dry.elevation_meters,
            Some(&p.dry.final_routing),
        ),
        (
            if p.scaled {
                "Scaled regime m=.4 / humid bed"
            } else {
                "Reference law m=.4 / humid bed"
            },
            &p.humid.elevation_meters,
            Some(&p.humid.final_routing),
        ),
    ]
}
fn atlas(
    g: &Geometry,
    initial: &[f64],
    a: &Preparation,
    b: &Preparation,
    facets: Option<&Preparation<terrain_patch_facets::Routing>>,
    humid: Option<&Preparation<humid_patch::Routing>>,
    power: Option<&PowerPair>,
) -> String {
    let minimum = initial.iter().copied().fold(f64::INFINITY, f64::min);
    let mut maximum = initial
        .iter()
        .chain(&a.elevation_meters)
        .chain(&b.elevation_meters)
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
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
    if let Some(p) = humid {
        maximum = maximum.max(
            p.elevation_meters
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max),
        );
        panels.push((
            "Humid prehistory / actual bed",
            &p.elevation_meters,
            Some(&p.final_routing),
        ));
    }
    if let Some(p) = power {
        for (label, h, flow) in power_panels(p) {
            maximum = maximum.max(h.iter().copied().fold(f64::NEG_INFINITY, f64::max));
            panels.push((label, h, flow));
        }
    }
    render_atlas(g, minimum, maximum, panels, humid.is_some())
}
fn render_atlas(
    g: &Geometry,
    minimum: f64,
    maximum: f64,
    panels: Vec<Panel<'_>>,
    humid: bool,
) -> String {
    let nx = g.settings.cells_x as usize;
    let ny = g.settings.cells_y as usize;
    let scale = 480. / nx as f64;
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
    write!(svg,"<text x=\"24\" y=\"558\" font-size=\"13\">{:.0}km chart extent · {:.0}m chart spacing · common elevation range {minimum:.1}–{maximum:.1}m</text>",f64::from(g.settings.cells_x)*g.settings.spacing_meters/1000.,g.settings.spacing_meters).unwrap();
    svg.push_str("<text x=\"24\" y=\"580\" font-size=\"13\">Actual bed / slope lighting1x. Cyan: static contributing-area links, not water discharge. Fixed outer collar is not ocean.</text>");
    if humid {
        svg.push_str("<text x=\"24\" y=\"600\" font-size=\"12\">Humid panel links assume fully supplied potential lakes; lake levels are NOT actual bed or funded water inventories.</text>");
    }
    svg.push_str("</svg>");
    svg
}
fn raw_fields<R, S>(
    path: &Path,
    initial: &[f64],
    u: &[f64],
    p: &Preparation<R, S>,
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
fn profiles<A, B, SA, SB>(
    g: &Geometry,
    h: &[f64],
    a: &Preparation<A, SA>,
    b: &Preparation<B, SB>,
) -> Value {
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
    let small = f64::from(g.settings.cells_x) * g.settings.spacing_meters == 20000.;
    let (half, interval, stride) = if small { (64, 125., 8) } else { (80, 250., 4) };
    let coordinates: Vec<_> = (-half..=half).map(|i| f64::from(i) * interval).collect();
    let horizontal: Vec<_> = coordinates
        .iter()
        .map(|&x| interpolate(g, h, x, 0.))
        .collect();
    let vertical: Vec<_> = coordinates
        .iter()
        .map(|&y| interpolate(g, h, 0., y))
        .collect();
    let differences: Vec<_> = horizontal
        .windows(stride + 1)
        .chain(vertical.windows(stride + 1))
        .map(|w| (w[stride] - w[0]) / 1000.)
        .collect();
    json!({"interpolation":"bilinear-cell-center-diagnostic-1","chartCoordinatesMeters":coordinates,
        "horizontalHeightsMeters":horizontal,"verticalHeightsMeters":vertical,
        "roughnessDistanceMeters":1000.,"transectRmsSlopeAt1km":(differences.iter().map(|v|v*v).sum::<f64>()/differences.len() as f64).sqrt(),
        "scope":format!("Common central{}km transects,{interval:.0}m query interval. Derived display/diagnostic queries only, not additional physical samples, channel/storage-preserving aggregation or a global coupling map.",if small {16}else{40})})
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
fn regime_scales(s: PowerSettings, chart_spacing: f64) -> Value {
    if s.erodibility_coefficient == 0. || s.diffusivity_square_meters_per_year == 0. {
        return json!({"defined":false,"reason":"Positive K and D are required for these competition scales."});
    }
    let length = (s.diffusivity_square_meters_per_year / s.erodibility_coefficient)
        .powf(1. / (2. * s.area_exponent + 1.));
    // This equivalent time form avoids squaring a tiny length before division.
    let time = length.powf(1. - 2. * s.area_exponent) / s.erodibility_coefficient;
    json!({"defined":true,"characteristicLengthMeters":length,"characteristicTimeYears":time,
        "chartSpacingOverCharacteristicLength":chart_spacing/length,"stepOverCharacteristicTime":s.step_years/time,
        "durationOverCharacteristicTime":s.step_years*s.steps as f64/time,
        "scope":"Dimensional scales for E=K A^m S and linear diffusion; not proof of resolved features or convergence. Spatially varying uplift has no single characteristic height."})
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
        humid,
        reference_law,
        scaled_regime,
    } = parse(&args)?;
    let profile = profile(scaled_regime);
    let field = fixed_geological_field::Field::new(reference(&seed))?;
    let perturb = Perturbation::new(&seed);
    let physical_field = physical_seeds
        .then(|| {
            physical_seed_field::Field::new(
                &seed,
                physical_seed_field::Settings {
                    radius_meters: 6371000.,
                    amplitude_meters: physical_amplitude,
                    length_scale: profile.seed_length_scale,
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
        for cells in profile.cells {
            let grid = terrain_patch_geometry::Settings {
                radius_meters: 6371000.,
                center_direction: center,
                orientation_radians: 0.,
                cells_x: cells,
                cells_y: cells,
                spacing_meters: profile.extent_meters / f64::from(cells),
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
            let humid_prepared = if humid {
                let begin = Instant::now();
                let p = terrain_patch_landscape::prepare_humid_facets(
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
                let file = format!("{case}-{cells}-humid.f64le");
                raw_fields(&directory.join(&file), &initial, &rates, &p)?;
                variants.push(record(&g, &p, "humid", &file, seconds));
                println!(
                    "{case} {cells} humid: actual bed closed {:.2}%, potential lake nodes{}, {seconds:.3}s",
                    p.final_routing.closed_fraction() * 100.,
                    p.final_routing.potential.summary.positive_depth_nodes
                );
                Some(p)
            } else {
                None
            };
            let power_prepared = if reference_law {
                let controls = PowerSettings {
                    erodibility_coefficient: profile.power_coefficient,
                    area_exponent: 0.4,
                    diffusivity_square_meters_per_year: diffusion,
                    step_years: duration / steps as f64,
                    steps,
                };
                let begin = Instant::now();
                let dry =
                    terrain_patch_landscape::prepare_facets_power(&g, &initial, &rates, controls)?;
                let seconds = begin.elapsed().as_secs_f64();
                let file = format!("{case}-{cells}-power.f64le");
                raw_fields(&directory.join(&file), &initial, &rates, &dry)?;
                variants.push(record(&g, &dry, "power", &file, seconds));
                let begin = Instant::now();
                let humid = terrain_patch_landscape::prepare_humid_facets_power(
                    &g, &initial, &rates, controls,
                )?;
                let seconds = begin.elapsed().as_secs_f64();
                let file = format!("{case}-{cells}-humid-power.f64le");
                raw_fields(&directory.join(&file), &initial, &rates, &humid)?;
                variants.push(record(&g, &humid, "humid-power", &file, seconds));
                println!(
                    "{case} {cells} {}: dry closed {:.2}%, humid bed closed {:.2}%",
                    if scaled_regime {
                        "scaled regime"
                    } else {
                        "reference law"
                    },
                    dry.final_routing.closed_fraction() * 100.,
                    humid.final_routing.closed_fraction() * 100.
                );
                Some(PowerPair {
                    dry,
                    humid,
                    scaled: scaled_regime,
                })
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
                    humid_prepared.as_ref(),
                    power_prepared.as_ref(),
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
            if let Some(p) = &humid_prepared {
                row["humidTransects"] = profiles(&g, &initial, &prepared[0], p);
            }
            if let Some(p) = &power_prepared {
                let panels = power_panels(p);
                let minimum = panels
                    .iter()
                    .flat_map(|p| p.1)
                    .copied()
                    .fold(f64::INFINITY, f64::min);
                let maximum = panels
                    .iter()
                    .flat_map(|p| p.1)
                    .copied()
                    .fold(f64::NEG_INFINITY, f64::max);
                let file = format!("{case}-{cells}-power.svg");
                write_new(
                    &directory.join(&file),
                    render_atlas(&g, minimum, maximum, panels, true).as_bytes(),
                )?;
                row["powerAtlasFile"] = json!(file);
                row["powerTransects"] = profiles(&g, &initial, &p.dry, &p.humid);
                row["powerRegimeScales"] = regime_scales(p.dry.settings, g.settings.spacing_meters);
                row["powerLaw"] = json!({"incisionModelVersion":planimulation_core::facet_incision::POWER_MODEL_VERSION,
                    "areaExponent":0.4,"slopeExponent":1,"erodibilityCoefficient":profile.power_coefficient,"erodibilityUnits":"meters^0.2/year",
                    "scope":if scaled_regime {"Scaled process-regime control. New extent, physical seed spectrum and K; not an isolated coefficient intervention or a qualified World."}else{"Reference incision-law pair only. Initial bed, uplift, duration, boundary and D are held fixed; not a literal reproduction of the published numerical setup."}});
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
            let row_name = format!("{case}-{cells}.json");
            if scaled_regime {
                row["comparisonProfile"] = json!("scaled-process-regime-1");
            }
            write_json(&directory.join(&row_name), &row)?;
            rows.push(if reference_law {
                fine_record::reference(&row_name, &row)?
            } else {
                row
            });
        }
    }
    let mut manifest = json!({"reportVersion":if scaled_regime {"fine-landscape-review-6"}else if reference_law {"fine-landscape-review-5"}else if humid {"fine-landscape-review-4"}else if physical_seeds {"fine-landscape-review-3"}else if facets {"fine-landscape-review-2"}else{"fine-landscape-review-1"},"durationYears":duration,"diffusivitySquareMetersPerYear":diffusion,"rows":rows,
        "fieldRecord":{"format":"headerless-f64-le-region-major-1","columns":["initialBedMeters","prescribedUpliftMetersPerYear","preparedBedMeters","acceptedUpliftDisplacementMeters","removedIncisionThicknessMeters","signedHillslopeHeightChangeMeters"]},
        "scope":"Bounded actual fine-bed experiments. Fixed outer cell-center collar is an external height reservoir, not ocean. Detached material is in a bookkeeping reservoir, not deposition or material proven to leave through an outlet. Paired lateral transfers and external exchange accounted; no global water, climate or sediment stock advanced. Equal outer chart extents; active collar positions vary with spacing. Native records are not World/checkpoints or camera-driven display detail. Manifest written last."});
    if reference_law {
        manifest["rowRecords"] = manifest.as_object_mut().unwrap().remove("rows").unwrap();
        manifest["rowIndexFormat"] = json!(fine_record::INDEX_FORMAT);
    }
    if scaled_regime {
        manifest["comparisonProfile"] = json!("scaled-process-regime-1");
    }
    write_json(&directory.join("manifest.json"), &manifest)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaled_profile_resolves_declared_lengths_and_keeps_legacy_controls_separate() {
        let args = [
            "fine-scaled-uncreated-unit",
            "seed",
            "128",
            "4800000",
            "1",
            "--scaled-regime",
        ]
        .map(str::to_owned);
        let parsed = parse(&args).unwrap();
        assert!(
            parsed.scaled_regime && parsed.reference_law && parsed.physical_seeds && parsed.humid
        );
        assert_eq!(parsed.physical_amplitude, 10.);
        let p = profile(parsed.scaled_regime);
        assert_eq!(p.cells, [40, 80, 160]);
        assert_eq!(p.extent_meters / f64::from(p.cells[2]), 125.);
        assert_eq!(p.seed_length_scale, 0.25);
        let s = regime_scales(
            PowerSettings {
                erodibility_coefficient: p.power_coefficient,
                area_exponent: 0.4,
                diffusivity_square_meters_per_year: 1.,
                step_years: 37500.,
                steps: 128,
            },
            125.,
        );
        assert!((s["characteristicLengthMeters"].as_f64().unwrap() - 500.).abs() < 1e-10);
        assert!((s["characteristicTimeYears"].as_f64().unwrap() - 250000.).abs() < 1e-8);
        assert!((s["chartSpacingOverCharacteristicLength"].as_f64().unwrap() - 0.25).abs() < 1e-14);
        assert!((s["stepOverCharacteristicTime"].as_f64().unwrap() - 0.15).abs() < 1e-14);
        let mut variants = args.to_vec();
        variants[2] = "256".into();
        assert!(parse(&variants).is_ok());
        variants[2] = "257".into();
        assert!(parse(&variants).is_err());
        variants[2] = "128".into();
        variants[4] = "0.1".into();
        assert!(parse(&variants).is_err());
        variants[4] = "1".into();
        variants[5] = "--reference-law".into();
        let legacy = parse(&variants).unwrap();
        assert!(!legacy.scaled_regime);
        assert_eq!(legacy.physical_amplitude, 1.);
        assert_eq!(profile(false).extent_meters, 50000.);
        assert_eq!(profile(false).power_coefficient, 2e-4);
        variants[2] = "256".into();
        assert!(parse(&variants).is_err());
    }
    #[test]
    fn scaled_common_transects_stay_inside_the_domain_and_use_actual_one_kilometer_differences() {
        let g = terrain_patch_geometry::build(terrain_patch_geometry::Settings {
            radius_meters: 6371000.,
            center_direction: [1., 0., 0.],
            orientation_radians: 0.,
            cells_x: 40,
            cells_y: 40,
            spacing_meters: 500.,
        })
        .unwrap();
        let h: Vec<_> = (0..1600)
            .map(|i| {
                let x = (i % 40) as f64 * 500. - 9750.;
                let y = (i / 40) as f64 * 500. - 9750.;
                10. + x / 1000. + y / 500.
            })
            .collect();
        let v = common_support(&g, &h);
        let coordinates = v["chartCoordinatesMeters"].as_array().unwrap();
        assert_eq!(coordinates.len(), 129);
        assert_eq!(coordinates[0], -8000.);
        assert_eq!(coordinates[128], 8000.);
        assert_eq!(
            coordinates[1].as_f64().unwrap() - coordinates[0].as_f64().unwrap(),
            125.
        );
        assert!((v["transectRmsSlopeAt1km"].as_f64().unwrap() - (2.5e-6_f64).sqrt()).abs() < 1e-14);
    }
    #[test]
    fn dimensional_competition_scales_recover_independent_half_and_two_fifths_powers() {
        let controls = PowerSettings {
            erodibility_coefficient: 1e-6,
            area_exponent: 0.5,
            diffusivity_square_meters_per_year: 1.,
            step_years: 1000.,
            steps: 10,
        };
        let a = regime_scales(controls, 250.);
        assert!((a["characteristicLengthMeters"].as_f64().unwrap() - 1000.).abs() < 1e-10);
        assert_eq!(a["characteristicTimeYears"], 1e6);
        let b = regime_scales(
            PowerSettings {
                erodibility_coefficient: 1.,
                area_exponent: 0.4,
                diffusivity_square_meters_per_year: 512.,
                step_years: 4.,
                ..controls
            },
            8.,
        );
        assert!((b["characteristicLengthMeters"].as_f64().unwrap() - 32.).abs() < 1e-12);
        assert!((b["characteristicTimeYears"].as_f64().unwrap() - 2.).abs() < 1e-12);
        assert!((b["chartSpacingOverCharacteristicLength"].as_f64().unwrap() - 0.25).abs() < 1e-14);
        assert!((b["stepOverCharacteristicTime"].as_f64().unwrap() - 2.).abs() < 1e-12);
        assert_eq!(
            regime_scales(
                PowerSettings {
                    diffusivity_square_meters_per_year: 0.,
                    ..controls
                },
                250.
            )["defined"],
            false
        );
    }
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
        args[5] = "--humid".into();
        let parsed = parse(&args).unwrap();
        assert!(parsed.facets && parsed.physical_seeds && parsed.humid);
        assert_eq!(parsed.physical_amplitude, 100.);
        args[5] = "--reference-law".into();
        let parsed = parse(&args).unwrap();
        assert!(parsed.reference_law && parsed.humid && parsed.physical_seeds);
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
