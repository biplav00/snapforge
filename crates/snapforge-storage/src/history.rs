use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, MutexGuard, OnceLock};
use std::thread;
use thiserror::Error;

const MAX_ENTRIES: usize = 100;

/// Process-wide lock held across every load→modify→save of history.json (and
/// across the thumbnail worker's "is this still indexed?" check + rename).
/// Screenshots, recordings and history deletes arrive on different threads
/// (GUI, QThreadPool workers, the recording stop thread); without this lock
/// two concurrent writers each load the same snapshot and the last save
/// silently drops the other's entry.
static HISTORY_LOCK: Mutex<()> = Mutex::new(());

fn history_lock() -> MutexGuard<'static, ()> {
    // The guarded data is `()`, so a poisoned lock carries no broken invariant.
    HISTORY_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A job queued onto the single thumbnail worker thread.
struct ThumbJob {
    /// Directory holding history.json — used to re-check the entry is still
    /// indexed before the finished thumbnail is moved into place.
    dir: PathBuf,
    image_path: String,
    thumb_path: PathBuf,
}

/// Single shared worker thread for thumbnail generation. Replaces the
/// "spawn one thread per screenshot" pattern which was unbounded.
fn thumbnail_tx() -> &'static Mutex<Option<mpsc::Sender<ThumbJob>>> {
    static TX: OnceLock<Mutex<Option<mpsc::Sender<ThumbJob>>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<ThumbJob>();
        let spawned = thread::Builder::new()
            .name("snapforge-thumbnails".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    run_thumb_job(&job);
                }
            });
        if let Err(e) = spawned {
            tracing::error!("[history] failed to spawn thumbnail worker: {}", e);
            return Mutex::new(None);
        }
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

/// Decode an image defensively: sniff the real format from the bytes (not
/// the extension) and cap dimensions / allocation so a hostile or corrupt
/// file can't exhaust memory. The release profile is `panic = "abort"`, so a
/// `catch_unwind` here would be dead code — bounding the decoder is the guard.
fn decode_bounded(path: &str) -> Option<image::DynamicImage> {
    let mut reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32_768);
    limits.max_image_height = Some(32_768);
    limits.max_alloc = Some(1024 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().ok()
}

fn is_indexed(dir: &Path, image_path: &str, thumb_path: &Path) -> bool {
    ScreenshotHistory::load_from(&ScreenshotHistory::history_path_in(dir)).is_ok_and(|h| {
        h.entries
            .iter()
            .any(|e| e.path == image_path && Path::new(&e.thumbnail_path) == thumb_path)
    })
}

