//! Read-only comparison of area routing on an already prepared physical patch.
use planimulation_core::{terrain_patch_facets, terrain_patch_geometry, terrain_patch_landscape};
use serde_json::{Value, json};
use std::{
    fmt::Write as _,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Fine-routing input exceeds its byte budget.".into());
    }
    Ok(bytes)
}
fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&bounded_read(path, 16 * 1024 * 1024)?).map_err(|e| e.to_string())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?
        .write_all(bytes)
        .map_err(|e| e.to_string())
}
fn output_args(args: &[String]) -> Result<(PathBuf, PathBuf, String), String> {
    if !(2..=3).contains(&args.len()) || args.iter().any(|v| v.trim().is_empty()) {
        return Err("Usage: fine_routing INPUT_ROW_JSON NEW_DIRECTORY [incision|coupled]".into());
    }
    let variant = args.get(2).cloned().unwrap_or("coupled".into());
    if variant != "incision" && variant != "coupled" {
        return Err("Unsupported retained-bed variant.".into());
    }
    let output = PathBuf::from(&args[1]);
    if std::fs::symlink_metadata(&output).is_ok()
        || output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .is_some_and(|p| !p.is_dir())
    {
        return Err("Fine-routing requires a new output directory with an existing parent.".into());
    }
    Ok((PathBuf::from(&args[0]), output, variant))
}
fn field_file(row: &Value, manifest: &Value, variant: &str) -> Result<String, String> {
    let columns = json!([
        "initialBedMeters",
        "prescribedUpliftMetersPerYear",
        "preparedBedMeters",
        "acceptedUpliftDisplacementMeters",
        "removedIncisionThicknessMeters",
        "signedHillslopeHeightChangeMeters"
    ]);
    if manifest["reportVersion"] != "fine-landscape-review-1"
        || manifest["fieldRecord"]["format"] != "headerless-f64-le-region-major-1"
        || manifest["fieldRecord"]["columns"] != columns
        || !manifest["rows"]
            .as_array()
            .is_some_and(|rows| rows.contains(row))
        || row["geometryModelVersion"] != terrain_patch_geometry::MODEL_VERSION
    {
        return Err("Unsupported or mismatched fine-landscape row/manifest.".into());
    }
    let file = row["variants"]
        .as_array()
        .and_then(|vs| vs.iter().find(|v| v["variant"] == variant))
        .and_then(|v| v["physicalFieldsFile"].as_str())
        .ok_or("Missing retained physical fields.")?;
    if Path::new(file).file_name().and_then(|n| n.to_str()) != Some(file) {
        return Err("Physical field reference must be a local filename.".into());
    }
    Ok(file.into())
}
fn bed(bytes: &[u8], cells: usize) -> Result<Vec<f64>, String> {
    if bytes.len() != cells * 48 {
        return Err("Wrong retained physical record length.".into());
    }
    let mut h = Vec::with_capacity(cells);
    for chunk in bytes.as_chunks::<48>().0 {
        let columns: [f64; 6] = std::array::from_fn(|j| {
            f64::from_le_bytes(chunk[j * 8..j * 8 + 8].try_into().unwrap())
        });
        if columns.iter().any(|v| !v.is_finite()) {
            return Err("Nonfinite retained physical field.".into());
        }
        h.push(columns[2]);
    }
    Ok(h)
}
fn atlas(
    g: &terrain_patch_geometry::Geometry,
    h: &[f64],
    old: &terrain_patch_landscape::Routing,
    new: &terrain_patch_facets::Routing,
) -> String {
    let nx = g.settings.cells_x as usize;
    let ny = g.settings.cells_y as usize;
    let scale = 480. / nx.max(ny) as f64;
    let minimum = h.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = h.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1030\" height=\"600\"><rect width=\"1030\" height=\"600\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style>",
    );
    for (panel, title) in [
        "Existing single receiver",
        "Triangular facets / angular area partition",
    ]
    .iter()
    .enumerate()
    {
        let origin = 24 + panel * 508;
        write!(svg,"<text x=\"{origin}\" y=\"28\" font-size=\"15\">{title}</text><g transform=\"translate({origin},50)\"><g transform=\"scale({scale})\"><g shape-rendering=\"crispEdges\">").unwrap();
        for (i, &z) in h.iter().enumerate() {
            let t = if minimum == maximum {
                0.5
            } else {
                (z - minimum) / (maximum - minimum)
            };
            let c = (55. + 180. * t).round() as u8;
            write!(svg,"<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" fill=\"#{c:02x}{c:02x}{c:02x}\"/>",i%nx,ny-1-i/nx,1.001,1.001).unwrap();
        }
        svg.push_str("</g></g></g>");
        write!(svg,"<g transform=\"translate({origin},50)\" fill=\"none\" stroke=\"#46b8d7\" stroke-width=\"0.75\" opacity=\"0.7\">").unwrap();
        let point = |i: usize| {
            (
                ((i % nx) as f64 + 0.5) * scale,
                ((ny - 1 - i / nx) as f64 + 0.5) * scale,
            )
        };
        for i in 0..h.len() {
            let links: Vec<_> = if panel == 0 {
                if old.receivers[i] as usize == i {
                    vec![]
                } else {
                    vec![(old.receivers[i] as usize, 1.)]
                }
            } else {
                new.descents[i].branches().map(|(j, w, _)| (j, w)).collect()
            };
            let area = if panel == 0 {
                old.contributing_area_square_meters[i]
            } else {
                new.contributing_area_square_meters[i]
            };
            if area < 5_000_000. {
                continue;
            }
            let (x, y) = point(i);
            for (j, w) in links {
                let (a, b) = point(j);
                write!(
                    svg,
                    "<path opacity=\"{w:.5}\" d=\"M {x:.3} {y:.3} L {a:.3} {b:.3}\"/>"
                )
                .unwrap();
            }
        }
        svg.push_str("</g>");
        write!(svg,"<g transform=\"translate({origin},50)\" fill=\"none\" stroke=\"#edb25d\" stroke-width=\"1\">").unwrap();
        let stride = (nx / 20).max(1);
        for row in (stride..ny - 1).step_by(stride) {
            for col in (stride..nx - 1).step_by(stride) {
                let i = row * nx + col;
                let (dx, dy) = if panel == 0 {
                    let j = old.receivers[i] as usize;
                    ((j % nx) as f64 - col as f64, (j / nx) as f64 - row as f64)
                } else {
                    new.descents[i]
                        .branches()
                        .fold((0., 0.), |(x, y), (j, _, alpha)| {
                            (
                                x + alpha * ((j % nx) as f64 - col as f64),
                                y + alpha * ((j / nx) as f64 - row as f64),
                            )
                        })
                };
                let norm = dx.hypot(dy);
                if norm == 0. {
                    continue;
                }
                let (x, y) = point(i);
                let a = x + 6. * dx / norm;
                let b = y - 6. * dy / norm;
                write!(svg,"<path d=\"M {x:.3} {y:.3} L {a:.3} {b:.3}\"/><circle cx=\"{a:.3}\" cy=\"{b:.3}\" r=\"0.8\"/>").unwrap();
            }
        }
        svg.push_str("</g>");
    }
    svg.push_str("<text x=\"24\" y=\"555\" font-size=\"13\">Identical retained physical bed in both panels. No erosion, water or material stock advanced.</text><text x=\"24\" y=\"577\" font-size=\"13\">Cyan: static area links above5km². Amber: descent directions (explicit discrete fallback on flats).</text></svg>");
    svg
}
fn main() -> Result<(), String> {
    let (input, output, variant) = output_args(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let parent = input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let row = read_json(&input)?;
    let manifest = read_json(&parent.join("manifest.json"))?;
    let filename = field_file(&row, &manifest, &variant)?;
    let settings: terrain_patch_geometry::Settings =
        serde_json::from_value(row["geometrySettings"].clone()).map_err(|e| e.to_string())?;
    settings.validate()?;
    let cells = settings.cells_x as usize * settings.cells_y as usize;
    if cells > 40000 {
        return Err("Fine-routing observer supports at most 40000 cells.".into());
    }
    let bytes = bounded_read(&parent.join(&filename), cells * 48)?;
    let h = bed(&bytes, cells)?;
    let g = terrain_patch_geometry::build(settings)?;
    let start = Instant::now();
    let old = terrain_patch_landscape::route(&g, &h)?;
    let old_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let router = terrain_patch_facets::Router::new(&g)?;
    let metric_seconds = start.elapsed().as_secs_f64();
    let start = Instant::now();
    let new = router.route(&h)?;
    let facet_seconds = start.elapsed().as_secs_f64();
    let mut error = 0.;
    let mut area = 0.;
    let mut maximum = 0_f64;
    for i in 0..cells {
        if new.descents[i].slope == 0. {
            continue;
        }
        let Some(v) = router.tangent_displacement(i, old.receivers[i] as usize) else {
            continue;
        };
        let d = new.descents[i].unit_tangent_direction;
        let angle = (v[0] * d[1] - v[1] * d[0])
            .abs()
            .atan2(v[0] * d[0] + v[1] * d[1]);
        let a = g.reference_areas_square_meters[i];
        error += a * angle * angle;
        area += a;
        maximum = maximum.max(angle);
    }
    let fingerprint = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    let svg = atlas(&g, &h, &old, &new);
    let report = json!({"reportVersion":"fine-routing-review-1","inputRow":input,"variant":variant,
        "geometrySettings":settings,"sourceIdentity":row["sourceIdentity"],"physicalFieldsFile":filename,
        "rawRecordFnv1a64Diagnostic":format!("{fingerprint:016x}"),"oldClosedFractionOfActiveArea":old.closed_fraction_of_active_area,
        "oldClosedSinkCount":old.closed_sink_count,"oldRoutingSeconds":old_seconds,"tangentMetricSeconds":metric_seconds,
        "facetRoutingSeconds":facet_seconds,"areaWeightedRmsDirectionChangeDegrees":if area>0.{(error/area).sqrt().to_degrees()}else{0.},
        "maximumDirectionChangeDegrees":maximum.to_degrees(),"facetRouting":new,
        "scope":"Read-only area routing on identical retained bed. Direction change is not an accuracy oracle. No erosion, seasonal water or material stock advances. FNV diagnostic is not security authentication. Observer cap40000 cells."});
    std::fs::create_dir(&output).map_err(|e| e.to_string())?;
    write_new(&output.join("comparison.svg"), svg.as_bytes())?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("comparison.json"))
        .map_err(|e| e.to_string())?;
    let mut file = std::io::BufWriter::new(file);
    serde_json::to_writer_pretty(&mut file, &report).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    println!(
        "Fine routing: {} cells, old {:.2}% closed, facets {:.2}% closed, {} two-receiver nodes.",
        cells,
        old.closed_fraction_of_active_area * 100.,
        report["facetRouting"]["closedFractionOfActiveArea"]
            .as_f64()
            .unwrap()
            * 100.,
        report["facetRouting"]["twoReceiverNodes"]
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_arguments_record_shapes_and_local_references() {
        for args in [
            vec![],
            vec!["x"],
            vec!["x", "."],
            vec!["x", "fine-uncreated", "bad"],
        ] {
            assert!(output_args(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        assert!(bed(&[0; 47], 1).is_err());
        let mut bytes = vec![0; 48];
        bytes[16..24].copy_from_slice(&17_f64.to_le_bytes());
        assert_eq!(bed(&bytes, 1).unwrap(), vec![17.]);
        bytes[..8].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(bed(&bytes, 1).is_err());
        assert!(field_file(&json!({}), &json!({}), "coupled").is_err());
    }

    #[test]
    fn input_rows_require_matching_manifest_layout_and_local_physical_filename() {
        let row = json!({"geometryModelVersion":terrain_patch_geometry::MODEL_VERSION,
            "variants":[{"variant":"coupled","physicalFieldsFile":"bed.f64le"}]});
        let mut manifest = json!({"reportVersion":"fine-landscape-review-1","rows":[row],
            "fieldRecord":{"format":"headerless-f64-le-region-major-1","columns":[
                "initialBedMeters","prescribedUpliftMetersPerYear","preparedBedMeters",
                "acceptedUpliftDisplacementMeters","removedIncisionThicknessMeters","signedHillslopeHeightChangeMeters"]}});
        assert_eq!(field_file(&row, &manifest, "coupled").unwrap(), "bed.f64le");
        assert!(field_file(&row, &manifest, "incision").is_err());
        let mut wrong = row.clone();
        wrong["variants"][0]["physicalFieldsFile"] = json!("../bed.f64le");
        assert!(field_file(&wrong, &manifest, "coupled").is_err());
        manifest["rows"] = json!([wrong]);
        assert!(field_file(&wrong, &manifest, "coupled").is_err());
        manifest["rows"] = json!([row]);
        manifest["fieldRecord"]["columns"][0] = json!("unrecognized");
        assert!(field_file(&row, &manifest, "coupled").is_err());
    }
}
