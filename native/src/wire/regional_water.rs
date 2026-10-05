//! Explicit protocol 14 for regional columns and finite reference-body owners.
//! Old seasonal writers keep refusing this different ownership contract.
use super::{MAX_SEASONAL_CHECKPOINT_BYTES, f64s, send_versioned};
use crate::seasonal_moisture::{
    ClosedLakeExchange, Model, REGIONAL_SURFACE_MODEL_VERSION, ReferenceWaterPool, Settings,
    SoilNumerics, State, Step, SurfaceNumerics, TerminalNumerics, surface_flow,
};
use serde_json::json;
use std::io::Write;

/// Product-adapter defaults, separate from the legacy simulation defaults.
pub fn regional_moisture_settings() -> Settings {
    Settings {
        initial_active_surface_depth_meters: 10.,
        max_coupled_step_seconds: 900,
        orography: Some(Default::default()),
        soil_numerics: Some(SoilNumerics::Compensated),
        surface_numerics: Some(SurfaceNumerics::Compensated),
        terminal_numerics: Some(TerminalNumerics::Compensated),
        reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
        closed_lake_exchange: Some(ClosedLakeExchange::FrozenRegionalSurfaceFlow(
            Default::default(),
        )),
        ..Default::default()
    }
}
fn check_mode(model: &Model) -> Result<(), String> {
    if model.model_version() != REGIONAL_SURFACE_MODEL_VERSION
        || model.checkpoint_schema_version() != 15
    {
        return Err("Regional water protocol requires model/schema 15.".into());
    }
    Ok(())
}
pub fn regional_moisture_checkpoint(
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
        json!({"kind":"regionalMoistureCheckpoint","schemaVersion":15,
        "modelVersion":model.model_version(),"elapsedSeconds":state.elapsed_seconds()}),
        &bytes,
        14,
    )
    .map_err(|e| e.to_string())
}

/// 22 regional f64 fields, then reference-body high/low fields. Body slots are
/// ascending nonzero initial body IDs; never duplicate their stock per wet cell.
pub fn regional_moisture(
    out: &mut impl Write,
    model: &Model,
    state: &State,
    step: Option<&Step>,
    interval_seconds: u32,
) -> Result<(), String> {
    check_mode(model)?;
    let count = state.surface_kilograms().len();
    if interval_seconds > 86400
        || u64::from(interval_seconds) > state.elapsed_seconds()
        || step.is_some() != (interval_seconds > 0)
        || step.is_some_and(|s| {
            s.surface_transfers.len() != count
                || s.runoff_transfers.len() != count
                || s.regional_surface_flow.is_none()
        })
    {
        return Err("Invalid regional-water display interval or transfer shape.".into());
    }
    let budget = model.budget(state)?;
    let view = model.regional_surface_observation(state)?;
    let transfers = model.regional_surface_transfer_observation(state)?;
    let body_high = state.reference_body_high_kilograms().unwrap();
    let body_low = state.reference_body_low_kilograms().unwrap();
    let mut bytes = Vec::with_capacity((22 * count + 2 * body_high.len()) * 8);
    for field in [
        state.surface_kilograms(),
        state.snow_kilograms(),
        state.soil_kilograms(),
        state.pending_runoff_kilograms(),
        state.terminal_water_kilograms(),
        state.vapor_kilograms(),
    ] {
        f64s(&mut bytes, field.iter().copied());
    }
    for component in 0..8 {
        f64s(
            &mut bytes,
            (0..count).map(|i| step.map_or(0., |s| s.surface_transfers[i].values()[component])),
        );
    }
    for component in 0..4 {
        f64s(
            &mut bytes,
            (0..count).map(|i| step.map_or(0., |s| s.runoff_transfers[i].values()[component])),
        );
    }
    f64s(&mut bytes, view.depth_meters.iter().copied());
    // Dry regional slots use zero, guarded by the regional liquid mask. Reference geometry remains a separate
    // initial-world field and is not inferred from the finite mobile body stock.
    f64s(
        &mut bytes,
        view.levels_meters.iter().map(|l| l.unwrap_or(0.)),
    );
    f64s(
        &mut bytes,
        transfers.cumulative_incoming_kilograms.iter().copied(),
    );
    f64s(
        &mut bytes,
        transfers.cumulative_outgoing_kilograms.iter().copied(),
    );
    f64s(&mut bytes, body_high.iter().copied());
    f64s(&mut bytes, body_low.iter().copied());
    let empty_flow = surface_flow::FlowBudget::default();
    let flow = step
        .and_then(|s| s.regional_surface_flow.as_ref())
        .unwrap_or(&empty_flow);
    let header = json!({"kind":"regionalMoisture","regionCount":count,"referenceBodyCount":body_high.len(),
        "modelVersion":model.model_version(),"surfaceModelVersion":model.surface_model_version(),
        "runoffModelVersion":crate::runoff_transport::MODEL_VERSION,"transportModelVersion":crate::moisture_transport::MODEL_VERSION,
        "temperatureModelVersion":crate::seasonal_temperature::MODEL_VERSION,"windModelVersion":crate::seasonal_wind::MODEL_VERSION,
        "orographicModelVersion":crate::orographic_response::MODEL_VERSION,"terminalStockModelVersion":crate::seasonal_moisture::TERMINAL_STOCK_MODEL_VERSION,
        "referenceBodyModelVersion":"reference-water-pool-1","closedLakeModelVersion":surface_flow::MODEL_VERSION,
        "regionalSurfaceObservationVersion":view.observation_version,"regionalTransferObservationVersion":transfers.observation_version,
        "settings":model.settings(),"temperatureSettings":model.temperature_settings(),"windSettings":model.wind_settings(),
        "elapsedSeconds":state.elapsed_seconds(),"intervalSeconds":interval_seconds,"coupledSubsteps":step.map_or(0,|s|s.coupled_substeps),
        "transportSubsteps":step.map_or(0,|s|s.transport_substeps),
        "maximumAbsoluteLocalExchangeResidualKilograms":step.map_or(0.,|s|s.maximum_absolute_local_exchange_residual_kilograms),
        "maximumAbsoluteRoutingResidualKilograms":step.map_or(0.,|s|s.maximum_absolute_routing_residual_kilograms),
        "explicitStabilityBoundSeconds":view.explicit_stability_bound_seconds,"regionalSurfaceFlow":flow,
        "cumulativeFaceTransferKilograms":transfers.cumulative_transferred_kilograms,"budget":budget});
    send_versioned(out, header, &bytes, 14).map_err(|e| e.to_string())
}
