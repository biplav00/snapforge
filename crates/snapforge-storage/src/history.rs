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
                        tracing::error!(
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
        // 32 MiB ceiling — history grows over time but should never approach this.
        // Caps memory amplification on a corrupt or hostile file.
        const MAX_HISTORY_BYTES: u64 = 32 * 1024 * 1024;
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() > MAX_HISTORY_BYTES {
                return Ok(Self::default());
            }
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
        // Include a hash of the FULL source path so two sources with the same
        // stem in different directories can't collide on one thumbnail file
        // (eviction / remove_entry would otherwise delete the survivor's thumb).
        let hash = path_hash(image_path);
        let thumb_filename = format!("{stem}_{hash:016x}_thumb.png");
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

/// Hash of a path string. Used to make thumbnail filenames unique per FULL
/// source path, not just per file stem. Only the derived filename is persisted
/// (in the entry's `thumbnail_path`), so cross-run/version stability is not required.
fn path_hash(path: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    h.finish()
}

/// Heuristic check for "ffmpeg was killed mid-write" — a file whose first 8
/// bytes look like an MP4 (ftyp box) but that never got a `moov` atom written.
/// Used by the FFI `snapforge_history_add` path to refuse to index garbage
/// after a SIGKILL. Returns `false` for non-mp4 paths, missing files, or any
/// IO error (we prefer a false negative to refusing a valid file).
///
/// Walks the top-level MP4 box structure from offset 0 (seeking over box
/// bodies, never reading them): the file is complete iff a top-level `moov`
/// box exists and is fully contained in the file. A truncated or invalid box
/// header means ffmpeg never finalized — incomplete. We deliberately do NOT
/// scan the tail for the `moov` fourcc: the moov box *contents* grow with
/// recording length, so on long recordings the fourcc (at the box start)
/// falls outside any fixed-size tail window.
pub fn is_incomplete_mp4(path: &str) -> bool {
    let p = std::path::Path::new(path);
    let ext_is_mp4 = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("mp4"));
    if !ext_is_mp4 {
        return false;
    }

    let Ok(mut f) = std::fs::File::open(p) else {
        return false;
    };
    use std::io::{Read, Seek, SeekFrom};

    let Ok(metadata) = f.metadata() else {
        return false;
    };
    let size = metadata.len();

    let mut header = [0u8; 8];
    if f.read_exact(&mut header).is_err() {
        return false;
    }
    // Bytes 4..8 = box type. Real MP4s start with an `ftyp` box.
    if &header[4..8] != b"ftyp" {
        return false;
    }
    if size < 16 {
        return true; // practically empty — treat as incomplete
    }

    // Walk top-level boxes: 8-byte header = u32 big-endian size + 4-byte type.
    let mut offset: u64 = 0;
    while offset < size {
        if offset + 8 > size {
            return true; // truncated box header
        }
        if f.seek(SeekFrom::Start(offset)).is_err() {
            return false;
        }
        let mut hdr = [0u8; 8];
        if f.read_exact(&mut hdr).is_err() {
            return false; // bytes exist per metadata, so this is a real IO error
        }
        let size32 = u64::from(u32::from_be_bytes([hdr[0], hdr[1], hdr[2], hdr[3]]));
        let box_type = [hdr[4], hdr[5], hdr[6], hdr[7]];
        let box_size = match size32 {
            // size == 0: box extends to end of file.
            0 => size - offset,
            // size == 1: actual size is the u64 `largesize` field that follows.
            1 => {
                if offset + 16 > size {
                    return true; // truncated largesize
                }
                let mut large = [0u8; 8];
                if f.read_exact(&mut large).is_err() {
                    return false;
                }
                let largesize = u64::from_be_bytes(large);
                if largesize < 16 {
                    return true; // can't even hold its own extended header
                }
                largesize
            }
            s if s < 8 => return true, // invalid: smaller than its own header
            s => s,
        };
        let Some(end) = offset.checked_add(box_size) else {
            return true; // overflow — garbage header
        };
        if end > size {
            return true; // box not fully contained — ffmpeg was killed mid-write
        }
        if box_type == *b"moov" {
            return false; // complete: a fully-contained top-level moov exists
        }
        offset = end;
    }
    true // reached EOF without finding a moov box
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a well-formed MP4 box: u32 BE size (header + payload) + 4-byte type.
    fn mp4_box(box_type: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(8 + payload.len());
        v.extend_from_slice(&u32::try_from(8 + payload.len()).unwrap().to_be_bytes());
        v.extend_from_slice(&box_type);
        v.extend_from_slice(payload);
        v
    }

    fn write_mp4(name: &str, contents: &[u8]) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(contents).unwrap();
        drop(f);
        let s = path.to_str().unwrap().to_string();
        (dir, s)
    }

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
        // ftyp + mdat only — ffmpeg was killed before finalizing the moov.
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&mp4_box(*b"mdat", &[0u8; 64]));
        let (_dir, path) = write_mp4("broken.mp4", &buf);
        assert!(is_incomplete_mp4(&path));
    }

    #[test]
    fn ftyp_with_moov_is_complete() {
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&mp4_box(*b"mdat", &[0u8; 64]));
        buf.extend_from_slice(&mp4_box(*b"moov", &[0u8; 32]));
        let (_dir, path) = write_mp4("ok.mp4", &buf);
        assert!(!is_incomplete_mp4(&path));
    }

    #[test]
    fn large_moov_with_fourcc_outside_64kb_tail_is_complete() {
        // Long recordings grow the moov sample tables; the `moov` fourcc sits
        // at the box START, far more than 64KB from EOF. The old tail scan
        // misclassified these as incomplete.
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&mp4_box(*b"mdat", &[0u8; 64]));
        buf.extend_from_slice(&mp4_box(*b"moov", &vec![0u8; 200 * 1024]));
        let (_dir, path) = write_mp4("long.mp4", &buf);
        assert!(!is_incomplete_mp4(&path));
    }

    #[test]
    fn truncated_last_box_is_incomplete() {
        // mdat header claims 1000 bytes but the file ends after 10 — the box
        // is not fully contained, so the file was cut off mid-write.
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&1000u32.to_be_bytes());
        buf.extend_from_slice(b"mdat");
        buf.extend_from_slice(&[0u8; 10]);
        let (_dir, path) = write_mp4("truncated.mp4", &buf);
        assert!(is_incomplete_mp4(&path));
    }

    #[test]
    fn truncated_box_header_is_incomplete() {
        // A dangling 4-byte fragment where a box header should be.
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&[0, 0, 0, 9]);
        let (_dir, path) = write_mp4("fragment.mp4", &buf);
        assert!(is_incomplete_mp4(&path));
    }

    #[test]
    fn size_zero_moov_extending_to_eof_is_complete() {
        // size == 0 means "box extends to end of file".
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(b"moov");
        buf.extend_from_slice(&[0u8; 32]);
        let (_dir, path) = write_mp4("eofbox.mp4", &buf);
        assert!(!is_incomplete_mp4(&path));
    }

    #[test]
    fn size_zero_non_moov_box_is_incomplete() {
        // A to-EOF mdat swallows the rest of the file; no moov ever appears.
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&0u32.to_be_bytes());
        buf.extend_from_slice(b"mdat");
        buf.extend_from_slice(&[0u8; 32]);
        let (_dir, path) = write_mp4("eofmdat.mp4", &buf);
        assert!(is_incomplete_mp4(&path));
    }

    #[test]
    fn largesize_moov_is_complete() {
        // size == 1: real size lives in the 64-bit largesize field.
        let payload = [0u8; 32];
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(b"moov");
        buf.extend_from_slice(&(16 + payload.len() as u64).to_be_bytes());
        buf.extend_from_slice(&payload);
        let (_dir, path) = write_mp4("large.mp4", &buf);
        assert!(!is_incomplete_mp4(&path));
    }

    #[test]
    fn truncated_largesize_moov_is_incomplete() {
        // largesize claims more bytes than the file holds.
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&1u32.to_be_bytes());
        buf.extend_from_slice(b"moov");
        buf.extend_from_slice(&4096u64.to_be_bytes());
        buf.extend_from_slice(&[0u8; 8]);
        let (_dir, path) = write_mp4("largetrunc.mp4", &buf);
        assert!(is_incomplete_mp4(&path));
    }

    #[test]
    fn path_hash_distinguishes_same_stem_different_dirs() {
        // Thumbnail names embed this hash so /a/shot.png and /b/shot.png
        // can't collide on one thumbnail file.
        assert_ne!(path_hash("/a/shot.png"), path_hash("/b/shot.png"));
        assert_eq!(path_hash("/a/shot.png"), path_hash("/a/shot.png"));
    }
}
