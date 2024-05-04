use std::{io::BufWriter, path::PathBuf, sync::Arc, time::Instant};

use anyhow::Result;
use clap::Parser;
use cliclack::{intro, note, outro, progress_bar, spinner};
use seed_maker::{
    sdv::{self, GameData, Locale},
    Progress, SeedFinder, SeedFinderConfig,
};

/// Stardew Valley seed finder.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    config_file: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    intro("Seed Maker")?;

    let spinner = spinner();
    spinner.start("Loading game data...");
    let game_data =
        GameData::from_content_dir(sdv::gamedata::get_game_content_path().unwrap()).unwrap();
    let locale =
        Locale::from_content_dir(sdv::gamedata::get_game_content_path().unwrap(), "en-EN")?;
    spinner.stop("Game data loaded.");

    let config_data = std::fs::read_to_string(args.config_file).unwrap();
    let config: SeedFinderConfig = serde_json::from_str(&config_data).unwrap();
    let finder = Arc::new(SeedFinder::new(&game_data, &config).unwrap());

    let pb = progress_bar(i32::MAX as u64);
    pb.start("Finding seeds...");
    let start = Instant::now();
    let progress = SeedFinder::find_seeds_async(finder.clone(), 1000);
    let mut last_progress = 0;
    let seeds = loop {
        match progress.recv().unwrap() {
            Progress::Progress(seeds_processed) => {
                pb.inc((seeds_processed as u64) - last_progress);
                last_progress = seeds_processed as u64;
            }
            Progress::Complete(seeds) => break seeds,
        }
    };
    let elapsed = start.elapsed();
    pb.stop("Seed finding done");
    for seed in &seeds {
        let mut buf = BufWriter::new(Vec::new());
        finder.report(&game_data, &locale, *seed, &mut buf)?;
        //let buf = buf.into_inner()?;
        let buf = buf.into_inner()?;
        let report = String::from_utf8_lossy(&buf);

        note(format!("{seed}"), report)?;
    }

    outro(format!(
        "Finished: {} seeds found in {}s.",
        seeds.len(),
        elapsed.as_secs_f32()
    ))?;
    Ok(())
}
