//! Read-only accepted-step seasonal summaries and matched parameter interventions.
//! This report never updates model stocks, forcing, tolerances, or checkpoints.
use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::{
        Model, SECONDS_PER_DAY, Settings, SoilNumerics, State, Step, SurfaceNumerics,
        TerminalNumerics,
    },
};
use serde_json::{Value, json};

const CALLER_SECONDS: u32 = 3600;
const YEAR_SECONDS: u64 = 365 * SECONDS_PER_DAY;

#[derive(Clone, Copy, Debug, Default)]
struct Sum {
    value: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, value: f64) {
        let adjusted = value - self.correction;
        let next = self.value + adjusted;
        self.correction = (next - self.value) - adjusted;
        self.value = next;
    }
}

fn month_at(seconds: u64) -> (u64, usize) {
    (
        seconds / YEAR_SECONDS,
        ((seconds % YEAR_SECONDS) / SECONDS_PER_DAY * 12 / 365) as usize,
    )
}
fn month_bounds(year: u64, month: usize) -> (u64, u64) {
    (
        year * YEAR_SECONDS + (month as u64 * 365).div_ceil(12) * SECONDS_PER_DAY,
        year * YEAR_SECONDS + ((month as u64 + 1) * 365).div_ceil(12) * SECONDS_PER_DAY,
    )
}

#[derive(Clone, Debug)]
struct Month {
    year: u64,
    month: usize,
    start: u64,
    end: u64,
    seconds: u64,
    // Global, initially dry land, positive-uplift land, other land.
    areas: [f64; 4],
    precipitation: [Sum; 4],
    runoff: [Sum; 4],
    soil_mass_seconds: [Sum; 4],
    snow_mass_seconds: [Sum; 4],
    evaporation: Sum,
    rainfall: Sum,
    snowfall: Sum,
}
impl Month {
    fn new(year: u64, month: usize, world: &World, uplift: &[f64]) -> Self {
        let (start, end) = month_bounds(year, month);
        let mut areas = [Sum::default(); 4];
        for (i, &area) in world.surface.areas.iter().enumerate() {
            areas[0].add(area);
            if world.water.depth_meters[i] == 0. {
                areas[1].add(area);
                areas[if uplift[i] > 0. { 2 } else { 3 }].add(area);
            }
        }
        Self {
            year,
            month,
            start,
            end,
            seconds: 0,
            areas: areas.map(|s| s.value),
            precipitation: [Sum::default(); 4],
            runoff: [Sum::default(); 4],
            soil_mass_seconds: [Sum::default(); 4],
            snow_mass_seconds: [Sum::default(); 4],
            evaporation: Sum::default(),
            rainfall: Sum::default(),
            snowfall: Sum::default(),
        }
    }
    fn value(&self) -> Value {
        let groups: Vec<_> = ["global", "initiallyDryLand", "positiveUpliftLand", "nonpositiveUpliftLand"]
            .iter().enumerate().map(|(i, name)| {
                let area = self.areas[i];
                json!({"group":name,"areaSquareMeters":area,
                    "precipitationTotalMillimetersWaterEquivalent":(area>0.).then(||self.precipitation[i].value/area),
                    "generatedRunoffTotalMillimetersWaterEquivalent":(area>0.).then(||self.runoff[i].value/area),
                    "rightEndpointTimeMeanSoilMillimetersWaterEquivalent":(area>0. && self.seconds>0).then(||self.soil_mass_seconds[i].value/area/self.seconds as f64),
                    "rightEndpointTimeMeanSnowMillimetersWaterEquivalent":(area>0. && self.seconds>0).then(||self.snow_mass_seconds[i].value/area/self.seconds as f64)})
            }).collect();
        json!({"yearIndex":self.year,"monthIndex":self.month,"startSeconds":self.start,"endSeconds":self.end,
            "acceptedSeconds":self.seconds,"complete":self.seconds==self.end-self.start,"groups":groups,
            "globalRainTotalMillimetersWaterEquivalent":self.rainfall.value/self.areas[0],
            "globalSnowfallTotalMillimetersWaterEquivalent":self.snowfall.value/self.areas[0],
            "globalEvaporationTotalMillimetersWaterEquivalent":self.evaporation.value/self.areas[0]})
    }
}