fn run_thumb_job(job: &ThumbJob) {
    // Cheap pre-check: skip the decode entirely if the entry was already
    // removed/evicted while the job sat in the queue.
    {
        let _g = history_lock();
        if !is_indexed(&job.dir, &job.image_path, &job.thumb_path) {
            return;
        }
    }
    let Some(img) = decode_bounded(&job.image_path) else {
        return;
    };
    let thumb = img.thumbnail(200, 200);
    // Write to a temp sibling and rename, so a reader never sees a partial
    // PNG and a crash never leaves a truncated thumbnail behind.
    let tmp = crate::unique_tmp_path(&job.thumb_path);
    if let Err(e) = thumb.save_with_format(&tmp, image::ImageFormat::Png) {
        tracing::warn!(
            "[history] thumbnail write failed for {}: {}",
            job.image_path,
            e
        );
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    // Re-check under the lock: if remove/clear/eviction ran during the
    // decode, drop the result instead of leaking an orphaned thumbnail.
    let _g = history_lock();
    if is_indexed(&job.dir, &job.image_path, &job.thumb_path) {
        if std::fs::rename(&tmp, &job.thumb_path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
}

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("history IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse history: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub path: String,
    pub timestamp: String,
    pub thumbnail_path: String,
}

/// The history index. Mutations are associated functions (`add_entry`,
/// `remove_entry`, `clear`, `update`) rather than `&mut self` methods so
/// each one performs its whole load→modify→save under [`HISTORY_LOCK`] —
/// there is no way to mutate a stale snapshot and save it over a newer one.
/// The `*_in(dir, ..)` variants take the history directory explicitly (tests);
/// the plain ones use the per-user config dir.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScreenshotHistory {
    pub entries: Vec<HistoryEntry>,
}

impl ScreenshotHistory {
    /// Directory that holds history.json and the thumbnails/ subfolder.
    pub fn default_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("snapforge")
    }

    fn history_path_in(dir: &Path) -> PathBuf {
        dir.join("history.json")
    }

    fn thumbnails_dir_in(dir: &Path) -> PathBuf {
        dir.join("thumbnails")
    }

    /// Read-only snapshot of the default history.
    pub fn load() -> Result<Self, HistoryError> {
        Self::load_in(&Self::default_dir())
    }

    pub fn load_in(dir: &Path) -> Result<Self, HistoryError> {
        let _g = history_lock();
        Self::load_from(&Self::history_path_in(dir))
    }

    /// Unlocked load; callers must hold [`HISTORY_LOCK`] (self-heal renames
    /// the bad file aside, which must not race a concurrent save).
    fn load_from(path: &Path) -> Result<Self, HistoryError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        // 32 MiB ceiling — history grows over time but should never approach this.
        // Caps memory amplification on a corrupt or hostile file.
        const MAX_HISTORY_BYTES: u64 = 32 * 1024 * 1024;
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() > MAX_HISTORY_BYTES {
                Self::preserve_bad(path, &format!("implausibly large ({} bytes)", meta.len()));
                return Ok(Self::default());
            }
        }
        // Raw bytes + from_slice: invalid UTF-8 is a parse error and goes
        // through the self-heal path rather than failing every capture.
        let bytes = std::fs::read(path)?;
        match serde_json::from_slice::<Self>(&bytes) {
            Ok(history) => Ok(history),
            Err(e) => {
                // Self-heal: one truncated/corrupt history.json must not break
                // every future capture. Move it aside, log, return defaults —
                // mirrors AppConfig::load(). Moving (not copying) means the
                // same bad file is backed up exactly once.
                Self::preserve_bad(path, &format!("corrupt ({e})"));
                Ok(Self::default())
            }
        }
    }

    fn preserve_bad(path: &Path, why: &str) {
        match crate::backup_file(path) {
            Ok(bak) => tracing::warn!(
                "[history] {} is {}; preserved as {}; using empty history",
                path.display(),
                why,
                bak.display()
            ),
            Err(e) => tracing::error!(
                "[history] {} is {}; backup failed: {}; using empty history",
                path.display(),
                why,
                e
            ),
        }
    }

    fn save_in(&self, dir: &Path) -> Result<(), HistoryError> {
        std::fs::create_dir_all(dir)?;
        let contents = serde_json::to_string_pretty(self)?;
        // Durable atomic write: unique temp + fsync + rename.
        crate::write_atomic(&Self::history_path_in(dir), contents.as_bytes())?;
        Ok(())
    }

    /// Run `f` on the current history and save the result, all under the
    /// process-wide history lock.
    pub fn update<R>(f: impl FnOnce(&mut Self) -> R) -> Result<R, HistoryError> {
        Self::update_in(&Self::default_dir(), f)
    }

    pub fn update_in<R>(dir: &Path, f: impl FnOnce(&mut Self) -> R) -> Result<R, HistoryError> {
        let _g = history_lock();
        let mut h = Self::load_from(&Self::history_path_in(dir))?;
        let r = f(&mut h);
        h.save_in(dir)?;
        Ok(r)
    }

    /// Index the file at `image_path`. Re-adding an already-indexed path
    /// replaces the old entry (moving it to newest). The thumbnail is
    /// generated on the shared background worker.
    pub fn add_entry(image_path: &str) -> Result<(), HistoryError> {
        Self::add_entry_in(&Self::default_dir(), image_path)
    }

    pub fn add_entry_in(dir: &Path, image_path: &str) -> Result<(), HistoryError> {
        let thumb_dir = Self::thumbnails_dir_in(dir);
        std::fs::create_dir_all(&thumb_dir)?;

        let stem = Path::new(image_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        // Include a hash of the FULL source path so two sources with the same
        // stem in different directories can't collide on one thumbnail file
        // (eviction / remove_entry would otherwise delete the survivor's thumb).
        let hash = path_hash(image_path);
        let thumb_path = thumb_dir.join(format!("{stem}_{hash:016x}_thumb.png"));
        let entry = HistoryEntry {
            path: image_path.to_string(),
            timestamp: chrono::Local::now().to_rfc3339(),
            thumbnail_path: thumb_path.display().to_string(),
        };

        let guard = history_lock();
        let mut hist = Self::load_from(&Self::history_path_in(dir))?;
        // Dedup by path. The replaced entry's thumbnail is the same file the
        // new job regenerates (path-derived name), so it is NOT deleted here.
        hist.entries.retain(|e| e.path != image_path);
        hist.entries.push(entry);

        // Cap at MAX_ENTRIES, oldest first. Evicted thumbnails are deleted
        // only after the save succeeds, so a failed save never leaves indexed
        // entries pointing at deleted thumbnails.
        let evicted: Vec<HistoryEntry> = if hist.entries.len() > MAX_ENTRIES {
            let excess = hist.entries.len() - MAX_ENTRIES;
            hist.entries.drain(..excess).collect()
        } else {
            Vec::new()
        };

        hist.save_in(dir)?;

        let live: HashSet<PathBuf> = hist
            .entries
            .iter()
            .map(|e| PathBuf::from(&e.thumbnail_path))
            .collect();
        for old in evicted {
            if !live.contains(Path::new(&old.thumbnail_path)) {
                let _ = std::fs::remove_file(&old.thumbnail_path);
            }
        }
        sweep_orphan_thumbnails(&thumb_dir, &live);
        drop(guard);

        enqueue_thumbnail(ThumbJob {
            dir: dir.to_path_buf(),
            image_path: image_path.to_string(),
            thumb_path,
        });
        Ok(())
    }

    /// Clear all entries, then remove every thumbnail file.
    pub fn clear() -> Result<(), HistoryError> {
        Self::clear_in(&Self::default_dir())
    }

    pub fn clear_in(dir: &Path) -> Result<(), HistoryError> {
        let _g = history_lock();
        Self::default().save_in(dir)?;
        sweep_orphan_thumbnails(&Self::thumbnails_dir_in(dir), &HashSet::new());
        Ok(())
    }

    /// Remove a single entry (and, after the save, its thumbnail) by path.
    pub fn remove_entry(path: &str) -> Result<(), HistoryError> {
        Self::remove_entry_in(&Self::default_dir(), path)
    }

    pub fn remove_entry_in(dir: &Path, path: &str) -> Result<(), HistoryError> {
        let _g = history_lock();
        let mut hist = Self::load_from(&Self::history_path_in(dir))?;
        let before = hist.entries.len();
        let mut removed = Vec::new();
        hist.entries.retain(|e| {
            if e.path == path {
                removed.push(e.thumbnail_path.clone());
                false
            } else {
                true
            }
        });
        if hist.entries.len() != before {
            hist.save_in(dir)?;
            for t in removed {
                let _ = std::fs::remove_file(t);
            }
        }
        Ok(())
    }
}

