use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use thiserror::Error;

const MAX_ENTRIES: usize = 100;

/// A job queued onto the single thumbnail worker thread.
struct ThumbJob {
    image_path: String,
    thumb_path: PathBuf,
}

/// Single shared worker thread for thumbnail generation. Replaces the
/// "spawn one thread per screenshot" pattern which was unbounded.
fn thumbnail_tx() -> &'static Mutex<Option<mpsc::Sender<ThumbJob>>> {
    static TX: OnceLock<Mutex<Option<mpsc::Sender<ThumbJob>>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<ThumbJob>();
        thread::Builder::new()
            .name("snapforge-thumbnails".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    // Isolate any panic inside image decode / save so one bad
                    // file can't take down the worker and block every future
                    // thumbnail job.
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if let Ok(img) = image::open(&job.image_path) {
                            let thumb = img.thumbnail(200, 200);
                            let _ = thumb.save(&job.thumb_path);
                        }
                    }));
                    if result.is_err() {
                        eprintln!(
                            "[history] thumbnail worker caught panic for {}",
                            job.image_path
                        );
                    }
                }
            })
            .ok();
        Mutex::new(Some(tx))
    })
}

fn enqueue_thumbnail(job: ThumbJob) {
    if let Ok(guard) = thumbnail_tx().lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(job);
        }
    }
}

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("history IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse history: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub path: String,
    pub timestamp: String,
    pub thumbnail_path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScreenshotHistory {
    pub entries: Vec<HistoryEntry>,
}

impl ScreenshotHistory {
    /// Directory that holds history.json and the thumbnails/ subfolder.
    fn history_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("snapforge")
    }

    fn history_path() -> PathBuf {
        Self::history_dir().join("history.json")
    }

    fn thumbnails_dir() -> PathBuf {
        Self::history_dir().join("thumbnails")
    }

    pub fn load() -> Result<Self, HistoryError> {
        let path = Self::history_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let history: Self = serde_json::from_str(&contents)?;
        Ok(history)
    }

    pub fn save(&self) -> Result<(), HistoryError> {
        let path = Self::history_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }

    /// Add an entry for the screenshot at `image_path`.
    /// Thumbnail is generated in a background thread to avoid blocking the UI.
    pub fn add_entry(&mut self, image_path: &str) -> Result<(), HistoryError> {
        let thumb_dir = Self::thumbnails_dir();
        std::fs::create_dir_all(&thumb_dir)?;

        let stem = std::path::Path::new(image_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let thumb_filename = format!("{stem}_thumb.png");
        let thumb_path = thumb_dir.join(&thumb_filename);
        let timestamp = chrono::Local::now().to_rfc3339();

        let entry = HistoryEntry {
            path: image_path.to_string(),
            timestamp,
            thumbnail_path: thumb_path.display().to_string(),
        };

        self.entries.push(entry);

        // Cap at MAX_ENTRIES, remove oldest first
        if self.entries.len() > MAX_ENTRIES {
            let excess = self.entries.len() - MAX_ENTRIES;
            for old in self.entries.drain(..excess) {
                let _ = std::fs::remove_file(&old.thumbnail_path);
            }
        }

        self.save()?;

        // Generate thumbnail on the single shared worker thread — bounded.
        enqueue_thumbnail(ThumbJob {
            image_path: image_path.to_string(),
            thumb_path,
        });

        Ok(())
    }

    /// Clear all entries and remove thumbnail files.
    pub fn clear(&mut self) -> Result<(), HistoryError> {
        for entry in &self.entries {
            let _ = std::fs::remove_file(&entry.thumbnail_path);
        }
        self.entries.clear();
        self.save()?;
        Ok(())
    }

    /// Remove a single entry (and its thumbnail) by source file path.
    pub fn remove_entry(&mut self, path: &str) -> Result<(), HistoryError> {
        if let Some(idx) = self.entries.iter().position(|e| e.path == path) {
            let removed = self.entries.remove(idx);
            let _ = std::fs::remove_file(&removed.thumbnail_path);
            self.save()?;
        }
        Ok(())
    }
}

/// Heuristic check for "ffmpeg was killed mid-write" — a file whose first 8
/// bytes look like an MP4 (ftyp box) but that never got a `moov` atom written.
/// Used by the FFI `snapforge_history_add` path to refuse to index garbage
/// after a SIGKILL. Returns `false` for non-mp4 paths, missing files, or any
/// IO error (we prefer a false negative to refusing a valid file).
pub fn is_incomplete_mp4(path: &str) -> bool {
    let p = std::path::Path::new(path);
    let ext_is_mp4 = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("mp4"))
        .unwrap_or(false);
    if !ext_is_mp4 {
        return false;
    }

    let Ok(mut f) = std::fs::File::open(p) else {
        return false;
    };
    use std::io::{Read, Seek, SeekFrom};

    let mut header = [0u8; 8];
    if f.read_exact(&mut header).is_err() {
        return false;
    }
    // Bytes 4..8 = box type. Real MP4s start with an `ftyp` box.
    if &header[4..8] != b"ftyp" {
        return false;
    }

    let Ok(metadata) = f.metadata() else {
        return false;
    };
    let size = metadata.len();
    if size < 16 {
        return true; // practically empty — treat as incomplete
    }

    // Scan the last 64KB for a `moov` atom. ffmpeg (faststart OFF) writes the
    // moov atom at the end of the file when it finalizes; an abruptly killed
    // ffmpeg never gets there.
    let tail_len = std::cmp::min(size, 64 * 1024);
    if f.seek(SeekFrom::End(-(tail_len as i64))).is_err() {
        return false;
    }
    let mut tail = vec![0u8; tail_len as usize];
    if f.read_exact(&mut tail).is_err() {
        return false;
    }
    !tail.windows(4).any(|w| w == b"moov")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn non_mp4_extension_never_incomplete() {
        assert!(!is_incomplete_mp4("foo.png"));
        assert!(!is_incomplete_mp4("foo.mov"));
        assert!(!is_incomplete_mp4("foo"));
    }

    #[test]
    fn missing_file_is_not_incomplete() {
        assert!(!is_incomplete_mp4("/definitely/not/here/xyz.mp4"));
    }

    #[test]
    fn ftyp_without_moov_is_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.mp4");
        let mut f = std::fs::File::create(&path).unwrap();
        // 4 bytes size + "ftyp" + some filler — no moov atom anywhere.
        let mut buf = vec![0u8; 0];
        buf.extend_from_slice(&[0, 0, 0, 0x20]);
        buf.extend_from_slice(b"ftyp");
        buf.extend_from_slice(&[0u8; 64]);
        f.write_all(&buf).unwrap();
        drop(f);
        assert!(is_incomplete_mp4(path.to_str().unwrap()));
    }

    #[test]
    fn ftyp_with_moov_is_complete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ok.mp4");
        let mut f = std::fs::File::create(&path).unwrap();
        let mut buf = vec![0u8; 0];
        buf.extend_from_slice(&[0, 0, 0, 0x20]);
        buf.extend_from_slice(b"ftyp");
        buf.extend_from_slice(&[0u8; 64]);
        buf.extend_from_slice(b"moov");
        buf.extend_from_slice(&[0u8; 16]);
        f.write_all(&buf).unwrap();
        drop(f);
        assert!(!is_incomplete_mp4(path.to_str().unwrap()));
    }
}
