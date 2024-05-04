//! # Seed Maker
//!
//! A library for finding seeds for [Stardew Valley](https://www.stardewvalley.net/).
//!
//! `seed-maker` is a configuration driven framework for finding seeds in
//!  Stardew Valley.  It can be broken down into:
//!
//! * **Predictors**: A collection of objects that can predict specific state
//!   of the game from a seed and match that state against specified conditions.
//! * **Configuration**: A [`serde`] compatible
//!   [configuration declaration](`SeedFinderConfig`) capable of describing
//!   seed finding conditions without writing code.
//! * **Seed Finding Engine**: A high-performance, multi-threaded engine that
//!   searches the seed space for seeds that match a given configuration.
//!
//! ## Predictors
//!
//! * [`DayRange`] / [`DayRangeConfig`]: Run a child predictor over a given day
//!   range
//! * [`Garbage`] / [`GarbageConfig`]: Predict items from trash cans around town.
//! * [`Geode`] / [`GeodeConfig`]: Predict items from breaking geodes.
//! * [`NightEvent`] / [`NightEventConfig`]: Predict night events like fairies
//!   and meteors.
//! * [`Weather`] / [`WeatherConfig`]: Predict weather.
//!
//! ## Example
//! ``` no_run
//! use seed_maker::{
//!     sdv::{GameData, Locale, gamedata::get_game_content_path},
//!     SeedFinder, SeedFinderConfig,
//! };
//!
//! // Load data from game files.
//! let game_data = GameData::from_content_dir(get_game_content_path().unwrap())?;
//! let locale = Locale::from_content_dir(get_game_content_path().unwrap(), "en-EN")?;
//!
//! // Load configuration from a JSON file using [`serde_json`]
//! let config_data = std::fs::read_to_string("test-config.json")?;
//! let config: SeedFinderConfig = serde_json::from_str(&config_data)?;
//!
//! // Create a new seed finder from the configuration.
//! let finder = SeedFinder::new(&game_data, &config)?;
//!
//! // Run the seed finder.
//! let seeds = finder.find_seeds();
//!
//! // Print out reports on found seeds.
//! for seed in seeds {
//!     println!("Seed: {seed}");
//!     finder.report(&game_data, &locale, seed, &mut std::io::stdout());
//!     println!("");
//! }
//! # seed_maker::Result::<()>::Ok(())
//!
//! ```

#![deny(missing_docs)]

use std::{
    fmt::Debug,
    io::Write,
    marker::PhantomData,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
};

use rayon::prelude::*;
use sdv::{
    predictor::PredictionGameState,
    rng::{HashedSeedGenerator, LegacySeedGenerator, SeedGenerator},
    GameData, Locale,
};
use serde::{Deserialize, Serialize};

pub use anyhow::Result;
pub use sdv;

mod garbage;
mod geode;
mod night_event;
mod weather;

pub use garbage::{Garbage, GarbageConfig};
pub use geode::{Geode, GeodeConfig};
pub use night_event::{NightEvent, NightEventConfig};
pub use weather::{Weather, WeatherConfig};

/// A trait describing a specific seed finding predictor
pub trait Predictor: Send + Sync + core::fmt::Debug {
    /// Run a prediction based on `state`
    ///
    /// Returns `Ok(true)` if the prediction matches, `Ok(false)` if the
    /// prediction doesn't match, or an error if an error was encounterd.
    fn predict(&self, state: &PredictionGameState) -> Result<bool>;

    /// Generate a report for a seed.
    ///
    /// Write a report for this predictor to `writer`.
    fn report(
        &self,
        game_data: &GameData,
        locale: &Locale,
        state: &PredictionGameState,
        writer: &mut dyn Write,
    ) -> Result<()>;
}

/// Configuration for the [`DayRange`] Predictor.
///
/// ## Example JSON
/// ```text
/// {
///    "type": "day_range",
///    "start_day": 1,
///    "end_day": 12,
///    "min_matches": 4,
///    "child": {
///        "type": "weather",
///        "is_rain": true
///    }
/// }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DayRangeConfig {
    /// Starting day of the range (inclusive).
    pub start_day: u32,

    /// Ending day of the range (inclusive).
    pub end_day: u32,

    /// Minimum number of successful days of the child predictor needed.
    pub min_matches: usize,

    /// [`PredictorConfig`] of the child predictor called every day
    /// between `start_day` and `end_day`.
    pub child: Box<PredictorConfig>,
}

/// Runs a child [`Predictor`] over a range of days.
///
/// Configured through [`DayRangeConfig`].
///
/// `DayRange` will run its child predector every day between `start_day` and
/// `end_day` (inclusive).  If the child predictor succeeds at least
/// `min_matches` during that period, `DayRange` will report a sucess.
pub struct DayRange<G: Sync + SeedGenerator> {
    start_day: u32,
    end_day: u32,
    min_matches: usize,
    child: Box<dyn Predictor>,
    phantom: PhantomData<G>,
}

