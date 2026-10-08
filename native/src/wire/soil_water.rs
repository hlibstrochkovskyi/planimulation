//! Protocol 15: real unified owners, paired histories and completed interval transfers.
use super::{MAX_SEASONAL_CHECKPOINT_BYTES, f64s, send_versioned};
use crate::{
    moisture_transport::total_mass,
    seasonal_moisture::regional_soil::{Model, NumericalPolicy, Settings, State, Step},
    surface_water::ponded_soil::Mass,
};
use serde_json::json;
use std::io::Write;

pub fn soil_moisture_settings() -> Settings {
    Settings {
        numerical_policy: NumericalPolicy::RetainDonor,
        ..Default::default()
    }
}
fn check_mode(model: &Model) -> Result<(), String> {
    if model.settings() != soil_moisture_settings()
        || model.temperature_settings() != Default::default()
        || model.wind_settings() != Default::default()
    {
        return Err("Soil-water desktop transport requires its pinned product settings.".into());
    }
    Ok(())
}
fn delta(after: Mass, before: Mass) -> Result<f64, String> {
    let value = total_mass(&[after.high - before.high, after.low, -before.low]);
    if !value.is_finite() || value < 0. {
        return Err("Soil-water cumulative transfer decreased or overflowed.".into());
    }
    Ok(value)
}
pub fn soil_moisture_checkpoint(
    out: &mut impl Write,
    model: &Model,
    state: &State,
) -> Result<(), String> {
    check_mode(model)?;
    model.budget(state)?;
    let bytes = serde_json::to_vec(&state.checkpoint()).map_err(|e| e.to_string())?;
    if bytes.len() >= MAX_SEASONAL_CHECKPOINT_BYTES {
        return Err("Seasonal checkpoint exceeds 64 MiB.".into());
    }
    send_versioned(
        out,
        json!({"kind":"soilMoistureCheckpoint","schemaVersion":1,
        "modelVersion":"regional-seasonal-water-1","elapsedSeconds":state.elapsed_seconds()}),
        &bytes,
        15,
    )
    .map_err(|e| e.to_string())
}

/// 37 regional f64 fields, then unique reference-body high/low fields.
/// No geometry or synthetic legacy terminal/liquid-runoff fields are included.
pub fn soil_moisture(
    out: &mut impl Write,
    model: &Model,
    state: &State,
    before: Option<&State>,
    step: Option<&Step>,
) -> Result<(), String> {
    check_mode(model)?;
    if before.is_some() != step.is_some() {
        return Err("Soil-water interval requires both source state and step.".into());
    }
    let seconds = if let Some(before) = before {
        model.budget(before)?;
        let dt = state
            .elapsed_seconds()
            .checked_sub(before.elapsed_seconds())
            .ok_or("Soil-water interval clock decreased.")?;
        if !(1..=86400).contains(&dt) {
            return Err("Invalid soil-water display interval.".into());
        }
        dt
    } else {
        0
    };
    let view = model.observe(state)?;
    if let Some(step) = step
        && (step.budget != view.budget
            || step.coupled_substeps == 0
            || step.coupled_substeps > 4096
            || step.atmospheric_substeps < step.coupled_substeps
            || step.atmospheric_substeps > 4096 * step.coupled_substeps
            || step.surface_substeps < 2 * step.coupled_substeps
            || step.surface_substeps > 32768 * step.coupled_substeps)
    {
        return Err("Soil-water step does not match the observed state.".into());
    }
    let n = view.stocks.liquid.len();
    let b = view.reference_bodies.len();
    let mut bytes = Vec::with_capacity((37 * n + 2 * b) * 8);
    for stocks in [
        &view.stocks.liquid,
        &view.stocks.soil,
        &view.stocks.snow,
        &view.stocks.vapor,
        &view.stocks.drainage,
    ] {
        for low in [false, true] {
            f64s(
                &mut bytes,
                stocks.iter().map(|m| if low { m.low } else { m.high }),
            );
        }
    }
    for component in 0..7 {
        for low in [false, true] {
            f64s(
                &mut bytes,
                view.cumulative_local_transfers.iter().map(|t| {
                    let mass = t.values()[component];
                    if low { mass.low } else { mass.high }
                }),
            );
        }
    }
    for component in 0..7 {
        let values = (0..n)
            .map(|r| {
                before.map_or(Ok(0.), |old| {
                    delta(
                        state.local_transfers()[r].values()[component],
                        old.local_transfers()[r].values()[component],
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        f64s(&mut bytes, values.into_iter());
    }
    for field in [
        &view.regional_liquid_depth_meters,
        &view.visible_water_depth_meters,
        &view.visible_water_level_meters,
        &view.cumulative_surface_incoming_kilograms,
        &view.cumulative_surface_outgoing_kilograms,
    ] {
        f64s(&mut bytes, field.iter().copied());
    }
    let departure = (0..n)
        .map(|r| {
            before.map_or(Ok(0.), |old| {
                delta(state.drainage_sent()[r], old.drainage_sent()[r])
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    f64s(&mut bytes, departure.into_iter());
    for low in [false, true] {
        f64s(
            &mut bytes,
            view.reference_bodies
                .iter()
                .map(|b| if low { b.liquid.low } else { b.liquid.high }),
        );
    }
    let header = json!({"kind":"soilMoisture","regionCount":n,"referenceBodyCount":b,
        "modelVersion":view.model_version,"soilModelVersion":view.soil_model_version,
        "transportModelVersion":view.transport_model_version,"surfaceFlowModelVersion":view.surface_flow_model_version,
        "runoffModelVersion":view.runoff_model_version,"observationModelVersion":view.observation_model_version,
        "temperatureModelVersion":crate::seasonal_temperature::MODEL_VERSION,
        "windModelVersion":crate::seasonal_wind::MODEL_VERSION,
        "orographicModelVersion":crate::orographic_response::MODEL_VERSION,
        "referenceBodyModelVersion":"reference-water-pool-1",
        "settings":view.settings,"temperatureSettings":model.temperature_settings(),"windSettings":model.wind_settings(),
        "elapsedSeconds":view.elapsed_seconds,"intervalSeconds":seconds,"budget":view.budget,"resolution":view.resolution,
        "coupledSubsteps":step.map_or(0,|s|s.coupled_substeps),
        "atmosphericSubsteps":step.map_or(0,|s|s.atmospheric_substeps),"surfaceSubsteps":step.map_or(0,|s|s.surface_substeps)});
    send_versioned(out, header, &bytes, 15).map_err(|e| e.to_string())
}
