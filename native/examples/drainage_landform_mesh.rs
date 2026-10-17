//! Apply native construction to retained unstructured physical geometry.
//! External source-bed preparation remains explicitly external; no eroder is
//! imported or executed here. Receivers/area are recomputed, not trusted input.
use planimulation_core::drainage_landforms::{self as landforms, Edge, Settings};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fmt::Write as _,
    io::{Read, Write},
    path::Path,
    time::Instant,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    report_version: String,
    points_meters: Vec<[f64; 2]>,
    triangles: Vec<[u32; 3]>,
    fixed_boundary: Vec<bool>,
    reference_areas_square_meters: Vec<f64>,
    prepared_bed_meters: Vec<f64>,
    identity: Value,
    settings: Value,
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| e.to_string())
}
fn areas(source: &Source, receivers: &[u32]) -> Result<(Vec<f64>, f64, f64), String> {
    let n = receivers.len();
    let mut incoming = vec![0_usize; n];
    let mut c: Vec<_> = source
        .reference_areas_square_meters
        .iter()
        .zip(&source.fixed_boundary)
        .map(|(&a, &fixed)| if fixed { 0. } else { a })
        .collect();
    let total: f64 = c.iter().sum();
    let mut corrections = vec![0.; n];
    for (i, &r) in receivers.iter().enumerate() {
        if i != r as usize {
            incoming[r as usize] += 1;
        }
    }
    let mut order: Vec<_> = (0..n).filter(|&i| incoming[i] == 0).collect();
    let mut cursor = 0;
    while cursor < order.len() {
        let i = order[cursor];
        cursor += 1;
        let r = receivers[i] as usize;
        if r == i {
            continue;
        }
        let amount = c[i] - corrections[r];
        let next = c[r] + amount;
        corrections[r] = (next - c[r]) - amount;
        c[r] = next;
        incoming[r] -= 1;
        if incoming[r] == 0 {
            order.push(r);
        }
    }
    if order.len() != n {
        return Err("Cyclic actual mesh drainage.".into());
    }
    let terminal: f64 = (0..n)
        .filter(|&i| receivers[i] as usize == i)
        .map(|i| c[i])
        .sum();
    let closed: f64 = (0..n)
        .filter(|&i| receivers[i] as usize == i && !source.fixed_boundary[i])
        .map(|i| c[i])
        .fold(0., |sum, value| sum + value);
    let residual = (terminal - total).abs() / total;
    if !residual.is_finite() || residual > 1e-12 {
        return Err("Actual mesh area does not reconcile.".into());
    }
    Ok((c, closed / total, residual))
}

