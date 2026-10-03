use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{Model, Step},
    wire,
};

fn world() -> World {
    let mut recipe: Recipe = serde_json::from_str(include_str!(
        "../../docs/scenarios/seasonal-temperature.json"
    ))
    .unwrap();
    recipe.subdivision = 2;
    World::generate(recipe).unwrap()
}

fn decode(bytes: &[u8]) -> (serde_json::Value, Vec<f64>) {
    let header_length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    let header = serde_json::from_slice(&bytes[4..4 + header_length]).unwrap();
    let body = bytes[4 + header_length..]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| f64::from_le_bytes(*chunk))
        .collect();
    (header, body)
}

fn assert_frame(
    model: &Model,
    state: &planimulation_core::seasonal_moisture::State,
    step: Option<&Step>,
    seconds: u32,
) {
    let mut bytes = Vec::new();
    wire::seasonal_moisture(&mut bytes, model, state, step, seconds).unwrap();
    let (header, body) = decode(&bytes);
    let n = state.surface_kilograms().len();
    assert_eq!(header["protocol"], 11);
    assert_eq!(header["kind"], "moisture");
    assert_eq!(header["modelVersion"], "seasonal-moisture-3");
    assert_eq!(header["byteLength"], n * 18 * 8);
    assert_eq!(header["regionCount"], n);
    assert_eq!(header["elapsedSeconds"], state.elapsed_seconds());
    assert_eq!(header["intervalSeconds"], seconds);
    assert_eq!(
        header["settings"],
        serde_json::to_value(model.settings()).unwrap()
    );
    assert_eq!(
        header["temperatureSettings"],
        serde_json::to_value(model.temperature_settings()).unwrap()
    );
    assert_eq!(
        header["windSettings"],
        serde_json::to_value(model.wind_settings()).unwrap()
    );
    assert_eq!(
        header["budget"],
        serde_json::to_value(model.budget(state).unwrap()).unwrap()
    );
    assert_eq!(body.len(), n * 18);
    assert_eq!(
        &body[..n * 6],
        state.owned_stocks().copied().collect::<Vec<_>>()
    );
    for component in 0..8 {
        let expected: Vec<_> = (0..n)
            .map(|i| step.map_or(0., |s| s.surface_transfers[i].values()[component]))
            .collect();
        assert_eq!(&body[(6 + component) * n..(7 + component) * n], expected);
    }
    for component in 0..4 {
        let expected: Vec<_> = (0..n)
            .map(|i| step.map_or(0., |s| s.runoff_transfers[i].values()[component]))
            .collect();
        assert_eq!(&body[(14 + component) * n..(15 + component) * n], expected);
    }
}

#[test]
fn display_wire_is_field_major_exact_headless_data_without_geometry_or_model_mutation() {
    let world = world();
    let initial_geometry = wire::arrays(&world);
    let model = Model::from_world(
        &world,
        Default::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut state = model.initial_state();
    assert_frame(&model, &state, None, 0);
    for _ in 0..5 {
        let step = model.advance(&mut state, 86400).unwrap();
        let saved = state.checkpoint();
        assert_frame(&model, &state, Some(&step), 86400);
        assert_frame(&model, &state, None, 0);
        assert_eq!(state.checkpoint(), saved);
        let mut rejected = Vec::new();
        assert!(wire::seasonal_moisture(&mut rejected, &model, &state, None, 1).is_err());
        assert!(wire::seasonal_moisture(&mut rejected, &model, &state, Some(&step), 0).is_err());
        assert!(rejected.is_empty());
    }
    assert_eq!(wire::arrays(&world), initial_geometry);
}