struct SeasonalSummary {
    months: Vec<Month>,
    next_seconds: u64,
    precipitation: Vec<Sum>,
    runoff: Vec<Sum>,
    evaporation: Sum,
}
impl SeasonalSummary {
    fn new(region_count: usize) -> Self {
        Self {
            months: Vec::new(),
            next_seconds: 0,
            precipitation: vec![Sum::default(); region_count],
            runoff: vec![Sum::default(); region_count],
            evaporation: Sum::default(),
        }
    }
    /// A complete accepted caller interval must belong to a single calendar bin.
    /// Do not divide an already integrated cross-month flow by elapsed fractions.
    fn record(
        &mut self,
        world: &World,
        model: &Model,
        state: &State,
        step: &Step,
        seconds: u32,
    ) -> Result<(), String> {
        let end = state.elapsed_seconds();
        let start = end
            .checked_sub(u64::from(seconds))
            .ok_or("Invalid summary interval.")?;
        let count = world.surface.areas.len();
        if seconds == 0
            || start != self.next_seconds
            || month_at(start) != month_at(end - 1)
            || self.precipitation.len() != count
            || step.precipitation_kilograms.len() != count
            || step.evaporation_kilograms.len() != count
            || step.surface_transfers.len() != count
            || state.soil_kilograms().len() != count
            || state.snow_kilograms().len() != count
        {
            return Err(
                "Summary requires contiguous accepted intervals wholly inside one month.".into(),
            );
        }
        let (year, month) = month_at(start);
        let uplift = &model
            .orographic_uplift()
            .ok_or("Summary requires recorded terrain uplift.")?[month];
        if self
            .months
            .last()
            .is_none_or(|m| (m.year, m.month) != (year, month))
        {
            self.months.push(Month::new(year, month, world, uplift));
        }
        let current = self.months.last_mut().unwrap();
        for (i, &rain) in step.precipitation_kilograms.iter().enumerate() {
            let f = step.surface_transfers[i];
            let runoff = f.liquid_runoff + f.soil_drainage;
            self.precipitation[i].add(rain);
            self.runoff[i].add(runoff);
            self.evaporation.add(step.evaporation_kilograms[i]);
            current.evaporation.add(step.evaporation_kilograms[i]);
            current.rainfall.add(f.rain);
            current.snowfall.add(f.snowfall);
            let land = world.water.depth_meters[i] == 0.;
            let indices = [
                Some(0),
                land.then_some(1),
                land.then_some(if uplift[i] > 0. { 2 } else { 3 }),
            ];
            for group in indices.into_iter().flatten() {
                current.precipitation[group].add(rain);
                current.runoff[group].add(runoff);
                // Add signed components separately: they describe one owned mass.
                current.soil_mass_seconds[group]
                    .add(state.soil_kilograms()[i] * f64::from(seconds));
                current.soil_mass_seconds[group]
                    .add(state.soil_low_kilograms().map_or(0., |low| low[i]) * f64::from(seconds));
                current.snow_mass_seconds[group]
                    .add(state.snow_kilograms()[i] * f64::from(seconds));
                current.snow_mass_seconds[group]
                    .add(state.snow_low_kilograms().map_or(0., |low| low[i]) * f64::from(seconds));
            }
        }
        current.seconds += u64::from(seconds);
        self.next_seconds = end;
        Ok(())
    }
    fn value(&self, world: &World) -> Value {
        json!({"samplingIntervalSeconds":CALLER_SECONDS,"elapsedSeconds":self.next_seconds,
            "months":self.months.iter().map(Month::value).collect::<Vec<_>>(),
            "regionalTotals":self.precipitation.iter().zip(&self.runoff).enumerate().map(|(i,(p,r))|json!({
                "region":i,"initiallyDry":world.water.depth_meters[i]==0.,
                "precipitationMillimetersWaterEquivalent":p.value/world.surface.areas[i],
                "generatedRunoffMillimetersWaterEquivalent":r.value/world.surface.areas[i]})).collect::<Vec<_>>()})
    }
}

