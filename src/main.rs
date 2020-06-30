use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Clap;
use log::{debug, info, warn};
use tabwriter::TabWriter;

use brd::converter;
use brd::ddr::{arc::ARC, musicdb, ssq::SSQ};
use brd::osu;
use brd::utils;
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
        name = "unarc",
        about = "Extracts files from DDR A ARC archives",
        display_order = 1
    )]
    UnARC(UnARC),
    #[clap(
        name = "musicdb",
        about = "Shows entries from musicdb (supports musicdb.xml and startup.arc from DDR A)",
        display_order = 1
    )]
    MusicDB(MusicDB),
    #[clap(
        about = "Converts DDR step charts to osu!mania beatmaps",
        display_order = 1
    )]
    DDR2osu(Box<DDR2osu>),
}

#[derive(Clap)]
struct UnARC {
    #[clap(short, long, about = "List available files and exit")]
    list_files: bool,
    #[clap(short = "f", long, about = "Only extract this file")]
    single_file: Option<PathBuf>,
    #[clap(name = "file")]
    file: PathBuf,
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
struct MusicDB {
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
        display_order = 1
    )]
    xwb_file: PathBuf,
    #[clap(
        short = "o",
        long = "out",
        name = "file.osz",
        about = "osu! beatmap archive",
        display_order = 1
    )]
    out_file: PathBuf,
    #[clap(
        short = "m",
        long = "musicdb",
        name = "musicdb.xml|startup.arc",
        about = "musicdb.xml or startup.arc for metadata",
        display_order = 1
    )]
    musicdb_file: Option<PathBuf>,
    #[clap(
        short = "n",
        name = "sound name",
        about = "Sound in wave bank, otherwise inferred from SSQ filename",
        display_order = 2
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

fn read_musicdb(path: &PathBuf) -> Result<musicdb::MusicDB> {
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

    match extension {
        "arc" => {
            let arc_data = fs::read(path)
                .with_context(|| format!("failed to read musicdb ARC file {}", path.display()))?;

            musicdb::MusicDB::parse_from_startup_arc(&arc_data)
                .context("failed to parse musicdb from ARC file")
        }
        _ => {
            if extension != "xml" {
                warn!("Did not find known extension (arc, xml), trying to parse as XML");
            }

            let musicdb_data = fs::read_to_string(path)
                .with_context(|| format!("failed to read musicdb XML file {}", path.display()))?;

            musicdb::MusicDB::parse(&musicdb_data).context("failed to parse musicdb XML")
        }
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
        SubCommand::UnARC(opts) => {
            let arc_data = fs::read(&opts.file)
                .with_context(|| format!("failed to read ARC file {}", &opts.file.display()))?;
            let arc = ARC::parse(&arc_data).context("failed to parse ARC file")?;

            let files = match opts.single_file {
                Some(path) => match arc.files.get(&path) {
                    Some(_) => vec![path],
                    None => return Err(anyhow!("File “{}” not found in archive", path.display())),
                },
                None => arc.files.keys().cloned().collect(),
            };

            for (path, data) in arc.files.iter() {
                if files.contains(&path) {
                    if opts.list_files {
                        println!("{}", path.display());
                    } else {
                        info!("Writing {}", path.display());
                        if let Some(parent) = path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(path, data).with_context(|| {
                            format!("failed to write file to “{}”", path.display())
                        })?;
                    }
                }
            }
        }
        SubCommand::MusicDB(opts) => {
            let musicdb = read_musicdb(&opts.file)?;

            let mut tw = TabWriter::new(io::stdout());

            writeln!(
                tw,
                "Code\tBasename\tName\tArtist\tBPM\tSeries\tDifficulties (Single)\t(Double)"
            )?;

            for song in musicdb.music {
                // Filter 0s
                let diff_lv: (Vec<&u8>, Vec<&u8>) = (
                    song.diff_lv[..5].iter().filter(|x| **x != 0).collect(),
                    song.diff_lv[5..].iter().filter(|x| **x != 0).collect(),
                );

                writeln!(
                    tw,
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    song.mcode,
                    song.basename,
                    song.title,
                    song.artist,
                    song.bpmmax,
                    song.series,
                    utils::join_display_values(diff_lv.0, ", "),
                    utils::join_display_values(diff_lv.1, ", ")
                )?;
            }

            tw.flush()?;
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

            let mut convert_options = opts.convert.clone();

            if let Some(musicdb_file) = &opts.musicdb_file {
                debug!("Reading metadata from {}", musicdb_file.display());
                let musicdb = read_musicdb(&musicdb_file)?;
                let musicdb_entry = musicdb
                    .get_entry_from_basename(sound_name)
                    .ok_or_else(|| anyhow!("Entry not found in musicdb"))?;
                if convert_options.metadata.title.is_none() {
                    info!("Using title from musicdb: “{}”", musicdb_entry.title);
                    convert_options.metadata.title = Some(musicdb_entry.title.clone());
                }
                if convert_options.metadata.artist.is_none() {
                    info!("Using artist from musicdb: “{}”", musicdb_entry.artist);
                    convert_options.metadata.artist = Some(musicdb_entry.artist.clone());
                }
                convert_options.metadata.levels = Some(musicdb_entry.diff_lv.clone());
            } else if convert_options.metadata.title.is_none() {
                convert_options.metadata.title = Some(sound_name.to_string());
            }

            let beatmaps = ssq
                .to_beatmaps(&convert_options)
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
