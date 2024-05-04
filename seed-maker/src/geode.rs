use std::{fmt::Debug, marker::PhantomData};

use sdv::{
    common::ItemId,
    predictor::{
        self,
        geode::{predict_single_geode, GeodeType},
        PredictionGameState,
    },
    rng::SeedGenerator,
    GameData, Locale,
};
use serde::{Deserialize, Serialize};

use crate::{Predictor, Result};

/// Configurations for [`Geode`].
///
/// ## Example
/// ```text
/// {
///    "type": "geode",
///    "item": "(O)378",
///    "quantity": 20,
///    "geode_type": "geode"
/// }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GeodeConfig {
    /// Item to search for.
    pub item: String,

    /// Minimum quanity required.
    pub quantity: u32,

    /// Type of geode used for search.
    pub geode_type: GeodeType,
}

/// Predictor for items received by breaking geodes.
#[derive(Clone)]
pub struct Geode<G: Send + Sync + SeedGenerator> {
    item: ItemId,
    quantity: u32,
    geode: predictor::geode::Geode,

    // Used for reporting.
    geode_type: GeodeType,

    phantom: PhantomData<G>,
}

impl<G: Send + Sync + SeedGenerator> Geode<G> {
    /// Create a new [`Geode`] from a [`GeodeConfig`].
    pub fn new(game_data: &GameData, config: &GeodeConfig) -> Result<Self> {
        let item = config.item.parse()?;
        Ok(Self {
            item,
            quantity: config.quantity,
            geode: predictor::geode::Geode::new(config.geode_type, game_data)?,
            geode_type: config.geode_type,
            phantom: PhantomData,
        })
    }
}

impl<G: Send + Sync + SeedGenerator> Debug for Geode<G> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Geode")
            .field("item", &self.item)
            .field("quantity", &self.quantity)
            .field("geode", &self.geode)
            .finish()
    }
}

impl<G: Send + Sync + SeedGenerator> Predictor for Geode<G> {
    fn predict(&self, state: &PredictionGameState) -> Result<bool> {
        let reward = predict_single_geode::<G>(&self.geode, state)?;
        Ok(reward.item == self.item && reward.quantity >= self.quantity)
    }

    fn report(
        &self,
        game_data: &GameData,
        locale: &Locale,
        state: &PredictionGameState,
        writer: &mut dyn std::io::prelude::Write,
    ) -> Result<()> {
        let reward = predict_single_geode::<G>(&self.geode, state)?;
        let item_name = game_data
            .get_object_by_id(&reward.item)?
            .display_name(locale);
        writeln!(
            writer,
            "{:?}: {} {}",
            self.geode_type, reward.quantity, item_name
        )?;
        Ok(())
    }
}
