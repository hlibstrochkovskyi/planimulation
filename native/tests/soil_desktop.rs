use planimulation_core::{
    Recipe, World,
    moisture_transport::total_mass,
    seasonal_moisture::regional_soil::{Checkpoint, Model, State, Step},
    wire,
};

fn decode(bytes: &[u8]) -> (serde_json::Value, &[u8]) {
    let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    (
        serde_json::from_slice(&bytes[4..4 + length]).unwrap(),
        &bytes[4 + length..],
    )
}
fn fixture() -> (World, Model) {
    let mut recipe: Recipe =
        serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json")).unwrap();
    recipe.subdivision = 2;
    let world = World::generate(recipe).unwrap();
    let model = Model::from_world(
        &world,
        wire::soil_moisture_settings(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    (world, model)
}
fn assert_frame(model: &Model, state: &State, before: Option<&State>, step: Option<&Step>) {
    let original = serde_json::to_vec(&state.checkpoint()).unwrap();
    let view = model.observe(state).unwrap();
    let mut bytes = Vec::new();
    wire::soil_moisture(&mut bytes, model, state, before, step).unwrap();
    let (h, data) = decode(&bytes);
    let n = view.stocks.liquid.len();
    let b = view.reference_bodies.len();
    assert_eq!(h["protocol"], 15);
    assert_eq!(h["kind"], "soilMoisture");
    assert_eq!(h["modelVersion"], "regional-seasonal-water-1");
    assert_eq!(h["regionCount"], n);
    assert_eq!(h["referenceBodyCount"], b);
    assert_eq!(h["byteLength"], (37 * n + 2 * b) * 8);
    assert_eq!(h["elapsedSeconds"], state.elapsed_seconds());
    assert_eq!(
        h["intervalSeconds"],
        before.map_or(0, |s| state.elapsed_seconds() - s.elapsed_seconds())
    );
    assert_eq!(
        h["budget"],
        serde_json::to_value(model.budget(state).unwrap()).unwrap()
    );
    let mut expected = Vec::new();
    for stock in [
        &view.stocks.liquid,
        &view.stocks.soil,
        &view.stocks.snow,
        &view.stocks.vapor,
        &view.stocks.drainage,
    ] {
        for low in [false, true] {
            expected.extend(stock.iter().map(|m| if low { m.low } else { m.high }));
        }
    }
    for component in 0..7 {
        for low in [false, true] {
            expected.extend(view.cumulative_local_transfers.iter().map(|t| {
                let m = t.values()[component];
                if low { m.low } else { m.high }
            }));
        }
    }
    let difference =
        |after: planimulation_core::surface_water::ponded_soil::Mass,
         old: planimulation_core::surface_water::ponded_soil::Mass| {
            total_mass(&[after.high - old.high, after.low, -old.low])
        };
    for component in 0..7 {
        expected.extend((0..n).map(|r| {
            before.map_or(0., |old| {
                difference(
                    state.local_transfers()[r].values()[component],
                    old.local_transfers()[r].values()[component],
                )
            })
        }));
    }
    for field in [
        &view.regional_liquid_depth_meters,
        &view.visible_water_depth_meters,
        &view.visible_water_level_meters,
        &view.cumulative_surface_incoming_kilograms,
        &view.cumulative_surface_outgoing_kilograms,
    ] {
        expected.extend(field);
    }
    expected.extend((0..n).map(|r| {
        before.map_or(0., |old| {
            difference(state.drainage_sent()[r], old.drainage_sent()[r])
        })
    }));
    for low in [false, true] {
        expected.extend(
            view.reference_bodies
                .iter()
                .map(|b| if low { b.liquid.low } else { b.liquid.high }),
        );
    }
    let expected_bytes: Vec<u8> = expected.iter().flat_map(|v| v.to_le_bytes()).collect();
    assert_eq!(
        data, expected_bytes,
        "Both components, including signed zeros, must survive transport."
    );
    assert_eq!(serde_json::to_vec(&state.checkpoint()).unwrap(), original);
}

#[test]
fn soil_wire_preserves_every_owner_history_and_exact_checkpoint_continuation() {
    let (world, model) = fixture();
    let geography = wire::arrays(&world);
    let mut state = model.initial_state();
    assert_frame(&model, &state, None, None);
    for _ in 0..3 {
        let before = state.clone();
        let step = model.advance(&mut state, 86400).unwrap();
        assert_frame(&model, &state, Some(&before), Some(&step));
        assert_frame(&model, &state, None, None);
        let mut bytes = Vec::new();
        wire::soil_moisture_checkpoint(&mut bytes, &model, &state).unwrap();
        let (h, data) = decode(&bytes);
        assert_eq!(h["protocol"], 15);
        assert_eq!(h["kind"], "soilMoistureCheckpoint");
        assert_eq!(h["schemaVersion"], 1);
        assert_eq!(data, serde_json::to_vec(&state.checkpoint()).unwrap());
        let cp: Checkpoint = serde_json::from_slice(data).unwrap();
        let (resumed, mut restored) = Model::restore(cp).unwrap();
        let mut continued = state.clone();
        model.advance(&mut continued, 3600).unwrap();
        resumed.advance(&mut restored, 3600).unwrap();
        assert_eq!(
            serde_json::to_vec(&continued.checkpoint()).unwrap(),
            serde_json::to_vec(&restored.checkpoint()).unwrap()
        );
    }
    assert_eq!(wire::arrays(&world), geography);
}

#[test]
fn soil_wire_refuses_wrong_product_settings_intervals_and_step_before_writing() {
    let (world, model) = fixture();
    let initial = model.initial_state();
    let mut state = initial.clone();
    let mut step = model.advance(&mut state, 900).unwrap();
    let mut refused = Vec::new();
    assert!(wire::soil_moisture(&mut refused, &model, &state, Some(&initial), None).is_err());
    assert!(wire::soil_moisture(&mut refused, &model, &state, None, Some(&step)).is_err());
    assert!(wire::soil_moisture(&mut refused, &model, &state, Some(&state), Some(&step)).is_err());
    assert!(
        wire::soil_moisture(&mut refused, &model, &initial, Some(&state), Some(&step)).is_err()
    );
    step.budget.stored_kilograms += 1e10;
    assert!(
        wire::soil_moisture(&mut refused, &model, &state, Some(&initial), Some(&step)).is_err()
    );
    for settings in [Default::default(), {
        let mut s = wire::soil_moisture_settings();
        s.max_coupled_step_seconds = 1800;
        s
    }] {
        let other =
            Model::from_world(&world, settings, Default::default(), Default::default()).unwrap();
        assert!(
            wire::soil_moisture(&mut refused, &other, &other.initial_state(), None, None).is_err()
        );
        assert!(
            wire::soil_moisture_checkpoint(&mut refused, &other, &other.initial_state()).is_err()
        );
    }
    assert!(refused.is_empty());
}
