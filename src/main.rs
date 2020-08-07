use std::convert::TryInto;
use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use clap::Clap;
use log::{debug, error, info, warn};
use pbr::ProgressBar;
use rayon::prelude::*;
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
    #[clap(
        name = "ddr2osu-batch",
        about = "Batch version of ddr2osu",
        display_order = 1
    )]
    BatchDDR2osu(BatchDDR2osu),
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
        name = "basename",
        about = "Sound in wave bank, otherwise inferred from SSQ filename",
        display_order = 2
    )]
    basename: Option<String>,
    #[clap(flatten)]
    convert: converter::ddr2osu::Config,
}

#[derive(Clap)]

struct BatchDDR2osu {
    #[clap(
        short = "s",
        long = "ssq",
        name = "ssq_dir",
        about = "directory with DDR step chart files",
        display_order = 1
    )]
    ssq_dir: PathBuf,
    #[clap(
        short = "x",
        long = "xwb",
        name = "xwb_dir",
        about = "directory with XAC3 wave bank files",
        display_order = 1
    )]
    xwb_dir: PathBuf,
    #[clap(
        short = "o",
        long = "out",
        name = "out_dir",
        about = "output directory",
        display_order = 1
    )]
    out_dir: PathBuf,
    #[clap(
        short = "m",
        long = "musicdb",
        name = "musicdb.xml|startup.arc",
        about = "musicdb.xml or startup.arc for metadata",
        display_order = 1
    )]
    musicdb_file: PathBuf,
    #[clap(flatten)]
    convert: converter::ddr2osu::Config,
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

fn ddr2osu(
    ssq_file: PathBuf,
    xwb_file: PathBuf,
    out_file: PathBuf,
    basename: String,
    convert_options: converter::ddr2osu::Config,
) -> Result<()> {
    debug!(
        "Converting {} and sound {} from {} to {}",
        ssq_file.display(),
        basename,
        xwb_file.display(),
        out_file.display()
    );

    let ssq_data = fs::read(&ssq_file)
        .with_context(|| format!("failed to read SSQ file {}", &ssq_file.display()))?;
    let ssq = SSQ::parse(&ssq_data).context("failed to parse SSQ file")?;

    let beatmaps = ssq
        .to_beatmaps(&convert_options)
        .context("failed to convert DDR step chart to osu!mania beatmap")?;

    let xwb_data = fs::read(&xwb_file)
        .with_context(|| format!("failed to read XWB file {}", &xwb_file.clone().display()))?;
    let wave_bank = WaveBank::parse(&xwb_data).context("failed to parse XWB file")?;

    let audio_data = wave_bank.sounds.get(&basename)
        .map(|sound| sound.to_wav().with_context(|| {
            format!(
                "failed to convert wave bank sound entry “{}” to WAV",
                basename
            )
        }))
        .unwrap_or_else(|| {
            if wave_bank.sounds.len() == 2 {
                warn!(
                    "Sound {} not found in wave bank, but it has two entries; assuming these are preview and full song",
                    basename
                );
                let mut sounds = wave_bank.sounds.values().collect::<Vec<&XWBSound>>();
                sounds.sort_unstable_by(|a, b| b.size.cmp(&a.size));
                sounds[0].to_wav().with_context(|| {
                    format!(
                        "failed to convert wave bank sound entry “{}” to WAV",
                        basename
                    )
                })
            } else {
                Err(anyhow!(
                    "Could not find matching sound in wave bank (searched for {})",
                    basename,
                ))
            }
        })?;

    let osz = osu::osz::Archive {
        beatmaps,
        assets: vec![("audio.wav", &audio_data)],
    };
    osz.write(&out_file)
        .with_context(|| format!("failed to write OSZ file to {}", out_file.display()))?;

    Ok(())
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

            let files = match &opts.single_file {
                Some(path) => {
                    if arc.has_file(&path) {
                        vec![path]
                    } else {
                        return Err(anyhow!("File “{}” not found in archive", path.display()));
                    }
                }
                None => arc.file_paths(),
            };

            for path in arc.file_paths() {
                if files.contains(&path) {
                    if opts.list_files {
                        println!("{}", path.display());
                    } else {
                        let data = arc.get_file(path)?.unwrap();
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
            let basename = opts.basename.clone().unwrap_or(
                opts.ssq_file
                    .file_stem()
                    .map(|stem| stem.to_str())
                    .flatten()
                    .map(|basename| basename.to_string())
                    .ok_or_else(|| {
                        anyhow!(
                        "Could not extract chart id from file name. Please specify it manually."
                    )
                    })?,
            );

            let mut convert_options = opts.convert;

            if let Some(musicdb_file) = &opts.musicdb_file {
                debug!("Reading metadata from {}", musicdb_file.display());
                let musicdb = read_musicdb(&musicdb_file)?;
                let musicdb_entry = musicdb
                    .get_entry_from_basename(&basename)
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
                convert_options.metadata.title = Some(basename.to_string());
            }

            ddr2osu(
                opts.ssq_file,
                opts.xwb_file,
                opts.out_file,
                basename,
                convert_options,
            )?
        }
        SubCommand::BatchDDR2osu(opts) => {
            let musicdb = read_musicdb(&opts.musicdb_file)?;

            fs::create_dir_all(&opts.out_dir)?;

            let pb = Arc::new(Mutex::new(ProgressBar::new(
                musicdb.music.len().try_into()?,
            )));
            musicdb.music.into_par_iter().for_each(|entry| {
                pb.lock().unwrap().message(&format!("{} ", entry.basename));
                pb.lock().unwrap().tick();

                let mut ssq_file = opts.ssq_dir.clone();
                ssq_file.push(&entry.basename);
                ssq_file.set_extension("ssq");
                let mut xwb_file = opts.xwb_dir.clone();
                xwb_file.push(&entry.basename);
                xwb_file.set_extension("xwb");
                let mut out_file = opts.out_dir.clone();
                out_file.push(format!("{} - {}.osz", entry.artist, entry.title).replace("/", "／"));

                let mut convert_options = opts.convert.clone();

                convert_options.metadata.title = Some(entry.title.clone());
                convert_options.metadata.artist = Some(entry.artist.clone());
                convert_options.metadata.levels = Some(entry.diff_lv.clone());

                ddr2osu(
                    ssq_file,
                    xwb_file,
                    out_file,
                    entry.basename.clone(),
                    convert_options,
                )
                .unwrap_or_else(move |err| {
                    error!(
                        "Could not convert {} ({}), continuing anyway",
                        entry.basename, err
                    )
                });

                pb.lock().unwrap().inc();
            })
        }
    }
    Ok(())
}
