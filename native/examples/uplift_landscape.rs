//! Controlled dry-prehistory comparison, not a selected desktop model/save.
#[path = "support/atlas_field.rs"]
mod atlas_field;
#[path = "support/drainage_overlay.rs"]
mod drainage_overlay;
use planimulation_core::{
    Recipe, World, continental_uplands, depression_analysis, geography_metrics, hash,
    landscape_evolution::{self, Settings},
    water::{Water, WaterSettings},
    wire,
};
use serde_json::{Value, json};
use std::{
    fmt::Write as _,
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

fn recipe(seed: &str, level: u32) -> Recipe {
    Recipe {
        schema_version: 1,
        model_version: continental_uplands::MODEL_VERSION.into(),
        random_version: "fnv1a-utf8-mulberry32-1".into(),
        seed: seed.into(),
        subdivision: level,
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
    }
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(bytes).map_err(|e| e.to_string())
}
fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut b = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    b.push(b'\n');
    write_new(path, &b)
}
fn parse(args: &[String]) -> Result<(PathBuf, u32, Option<String>, usize), String> {
    if !(2..=4).contains(&args.len()) || args[0].trim().is_empty() {
        return Err(
            "Usage: uplift_landscape NEW_DIRECTORY SUBDIVISION(4..6) [SEED] [--steps=16|32|64]"
                .into(),
        );
    }
    let level = args[1]
        .parse::<u32>()
        .map_err(|_| "Invalid preparation subdivision.")?;
    if !(4..=6).contains(&level) {
        return Err("Preparation comparison supports subdivisions 4–6.".into());
    }
    let mut seed = None;
    let mut steps = 32;
    let mut saw_steps = false;
    for value in &args[2..] {
        if let Some(v) = value.strip_prefix("--steps=") {
            steps = v
                .parse::<usize>()
                .map_err(|_| "Invalid preparation step control.")?;
            if saw_steps || ![16, 32, 64].contains(&steps) {
                return Err("Step control must be exactly one of 16, 32 or 64.".into());
            }
            saw_steps = true;
        } else if value.starts_with("--") || seed.is_some() {
            return Err("Unknown/duplicate preparation argument.".into());
        } else {
            seed = Some(value.clone());
        }
    }
    recipe(seed.as_deref().unwrap_or("first-light"), level).validate()?;
    Ok((PathBuf::from(&args[0]), level, seed, steps))
}
fn cases(seed: Option<String>) -> Vec<(String, String, f64, f64, f64)> {
    if let Some(seed) = seed {
        return vec![(seed, "selected".into(), 1., 1., 300.)];
    }
    let mut cases = vec![
        ("first-light".into(), "ordinary-00".into(), 1., 1., 300.),
        ("4836187e".into(), "ordinary-01".into(), 1., 1., 300.),
    ];
    for i in 0..18 {
        cases.push((
            format!("geography-{i:02}"),
            format!("ordinary-{:02}", i + 2),
            1.,
            1.,
            300.,
        ));
    }
    cases.extend([
        ("first-light".into(), "fragmented".into(), 0.5, 1., 300.),
        ("first-light".into(), "broad".into(), 2., 1., 300.),
        ("first-light".into(), "low-relief".into(), 1., 0., 0.),
    ]);
    cases
}
fn atlas(world: &World, fields: &[(f64, Vec<f64>, Water)], settings: Settings) -> String {
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1600\" height=\"1474\"><rect width=\"1600\" height=\"1474\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style><text x=\"24\" y=\"30\" font-size=\"20\">Uplift / erosion balance — actual beds + STATIC potential drainage, NOT active rivers</text><defs><clipPath id=\"clip\"><rect width=\"760\" height=\"380\"/></clipPath></defs>",
    );
    let panels = std::iter::once((
        "Original bed · no preparation".to_owned(),
        &world.terrain.elevation,
        &world.water,
    ))
    .chain(fields.iter().map(|(scale, bed, water)| {
        let title = if *scale == 0. {
            "Incision-only control".to_owned()
        } else {
            format!("Structural forcing: characteristic height {scale:.0} m")
        };
        (title, bed, water)
    }));
    for (i, (title, bed, water)) in panels.enumerate() {
        let x = 24 + (i % 2) * 792;
        let y = 68 + (i / 2) * 434;
        write!(svg,"<text x=\"{x}\" y=\"{y}\" font-size=\"14\">{title}</text><g transform=\"translate({x},{})\" clip-path=\"url(#clip)\">",y+16).unwrap();
        atlas_field::append_field(&mut svg, &world.surface, |j| {
            atlas_field::terrain_color(bed[j] - water.level_meters, water.depth_meters[j])
        });
        drainage_overlay::append(
            &mut svg,
            &world.surface,
            bed,
            &water.body_ids,
            50_000_000_000.,
        );
        svg.push_str("</g>");
    }
    writeln!(svg,"<text x=\"816\" y=\"1010\" font-size=\"16\">Blue links: static bed receivers</text><text x=\"816\" y=\"1040\" font-size=\"14\">Shown from 50,000 km² contributing dry area.</text><text x=\"816\" y=\"1070\" font-size=\"14\">Yellow circles: existing closed dry terminals.</text><text x=\"816\" y=\"1100\" font-size=\"14\">No rainfall, discharge or sink correction is implied.</text><text x=\"24\" y=\"1392\" font-size=\"13\">Same initial bed, K=1e-6/year, {} x {:.0} years; uplift rate = K x characteristic height x initial convergence / 6000 m.</text><text x=\"24\" y=\"1420\" font-size=\"13\">Fixed common height/depth palette: 0–4500 / 0–5000 m; saturation explicit. Final initial water retains original volume, not refitted coverage.</text><text x=\"24\" y=\"1448\" font-size=\"13\">Dry sinks are retained, not forcibly connected. Uplift displacement and exported erosion material have separate reference-prism ledgers.</text></svg>",settings.steps,settings.step_years).unwrap();
    svg
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (directory, level, seed, steps) = parse(&args)?;
    let selected = seed.is_some();
    std::fs::create_dir(&directory)
        .map_err(|e| format!("Cannot create new comparison directory: {e}"))?;
    let settings = Settings {
        erodibility_per_year: 1e-6,
        step_years: 800000. / steps as f64,
        steps,
    };
    let mut rows = Vec::new();
    for (seed, name, scale, relief, detail) in cases(seed) {
        let mut r = recipe(&seed, level);
        r.continental_scale = scale;
        r.relief_scale = relief;
        r.detail_amplitude_meters = detail;
        let world = World::generate(r)?;
        let mut bytes = serde_json::to_vec(&world.recipe).map_err(|e| e.to_string())?;
        bytes.extend_from_slice(&wire::arrays(&world));
        let initial_checksum = format!("{:08x}", hash(&bytes));
        drop(bytes);
        let initial = json!({"recipe":world.recipe,"nativeCompatibleInitialChecksum":initial_checksum,
            "metrics":geography_metrics::analyze(&world.surface,&world.terrain.elevation,&world.water)?,
            "depressionGeometry":depression_analysis::analyze(&world.surface,&world.terrain.elevation,&world.water)?,
            "initialWaterLevelMeters":world.water.level_meters,"initialWaterVolumeCubicMeters":world.water.resolved_volume_cubic_meters});
        write_json(
            &directory.join(format!("{name}-initial.json")),
            &json!({"initial":initial,"initialElevationMeters":world.terrain.elevation}),
        )?;
        let mut variants = Vec::new();
        let mut fields = Vec::new();
        for characteristic in [0., 500., 1000., 2000.] {
            let rates = landscape_evolution::structural_uplift_rates(
                &world.terrain.convergence,
                &world.water,
                settings.erodibility_per_year,
                characteristic,
            )?;
            let started = Instant::now();
            let p = landscape_evolution::prepare(
                &world.surface,
                &world.terrain.elevation,
                &world.water,
                &rates,
                settings,
            )?;
            let seconds = started.elapsed().as_secs_f64();
            let water = Water::generate(
                &world.surface,
                &p.elevation_meters,
                &WaterSettings::Volume {
                    volume_cubic_meters: world.water.resolved_volume_cubic_meters,
                },
            )?;
            let summary = json!({"characteristicHeightMeters":characteristic,"settings":settings,"modelVersion":p.model_version,
                "metrics":geography_metrics::analyze(&world.surface,&p.elevation_meters,&water)?,
                "depressionGeometry":depression_analysis::analyze(&world.surface,&p.elevation_meters,&water)?,
                "upliftDisplacementPrismCubicMeters":p.uplift_displacement_prism_cubic_meters,
                "exportedErosionMaterialCubicMeters":p.exported_erosion_material_cubic_meters,
                "netBedPrismChangeCubicMeters":p.independent_net_bed_prism_change_cubic_meters,
                "relativePrismLedgerResidual":p.relative_prism_ledger_residual,
                "initializationWater":{"levelMeters":water.level_meters,"resolvedVolumeCubicMeters":water.resolved_volume_cubic_meters},
                "preparationSeconds":seconds});
            write_json(
                &directory.join(format!("{name}-height-{characteristic:.0}.json")),
                &json!({"initial":initial,"summary":summary,
                "upliftRatesMetersPerYear":rates,"preparation":p,
                "candidateInitialWater":{"levelMeters":water.level_meters,"depthMeters":water.depth_meters,"bodyIds":water.body_ids}}),
            )?;
            println!(
                "{name}, height {characteristic:.0} m: {seconds:.3} s preparation; prism residual {:.3e}.",
                p.relative_prism_ledger_residual
            );
            variants.push(summary);
            fields.push((characteristic, p.elevation_meters, water));
        }
        let atlas_name = format!("{name}.svg");
        write_new(
            &directory.join(&atlas_name),
            atlas(&world, &fields, settings).as_bytes(),
        )?;
        rows.push(json!({"case":name,"seed":seed,"category":if selected {"selected"}else if name.starts_with("ordinary-"){"ordinary"}else{"directed"},
            "initial":initial,"variants":variants,"atlasFile":atlas_name}));
    }
    write_json(
        &directory.join("manifest.json"),
        &json!({"reportVersion":"uplift-landscape-review-1","subdivision":level,
        "modelVersion":landscape_evolution::MODEL_VERSION,"rows":rows,
        "scope":"Bounded synthetic dry prehistory on unchanged uplands initial family. Original wet beds/base level fixed; one final initialization fitting retains original water volume. No deposition, evolving plates, climate, physical sink correction or selected desktop preparation model.",
        "accountingScope":"Uplift height-prism is kinematic displacement-equivalent bookkeeping, not newly created rock mass. Exported erosion uses a separate reference-area-equivalent material reservoir.",
        "outputScope":"New-directory-only diagnostic files, not transactional/resumable simulation checkpoints. Manifest written last; timings are observations, not physical identity."}),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_and_case_selection_are_explicit() {
        for args in [
            vec![],
            vec!["out"],
            vec!["out", "3"],
            vec!["out", "7"],
            vec!["out", "6", ""],
            vec!["out", "6", "seed", "extra"],
            vec!["out", "6", "--steps=17"],
            vec!["out", "6", "--steps=32", "--steps=64"],
        ] {
            assert!(parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
        assert_eq!(cases(None).len(), 23);
        assert_eq!(cases(Some("first-light".into())).len(), 1);
        assert_eq!(
            parse(&["out".into(), "6".into(), "seed".into()]).unwrap().1,
            6
        );
        assert_eq!(
            parse(&[
                "out".into(),
                "6".into(),
                "first-light".into(),
                "--steps=64".into()
            ])
            .unwrap()
            .3,
            64
        );
    }
}
