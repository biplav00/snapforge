fn main() {
    // If ffmpeg binary exists in binaries/, tell Tauri to bundle it as a resource.
    // Run scripts/download-ffmpeg.sh before `cargo tauri build` to include FFmpeg.
    let target = std::env::var("TAURI_ENV_TARGET_TRIPLE")
        .or_else(|_| std::env::var("TARGET"))
        .unwrap_or_default();

    let ffmpeg_name = if target.contains("windows") {
        format!("ffmpeg-{}.exe", target)
    } else {
        format!("ffmpeg-{}", target)
    };

    let ffmpeg_path = std::path::Path::new("binaries").join(&ffmpeg_name);
    if ffmpeg_path.exists() {
        println!(
            "cargo:warning=Bundling FFmpeg from {}",
            ffmpeg_path.display()
        );
        // Copy to output directory so it's next to the binary
        if let Ok(out_dir) = std::env::var("OUT_DIR") {
            let dest = std::path::Path::new(&out_dir)
                .ancestors()
                .nth(3)
                .unwrap_or(std::path::Path::new("."))
                .join(&ffmpeg_name);
            let _ = std::fs::copy(&ffmpeg_path, &dest);
        }
    }

    tauri_build::build()
}
