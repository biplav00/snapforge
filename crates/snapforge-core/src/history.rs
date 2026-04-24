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
                    if let Ok(img) = image::open(&job.image_path) {
                        let thumb = img.thumbnail(200, 200);
                        let _ = thumb.save(&job.thumb_path);
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

/// Detect the media kind of a file from its extension.
/// Returns "video" for mp4/mov/gif, "image" for everything else.
pub fn media_kind(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "mp4" | "mov" | "m4v" | "webm" => "video",
        _ => "image",
    }
}
