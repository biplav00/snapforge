use clap::{Parser, Subcommand};
use screen_core::config::AppConfig;
use screen_core::types::{CaptureFormat, Rect};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(name = "screen", about = "ScreenSnap — screenshot and screen recording tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Take a screenshot
    Capture {
        /// Capture the full screen
        #[arg(long)]
        fullscreen: bool,

        /// Use the last remembered region
        #[arg(long)]
        last_region: bool,

        /// Capture a specific region: x,y,width,height
        #[arg(long, value_parser = parse_region)]
        region: Option<Rect>,

        /// Output format: png, jpg, webp
        #[arg(long, short)]
        format: Option<String>,

        /// Output file path (overrides default)
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Image quality for jpg/webp (1-100)
        #[arg(long, short, default_value = "90", value_parser = clap::value_parser!(u8).range(1..=100))]
        quality: u8,

        /// Display index (0 = main display)
        #[arg(long, short, default_value = "0")]
        display: usize,
    },

    /// Record the screen
    Record {
        /// Record the full screen
        #[arg(long)]
        fullscreen: bool,

        /// Record a specific region: x,y,width,height
        #[arg(long, value_parser = parse_region)]
        region: Option<Rect>,

        /// Output format: mp4, gif
        #[arg(long, short, default_value = "mp4")]
        format: String,

        /// Frame rate
        #[arg(long, default_value = "30")]
        fps: u32,

        /// Output file path
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Display index
        #[arg(long, short, default_value = "0")]
        display: usize,
    },
}

fn parse_region(s: &str) -> Result<Rect, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err("region must be x,y,width,height".to_string());
    }
    Ok(Rect {
        x: parts[0].trim().parse().map_err(|_| "invalid x")?,
        y: parts[1].trim().parse().map_err(|_| "invalid y")?,
        width: parts[2].trim().parse().map_err(|_| "invalid width")?,
        height: parts[3].trim().parse().map_err(|_| "invalid height")?,
    })
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Capture {
            fullscreen,
            last_region,
            region,
            format,
            output,
            quality,
            display,
        } => {
            if let Err(e) = handle_capture(fullscreen, last_region, region, format, output, quality, display) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
        Commands::Record {
            fullscreen,
            region,
            format,
            fps,
            output,
            display,
        } => {
            if let Err(e) = handle_record(fullscreen, region, format, fps, output, display) {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        }
    }
}

fn handle_capture(
    fullscreen: bool,
    last_region: bool,
    region: Option<Rect>,
    format_str: Option<String>,
    output: Option<PathBuf>,
    quality: u8,
    display: usize,
) -> Result<(), screen_core::ScreenError> {
    let config = AppConfig::load()?;

    let format = format_str
        .as_deref()
        .and_then(CaptureFormat::from_extension)
        .unwrap_or(config.screenshot_format);

    let save_path = output.unwrap_or_else(|| config.save_file_path());

    if let Some(region) = region {
        let path = screen_core::screenshot_region(display, region, &save_path, format, quality, config.auto_copy_clipboard)?;
        println!("Saved to: {}", path.display());

        if config.remember_last_region {
            let mut config = config;
            config.last_region = Some(screen_core::types::LastRegion { display, rect: region });
            let _ = config.save();
        }
    } else if last_region {
        match config.last_region {
            Some(last) => {
                let path = screen_core::screenshot_region(last.display, last.rect, &save_path, format, quality, config.auto_copy_clipboard)?;
                println!("Saved to: {}", path.display());
            }
            None => {
                eprintln!("No last region saved. Use --region or capture interactively first.");
                std::process::exit(1);
            }
        }
    } else if fullscreen {
        let path = screen_core::screenshot_fullscreen(display, &save_path, format, quality, config.auto_copy_clipboard)?;
        println!("Saved to: {}", path.display());
    } else {
        // Launch the Tauri overlay app for interactive region selection
        let exe = std::env::current_exe().unwrap_or_default();
        let app_dir = exe.parent().unwrap_or(std::path::Path::new("."));
        let app_path = app_dir.join("screensnap-app");

        if app_path.exists() {
            let status = std::process::Command::new(&app_path)
                .status()
                .map_err(|e| {
                    eprintln!("Failed to launch overlay: {}", e);
                    std::process::exit(1);
                })
                .unwrap();

            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        } else {
            eprintln!("Interactive region selection requires the ScreenSnap app.");
            eprintln!("Use --fullscreen, --region, or --last-region for CLI-only capture.");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn handle_record(
    fullscreen: bool,
    region: Option<Rect>,
    format_str: String,
    fps: u32,
    output: Option<PathBuf>,
    display: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    screen_core::record::check_ffmpeg()?;

    let config = screen_core::config::AppConfig::load()?;
    let recording_format = match format_str.to_lowercase().as_str() {
        "gif" => screen_core::config::RecordingFormat::Gif,
        _ => screen_core::config::RecordingFormat::Mp4,
    };

    let output_path = output.unwrap_or_else(|| config.recording_file_path());

    let record_region = if let Some(r) = region {
        Some(r)
    } else if fullscreen {
        None
    } else {
        eprintln!("Specify --fullscreen or --region for CLI recording.");
        std::process::exit(1);
    };

    let record_config = screen_core::record::RecordConfig {
        display,
        region: record_region,
        output_path: output_path.clone(),
        ffmpeg_path: None,
        format: recording_format,
        fps,
        quality: config.recording.quality,
    };

    println!("Recording to: {} (press Ctrl+C to stop)", output_path.display());

    let handle = screen_core::record::ffmpeg::start_recording(record_config)?;

    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    while running.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    println!("\nStopping recording...");
    handle.stop()?;
    println!("Saved to: {}", output_path.display());

    Ok(())
}
