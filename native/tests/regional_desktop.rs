use planimulation_core::{
    Recipe, World,
    seasonal_moisture::{Checkpoint, Model, State, Step},
    wire,
};

fn decode(bytes: &[u8]) -> (serde_json::Value, &[u8]) {
    let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    (
        serde_json::from_slice(&bytes[4..4 + length]).unwrap(),
        &bytes[4 + length..],
    )
}

fn assert_frame(model: &Model, state: &State, step: Option<&Step>, seconds: u32) {
    let before = state.checkpoint();
    let mut bytes = Vec::new();
    wire::regional_moisture(&mut bytes, model, state, step, seconds).unwrap();
    let (h, data) = decode(&bytes);
    let fields: Vec<_> = data
        .as_chunks::<8>()
        .0
        .iter()
        .map(|v| f64::from_le_bytes(*v))
        .collect();
    let n = state.surface_kilograms().len();
    let bodies = state.reference_body_high_kilograms().unwrap().len();
    assert_eq!(h["protocol"], 14);
    assert_eq!(h["kind"], "regionalMoisture");
    assert_eq!(h["modelVersion"], "seasonal-moisture-15");
    assert_eq!(h["regionCount"], n);
    assert_eq!(h["referenceBodyCount"], bodies);
    assert_eq!(h["byteLength"], (22 * n + 2 * bodies) * 8);
    assert_eq!(h["intervalSeconds"], seconds);
    assert_eq!(h["elapsedSeconds"], state.elapsed_seconds());
    assert_eq!(
        h["budget"],
        serde_json::to_value(model.budget(state).unwrap()).unwrap()
    );
    let regional_stocks: Vec<_> = [
        state.surface_kilograms(),
        state.snow_kilograms(),
        state.soil_kilograms(),
        state.pending_runoff_kilograms(),
        state.terminal_water_kilograms(),
        state.vapor_kilograms(),
    ]
    .into_iter()
    .flatten()
    .copied()
    .collect();
    assert_eq!(&fields[..6 * n], regional_stocks);
    for component in 0..8 {
        let expected: Vec<_> = (0..n)
            .map(|i| step.map_or(0., |s| s.surface_transfers[i].values()[component]))
            .collect();
        assert_eq!(&fields[(6 + component) * n..(7 + component) * n], expected);
    }
    for component in 0..4 {
        let expected: Vec<_> = (0..n)
            .map(|i| step.map_or(0., |s| s.runoff_transfers[i].values()[component]))
            .collect();
        assert_eq!(
            &fields[(14 + component) * n..(15 + component) * n],
            expected
        );
    }
    let view = model.regional_surface_observation(state).unwrap();
    let flow = model.regional_surface_transfer_observation(state).unwrap();
    assert_eq!(&fields[18 * n..19 * n], view.depth_meters);
    assert_eq!(
        &fields[19 * n..20 * n],
        view.levels_meters
            .iter()
            .map(|v| v.unwrap_or(0.))
            .collect::<Vec<_>>()
    );
    assert_eq!(&fields[20 * n..21 * n], flow.cumulative_incoming_kilograms);
    assert_eq!(&fields[21 * n..22 * n], flow.cumulative_outgoing_kilograms);
    assert_eq!(
        h["cumulativeFaceTransferKilograms"],
        flow.cumulative_transferred_kilograms
    );
    assert_eq!(
        &fields[22 * n..22 * n + bodies],
        state.reference_body_high_kilograms().unwrap()
    );
    assert_eq!(
        &fields[22 * n + bodies..],
        state.reference_body_low_kilograms().unwrap()
    );
    assert_eq!(
        state.checkpoint(),
        before,
        "Observations must not alter checkpointed state."
    );
}

#[test]
fn regional_wire_and_complete_checkpoint_preserve_native_ownership_and_state() {
    let mut recipe: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    recipe.subdivision = 2;
    let world = World::generate(recipe).unwrap();
    let geometry = wire::arrays(&world);
    let model = Model::from_world(
        &world,
        wire::regional_moisture_settings(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut state = model.initial_state();
    assert_frame(&model, &state, None, 0);
    for _ in 0..3 {
        let step = model.advance(&mut state, 86400).unwrap();
        assert_frame(&model, &state, Some(&step), 86400);
        assert_frame(&model, &state, None, 0);
        let mut bytes = Vec::new();
        wire::regional_moisture_checkpoint(&mut bytes, &model, &state).unwrap();
        let (h, data) = decode(&bytes);
        assert_eq!(h["protocol"], 14);
        assert_eq!(h["kind"], "regionalMoistureCheckpoint");
        assert_eq!(h["schemaVersion"], 15);
        let cp: Checkpoint = serde_json::from_slice(data).unwrap();
        assert_eq!(cp, state.checkpoint());
        let (restored, mut replay) = Model::restore(cp).unwrap();
        let mut continued = state.clone();
        restored.advance(&mut replay, 900).unwrap();
        model.advance(&mut continued, 900).unwrap();
        assert_eq!(continued, replay);
        let mut refused = Vec::new();
        assert!(wire::seasonal_moisture(&mut refused, &model, &state, None, 0).is_err());
        assert!(wire::seasonal_checkpoint(&mut refused, &model, &state).is_err());
        assert!(wire::regional_moisture(&mut refused, &model, &state, None, 900).is_err());
        assert!(wire::regional_moisture(&mut refused, &model, &state, Some(&step), 0).is_err());
        assert!(refused.is_empty());
    }
    let legacy = Model::from_world(
        &world,
        Default::default(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let mut refused = Vec::new();
    assert!(
        wire::regional_moisture(&mut refused, &legacy, &legacy.initial_state(), None, 0).is_err()
    );
    assert!(
        wire::regional_moisture_checkpoint(&mut refused, &legacy, &legacy.initial_state()).is_err()
    );
    assert!(refused.is_empty());
    assert_eq!(wire::arrays(&world), geometry);
}