fn settings() -> Settings {
    Settings {
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        max_coupled_step_seconds: 900,
        ..Default::default()
    }
}
fn variants(extended: bool) -> Vec<(&'static str, Settings)> {
    let baseline = settings();
    let mut zero = baseline;
    zero.orography.as_mut().unwrap().strength = 0.;
    let mut strong = baseline;
    strong.orography.as_mut().unwrap().strength = 2.;
    let mut cases = vec![
        ("baseline", baseline),
        ("noExtraUpslope", zero),
        ("doubleUpslopeStrength", strong),
    ];
    if extended {
        let mut fast = baseline;
        fast.orography
            .as_mut()
            .unwrap()
            .uplift_response_height_meters = 500.;
        let mut slow = baseline;
        slow.orography
            .as_mut()
            .unwrap()
            .uplift_response_height_meters = 2000.;
        let mut shallow = baseline;
        shallow.surface.soil_capacity_kilograms_per_square_meter = 75.;
        let mut deep = baseline;
        deep.surface.soil_capacity_kilograms_per_square_meter = 300.;
        cases.extend([
            ("halfUpslopeResponseHeight", fast),
            ("doubleUpslopeResponseHeight", slow),
            ("halfSoilCapacity", shallow),
            ("doubleSoilCapacity", deep),
        ]);
    }
    cases
}

struct Run {
    state: State,
    summary: SeasonalSummary,
    report: Value,
    passed: bool,
}
fn run(world: &World, settings: Settings, days: u32) -> Result<Run, String> {
    if !(1..=730).contains(&days) {
        return Err("Seasonal diagnostic duration must be 1–730 days.".into());
    }
    let model = Model::from_world(world, settings, Default::default(), Default::default())?;
    let mut state = model.initial_state();
    let mut summary = SeasonalSummary::new(world.surface.areas.len());
    let mut maximum_global: f64 = 0.;
    let mut maximum_local: f64 = 0.;
    let mut failure = None;
    let mut atomic = None;
    for _ in 0..days * 24 {
        // Copy only the accepted state needed for a possible atomicity witness.
        let before = state.clone();
        match model.advance(&mut state, CALLER_SECONDS) {
            Ok(step) => {
                maximum_global = maximum_global.max(
                    step.budget.residual_kilograms.abs()
                        / step.budget.initial_mobile_water_kilograms.max(1.),
                );
                maximum_local =
                    maximum_local.max(step.budget.maximum_relative_local_surface_ledger_residual);
                summary.record(world, &model, &state, &step, CALLER_SECONDS)?;
            }
            Err(error) => {
                atomic = Some(before == state);
                failure = Some(error);
                break;
            }
        }
    }
    let budget = model.budget(&state)?;
    let precipitation = total_mass(
        &summary
            .precipitation
            .iter()
            .map(|s| s.value)
            .collect::<Vec<_>>(),
    );
    let generated_runoff = total_mass(&summary.runoff.iter().map(|s| s.value).collect::<Vec<_>>());
    let expected_runoff = budget.cumulative_surface_transfers.liquid_runoff
        + budget.cumulative_surface_transfers.soil_drainage;
    let reconciliation = [
        precipitation - budget.cumulative_precipitation_kilograms,
        summary.evaporation.value - budget.cumulative_evaporation_kilograms,
        generated_runoff - expected_runoff,
    ];
    let scale = [
        budget.cumulative_precipitation_kilograms,
        budget.cumulative_evaporation_kilograms,
        expected_runoff,
    ];
    let reconciled = reconciliation
        .iter()
        .zip(scale)
        .all(|(&d, s)| d.abs() <= 1e-12 * s.max(1.));
    let checkpoint = state.checkpoint();
    let text = serde_json::to_string(&checkpoint).map_err(|e| e.to_string())?;
    let (restored_model, mut restored) =
        Model::restore(serde_json::from_str(&text).map_err(|e| e.to_string())?)?;
    let roundtrip = state == restored
        && text == serde_json::to_string(&restored.checkpoint()).map_err(|e| e.to_string())?;
    let mut continued = state.clone();
    let a = model.advance(&mut continued, CALLER_SECONDS);
    let b = restored_model.advance(&mut restored, CALLER_SECONDS);
    let continued_successfully = a.is_ok() && b.is_ok();
    let continuation = a.as_ref().map(|_| ()).map_err(String::as_str)
        == b.as_ref().map(|_| ()).map_err(String::as_str)
        && continued == restored;
    let passed =
        failure.is_none() && reconciled && roundtrip && continuation && continued_successfully;
    let report = json!({"requestedDays":days,"elapsedSeconds":state.elapsed_seconds(),"passed":passed,
        "modelVersion":model.model_version(),"schemaVersion":model.checkpoint_schema_version(),
        "surfaceModelVersion":model.surface_model_version(),"terminalStockModelVersion":checkpoint.terminal_stock_model_version,
        "transportModelVersion":checkpoint.transport_model_version,"runoffModelVersion":checkpoint.runoff_model_version,
        "temperatureModelVersion":checkpoint.temperature_model_version,"windModelVersion":checkpoint.wind_model_version,
        "orographicModelVersion":checkpoint.orographic_model_version,
        "settings":settings,"temperatureSettings":model.temperature_settings(),"windSettings":model.wind_settings(),
        "actualMaximumCoupledStepSeconds":model.maximum_coupled_step_seconds()?,"budget":budget,
        "maximumRelativeMassResidual":maximum_global,"maximumRelativeLocalLedgerResidual":maximum_local,
        "acceptedSummaryReconciliationKilograms":{"precipitation":reconciliation[0],"evaporation":reconciliation[1],"generatedRunoff":reconciliation[2]},
        "acceptedSummaryReconciled":reconciled,"checkpointRoundTripExact":roundtrip,"checkpointContinuationExact":continuation,
        "checkpointContinuationSucceeded":continued_successfully,
        "failure":failure,"failedIntervalAtomic":atomic,"seasonalSummary":summary.value(world)});
    Ok(Run {
        state,
        summary,
        report,
        passed,
    })
}

