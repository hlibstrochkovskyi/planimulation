//! Gradient-side mobility assembly comparison, not a seasonal checkpoint.
use planimulation_core::seasonal_moisture::regional_soil::{surface_evolution, surface_front};
use std::io::{BufWriter, Write};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || !["--dry", "--wet"].contains(&args[1].as_str()) {
        return Err("Usage: surface_incident_report NEW_OUTPUT.json --dry|--wet".into());
    }
    if std::path::Path::new(&args[0]).exists() {
        return Err("Report output already exists.".into());
    }
    let report = if args[1] == "--dry" {
        surface_front::incident_report()?
    } else {
        surface_evolution::incident_report()?
    };
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
