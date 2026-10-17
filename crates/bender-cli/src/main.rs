//! `bender`: command-line renderer for bender-rs.

use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

use bender::{BendEffect, SettingKind, SettingValue, setting_groups};
use bender_gst::{OutputCodec, RenderJob, RenderSettings, RenderStatus};
use clap::{Parser, ValueEnum};

/// Render images and videos through emulated circuit-bent video hardware.
#[derive(Parser, Debug)]
#[command(name = "bender", version)]
struct Args {
    /// Input video or image.
    #[arg(short, long, required_unless_present_any = ["list_settings", "dump_preset"])]
    input: Option<PathBuf>,
    /// Output file (.mp4, .mkv or .png).
    #[arg(short, long, required_unless_present_any = ["list_settings", "dump_preset"])]
    output: Option<PathBuf>,
    /// JSON preset to start from.
    #[arg(long)]
    preset: Option<PathBuf>,
    /// Override a setting, e.g. `sync.h_skew=0.2` (repeatable).
    #[arg(long = "set", value_name = "ID=VALUE")]
    set: Vec<String>,
    /// Random seed (overrides preset and --set).
    #[arg(long, allow_hyphen_values = true)]
    seed: Option<i32>,
    /// Output codec; inferred from the output extension when omitted.
    #[arg(long, value_enum)]
    codec: Option<Codec>,
    /// H.264 quantizer (lower is better quality).
    #[arg(long, default_value_t = 18, value_parser = clap::value_parser!(u32).range(0..=51))]
    quantizer: u32,
    /// Processing/output height in scanlines (0 = native resolution).
    #[arg(long, default_value_t = 480)]
    scanlines: u32,
    /// Replace an existing output file.
    #[arg(long)]
    overwrite: bool,
    /// Print every setting with its range and default, then exit.
    #[arg(long)]
    list_settings: bool,
    /// Print the resulting preset JSON, then exit.
    #[arg(long)]
    dump_preset: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Codec {
    H264,
    Ffv1,
    Png,
}

fn main() -> ExitCode {
    env_logger::init();
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    if args.list_settings {
        list_settings();
        return Ok(());
    }

    let mut effect = match &args.preset {
        Some(path) => BendEffect::from_json(&std::fs::read_to_string(path)?)?,
        None => BendEffect::default(),
    };
    for entry in &args.set {
        let (id, value) = entry
            .split_once('=')
            .ok_or_else(|| format!("invalid --set '{entry}'; expected ID=VALUE"))?;
        effect.set_by_id(id.trim(), value.trim())?;
    }
    if let Some(seed) = args.seed {
        effect.seed = seed;
    }

    if args.dump_preset {
        println!("{}", effect.to_json());
        return Ok(());
    }

    let (Some(input), Some(output)) = (args.input, args.output) else {
        return Err("--input and --output are required".into());
    };
    let codec = match args.codec {
        Some(Codec::H264) => OutputCodec::H264 { quantizer: 18 },
        Some(Codec::Ffv1) => OutputCodec::Ffv1,
        Some(Codec::Png) => OutputCodec::Png,
        None => OutputCodec::from_extension(&output).ok_or_else(|| {
            let ext = output
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();
            format!("cannot infer codec from '{ext}'; pass --codec")
        })?,
    };
    let codec = match codec {
        OutputCodec::H264 { .. } => OutputCodec::H264 {
            quantizer: args.quantizer,
        },
        other => other,
    };
    if output.exists() && !args.overwrite {
        return Err(format!(
            "output '{}' already exists; pass --overwrite",
            output.display()
        )
        .into());
    }

    bender_gst::init()?;
    let job = RenderJob::start(RenderSettings {
        input,
        output,
        codec,
        scanlines: args.scanlines,
        effect,
    })?;
    let mut stderr = std::io::stderr();
    loop {
        match job.status() {
            RenderStatus::Running { progress } => {
                match progress {
                    Some(p) => write!(stderr, "\rrendering {:5.1}%", p * 100.0)?,
                    None => write!(stderr, "\rrendering…")?,
                }
                stderr.flush()?;
                thread::sleep(Duration::from_millis(200));
            }
            RenderStatus::Done => {
                writeln!(stderr, "\rrendering 100.0%")?;
                return Ok(());
            }
            RenderStatus::Failed(msg) => {
                writeln!(stderr)?;
                return Err(msg.into());
            }
            RenderStatus::Cancelled => {
                writeln!(stderr)?;
                return Err("render cancelled".into());
            }
        }
    }
}

fn list_settings() {
    let defaults = BendEffect::default();
    println!(
        "{:<28} int  default={} — random seed",
        "seed", defaults.seed
    );
    for group in setting_groups() {
        for d in &group.settings {
            let kind = match d.kind {
                SettingKind::Float {
                    min,
                    max,
                    logarithmic,
                } => format!(
                    "float {min}..{max}{}",
                    if logarithmic { " (log)" } else { "" }
                ),
                SettingKind::Int { min, max } => format!("int {min}..{max}"),
                SettingKind::Bool => "bool".to_owned(),
                SettingKind::Enum { options } => {
                    let names: Vec<_> = options.iter().map(|(n, _)| *n).collect();
                    format!("enum {}", names.join("|"))
                }
            };
            let value = match ((d.get)(&defaults), d.kind) {
                (SettingValue::Float(v), _) => v.to_string(),
                (SettingValue::Int(v), _) => v.to_string(),
                (SettingValue::Bool(v), _) => v.to_string(),
                (SettingValue::Enum(i), SettingKind::Enum { options }) => options
                    .get(i as usize)
                    .map_or_else(|| i.to_string(), |(n, _)| (*n).to_owned()),
                (SettingValue::Enum(i), _) => i.to_string(),
            };
            println!("{:<28} {kind} default={value} — {}", d.id, d.description);
        }
    }
}
