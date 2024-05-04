use std::{fmt::Debug, marker::PhantomData};

use sdv::{
    predictor::{self, night_event::predict_night_event, PredictionGameState},
    rng::SeedGenerator,
    GameData, Locale,
};
use serde::{Deserialize, Serialize};

use crate::{Predictor, Result};

/// Configuration for [`NightEvent`].
///
/// ## Example JSON
/// ```text
/// {
///     "type": "night_event",
///     "event": "fairy"
/// }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NightEventConfig {
    /// Night event to search for.
    pub event: predictor::night_event::NightEvent,
}

/// Predictor for night events like fairies and meteors.
pub struct NightEvent<G: Send + Sync + SeedGenerator> {
    event: predictor::night_event::NightEvent,
    phantom: PhantomData<G>,
}

impl<G: Send + Sync + SeedGenerator> NightEvent<G> {
    /// Create a new [`NightEvent`] from a [`NightEventConfig`].
    pub fn new(config: &NightEventConfig) -> Result<Self> {
        Ok(Self {
            event: config.event.clone(),
            phantom: PhantomData,
        })
    }
}

impl<G: Send + Sync + SeedGenerator> Debug for NightEvent<G> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NightEvent")
            .field("event", &self.event)
            .finish()
    }
}

impl<G: Send + Sync + SeedGenerator> Predictor for NightEvent<G> {
    fn predict(&self, state: &PredictionGameState) -> Result<bool> {
        let mut state = state.clone();
        let night_event = predict_night_event::<G>(&mut state);
        Ok(night_event == self.event)
    }

    fn report(
        &self,
        _game_data: &GameData,
        _locale: &Locale,
        state: &PredictionGameState,
        writer: &mut dyn std::io::prelude::Write,
    ) -> Result<()> {
        let mut state = state.clone();
        let night_event = predict_night_event::<G>(&mut state);
        writeln!(writer, "Night Event: {night_event:?}")?;
        Ok(())
    }
}
