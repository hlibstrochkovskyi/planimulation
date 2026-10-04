use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{
        Checkpoint, Model, Settings, SoilNumerics, SurfaceNumerics, TERMINAL_STOCK_MODEL_VERSION,
        TerminalNumerics,
    },
    wire,
};

fn model(limit: u32) -> Model {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.subdivision = 2;
    recipe.radius_meters = 1_000_000.;
    Model::from_world(
        &World::generate(recipe).unwrap(),
        Settings {
            orography: Some(Default::default()),
            soil_numerics: Some(SoilNumerics::Compensated),
            surface_numerics: Some(SurfaceNumerics::Compensated),
            terminal_numerics: Some(TerminalNumerics::Compensated),
            max_coupled_step_seconds: limit,
            ..Default::default()
        },
        Default::default(),
        Default::default(),
    )
    .unwrap()
}

#[test]
fn terminal_components_survive_checkpoint_and_exact_continuation() {
    let model = model(3600);
    let mut state = model.initial_state();
    for _ in 0..31 {
        model.advance(&mut state, 86400).unwrap();
    }
    assert!(
        state
            .terminal_low_kilograms()
            .unwrap()
            .iter()
            .any(|v| *v != 0.)
    );
    let text = serde_json::to_string(&state.checkpoint()).unwrap();
    let (resumed_model, mut resumed) =
        Model::restore(serde_json::from_str(&text).unwrap()).unwrap();
    assert_eq!(serde_json::to_string(&resumed.checkpoint()).unwrap(), text);
    model.advance(&mut state, 3600).unwrap();
    resumed_model.advance(&mut resumed, 3600).unwrap();
    assert_eq!(resumed, state);
}

#[test]
fn terminal_settings_shapes_pins_and_day_zero_cannot_be_forged() {
    let cp = model(60).initial_state().checkpoint();
    assert_eq!(cp.schema_version, 7);
    assert_eq!(cp.model_version, "seasonal-moisture-7");
    assert_eq!(
        cp.terminal_stock_model_version.as_deref(),
        Some(TERMINAL_STOCK_MODEL_VERSION)
    );
    for edit in 0..8 {
        let mut bad = cp.clone();
        match edit {
            0 => bad.terminal_low_kilograms = None,
            1 => {
                bad.terminal_low_kilograms.as_mut().unwrap().pop();
            }
            2 => bad.terminal_low_kilograms.as_mut().unwrap()[0] = f64::NAN,
            3 => bad.terminal_low_kilograms.as_mut().unwrap()[0] = f64::from_bits(1),
            4 => bad.terminal_stock_model_version = Some("terminal-stock-compensated-0".into()),
            5 => bad.settings.terminal_numerics = None,
            6 => bad.surface_low_kilograms = None,
            _ => {
                bad.schema_version = 6;
                bad.model_version = "seasonal-moisture-6".into();
            }
        }
        assert!(Model::restore(bad).is_err(), "edit {edit}");
    }
    for key in ["terminalLowKilograms", "terminalStockModelVersion"] {
        let mut value = serde_json::to_value(&cp).unwrap();
        value[key] = serde_json::Value::Null;
        assert!(serde_json::from_value::<Checkpoint>(value).is_err());
    }
    assert!(
        Settings {
            terminal_numerics: Some(TerminalNumerics::Compensated),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    let mut legacy = cp;
    legacy.settings.terminal_numerics = None;
    legacy.terminal_low_kilograms = None;
    legacy.terminal_stock_model_version = None;
    legacy.schema_version = 6;
    legacy.model_version = "seasonal-moisture-6".into();
    let (legacy_model, legacy_state) = Model::restore(legacy).unwrap();
    let encoded = serde_json::to_value(legacy_state.checkpoint()).unwrap();
    assert!(encoded.get("terminalLowKilograms").is_none());
    assert!(encoded.get("terminalStockModelVersion").is_none());
    assert!(encoded["settings"].get("terminalNumerics").is_none());
    let mut bytes = Vec::new();
    assert!(wire::seasonal_checkpoint(&mut bytes, &legacy_model, &legacy_state).is_err());
    assert!(bytes.is_empty());
}

fn packet(bytes: &[u8]) -> (serde_json::Value, &[u8]) {
    let count = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    (
        serde_json::from_slice(&bytes[4..4 + count]).unwrap(),
        &bytes[4 + count..],
    )
}
#[test]
fn protocol_thirteen_exports_complete_components_but_only_rounded_display_fields() {
    let model = model(3600);
    let mut state = model.initial_state();
    let step = model.advance(&mut state, 86400).unwrap();
    let before = state.clone();
    let mut bytes = Vec::new();
    wire::seasonal_moisture(&mut bytes, &model, &state, Some(&step), 86400).unwrap();
    let (header, body) = packet(&bytes);
    assert_eq!(header["kind"], "preciseMoisture");
    assert_eq!(header["protocol"], 13);
    assert_eq!(
        header["terminalStockModelVersion"],
        TERMINAL_STOCK_MODEL_VERSION
    );
    assert_eq!(body.len(), state.surface_kilograms().len() * 18 * 8);
    for (index, value) in state.owned_stocks().enumerate() {
        assert_eq!(
            *value,
            f64::from_le_bytes(body[index * 8..index * 8 + 8].try_into().unwrap())
        );
    }
    bytes.clear();
    wire::seasonal_checkpoint(&mut bytes, &model, &state).unwrap();
    let (header, body) = packet(&bytes);
    assert_eq!(header["kind"], "preciseMoistureCheckpoint");
    assert_eq!(header["schemaVersion"], 7);
    assert_eq!(
        serde_json::from_slice::<Checkpoint>(body).unwrap(),
        state.checkpoint()
    );
    assert_eq!(state, before);
}

#[test]
fn terminal_precision_keeps_batching_and_invalid_advance_atomic() {
    let model = model(60);
    let mut daily = model.initial_state();
    let mut hourly = daily.clone();
    let mut observed = 0;
    model
        .advance_terminal_observed(&mut daily, 86400, |_| observed += 1)
        .unwrap();
    assert!(observed > 0);
    for _ in 0..24 {
        model.advance(&mut hourly, 3600).unwrap();
    }
    assert_eq!(daily, hourly);
    let saved = daily.clone();
    assert!(model.advance(&mut daily, 86401).is_err());
    assert_eq!(daily, saved);
}
