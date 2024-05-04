use std::{fmt::Debug, marker::PhantomData};

use anyhow::anyhow;
use sdv::{
    predictor::weather::{predict_weather, WeatherLocation},
    rng::SeedGenerator,
    GameData, Locale,
};
use serde::{Deserialize, Serialize};

use crate::{Predictor, Result};

/// Configuration for [`Weather`].
///
/// ## Example JSON
/// ```text
/// "child": {
///     "type": "weather",
///     "is_rain": true
/// }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WeatherConfig {
    /// Set to true to require rain.
    ///
    /// Defaults to false.
    #[serde(default)]
    pub is_rain: bool,

    /// Set to true to require storm.
    ///
    /// Defaults to false.
    #[serde(default)]
    pub is_storm: bool,

    /// Set to true to require chance of storm.
    ///
    /// Defaults to false.
    #[serde(default)]
    pub maybe_storm: bool,
}

/// Predictor for a day's weather.
pub struct Weather<G: Send + Sync + SeedGenerator> {
    is_rain: bool,
    is_storm: bool,
    maybe_storm: bool,
    location: WeatherLocation,
    phantom: PhantomData<G>,
}

impl<G: Send + Sync + SeedGenerator> Weather<G> {
    /// Create a new [`Weather`] Predictor from a [`WeatherConfig`].
    pub fn new(game_data: &GameData, config: &WeatherConfig) -> Result<Self> {
        let location = game_data
            .location_contexts
            .get("Default")
            .ok_or_else(|| anyhow!("can't find default location context"))?
            .into();
        Ok(Self {
            is_rain: config.is_rain,
            is_storm: config.is_storm,
            maybe_storm: config.maybe_storm,
            location,
            phantom: PhantomData,
        })
    }
}

impl<G: Send + Sync + SeedGenerator> Debug for Weather<G> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Weather")
            .field("is_rain", &self.is_rain)
            .field("is_storm", &self.is_storm)
            .field("maybe_storm", &self.maybe_storm)
            .field("location", &self.location)
            .finish()
    }
}

impl<G: Send + Sync + SeedGenerator> Predictor for Weather<G> {
    fn predict(&self, state: &sdv::predictor::PredictionGameState) -> Result<bool> {
        let weather = predict_weather::<G>(&self.location, state);
        Ok((!self.is_rain || (weather.rain + weather.storm) >= 1.0)
            && (!self.is_storm || weather.storm >= 1.0)
            && (!self.maybe_storm || weather.storm >= 0.0))
    }

    fn report(
        &self,
        _game_data: &GameData,
        _locale: &Locale,
        state: &sdv::predictor::PredictionGameState,
        writer: &mut dyn std::io::prelude::Write,
    ) -> Result<()> {
        let weather = predict_weather::<G>(&self.location, state);
        let chances = [
            (weather.sun, "Sun"),
            (weather.rain, "Rain"),
            (weather.wind, "Wind"),
            (weather.storm, "Storm"),
            (weather.snow, "Snow"),
            (weather.fesival, "Festival"),
            (weather.green_rain, "Green Rain"),
        ]
        .iter()
        .filter_map(|(chance, name)| {
            if *chance > 0.0 {
                Some(format!("{:2.1}% {}", chance * 100.0, name))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
        writeln!(writer, "Weather: {}", chances.join(", "))?;

        Ok(())
    }
}
