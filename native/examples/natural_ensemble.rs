//! Milestone E ensemble. See docs/natural-ensemble.md for the checks and units.
#[path = "support/natural_world.rs"]
mod natural_world;

use planimulation_core::climate_normals::{self, Biome, Normals};
use planimulation_core::rivers::{self, Rivers};
use planimulation_core::vegetation::{self, Model, Settings, State};
use planimulation_core::wild_food::{self, RegionFood};
use planimulation_core::{Recipe, World};
use serde::Serialize;
use serde_json::{Value, json};
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fs, thread};

const YEARS: u32 = 1000;
const NAMED: [&str; 3] = ["first-light", "5c151f46", "4836187e"];
const BIOMES: [&str; 11] = [
    "water",
    "ice",
    "tundra",
    "borealForest",
    "temperateForest",
    "temperateGrassland",
    "coldDesert",
    "hotDesert",
    "savanna",
    "tropicalSeasonalForest",
    "tropicalRainforest",
];
const FOREST: [u8; 4] = [
    Biome::BorealForest as u8,
    Biome::TemperateForest as u8,
    Biome::TropicalSeasonalForest as u8,
    Biome::TropicalRainforest as u8,
];
const MAX_DRIFT_PP: f64 = 2.;
const MAX_FLICKER_RATE: f64 = 0.015;
const NPP_PG: (f64, f64) = (50., 250.);
const RECOVERY_MARGIN: f64 = 0.05;
const LONG_RECOVERY: u32 = 100;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Climate {
    land_fraction: f64,
    biome_area_shares: [f64; BIOMES.len()],
    land_precipitation_mean_mm: f64,
    land_temperature_mean_celsius: f64,
    ice_free_land_km2: f64,
    land_runoff_mm: f64,
    runoff_precipitation_ratio: f64,
    ocean_inflow_km3: f64,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Drought {
    region: Option<usize>,
    end_year: Option<u32>,
    woody_before: Option<f64>,
    woody_after: Option<f64>,
    three_year_woody_loss: f64,
    precipitation_factors: Option<[f64; 3]>,
    recovery_years: Option<u32>,
    follow_up_years: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Vegetation {
    forest_share_year0: f64,
    forest_share_min: f64,
    forest_share_max: f64,
    forest_share_end: f64,
    forest_share_first_half_mean: f64,
    forest_share_second_half_mean: f64,
    forest_share_drift_pp: f64,
    class_changes_per_land_region_per_year: f64,
    npp_mean_pg: f64,
    npp_min_pg: f64,
    npp_max_pg: f64,
    global_land_factor_sd: f64,
    worst_drought: Drought,
}

#[derive(Clone, Copy, Serialize)]
struct Distribution {
    min: f64,
    mean: f64,
    max: f64,
}

impl Distribution {
    fn of(values: impl Iterator<Item = f64> + Clone) -> Self {
        let (min, sum, max, count) = values.fold(
            (f64::INFINITY, 0., f64::NEG_INFINITY, 0),
            |(min, sum, max, count), value| {
                (min.min(value), sum + value, max.max(value), count + 1)
            },
        );
        Self {
            min,
            mean: sum / f64::from(count),
            max,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Food {
    capacity_people: Distribution,
    capacity_people_per_ice_free_km2: Distribution,
    plant_share_of_energy: f64,
    minimum_game_ratio: f64,
}

struct RangeCheck {
    min: f64,
    max: f64,
    violations: u64,
}

impl Default for RangeCheck {
    fn default() -> Self {
        Self {
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            violations: 0,
        }
    }
}

impl RangeCheck {
    fn observe(&mut self, value: f64) {
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.violations += u64::from(!(0. ..=1.).contains(&value));
    }
}

#[derive(Default)]
struct Diagnostics {
    fractions: RangeCheck,
    frozen_class_mismatches: u64,
    capacity_violations: u64,
    max_capacity_excess: f64,
    max_capacity_ratio: f64,
}

impl Diagnostics {
    fn observe(
        &mut self,
        normals: &Normals,
        state: &State,
        regions: &[RegionFood],
        land: &[usize],
    ) {
        for &i in land {
            let food = &regions[i];
            let class = normals.biome[i];
            self.frozen_class_mismatches +=
                u64::from(class <= Biome::Tundra as u8 && state.label[i] != class);
            for value in [
                state.woody[i],
                state.herb[i],
                state.woody[i] + state.herb[i],
                state.established_cover[i],
                food.access,
            ]
            .into_iter()
            .chain(food.season)
            {
                self.fractions.observe(value);
            }
            let excess = food.capacity - food.stored_capacity;
            self.max_capacity_excess = self.max_capacity_excess.max(excess);
            self.capacity_violations += u64::from(
                !food.stored_capacity.is_finite()
                    || !(0. ..=food.stored_capacity).contains(&food.capacity),
            );
            if food.stored_capacity > 0. {
                self.max_capacity_ratio = self
                    .max_capacity_ratio
                    .max(food.capacity / food.stored_capacity);
            }
        }
    }
}

#[derive(Serialize)]
struct Law {
    name: &'static str,
    measured: Value,
    pass: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    seed: String,
    case: &'static str,
    years: u32,
    recipe: Recipe,
    vegetation_settings: Settings,
    climate: Climate,
    vegetation: Vegetation,
    wild_food: Food,
    laws: Vec<Law>,
    summary_hash: String,
    problematic_reasons: Vec<String>,
}

fn measure(case: &'static str, seed: &str, settings: Settings) -> Result<Run, String> {
    let recipe = natural_world::recipe(seed, 6);
    let world = World::generate(recipe.clone())?;
    let normals = Normals::from_world(&world)?;
    let areas = &world.surface.areas;
    let mut biome_areas = [0.; BIOMES.len()];
    for (&area, &biome) in areas.iter().zip(&normals.biome) {
        biome_areas[biome as usize] += area;
    }
    let land: Vec<usize> = world
        .water
        .body_ids
        .iter()
        .enumerate()
        .filter_map(|(i, &body)| (body == 0).then_some(i))
        .collect();
    let land_area: f64 = land.iter().map(|&i| areas[i]).sum();
    let total_area: f64 = areas.iter().sum();
    let mean = |values: &[f64]| land.iter().map(|&i| areas[i] * values[i]).sum::<f64>() / land_area;
    let water = Rivers::from_world(&world, &normals)?.totals;
    let climate = Climate {
        land_fraction: land_area / total_area,
        biome_area_shares: biome_areas.map(|a| a / total_area),
        land_precipitation_mean_mm: mean(&normals.annual_precipitation_mm),
        land_temperature_mean_celsius: mean(&normals.annual_mean_celsius),
        ice_free_land_km2: (land_area - biome_areas[Biome::Ice as usize]) / 1e6,
        land_runoff_mm: water.land_runoff / water.land_area * 1e3,
        runoff_precipitation_ratio: water.land_runoff / water.land_precipitation,
        ocean_inflow_km3: water.ocean_discharge / 1e9,
    };
    if climate.ice_free_land_km2 <= 0. {
        return Err("The fixed ensemble recipe must generate ice-free land.".into());
    }

    let (model, mut state) = Model::new(&world, &normals, settings)?;
    let (food_model, mut food) =
        wild_food::Model::new(&world, &normals, &model, &state, Default::default())?;
    let mut regions = food_model.regions(&model, &state, &food);
    let mut diagnostics = Diagnostics::default();
    diagnostics.observe(&normals, &state, &regions, &land);
    let forest0 = land
        .iter()
        .filter(|&&i| FOREST.contains(&state.label[i]))
        .map(|&i| areas[i])
        .sum::<f64>()
        / land_area;
    let mut previous = vec![state.woody.clone(); 3];
    let mut drought = Drought::default();
    for year in 1..=YEARS {
        model.advance(&mut state, 1)?;
        food_model.step_with_regions(&model, &state, &mut food, &mut regions)?;
        diagnostics.observe(&normals, &state, &regions, &land);
        let oldest = &mut previous[(year % 3) as usize];
        if year >= 3 {
            for &i in &land {
                let loss = oldest[i] - state.woody[i];
                if loss > drought.three_year_woody_loss {
                    drought = Drought {
                        region: Some(i),
                        end_year: Some(year),
                        woody_before: Some(oldest[i]),
                        woody_after: Some(state.woody[i]),
                        three_year_woody_loss: loss,
                        ..Default::default()
                    };
                }
            }
        }
        oldest.copy_from_slice(&state.woody);
    }
    if let (Some(region), Some(year), Some(before)) =
        (drought.region, drought.end_year, drought.woody_before)
    {
        let replay = model.region_series(&state, region)?;
        let end = year as usize;
        drought.precipitation_factors = Some(std::array::from_fn(|j| {
            replay[end - 2 + j].precipitation_factor
        }));
        drought.recovery_years = replay[end..]
            .iter()
            .position(|entry| entry.woody >= before - RECOVERY_MARGIN)
            .map(|years| years as u32);
        drought.follow_up_years = Some(YEARS - year);
    }

    let summaries = &state.summaries;
    let forest_stats =
        Distribution::of(std::iter::once(forest0).chain(summaries.iter().map(|s| s.forest_share)));
    let (first, second) = summaries.split_at(YEARS as usize / 2);
    let first_mean = Distribution::of(first.iter().map(|s| s.forest_share)).mean;
    let second_mean = Distribution::of(second.iter().map(|s| s.forest_share)).mean;
    let npp = Distribution::of(summaries.iter().map(|s| s.productivity_petagrams));
    let factor_mean = Distribution::of(summaries.iter().map(|s| s.precipitation_factor)).mean;
    let factor_sd = Distribution::of(
        summaries
            .iter()
            .map(|s| (s.precipitation_factor - factor_mean).powi(2)),
    )
    .mean
    .sqrt();
    let class_changes: u64 = summaries.iter().map(|s| u64::from(s.label_changes)).sum();
    let vegetation = Vegetation {
        forest_share_year0: forest0,
        forest_share_min: forest_stats.min,
        forest_share_max: forest_stats.max,
        forest_share_end: summaries.last().unwrap().forest_share,
        forest_share_first_half_mean: first_mean,
        forest_share_second_half_mean: second_mean,
        forest_share_drift_pp: 100. * (second_mean - first_mean).abs(),
        class_changes_per_land_region_per_year: class_changes as f64
            / land.len() as f64
            / f64::from(YEARS),
        npp_min_pg: npp.min,
        npp_mean_pg: npp.mean,
        npp_max_pg: npp.max,
        global_land_factor_sd: factor_sd,
        worst_drought: drought,
    };
    let plant: f64 = food.summaries.iter().map(|s| s.plant_kcal).sum();
    let game: f64 = food.summaries.iter().map(|s| s.game_kcal).sum();
    let wild_food = Food {
        capacity_people: Distribution::of(food.summaries.iter().map(|s| s.capacity_people)),
        capacity_people_per_ice_free_km2: Distribution::of(
            food.summaries
                .iter()
                .map(|s| s.capacity_people / climate.ice_free_land_km2),
        ),
        plant_share_of_energy: plant / (plant + game),
        minimum_game_ratio: Distribution::of(food.summaries.iter().map(|s| s.game_ratio)).min,
    };
    let check = &mut diagnostics.fractions;
    for value in climate
        .biome_area_shares
        .into_iter()
        .chain([
            climate.land_fraction,
            climate.runoff_precipitation_ratio,
            forest0,
            wild_food.plant_share_of_energy,
        ])
        .chain(
            summaries
                .iter()
                .flat_map(|s| [s.forest_share, s.woody_cover, s.total_cover]),
        )
    {
        check.observe(value);
    }

    let laws = vec![
        Law {
            name: "fraction bounds",
            measured: json!({"min": check.min, "max": check.max, "violations": check.violations}),
            pass: check.violations == 0,
        },
        Law {
            name: "forest drift",
            measured: json!(vegetation.forest_share_drift_pp),
            pass: vegetation.forest_share_drift_pp <= MAX_DRIFT_PP,
        },
        Law {
            name: "class flicker",
            measured: json!(vegetation.class_changes_per_land_region_per_year),
            pass: vegetation.class_changes_per_land_region_per_year <= MAX_FLICKER_RATE,
        },
        Law {
            name: "annual NPP band",
            measured: json!(npp),
            pass: summaries
                .iter()
                .all(|s| (NPP_PG.0..=NPP_PG.1).contains(&s.productivity_petagrams)),
        },
        Law {
            name: "ice and tundra stability",
            measured: json!(diagnostics.frozen_class_mismatches),
            pass: diagnostics.frozen_class_mismatches == 0,
        },
        Law {
            name: "capacity at most stored capacity",
            measured: json!({
                "maxExcessPeoplePerKm2": diagnostics.max_capacity_excess, "maxRatio": diagnostics.max_capacity_ratio,
                "violations": diagnostics.capacity_violations,
            }),
            pass: diagnostics.capacity_violations == 0,
        },
    ];
    let bytes =
        serde_json::to_vec(&(&climate, summaries, &food.summaries)).map_err(|e| e.to_string())?;
    let hash = bytes.into_iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    Ok(Run {
        seed: seed.into(),
        case,
        years: YEARS,
        recipe,
        vegetation_settings: settings,
        climate,
        vegetation,
        wild_food,
        laws,
        summary_hash: format!("{hash:016x}"),
        problematic_reasons: Vec::new(),
    })
}

fn checked_run(case: &'static str, seed: &str, settings: Settings) -> Result<Run, String> {
    let mut run = measure(case, seed, settings)?;
    let rerun = measure(case, seed, settings)?;
    run.laws.push(Law {
        name: "deterministic rerun",
        measured: json!({"first": run.summary_hash, "rerun": rerun.summary_hash}),
        pass: run.summary_hash == rerun.summary_hash,
    });
    run.problematic_reasons = run
        .laws
        .iter()
        .filter(|law| !law.pass)
        .map(|law| format!("{} failed ({})", law.name, law.measured))
        .collect();
    let drought = &run.vegetation.worst_drought;
    match (drought.recovery_years, drought.follow_up_years) {
        (Some(years), _) if years > LONG_RECOVERY => run
            .problematic_reasons
            .push(format!("worst woody loss took {years} years to recover")),
        (None, Some(years)) if years >= LONG_RECOVERY => run.problematic_reasons.push(format!(
            "worst woody loss was unrecovered after {years} years"
        )),
        _ => {}
    }
    Ok(run)
}

fn settings_cases() -> [(&'static str, Settings); 5] {
    let defaults = Settings::default();
    [
        (
            "variability-0",
            Settings {
                humid_variability: 0.,
                arid_variability: 0.,
                ..defaults
            },
        ),
        (
            "variability-1.5",
            Settings {
                humid_variability: defaults.humid_variability * 1.5,
                arid_variability: defaults.arid_variability * 1.5,
                ..defaults
            },
        ),
        (
            "seed-establishment-0",
            Settings {
                seed_establishment: 0.,
                ..defaults
            },
        ),
        (
            "spin-up-10",
            Settings {
                spin_up_years: 10,
                ..defaults
            },
        ),
        (
            "spin-up-1000",
            Settings {
                spin_up_years: 1000,
                ..defaults
            },
        ),
    ]
}

fn write_report(
    path: &Path,
    workers: usize,
    standard: &[Run],
    extremes: &[Run],
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fmt::Write;
    fs::create_dir_all(path)?;
    let output = json!({
        "years": YEARS,
        "seedWorkers": workers,
        "models": {"climate": climate_normals::MODEL_VERSION, "vegetation": vegetation::MODEL_VERSION,
            "wildFood": wild_food::MODEL_VERSION, "rivers": rivers::MODEL_VERSION},
        "riversSettings": rivers::Settings::default(),
        "wildFoodSettings": wild_food::Settings::default(),
        "biomeOrder": BIOMES,
        "lawThresholds": {"maxDriftPp": MAX_DRIFT_PP, "maxFlickerRate": MAX_FLICKER_RATE,
            "nppMinPg": NPP_PG.0, "nppMaxPg": NPP_PG.1},
        "woodyRecoveryThreshold": RECOVERY_MARGIN,
        "longRecoveryYears": LONG_RECOVERY,
        "standard": standard, "extremes": extremes,
    });
    fs::write(
        path.join("ensemble.json"),
        serde_json::to_string_pretty(&output)? + "\n",
    )?;
    let mut md = format!(
        "# Natural-world ensemble\n\n{} standard seeds and {} extreme cases, {YEARS} years each. Each case ran twice.\n\n",
        standard.len(),
        extremes.len()
    );
    for (name, runs) in [("Standard", standard), ("Extremes", extremes)] {
        writeln!(
            md,
            "## {name}\n\n| Seed | Case | Land | Rain mm/yr | °C | Forest 0 / min / max / end | Drift pp | Flicker | Mean NPP Pg/yr | Mean foragers M | Laws |"
        )?;
        writeln!(
            md,
            "| --- | --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: |"
        )?;
        for r in runs {
            let v = &r.vegetation;
            writeln!(
                md,
                "| {} | {} | {:.1}% | {:.0} | {:.1} | {:.1} / {:.1} / {:.1} / {:.1}% | {:.2} | {:.2}% | {:.1} | {:.2} | {}/{} |",
                r.seed,
                r.case,
                r.climate.land_fraction * 100.,
                r.climate.land_precipitation_mean_mm,
                r.climate.land_temperature_mean_celsius,
                v.forest_share_year0 * 100.,
                v.forest_share_min * 100.,
                v.forest_share_max * 100.,
                v.forest_share_end * 100.,
                v.forest_share_drift_pp,
                v.class_changes_per_land_region_per_year * 100.,
                v.npp_mean_pg,
                r.wild_food.capacity_people.mean / 1e6,
                r.laws.iter().filter(|l| l.pass).count(),
                r.laws.len()
            )?;
        }
        writeln!(md)?;
    }
    writeln!(
        md,
        "## Climate-normal runoff\n\n| Seed | Runoff mm/yr | Runoff / precipitation | Ocean inflow km³/yr |"
    )?;
    writeln!(md, "| --- | ---: | ---: | ---: |")?;
    for r in standard {
        writeln!(
            md,
            "| {} | {:.1} | {:.1}% | {:.0} |",
            r.seed,
            r.climate.land_runoff_mm,
            100. * r.climate.runoff_precipitation_ratio,
            r.climate.ocean_inflow_km3
        )?;
    }
    writeln!(md, "\n## Flagged runs\n")?;
    for r in standard
        .iter()
        .chain(extremes)
        .filter(|r| !r.problematic_reasons.is_empty())
    {
        writeln!(
            md,
            "- `{}` ({}). {}",
            r.seed,
            r.case,
            r.problematic_reasons.join(". ")
        )?;
    }
    writeln!(
        md,
        "\n[Measurements and law results for every seed](ensemble.json)"
    )?;
    fs::write(path.join("ensemble.md"), md)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut threads = 4;
    let mut output = PathBuf::from("artifacts/natural-ensemble");
    let mut only_seed = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--threads" => threads = args.next().ok_or("Missing --threads count")?.parse()?,
            "--output" => output = args.next().ok_or("Missing --output directory")?.into(),
            "--seed" => only_seed = Some(args.next().ok_or("Missing --seed name")?),
            "-h" | "--help" => {
                println!("natural_ensemble [--threads N] [--output DIR] [--seed SEED]");
                return Ok(());
            }
            _ => return Err(format!("Unknown argument {arg}").into()),
        }
    }
    if threads == 0 {
        return Err("--threads must be positive".into());
    }
    let mut jobs: Vec<_> = NAMED
        .into_iter()
        .map(|seed| ("standard", seed.to_owned(), Settings::default()))
        .collect();
    // A fixed, unfiltered seed sequence, independent of simulation random streams.
    jobs.extend((1..=24).map(|i| {
        (
            "standard",
            format!("natural-ensemble-{i:02}"),
            Settings::default(),
        )
    }));
    jobs.extend(settings_cases().into_iter().flat_map(|(case, settings)| {
        NAMED
            .into_iter()
            .map(move |seed| (case, seed.to_owned(), settings))
    }));
    if let Some(seed) = only_seed {
        jobs.retain(|(_, candidate, _)| *candidate == seed);
        if jobs.is_empty() {
            return Err(format!("Unknown ensemble seed {seed}").into());
        }
    }
    let standard_count = jobs
        .iter()
        .take_while(|(case, _, _)| *case == "standard")
        .count();
    let threads = threads.min(jobs.len());
    fs::create_dir_all(&output)?;
    let partial_path = output.join(".ensemble-in-progress.jsonl");
    let partial = Mutex::new(BufWriter::new(fs::File::create(&partial_path)?));
    let next_job = AtomicUsize::new(0);
    let mut runs = thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| -> Result<Vec<_>, String> {
                    let mut runs = Vec::new();
                    loop {
                        let i = next_job.fetch_add(1, Ordering::Relaxed);
                        let Some((case, seed, settings)) = jobs.get(i) else {
                            return Ok(runs);
                        };
                        let run = checked_run(case, seed, *settings)
                            .map_err(|error| format!("{case} {seed}: {error}"))?;
                        {
                            let mut writer = partial.lock().unwrap();
                            serde_json::to_writer(&mut *writer, &run).map_err(|e| e.to_string())?;
                            writeln!(writer)
                                .and_then(|_| writer.flush())
                                .map_err(|e| e.to_string())?;
                        }
                        eprintln!("{case} {seed}: {}", run.summary_hash);
                        runs.push((i, run));
                    }
                })
            })
            .collect();
        let mut runs = Vec::new();
        for worker in workers {
            runs.extend(worker.join().expect("ensemble worker panicked")?);
        }
        Ok::<_, String>(runs)
    })?;
    runs.sort_unstable_by_key(|(i, _)| *i);
    let runs: Vec<_> = runs.into_iter().map(|(_, run)| run).collect();
    let (standard, extremes) = runs.split_at(standard_count);
    write_report(&output, threads, standard, extremes)?;
    drop(partial);
    fs::remove_file(partial_path)?;
    Ok(())
}
