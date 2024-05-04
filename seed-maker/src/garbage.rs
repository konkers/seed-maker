use std::{fmt::Debug, marker::PhantomData};

use sdv::{
    common::{items, ItemId},
    predictor::{
        garbage::{predict_garbage, GarbageCan, GarbageCanLocation},
        PredictionGameState,
    },
    rng::SeedGenerator,
    GameData, Locale,
};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::{Predictor, Result};

/// Configuration for [`Garbage`].
///
/// ## Example JSON
/// ```text
/// {
///    "type": "garbage",
///    "items": [
///        "(O)535",
///        "DISH_OF_THE_DAY"
///    ]
/// }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GarbageConfig {
    /// List of items required to be found in garbage cans.
    pub items: Vec<String>,
}

/// Predictor for garbage cans in town.
#[derive(Clone)]
pub struct Garbage<G: Send + Sync + SeedGenerator> {
    items: Vec<ItemId>,
    cans: Vec<GarbageCan>,
    phantom: PhantomData<G>,
}

impl<G: Send + Sync + SeedGenerator> Garbage<G> {
    /// Create a new [`Garbage`] from a [`GarbageConfig`].
    pub fn new(game_data: &GameData, config: &GarbageConfig) -> Result<Self> {
        let cans: Vec<_> = GarbageCanLocation::iter()
            .map(|location| GarbageCan::new(location, &game_data.garbage_cans))
            .collect::<Result<Vec<_>>>()?;
        let items = config
            .items
            .iter()
            .map(|name| name.parse::<ItemId>())
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            items,
            cans,
            phantom: PhantomData,
        })
    }
}

impl<G: Send + Sync + SeedGenerator> Debug for Garbage<G> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Garbage")
            .field("items", &self.items)
            .field("cans", &self.cans)
            .finish()
    }
}

impl<G: Send + Sync + SeedGenerator> Predictor for Garbage<G> {
    fn predict(&self, state: &PredictionGameState) -> Result<bool> {
        let mut results = Vec::new();
        for can in &self.cans {
            if let Some(prediction) = predict_garbage::<G>(can, state)? {
                results.push(prediction.0);
            }
        }
        for item in &self.items {
            if !results.iter().any(|drop| drop.item == *item) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn report(
        &self,
        game_data: &GameData,
        locale: &Locale,
        state: &PredictionGameState,
        writer: &mut dyn std::io::prelude::Write,
    ) -> Result<()> {
        writeln!(writer, "Garbage:")?;
        for can in &self.cans {
            if let Some((drop, min_luck)) = predict_garbage::<G>(can, state)? {
                let item_name = match drop.item {
                    items::DISH_OF_THE_DAY => "Dish of the Day",
                    item => game_data.get_object_by_id(&item)?.display_name(locale),
                };
                writeln!(
                    writer,
                    "  {}: {} {} (minluck: {:.3})",
                    can.location, drop.quantity, item_name, min_luck
                )?
            }
        }
        Ok(())
    }
}
