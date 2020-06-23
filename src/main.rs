use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Clap;
use log::{debug, error, info, warn};

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

fn error(message: String) -> Result<()> {
    error!("{}", message);
    Err(anyhow!(message))
}

fn read_file(name: &PathBuf) -> Result<Vec<u8>> {
    let mut file = File::open(name)?;
    let mut data = vec![];
    file.read_to_end(&mut data)?;
    Ok(data)
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
            let xwb_data = read_file(&opts.file)?;
            let wave_bank = WaveBank::parse(&xwb_data)?;
            info!("Opened wave bank “{}” from {:?}", wave_bank.name, opts.file);

            match opts.single_entry {
                Some(name) => {
                    let sound = match wave_bank.sounds.get(&name) {
                        Some(sound) => sound,
                        None => return error(format!("Entry {} not found in wave bank", name)),
                    };
                    let out_file = format!("{}.wav", name);
                    let mut wav_file = File::create(out_file)?;
                    wav_file.write_all(&sound.to_wav()?)?;
                }
                None => {
                    for (name, sound) in wave_bank.sounds {
                        if opts.list_entries {
                            println!("{}", name);
                            continue;
                        }
                        info!("Extracting {}", name);
                        let out_file = format!("{}.wav", name);
                        let mut wav_file = File::create(out_file)?;
                        wav_file.write_all(&sound.to_wav()?)?;
                    }
                }
            }
        }
        SubCommand::DDR2osu(opts) => {
            let sound_name = &opts
                .sound_name
                .unwrap_or(match get_basename(&opts.ssq_file) {
                    Some(basename) => basename.to_string(),
                    None => return error(
                        "Could not extract chart id from file name. Please specify it manually."
                            .to_string(),
                    ),
                });

            debug!(
                "Converting {:?} and sound {} from {:?} to {:?}",
                opts.ssq_file, sound_name, opts.xwb_file, opts.out_file
            );

            let ssq_data = read_file(&opts.ssq_file)?;
            let ssq = SSQ::parse(&ssq_data)?;

            let convert_config = opts.convert;
            let beatmaps = ssq.to_beatmaps(&convert_config)?;

            let xwb_data = read_file(&opts.xwb_file)?;
            let wave_bank = WaveBank::parse(&xwb_data)?;

            let audio_data = if wave_bank.sounds.contains_key(sound_name) {
                wave_bank.sounds.get(sound_name).unwrap().to_wav()?
            } else if wave_bank.sounds.len() == 2 {
                warn!(
                    "Sound {} not found in wave bank, but it has two entries; assuming these are preview and full song",
                    sound_name
                );
                let mut sounds = wave_bank.sounds.values().collect::<Vec<&XWBSound>>();
                sounds.sort_unstable_by(|a, b| b.size.cmp(&a.size));
                sounds[0].to_wav()?
            } else {
                return error(format!(
                    "Could not find matching sound in wave bank (searched for {})",
                    sound_name,
                ));
            };

            let osz = osu::osz::Archive {
                beatmaps,
                assets: vec![("audio.wav", &audio_data)],
            };
            osz.write(&opts.out_file)?;
        }
    }
    Ok(())
}