fn atlas(source: &Source, h: &[f64], receiver: &[u32], collected: &[f64]) -> (String, f64, f64) {
    let min_x = source
        .points_meters
        .iter()
        .map(|p| p[0])
        .fold(f64::INFINITY, f64::min);
    let max_x = source
        .points_meters
        .iter()
        .map(|p| p[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = source
        .points_meters
        .iter()
        .map(|p| p[1])
        .fold(f64::INFINITY, f64::min);
    let max_y = source
        .points_meters
        .iter()
        .map(|p| p[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let scale = 480. / (max_x - min_x).max(max_y - min_y);
    let point = |i: u32| {
        let p = source.points_meters[i as usize];
        [(p[0] - min_x) * scale, (max_y - p[1]) * scale]
    };
    let mut out = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1550\" height=\"620\"><rect width=\"1550\" height=\"620\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style>",
    );
    let mut maximum_gradient = 0_f64;
    let mut steep_area = 0.;
    let mut total_area = 0.;
    for (panel, (title, bed, links)) in [
        (
            "Retained external source bed",
            source.prepared_bed_meters.as_slice(),
            false,
        ),
        ("Native constructed physical bed", h, false),
        ("Native actual dry collection", h, true),
    ]
    .into_iter()
    .enumerate()
    {
        let origin = 24. + panel as f64 * 512.;
        let min = bed.iter().copied().fold(f64::INFINITY, f64::min);
        let max = bed.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        write!(out,"<text x=\"{origin}\" y=\"28\" font-size=\"18\">{title}</text><text x=\"{origin}\" y=\"49\" font-size=\"13\">Physical height {min:.1} to {max:.1} m</text><g transform=\"translate({origin},66)\">").unwrap();
        for &[ia, ib, ic] in &source.triangles {
            let [a, b, c] = [ia, ib, ic].map(|i| source.points_meters[i as usize]);
            let [v, w] = [[b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]];
            let det = v[0] * w[1] - v[1] * w[0];
            let da = bed[ib as usize] - bed[ia as usize];
            let db = bed[ic as usize] - bed[ia as usize];
            let gx = (da * w[1] - db * v[1]) / det;
            let gy = (v[0] * db - w[0] * da) / det;
            let gradient = (gx * gx + gy * gy).sqrt();
            if panel == 1 {
                maximum_gradient = maximum_gradient.max(gradient);
                total_area += det.abs() / 2.;
                if gradient > std::f64::consts::FRAC_PI_6.tan() * (1. + 1e-10) {
                    steep_area += det.abs() / 2.;
                }
            }
            let light =
                0.25 + 0.75 * ((gx - gy + 1.) / (3. * (gx * gx + gy * gy + 1.)).sqrt()).max(0.);
            let color = (255. * light).round() as u8;
            let [a, b, c] = [ia, ib, ic].map(point);
            write!(out,"<path d=\"M {:.4} {:.4} L {:.4} {:.4} L {:.4} {:.4} Z\" fill=\"#{color:02x}{color:02x}{color:02x}\"/>",a[0],a[1],b[0],b[1],c[0],c[1]).unwrap();
        }
        if links {
            out.push_str("<g stroke=\"#46b8d7\" stroke-width=\"0.65\" opacity=\"0.8\">");
            for i in 0..h.len() {
                let r = receiver[i];
                if r as usize != i && collected[i] >= 1e6 {
                    let a = point(i as u32);
                    let b = point(r);
                    write!(
                        out,
                        "<path d=\"M {:.4} {:.4} L {:.4} {:.4}\"/>",
                        a[0], a[1], b[0], b[1]
                    )
                    .unwrap();
                }
            }
            out.push_str("</g>");
        }
        out.push_str("</g>");
    }
    out.push_str("<text x=\"24\" y=\"574\" font-size=\"14\">Actual physical triangles / same normal lighting, slopes 1x / no height normalization or interpolation</text><text x=\"24\" y=\"599\" font-size=\"13\">Cyan: rebuilt actual dry source area above 1 km2, not discharge. Edge bounds do not bound triangle gradients. Not a global World.</text></svg>");
    (out, maximum_gradient, steep_area / total_area)
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: drainage_landform_mesh SOURCE_EXTERNAL_REPORT NEW_DIRECTORY".into());
    }
    let output = Path::new(&args[1]);
    if output.exists() {
        return Err("A new output directory is required.".into());
    }
    let mut data = Vec::new();
    std::fs::File::open(&args[0])
        .map_err(|e| e.to_string())?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > 32 * 1024 * 1024 {
        return Err("Source exceeds its byte bound.".into());
    }
    let source: Source = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    let n = source.points_meters.len();
    if source.report_version != "external-trimesh-review-1"
        || !(4..=40000).contains(&n)
        || source.prepared_bed_meters.len() != n
        || source.fixed_boundary.len() != n
        || source.reference_areas_square_meters.len() != n
        || source.fixed_boundary.iter().all(|v| !v)
        || source.fixed_boundary.iter().all(|v| *v)
        || source.triangles.is_empty()
        || source.triangles.len() > 80000
        || source
            .points_meters
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 1e6)
    {
        return Err("Unsupported source mesh fields.".into());
    }
    let mut pairs = BTreeSet::new();
    let mut triangle_keys = BTreeSet::new();
    let mut triangle_area = 0.;
    for &t in &source.triangles {
        if t.iter().any(|&i| i as usize >= n) {
            return Err("Invalid triangle index.".into());
        }
        let mut key = t;
        key.sort_unstable();
        if !triangle_keys.insert(key) {
            return Err("Duplicate source triangle.".into());
        }
        let [a, b, c] = t.map(|i| source.points_meters[i as usize]);
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if !det.is_finite() || det == 0. {
            return Err("Degenerate source triangle.".into());
        }
        triangle_area += det.abs() / 2.;
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            pairs.insert((a.min(b), a.max(b)));
        }
    }
    let reference_area: f64 = source.reference_areas_square_meters.iter().sum();
    let area_residual = (triangle_area - reference_area).abs() / triangle_area;
    if !area_residual.is_finite() || area_residual > 1e-10 {
        return Err("Triangle and source reference areas disagree.".into());
    }
    let edges: Vec<_> = pairs
        .into_iter()
        .map(|(a, b)| {
            let p = source.points_meters[a as usize];
            let q = source.points_meters[b as usize];
            let dx = p[0] - q[0];
            let dy = p[1] - q[1];
            Edge {
                a,
                b,
                distance_meters: (dx * dx + dy * dy).sqrt(),
            }
        })
        .collect();
    let settings = Settings {
        channel_head_area_square_meters: 250000.,
        channel_head_slope: 0.1,
        channel_concavity: 0.5,
        maximum_graph_slope: std::f64::consts::FRAC_PI_6.tan(),
    };
    let started = Instant::now();
    let structural = landforms::dry_receivers(
        &source.prepared_bed_meters,
        &source.reference_areas_square_meters,
        &source.fixed_boundary,
        &edges,
    )?;
    let construction = landforms::construct(
        &source.prepared_bed_meters,
        &source.reference_areas_square_meters,
        &source.fixed_boundary,
        &edges,
        &structural,
        settings,
    )?;
    let actual = landforms::dry_receivers(
        &construction.elevation_meters,
        &source.reference_areas_square_meters,
        &source.fixed_boundary,
        &edges,
    )?;
    let (collected, closed, terminal_residual) = areas(&source, &actual)?;
    let elapsed = started.elapsed().as_secs_f64();
    let (image, gradient, steep) =
        atlas(&source, &construction.elevation_meters, &actual, &collected);
    let input_hash = data.iter().fold(0xcbf29ce484222325_u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    let report = json!({"reportVersion":"native-drainage-landform-mesh-1",
        "inputIdentity":{"sourceReportFnv1a64":format!("{input_hash:016x}"),"hashScope":"Exact source bytes; diagnostic binding, not security authentication.","sourceLabels":source.identity},
        "externalSourcePreparationSettings":source.settings,"pointsMeters":source.points_meters,"triangles":source.triangles,
        "fixedBoundary":source.fixed_boundary,"referenceAreasSquareMeters":source.reference_areas_square_meters,
        "sourceBedMeters":source.prepared_bed_meters,"structuralActualDryReceivers":structural,
        "construction":construction,"actualDryReceivers":actual,"actualDryContributingAreaSquareMeters":collected,
        "metrics":{"closedDryFractionOfActiveArea":closed,"terminalAreaRelativeResidual":terminal_residual,
            "triangleAreaRelativeResidual":area_residual,"maximumTriangleGradient":gradient,"triangleAreaFractionAbove30Degrees":steep},
        "elapsedSeconds":elapsed,"scope":"Native lower-only construction on retained external mesh/bed. Source erosion is NOT generated by this command. Both receiver graphs are independently rebuilt on their respective actual heights. Graph-edge slope bound is not a triangle-gradient bound. No water, sediment delivery, global World, checkpoint or desktop integration."});
    let encoded = serde_json::to_vec(&report).map_err(|e| e.to_string())?;
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    write_new(&output.join("physical.json"), &encoded)?;
    write_new(&output.join("comparison.svg"), image.as_bytes())?;
    write_new(&output.join("manifest.json"),&serde_json::to_vec_pretty(&json!({"reportVersion":"native-drainage-landform-mesh-1","records":["physical.json"],"atlases":["comparison.svg"],"constructionModelVersion":landforms::MODEL_VERSION})).map_err(|e|e.to_string())?)?;
    println!(
        "Native construction/route: {elapsed:.3}s, {n} points, closed {:.3}%, max triangle gradient {gradient:.4}",
        closed * 100.
    );
    Ok(())
}
