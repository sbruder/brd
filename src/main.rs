use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Clap;
use log::{debug, info, warn};

use brd::converter;
use brd::ddr::ssq::SSQ;
use brd::osu;
use brd::xact3::xwb::{Sound as XWBSound, WaveBank};

#[derive(Clap)]
#[clap()]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    #[clap(
        name = "unxwb",
        about = "Extracts sounds from XWB wave banks",
        display_order = 1
    )]
    UnXWB(UnXWB),
    #[clap(
        about = "Converts DDR step charts to osu!mania beatmaps",
        display_order = 1
    )]
    DDR2osu(DDR2osu),
}

#[derive(Clap)]
struct UnXWB {
    #[clap(short, long, about = "List available sounds and exit")]
    list_entries: bool,
    #[clap(short = "e", long, about = "Only extract this entry")]
    single_entry: Option<String>,
    #[clap(name = "file")]
    file: PathBuf,
}

#[derive(Clap)]
struct DDR2osu {
    #[clap(
        short = "s",
        long = "ssq",
        name = "file.ssq",
        about = "DDR step chart file",
        display_order = 1
    )]
    ssq_file: PathBuf,
    #[clap(
        short = "x",
        long = "xwb",
        name = "file.xwb",
        about = "XAC3 wave bank file",
        display_order = 2
    )]
    xwb_file: PathBuf,
    #[clap(
        short = "o",
        long = "out",
        name = "file.osz",
        about = "osu! beatmap archive",
        display_order = 3
    )]
    out_file: PathBuf,
    #[clap(
        short = "n",
        name = "sound name",
        about = "Sound in wave bank, otherwise inferred from SSQ filename",
        display_order = 4
    )]
    sound_name: Option<String>,
    #[clap(flatten)]
    convert: converter::ddr2osu::Config,
}

fn get_basename(path: &PathBuf) -> Option<&str> {
    match path.file_stem() {
        Some(stem) => stem.to_str(),
        None => None,
    }
}

fn main() -> Result<()> {
    pretty_env_logger::init();

    let opts: Opts = Opts::parse();

    match opts.subcmd {
        SubCommand::UnXWB(opts) => {
            let xwb_data = fs::read(&opts.file)
                .with_context(|| format!("failed to read XWB file {}", &opts.file.display()))?;
            let wave_bank = WaveBank::parse(&xwb_data).context("failed to parse XWB file")?;
            info!(
                "Opened wave bank “{}” from {}",
                wave_bank.name,
                &opts.file.display()
            );

            let entries = match opts.single_entry {
                Some(name) => match wave_bank.sounds.get(&name) {
                    Some(_) => vec![name],
                    None => return Err(anyhow!("Entry “{}” not found in wave bank", name)),
                },
                None => wave_bank.sounds.keys().cloned().collect(),
            };

            for (name, sound) in wave_bank.sounds {
                if entries.contains(&name) {
                    if opts.list_entries {
                        println!("{}", name);
                        continue;
                    }
                    info!("Extracting {}", name);
                    let file_name = format!("{}.wav", name);
                    fs::write(
                        file_name.clone(),
                        &sound.to_wav().with_context(|| {
                            format!("failed to convert wave bank sound entry “{}” to WAV", name)
                        })?,
                    )
                    .with_context(|| format!("failed to write sound to {}", file_name))?;
                }
            }
        }
        SubCommand::DDR2osu(opts) => {
            let sound_name =
                &opts
                    .sound_name
                    .clone()
                    .unwrap_or(match get_basename(&opts.ssq_file) {
                        Some(basename) => basename.to_string(),
                        None => {
                            return Err(anyhow!(
                        "Could not extract chart id from file name. Please specify it manually."))
                        }
                    });

            debug!(
                "Converting {} and sound {} from {} to {}",
                opts.ssq_file.display(),
                sound_name,
                opts.xwb_file.display(),
                opts.out_file.display()
            );

            let ssq_data = fs::read(&opts.ssq_file)
                .with_context(|| format!("failed to read SSQ file {}", &opts.ssq_file.display()))?;
            let ssq = SSQ::parse(&ssq_data).context("failed to parse SSQ file")?;

            let beatmaps = ssq
                .to_beatmaps(&opts.convert)
                .context("failed to convert DDR step chart to osu!mania beatmap")?;

            let xwb_data = fs::read(&opts.xwb_file).with_context(|| {
                format!(
                    "failed to read XWB file {}",
                    &opts.xwb_file.clone().display()
                )
            })?;
            let wave_bank = WaveBank::parse(&xwb_data).context("failed to parse XWB file")?;

            let audio_data = if wave_bank.sounds.contains_key(sound_name) {
                wave_bank
                    .sounds
                    .get(sound_name)
                    .unwrap()
                    .to_wav()
                    .with_context(|| {
                        format!(
                            "failed to convert wave bank sound entry “{}” to WAV",
                            sound_name
                        )
                    })?
            } else if wave_bank.sounds.len() == 2 {
                warn!(
                    "Sound {} not found in wave bank, but it has two entries; assuming these are preview and full song",
                    sound_name
                );
                let mut sounds = wave_bank.sounds.values().collect::<Vec<&XWBSound>>();
                sounds.sort_unstable_by(|a, b| b.size.cmp(&a.size));
                sounds[0].to_wav().with_context(|| {
                    format!(
                        "failed to convert wave bank sound entry “{}” to WAV",
                        sound_name
                    )
                })?
            } else {
                return Err(anyhow!(
                    "Could not find matching sound in wave bank (searched for {})",
                    sound_name,
                ));
            };

            let osz = osu::osz::Archive {
                beatmaps,
                assets: vec![("audio.wav", &audio_data)],
            };
            osz.write(&opts.out_file).with_context(|| {
                format!("failed to write OSZ file to {}", opts.out_file.display())
            })?;
        }
    }
    Ok(())
}