fn paired(world: &World, baseline: &Run, changed: &Run) -> Value {
    if !baseline.passed
        || !changed.passed
        || baseline.state.elapsed_seconds() != changed.state.elapsed_seconds()
        || baseline.report["actualMaximumCoupledStepSeconds"]
            != changed.report["actualMaximumCoupledStepSeconds"]
    {
        return json!({"available":false,"reason":"Both complete runs with identical actual coupling are required."});
    }
    let difference = baseline
        .state
        .owned_stocks()
        .zip(changed.state.owned_stocks())
        .map(|(a, b)| (a - b).abs())
        .sum::<f64>();
    let mut lows = 0.;
    for (a, b) in [
        (
            baseline.state.soil_low_kilograms(),
            changed.state.soil_low_kilograms(),
        ),
        (
            baseline.state.snow_low_kilograms(),
            changed.state.snow_low_kilograms(),
        ),
        (
            baseline.state.surface_low_kilograms(),
            changed.state.surface_low_kilograms(),
        ),
        (
            baseline.state.terminal_low_kilograms(),
            changed.state.terminal_low_kilograms(),
        ),
    ] {
        lows += a
            .unwrap()
            .iter()
            .zip(b.unwrap())
            .map(|(a, b)| (a - b).abs())
            .sum::<f64>();
    }
    let regions: Vec<_>=world.surface.areas.iter().enumerate().map(|(i,&area)|json!({"region":i,
        "precipitationChangeMillimetersWaterEquivalent":(changed.summary.precipitation[i].value-baseline.summary.precipitation[i].value)/area,
        "generatedRunoffChangeMillimetersWaterEquivalent":(changed.summary.runoff[i].value-baseline.summary.runoff[i].value)/area})).collect();
    json!({"available":true,"direction":"changed minus baseline at identical region IDs",
        "relativeEndpointStockComponentL1Bound":(difference+lows)/baseline.report["budget"]["initialMobileWaterKilograms"].as_f64().unwrap().max(1.),
        "regionalChanges":regions})
}