/// Delete thumbnail files no live entry references. In-flight worker temp
/// files (hidden `.…tmp-…`) are skipped unless older than an hour (left by a
/// crashed process). Caller holds [`HISTORY_LOCK`].
fn sweep_orphan_thumbnails(thumb_dir: &Path, live: &HashSet<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(thumb_dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            let stale = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age.as_secs() > 3600);
            if stale && name.contains(".tmp-") {
                let _ = std::fs::remove_file(&p);
            }
            continue;
        }
        if p.is_file() && !live.contains(&p) {
            let _ = std::fs::remove_file(&p);
        }
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

/// Heuristic check for "ffmpeg was killed mid-write / never produced output".
/// Used to refuse to index garbage after a SIGKILL or a disk-full stop.
///
/// For a `.mp4` path returns `true` when the file is missing, smaller than 16
/// bytes (can't hold even an `ftyp` box + one header), or starts with an
/// `ftyp` box but has no fully-contained top-level `moov` box. Returns `false`
/// for non-mp4 paths, files that don't start with `ftyp` (not ours to judge),
/// and IO errors other than "not found" (prefer a false negative to refusing
/// a valid file).
///
/// Walks the top-level MP4 box structure from offset 0 (seeking over box
/// bodies, never reading them): the file is complete iff a top-level `moov`
/// box exists and is fully contained in the file. We deliberately do NOT
/// scan the tail for the `moov` fourcc: the moov box *contents* grow with
/// recording length, so on long recordings the fourcc (at the box start)
/// falls outside any fixed-size tail window.
///
/// Fragmented MP4 (`+frag_keyframe+empty_moov`) writes `ftyp`, then a small
/// complete `moov` up front, then `moof`/`mdat` pairs: the walk finds that
/// early `moov` and reports complete — correct, since every finished fragment
/// is playable even if the process died mid-way through a later one.
pub fn is_incomplete_mp4(path: &str) -> bool {
    let p = Path::new(path);
    let ext_is_mp4 = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("mp4"));
    if !ext_is_mp4 {
        return false;
    }

    let mut f = match std::fs::File::open(p) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
        Err(_) => return false,
    };
    use std::io::{Read, Seek, SeekFrom};

    let Ok(metadata) = f.metadata() else {
        return false;
    };
    let size = metadata.len();
    if size < 16 {
        return true; // zero-byte / practically empty — never finalized
    }

    let mut header = [0u8; 8];
    if f.read_exact(&mut header).is_err() {
        return false;
    }
    // Bytes 4..8 = box type. Real MP4s start with an `ftyp` box.
    if &header[4..8] != b"ftyp" {
        return false;
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
    fn missing_file_is_incomplete() {
        assert!(is_incomplete_mp4("/definitely/not/here/xyz.mp4"));
    }

    #[test]
    fn zero_byte_and_tiny_mp4_are_incomplete() {
        let (_d, empty) = write_mp4("empty.mp4", &[]);
        assert!(is_incomplete_mp4(&empty));
        let (_d2, tiny) = write_mp4("tiny.mp4", &[0, 0, 0, 8, b'f', b't', b'y', b'p']);
        assert!(is_incomplete_mp4(&tiny));
    }

    #[test]
    fn fragmented_mp4_with_early_moov_is_complete() {
        // +frag_keyframe+empty_moov layout: ftyp, small moov, moof/mdat pairs.
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&mp4_box(*b"moov", &[0u8; 40]));
        buf.extend_from_slice(&mp4_box(*b"moof", &[0u8; 16]));
        buf.extend_from_slice(&mp4_box(*b"mdat", &[0u8; 64]));
        buf.extend_from_slice(&mp4_box(*b"moof", &[0u8; 16]));
        buf.extend_from_slice(&mp4_box(*b"mdat", &[0u8; 64]));
        let (_dir, path) = write_mp4("frag.mp4", &buf);
        assert!(!is_incomplete_mp4(&path));
    }

    #[test]
    fn fragmented_mp4_killed_mid_fragment_is_still_complete() {
        // The last mdat is truncated, but the early moov + finished fragments
        // are playable — must not be misreported as incomplete.
        let mut buf = mp4_box(*b"ftyp", &[0u8; 24]);
        buf.extend_from_slice(&mp4_box(*b"moov", &[0u8; 40]));
        buf.extend_from_slice(&mp4_box(*b"moof", &[0u8; 16]));
        buf.extend_from_slice(&1000u32.to_be_bytes());
        buf.extend_from_slice(b"mdat");
        buf.extend_from_slice(&[0u8; 10]);
        let (_dir, path) = write_mp4("fragcut.mp4", &buf);
        assert!(!is_incomplete_mp4(&path));
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
    fn corrupt_history_loads_as_defaults_and_leaves_backup() {
        // Self-heal: a truncated/corrupt history.json must load as an empty
        // history (not an error that breaks every capture) and preserve the
        // corrupt bytes as a .bak-<ts> sibling.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        std::fs::write(&path, "{ \"entries\": [ truncated").unwrap();

        let history = ScreenshotHistory::load_in(dir.path()).expect("corrupt file must not error");
        assert!(history.entries.is_empty(), "should fall back to defaults");

        let backup_exists = std::fs::read_dir(dir.path()).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("history.json.bak-")
        });
        assert!(backup_exists, "corrupt file must be preserved as .bak-<ts>");
        // Moved aside, not copied: a second load does not back it up again.
        let _ = ScreenshotHistory::load_in(dir.path()).unwrap();
        assert_eq!(count_backups(dir.path()), 1);
    }

    fn count_backups(dir: &Path) -> usize {
        std::fs::read_dir(dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("history.json.bak-")
            })
            .count()
    }

    #[test]
    fn non_utf8_history_self_heals() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("history.json"), [0xff, 0xfe, 0x80, b'[']).unwrap();
        let h = ScreenshotHistory::load_in(dir.path()).expect("non-UTF-8 must not error");
        assert!(h.entries.is_empty());
        assert_eq!(count_backups(dir.path()), 1);
    }

    #[test]
    fn concurrent_add_entry_loses_no_updates() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().to_path_buf();
        let threads: Vec<_> = (0..8)
            .map(|t| {
                let base = base.clone();
                std::thread::spawn(move || {
                    for i in 0..10 {
                        ScreenshotHistory::add_entry_in(&base, &format!("/x/t{t}-{i}.png"))
                            .unwrap();
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        let raw = std::fs::read(base.join("history.json")).unwrap();
        let parsed: ScreenshotHistory = serde_json::from_slice(&raw).expect("valid JSON");
        assert_eq!(
            parsed.entries.len(),
            80,
            "every concurrent add must survive"
        );
    }

    #[test]
    fn duplicate_path_is_deduplicated() {
        let dir = tempfile::tempdir().unwrap();
        ScreenshotHistory::add_entry_in(dir.path(), "/x/a.png").unwrap();
        ScreenshotHistory::add_entry_in(dir.path(), "/x/b.png").unwrap();
        ScreenshotHistory::add_entry_in(dir.path(), "/x/a.png").unwrap();
        let h = ScreenshotHistory::load_in(dir.path()).unwrap();
        let paths: Vec<_> = h.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["/x/b.png", "/x/a.png"]);
    }

    #[test]
    fn remove_and_clear_delete_thumbnails() {
        let dir = tempfile::tempdir().unwrap();
        ScreenshotHistory::add_entry_in(dir.path(), "/x/a.png").unwrap();
        ScreenshotHistory::add_entry_in(dir.path(), "/x/b.png").unwrap();
        let h = ScreenshotHistory::load_in(dir.path()).unwrap();
        // Simulate finished thumbnails + one orphan.
        for e in &h.entries {
            std::fs::write(&e.thumbnail_path, b"png").unwrap();
        }
        let orphan = dir.path().join("thumbnails").join("orphan_thumb.png");
        std::fs::write(&orphan, b"png").unwrap();

        ScreenshotHistory::remove_entry_in(dir.path(), "/x/a.png").unwrap();
        assert!(!Path::new(&h.entries[0].thumbnail_path).exists());
        assert!(Path::new(&h.entries[1].thumbnail_path).exists());

        ScreenshotHistory::clear_in(dir.path()).unwrap();
        assert!(ScreenshotHistory::load_in(dir.path())
            .unwrap()
            .entries
            .is_empty());
        assert!(!Path::new(&h.entries[1].thumbnail_path).exists());
        assert!(!orphan.exists(), "clear sweeps orphaned thumbnails");
    }

    #[test]
    fn update_in_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let n = ScreenshotHistory::update_in(dir.path(), |h| {
            h.entries.push(HistoryEntry {
                path: "/x/u.png".into(),
                timestamp: "t".into(),
                thumbnail_path: "/nowhere".into(),
            });
            h.entries.len()
        })
        .unwrap();
        assert_eq!(n, 1);
        assert_eq!(
            ScreenshotHistory::load_in(dir.path())
                .unwrap()
                .entries
                .len(),
            1
        );
    }

    #[test]
    fn missing_history_loads_as_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let history = ScreenshotHistory::load_in(dir.path()).unwrap();
        assert!(history.entries.is_empty());
    }

    #[test]
    fn path_hash_distinguishes_same_stem_different_dirs() {
        // Thumbnail names embed this hash so /a/shot.png and /b/shot.png
        // can't collide on one thumbnail file.
        assert_ne!(path_hash("/a/shot.png"), path_hash("/b/shot.png"));
        assert_eq!(path_hash("/a/shot.png"), path_hash("/a/shot.png"));
    }
}
