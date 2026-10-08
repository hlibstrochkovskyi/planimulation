//! One selected seasonal family per process; legacy and unified owners cannot coexist.
use planimulation_core::{
    seasonal_moisture::{self, regional_soil},
    wire,
};
use std::io::Write;

pub enum SeasonalSession {
    Legacy(seasonal_moisture::Model, seasonal_moisture::State),
    Soil(regional_soil::Model, regional_soil::State),
}

impl SeasonalSession {
    pub fn advance(&mut self, out: &mut impl Write, seconds: u32) -> Result<(), String> {
        match self {
            Self::Legacy(model, state) => {
                let step = if seconds == 0 {
                    None
                } else {
                    Some(model.advance(state, seconds)?)
                };
                super::write_moisture(out, model, state, step.as_ref(), seconds)
            }
            Self::Soil(model, state) => {
                if seconds == 0 {
                    return wire::soil_moisture(out, model, state, None, None);
                }
                let mut candidate = state.clone();
                let step = model.advance(&mut candidate, seconds)?;
                wire::soil_moisture(out, model, &candidate, Some(state), Some(&step))?;
                *state = candidate;
                Ok(())
            }
        }
    }
    pub fn export(&self, out: &mut impl Write) -> Result<(), String> {
        match self {
            Self::Soil(model, state) => wire::soil_moisture_checkpoint(out, model, state),
            Self::Legacy(model, state) => {
                if model.model_version() == seasonal_moisture::REGIONAL_SURFACE_MODEL_VERSION {
                    wire::regional_moisture_checkpoint(out, model, state)
                } else {
                    wire::seasonal_checkpoint(out, model, state)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use planimulation_core::{Recipe, World};
    struct FailedWriter;
    impl Write for FailedWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("Injected output failure."))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn soil_session_does_not_publish_a_step_when_output_fails() {
        let mut recipe: Recipe =
            serde_json::from_str(include_str!("../../docs/scenarios/spill-connections.json"))
                .unwrap();
        recipe.subdivision = 2;
        let world = World::generate(recipe).unwrap();
        let model = regional_soil::Model::from_world(
            &world,
            wire::soil_moisture_settings(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let state = model.initial_state();
        let original = serde_json::to_vec(&state.checkpoint()).unwrap();
        let mut session = SeasonalSession::Soil(model, state);
        assert!(session.advance(&mut FailedWriter, 900).is_err());
        if let SeasonalSession::Soil(_, state) = &session {
            assert_eq!(serde_json::to_vec(&state.checkpoint()).unwrap(), original);
        } else {
            panic!("Wrong family.");
        }
        session.advance(&mut Vec::new(), 900).unwrap();
        if let SeasonalSession::Soil(_, state) = &session {
            assert_eq!(state.elapsed_seconds(), 900);
        }
    }
}