impl<G: 'static + Send + Sync + SeedGenerator> DayRange<G> {
    /// Create a new [`DayRange`] predictor from a [`DayRangeConfig`].
    pub fn new(game_data: &GameData, config: &DayRangeConfig) -> Result<Self> {
        let child = config.child.predictor::<G>(game_data)?;
        Ok(Self {
            start_day: config.start_day,
            end_day: config.end_day,
            min_matches: config.min_matches,
            child,
            phantom: PhantomData,
        })
    }
}

impl<G: Send + Sync + SeedGenerator> Debug for DayRange<G> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DayRange")
            .field("start_day", &self.start_day)
            .field("end_day", &self.end_day)
            .field("min_matches", &self.min_matches)
            .field("child", &self.child)
            .finish()
    }
}

impl<G: Send + Sync + SeedGenerator> Predictor for DayRange<G> {
    fn predict(&self, state: &PredictionGameState) -> Result<bool> {
        let mut sucesses = 0;

        for day in self.start_day..=self.end_day {
            let state = PredictionGameState {
                days_played: day,
                ..*state
            };
            if self.child.predict(&state)? {
                sucesses += 1;
            }
        }

        Ok(sucesses >= self.min_matches)
    }

    fn report(
        &self,
        game_data: &GameData,
        locale: &Locale,
        state: &PredictionGameState,
        writer: &mut dyn Write,
    ) -> Result<()> {
        for day in self.start_day..=self.end_day {
            let state = PredictionGameState {
                days_played: day,
                ..*state
            };
            if self.child.predict(&state)? {
                write!(writer, "Day {day} ")?;
                self.child.report(game_data, locale, &state, writer)?;
            }
        }
        Ok(())
    }
}

fn one() -> u32 {
    1
}

/// Type of RNG seeding used.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RngType {
    /// Default 1.6 RNG seeding.
    Hashed,

    /// Legacy RNG setting in 1.6.
    Legacy,
}

impl Default for RngType {
    fn default() -> Self {
        Self::Hashed
    }
}

/// Game state configuration.
///
/// Used in [`SeedFinderConfig`] to set the initial state of the configured
/// predictors.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SeedFinderStateConfig {
    /// Mutiplayer ID.
    ///
    /// Random ID set on game creation.  `<UniqueMultiplayerID>` from the save
    /// file.  Only used by the [`Geode`] predictor.
    ///
    /// Defaults to 0
    #[serde(default)]
    pub multiplayer_id: i64,

    /// Day of interest.
    ///
    /// Starts as 1 on Year 1, Spring 1.  Increments every day.  Does not reset
    /// at the begining of month or year.
    pub day: u32,

    /// Daily Luck.
    ///
    /// Defaults to 0
    #[serde(default)]
    pub daily_luck: f64,

    /// Number of geodes cracked plus one.
    ///
    /// The game increments the geodes_cracked counter before determining its
    /// contents
    #[serde(default = "one")]
    pub geodes_cracked: u32,

    /// Deepest mine level reached.
    #[serde(default)]
    pub deepest_mine_level: u32,
}

impl From<SeedFinderStateConfig> for PredictionGameState {
    fn from(config: SeedFinderStateConfig) -> Self {
        PredictionGameState {
            multiplayer_id: config.multiplayer_id,
            days_played: config.day,
            daily_luck: config.daily_luck,
            geodes_cracked: config.geodes_cracked,
            deepest_mine_level: config.deepest_mine_level,
            ..Default::default()
        }
    }
}

/// Configuration of a single predictor.
///
/// Uses `#[serde(tag = "type")]` so a JSON [`GeodeConfig`] would look like:
///
/// ```text
/// {
///     "type": "geode",
///     "item": "(O)378",
///     "quantity": 20,
///     "geode_type": "geode"
/// }
/// ```
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PredictorConfig {
    /// A [`DayRange`] predictor.
    DayRange(DayRangeConfig),

    /// A [`Garbage`] predictor.
    Garbage(GarbageConfig),

    /// A [`Geode`] predictor.
    Geode(GeodeConfig),

    /// A [`NightEvent`] predictor.
    NightEvent(NightEventConfig),

    /// A [`Weather`] predictor.
    Weather(WeatherConfig),
}

impl PredictorConfig {
    /// Create a new [`Predictor`] using this configuration.
    ///
    /// Returns a `Box<dyn Predictor>` of the new predictor.
    pub fn predictor<G: 'static + Send + Sync + SeedGenerator>(
        &self,
        game_data: &GameData,
    ) -> Result<Box<dyn Predictor>> {
        match self {
            PredictorConfig::DayRange(config) => {
                let p = DayRange::<G>::new(game_data, config)?;
                Ok(Box::new(p))
            }
            PredictorConfig::Garbage(config) => {
                let p = Garbage::<G>::new(game_data, config)?;
                Ok(Box::new(p))
            }
            PredictorConfig::Geode(config) => {
                let p = Geode::<G>::new(game_data, config)?;
                Ok(Box::new(p))
            }
            PredictorConfig::NightEvent(config) => {
                let p = NightEvent::<G>::new(config)?;
                Ok(Box::new(p))
            }
            PredictorConfig::Weather(config) => {
                let p = Weather::<G>::new(game_data, config)?;
                Ok(Box::new(p))
            }
        }
    }
}

