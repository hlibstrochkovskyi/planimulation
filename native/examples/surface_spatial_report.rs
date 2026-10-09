//! Fixed continuous inputs and independent analytic sphere diffusion reference.
use planimulation_core::seasonal_moisture::regional_soil::surface_verification;
use std::io::{BufWriter, Write};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("Usage: surface_spatial_report NEW_OUTPUT.json".into());
    }
    if std::path::Path::new(&args[0]).exists() {
        return Err("Report output already exists.".into());
    }
    let report = surface_verification::report()?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[0])
        .map_err(|e| e.to_string())?;
    let mut file = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut file, &report).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    Ok(())
}