fn study(smoke: bool, refined: bool) -> Result<Value, String> {
    let base: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .map_err(|e| e.to_string())?;
    let mut cases = Vec::new();
    let mut worlds = Vec::new();
    let mut failures = 0;
    for seed in if smoke {
        vec!["seasonal-reference"]
    } else {
        vec!["seasonal-reference", "moisture-coast", "moisture-interior"]
    } {
        let mut recipe = base.clone();
        recipe.seed = seed.into();
        recipe.subdivision = if smoke { 1 } else { 2 };
        recipe.radius_meters = 1_000_000.;
        let world = World::generate(recipe.clone())?;
        let inputs = variants(seed == "seasonal-reference");
        // Resolve one shared bound before any run, including stronger responses.
        let mut common = if refined { 450 } else { 900 };
        for (_, s) in &inputs {
            common = common.min(
                Model::from_world(&world, *s, Default::default(), Default::default())?
                    .maximum_coupled_step_seconds()? as u32,
            );
        }
        let reference =
            Model::from_world(&world, inputs[0].1, Default::default(), Default::default())?;
        let uplift = reference.orographic_uplift().unwrap();
        worlds.push(json!({"recipe":recipe,"regionCount":world.surface.areas.len(),
            "regionalGeography":world.surface.areas.iter().enumerate().map(|(i,&area)|json!({
                "region":i,"areaSquareMeters":area,"initiallyDry":world.water.depth_meters[i]==0.,
                "bedElevationMeters":world.terrain.elevation[i],
                "airFacingElevationMeters":if world.water.depth_meters[i]==0. {world.terrain.elevation[i]} else {world.water.level_meters},
                "monthlyUpliftMetersPerSecond":uplift.iter().map(|month|month[i]).collect::<Vec<_>>()
            })).collect::<Vec<_>>() }));
        let mut baseline = None;
        for (name, mut settings) in inputs {
            settings.max_coupled_step_seconds = common;
            eprintln!("Seasonal sensitivity: {seed}, {name}, coupled bound {common} s");
            match run(&world, settings, if smoke { 32 } else { 730 }) {
                Ok(result) => {
                    let comparison = if name == "baseline" {
                        None
                    } else {
                        Some(baseline.as_ref().map_or_else(
                            || json!({"available":false,"reason":"No accepted baseline is available."}),
                            |b| paired(&world, b, &result),
                        ))
                    };
                    let qualified = result.passed
                        && comparison
                            .as_ref()
                            .is_none_or(|value| value["available"] == true);
                    failures += usize::from(!qualified);
                    cases.push(
                        json!({"recipe":recipe,"intervention":name,"resolvedSettings":settings,"qualified":qualified,
                        "report":result.report,"pairedWithBaseline":comparison}),
                    );
                    if name == "baseline" {
                        baseline = Some(result);
                    }
                }
                Err(error) => {
                    failures += 1;
                    cases.push(json!({"recipe":recipe,"intervention":name,"resolvedSettings":settings,"failure":error}));
                }
            }
        }
    }
    Ok(
        json!({"reportVersion":1,"suite":if smoke {"32-day smoke"} else if refined {"two-year refined matched seasonal sensitivity"} else {"two-year matched seasonal sensitivity"},
        "qualificationPassed":failures==0,"failureCount":failures,"worlds":worlds,"cases":cases,
        "scope":"Accepted hourly observations only. Numbered month totals retain separate year indices; no climatology or spin-up qualification. Soil/snow means use right-endpoint quadrature with signed low components. Generated runoff is donor production, not routed gross edge departures or hydraulic discharge. Fixed-world parameter interventions use shared actual coupling; spatial groups use raw monthly uplift and initial dry land, not inferred rain-shadow labels. No model/default/protocol change, physical calibration, cross-resolution convergence, or universal monotonic response is claimed."}),
    )
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (smoke, refined) = match args.as_slice() {
        [] => (false, false),
        [arg] if arg == "--smoke" => (true, false),
        [arg] if arg == "--refined" => (false, true),
        _ => return Err("Usage: seasonal_sensitivity_report [--smoke | --refined]".into()),
    };
    let report = study(smoke, refined)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    if report["qualificationPassed"] != true {
        return Err("Seasonal sensitivity contains retained failures; inspect its JSON.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> World {
        let mut recipe: Recipe = serde_json::from_str(include_str!(
            "../../docs/scenarios/seasonal-temperature.json"
        ))
        .unwrap();
        recipe.subdivision = 1;
        recipe.radius_meters = 1_000_000.;
        World::generate(recipe).unwrap()
    }
    #[test]
    fn numbered_months_partition_each_year_without_boundary_leakage() {
        for year in 0..3 {
            let mut seconds = 0;
            for month in 0..12 {
                let (start, end) = month_bounds(year, month);
                assert_eq!(month_at(start), (year, month));
                assert_eq!(month_at(end - 1), (year, month));
                assert_eq!(
                    month_at(end),
                    if month == 11 {
                        (year + 1, 0)
                    } else {
                        (year, month + 1)
                    }
                );
                seconds += end - start;
            }
            assert_eq!(seconds, YEAR_SECONDS);
        }
    }
    #[test]
    fn summaries_are_read_only_and_reconcile_with_accepted_ledgers() {
        let world = world();
        let first = run(&world, settings(), 32).unwrap();
        assert!(first.passed);
        assert_eq!(first.summary.months.len(), 2);
        assert!(
            first.summary.months[0].value()["complete"]
                .as_bool()
                .unwrap()
        );
        assert!(
            !first.summary.months[1].value()["complete"]
                .as_bool()
                .unwrap()
        );
        let model =
            Model::from_world(&world, settings(), Default::default(), Default::default()).unwrap();
        let mut independent = model.initial_state();
        for _ in 0..32 * 24 {
            model.advance(&mut independent, CALLER_SECONDS).unwrap();
        }
        assert_eq!(first.state, independent);
        let second = run(&world, settings(), 32).unwrap();
        assert_eq!(first.report, second.report);
        assert_eq!(
            paired(&world, &first, &second)["relativeEndpointStockComponentL1Bound"],
            0.
        );
    }
    #[test]
    fn repeated_or_cross_month_observations_reject_before_summary_mutation() {
        let world = world();
        let model =
            Model::from_world(&world, settings(), Default::default(), Default::default()).unwrap();
        let mut state = model.initial_state();
        let mut summary = SeasonalSummary::new(world.surface.areas.len());
        let step = model.advance(&mut state, CALLER_SECONDS).unwrap();
        summary
            .record(&world, &model, &state, &step, CALLER_SECONDS)
            .unwrap();
        let saved = summary.value(&world);
        assert!(
            summary
                .record(&world, &model, &state, &step, CALLER_SECONDS)
                .is_err()
        );
        assert_eq!(summary.value(&world), saved);
        assert!(summary.record(&world, &model, &state, &step, 0).is_err());
        assert_eq!(summary.value(&world), saved);
        let mut summary = SeasonalSummary::new(world.surface.areas.len());
        summary.next_seconds = 31 * SECONDS_PER_DAY - 3600;
        for _ in 0..31 * 24 - 2 {
            model.advance(&mut state, CALLER_SECONDS).unwrap();
        }
        let step = model.advance(&mut state, 7200).unwrap();
        let saved = summary.value(&world);
        assert!(summary.record(&world, &model, &state, &step, 7200).is_err());
        assert_eq!(summary.value(&world), saved);
    }
    #[test]
    fn zero_area_groups_are_null_and_not_invented_zero_rain() {
        let world = world();
        let month = Month::new(0, 0, &world, &vec![1.; world.surface.areas.len()]);
        let value = month.value();
        let group = &value["groups"][3];
        assert_eq!(group["areaSquareMeters"], 0.);
        assert!(group["precipitationTotalMillimetersWaterEquivalent"].is_null());
        assert!(group["rightEndpointTimeMeanSoilMillimetersWaterEquivalent"].is_null());
    }

    #[test]
    fn sampled_columns_use_physical_area_and_time_not_region_counts() {
        let world = world();
        let mut month = Month::new(0, 0, &world, &vec![1.; world.surface.areas.len()]);
        month.areas[1] = 8.;
        month.seconds = 10;
        month.precipitation[1].add(24.);
        month.runoff[1].add(16.);
        month.soil_mass_seconds[1].add(320.);
        month.snow_mass_seconds[1].add(80.);
        let value = month.value();
        let group = &value["groups"][1];
        assert_eq!(group["precipitationTotalMillimetersWaterEquivalent"], 3.);
        assert_eq!(group["generatedRunoffTotalMillimetersWaterEquivalent"], 2.);
        assert_eq!(
            group["rightEndpointTimeMeanSoilMillimetersWaterEquivalent"],
            4.
        );
        assert_eq!(
            group["rightEndpointTimeMeanSnowMillimetersWaterEquivalent"],
            1.
        );
    }

    #[test]
    fn matched_parameter_controls_do_not_confuse_rate_identity_with_independent_parameters() {
        let world = world();
        let variants = variants(true);
        let strong = run(&world, variants[2].1, 8).unwrap();
        let half_height = run(&world, variants[3].1, 8).unwrap();
        assert!(strong.passed && half_height.passed);
        assert_ne!(strong.report["settings"], half_height.report["settings"]);
        assert_eq!(
            strong.report["seasonalSummary"],
            half_height.report["seasonalSummary"]
        );
        assert_eq!(strong.report["budget"], half_height.report["budget"]);
        assert_eq!(
            paired(&world, &strong, &half_height)["relativeEndpointStockComponentL1Bound"],
            0.
        );
        let mut failed = half_height;
        failed.passed = false;
        assert_eq!(paired(&world, &strong, &failed)["available"], false);
        assert!(run(&world, settings(), 0).is_err());
        assert!(run(&world, settings(), 731).is_err());
    }
}
