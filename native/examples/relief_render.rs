//! Re-render stored physical fields without rerunning or advancing preparation.
#[path = "support/relief_display.rs"]
mod relief_display;
use planimulation_core::{
    Recipe, Surface, geography_metrics,
    water::{Water, WaterSettings},
};
use serde_json::Value;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

fn decode_fields(bytes: &[u8], n: usize) -> Result<Vec<f64>, String> {
    if !(1..=1_000_000).contains(&n) || n.checked_mul(64) != Some(bytes.len()) {
        return Err("Invalid relief field record dimensions.".into());
    }
    let mut bed = Vec::with_capacity(n);
    for region in bytes.as_chunks::<64>().0 {
        for (j, column) in region.as_chunks::<8>().0.iter().enumerate() {
            let value = f64::from_le_bytes(*column);
            if !value.is_finite() || j != 0 && j != 2 && value < 0. {
                return Err("Invalid relief field value or nonnegative inventory.".into());
            }
            if j == 2 {
                bed.push(value);
            }
        }
    }
    Ok(bed)
}
fn parse(args: &[String]) -> Result<(PathBuf, PathBuf), String> {
    if args.len() != 2 || args.iter().any(|s| s.trim().is_empty()) {
        return Err("Usage: relief_render NEW_OUTPUT.svg VARIANT_REPORT.json".into());
    }
    let output = PathBuf::from(&args[0]);
    let input = PathBuf::from(&args[1]);
    if std::fs::symlink_metadata(&output).is_ok() {
        return Err("Relief rendering requires a new output.".into());
    }
    if output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .is_some_and(|p| !p.is_dir())
    {
        return Err("Relief rendering output parent must exist.".into());
    }
    Ok((output, input))
}
fn read_json(path: &Path) -> Result<Value, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 32_000_000 {
        return Err("Relief metadata exceeds the read-only renderer bound.".into());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (output, input) = parse(&args)?;
    let record = read_json(&input)?;
    let directory = input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let manifest = read_json(&directory.join("manifest.json"))?;
    if !matches!(
        manifest["reportVersion"].as_str(),
        Some(
            "physical-relief-review-1"
                | "physical-relief-review-2"
                | "geodesic-belt-review-1"
                | "geodesic-belt-review-2"
        )
    ) || manifest["fieldRecord"]["format"] != "headerless-f64-le-region-major-1"
        || manifest["fieldRecord"]["columns"]
            != serde_json::json!([
                "initialBedMeters",
                "upliftRateMetersPerYear",
                "preparedBedMeters",
                "activeSedimentThicknessMeters",
                "pendingSedimentVolumeCubicMeters",
                "cumulativeErosionThicknessMeters",
                "cumulativeDepositionThicknessMeters",
                "acceptedUpliftDisplacementMeters"
            ])
    {
        return Err("Unsupported completed relief field record.".into());
    }
    let level = record["samplingSubdivision"]
        .as_u64()
        .filter(|v| (6..=8).contains(v))
        .ok_or("Unsupported relief rendering sampling level.")? as u32;
    if manifest["samplingSubdivision"].as_u64() != Some(u64::from(level)) {
        return Err("Relief sampling metadata mismatch.".into());
    }
    let parameters: Recipe =
        serde_json::from_value(record["referenceRecipe"].clone()).map_err(|e| e.to_string())?;
    parameters.validate()?;
    let n = 10 * 4_usize.pow(level) + 2;
    if record["preparedMetrics"]["regionCount"].as_u64() != Some(n as u64)
        || manifest["regions"].as_u64() != Some(n as u64)
    {
        return Err("Relief canonical region count mismatch.".into());
    }
    let filename = record["physicalFieldsFile"]
        .as_str()
        .ok_or("Missing raw relief fields.")?;
    if Path::new(filename).file_name().and_then(|p| p.to_str()) != Some(filename)
        || !filename.ends_with(".f64le")
    {
        return Err("Raw relief fields must be a sibling basename.".into());
    }
    let field_path = directory.join(filename);
    if std::fs::metadata(&field_path)
        .map_err(|e| e.to_string())?
        .len()
        != n as u64 * 64
    {
        return Err("Raw relief field size mismatch.".into());
    }
    let bed = decode_fields(&std::fs::read(&field_path).map_err(|e| e.to_string())?, n)?;
    let surface = Surface::build_generation_probe(level, parameters.radius_meters)?;
    let volume = record["originalWaterVolumeCubicMeters"]
        .as_f64()
        .ok_or("Missing relief water inventory.")?;
    let water = Water::generate(
        &surface,
        &bed,
        &WaterSettings::Volume {
            volume_cubic_meters: volume,
        },
    )?;
    if record["finalWaterLevelMeters"].as_f64() != Some(water.level_meters) {
        return Err("Recorded final water level does not reproduce.".into());
    }
    // Exact independent morphology/drainage check detects mismatched geometry,
    // water, region ordering or a corrupt bed before rendering any output.
    let observed = serde_json::to_value(geography_metrics::analyze(&surface, &bed, &water)?)
        .map_err(|e| e.to_string())?;
    if observed != record["preparedMetrics"] {
        return Err("Stored relief geography does not reproduce.".into());
    }
    let svg = relief_display::single(&surface, &bed, &water)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|e| e.to_string())?;
    file.write_all(svg.as_bytes()).map_err(|e| e.to_string())?;
    println!("Rendered {n} canonical regions from immutable stored fields; no simulation step.");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn field_dimensions_values_and_preallocation_refusals_are_explicit() {
        let values = [-1_f64, 0., 2., 0., 0., 0., 0., 0.];
        let bytes: Vec<_> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(decode_fields(&bytes, 1).unwrap(), [2.]);
        assert!(decode_fields(&bytes, 2).is_err());
        assert!(decode_fields(&bytes, 1_000_001).is_err());
        let mut bad = bytes.clone();
        bad[32..40].copy_from_slice(&(-1_f64).to_le_bytes());
        assert!(decode_fields(&bad, 1).is_err());
        bad = bytes;
        bad[16..24].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(decode_fields(&bad, 1).is_err());
        assert!(parse(&[]).is_err());
        assert!(parse(&[".".into(), "missing".into()]).is_err());
    }
}