/// Top level configuration for as [`SeedFinder`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SeedFinderConfig {
    #[serde(default)]
    /// Type of RNG used in this seed finding.
    pub rng_type: RngType,

    /// Maximum number of seeds to find.
    pub max_seeds: usize,

    /// Intial game state for seed finding.
    pub game_state: SeedFinderStateConfig,

    /// Conditions used to validate seeds.
    pub predictors: Vec<PredictorConfig>,
}

/// Core seed finding object.
#[derive(Debug)]
pub struct SeedFinder {
    max_seeds: usize,
    initial_state: PredictionGameState,
    predictors: Vec<Box<dyn Predictor>>,
}

/// Progress Event.
#[derive(Debug)]
pub enum Progress {
    /// A report of number of seeds searched.
    Progress(usize),

    /// The results of a compleated search.
    Complete(Vec<i32>),
}

impl SeedFinder {
    /// Create a new `SeedFinder`
    pub fn new(game_data: &GameData, config: &SeedFinderConfig) -> Result<Self> {
        let initial_state = config.game_state.clone().into();
        let predictors = match config.rng_type {
            RngType::Hashed => config
                .predictors
                .iter()
                .map(|config| config.predictor::<HashedSeedGenerator>(game_data))
                .collect::<Result<Vec<_>>>()?,
            RngType::Legacy => config
                .predictors
                .iter()
                .map(|config| config.predictor::<LegacySeedGenerator>(game_data))
                .collect::<Result<Vec<_>>>()?,
        };

        Ok(Self {
            max_seeds: config.max_seeds,
            initial_state,
            predictors,
        })
    }

    /// Find seeds synchronously
    pub fn find_seeds(&self) -> Vec<i32> {
        (0..i32::MAX)
            .into_par_iter()
            .filter(|seed| {
                let state = PredictionGameState {
                    game_id: *seed as u32,
                    ..self.initial_state
                };

                for predictor in &self.predictors {
                    if !predictor.predict(&state).unwrap() {
                        return false;
                    }
                }
                true
            })
            .take_any(self.max_seeds)
            .collect()
    }

    /// Asynchronously find seeds
    ///
    /// Runs a seed search in the background while delivering progress and the
    /// eventual restults through the returned `Receiver<Progress>` channel.
    ///
    /// Note: This does not use Futures or async/await.
    pub fn find_seeds_async(finder: Arc<Self>, steps: usize) -> Receiver<Progress> {
        let seeds_processed = Arc::new(AtomicUsize::new(0));
        let step_size = i32::MAX as usize / steps;

        let range = 0..i32::MAX;
        let (tx, rx) = mpsc::channel();

        rayon::spawn({
            move || {
                let seeds = range
                    .into_par_iter()
                    .filter(|seed| {
                        // Looking directly at the seed to tell if we've crossed as
                        // progress step boundary can yield bursty progress result
                        // however incrementing the counter every seed for accureate
                        // step counting add significant overhead (~10s).  With
                        // 1000 steps the progress updates appar smooth and don't
                        // introduce significant overhead.
                        if *seed as usize % step_size == 0 {
                            let cur = seeds_processed.fetch_add(step_size, Ordering::Relaxed) + 1;
                            let _ = tx.send(Progress::Progress(cur));
                        }

                        let state = PredictionGameState {
                            game_id: *seed as u32,
                            ..finder.initial_state
                        };

                        for predictor in &finder.predictors {
                            if !predictor.predict(&state).unwrap() {
                                return false;
                            }
                        }
                        true
                    })
                    .take_any(finder.max_seeds)
                    .collect();
                let _ = tx.send(Progress::Complete(seeds));
            }
        });
        rx
    }

    /// Generate report for a seed
    ///
    /// Generates a report for `seed` and writes it to `writer`.  The report
    /// will include a section for each of the predictors configured in the
    /// [`SeedFinderConfig`] used to create the [`SeedFinder`].  `game_data`
    /// and `locale` are needed to allow reports to look up aditional
    /// information like items names.
    pub fn report(
        &self,
        game_data: &GameData,
        locale: &Locale,
        seed: i32,
        writer: &mut dyn Write,
    ) -> Result<()> {
        let state = PredictionGameState {
            game_id: seed as u32,
            ..self.initial_state
        };

        for predictor in &self.predictors {
            predictor.report(game_data, locale, &state, writer)?;
        }
        Ok(())
    }
}
