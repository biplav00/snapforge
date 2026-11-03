//! Filesystem- and OS-clipboard-touching persistence layer for snapforge.
//! Owns the on-disk config, history index, and the clipboard image bridge.

pub mod clipboard;
pub mod config;
pub mod history;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonic per-process counter so concurrent writers never share a temp name.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A sibling temp path unique to this process + call: `.<name>.tmp-<pid>-<n>`.
/// Hidden (leading dot) so directory sweeps can recognise and skip it.
pub(crate) fn unique_tmp_path(path: &Path) -> PathBuf {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{name}.tmp-{}-{n}", std::process::id()))
}

/// Write `bytes` to `path` durably and atomically: write a uniquely-named
/// sibling temp file, fsync it, rename it over `path`, then (best-effort)
/// fsync the directory so the rename itself survives a power loss.
///
/// The unique temp name means two threads (or two processes) saving the same
/// file can't interleave writes into one shared `.tmp`; whichever rename lands
/// last wins with a complete file. Callers that do load→modify→save must still
/// hold their own lock (see `history::ScreenshotHistory`) to avoid lost updates.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = unique_tmp_path(path);
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return result;
    }
    // Best-effort: persist the directory entry. Not all platforms allow
    // opening a directory as a file (Windows), so failures are ignored.
    if let Some(dir) = path.parent() {
        if let Ok(d) = std::fs::File::open(if dir.as_os_str().is_empty() {
            Path::new(".")
        } else {
            dir
        }) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

/// Move an unreadable (corrupt / implausibly large / non-UTF-8) file aside as
/// a timestamped `.bak-<ts>` sibling so the next save can't destroy the
/// user's only copy. The original is *renamed*, not copied, so the next load
/// sees no file and the same bad bytes are never backed up twice.
pub(crate) fn backup_file(path: &Path) -> std::io::Result<PathBuf> {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let base = format!(
        "{}.bak-{}",
        path.file_name()
            .map_or_else(|| "file".into(), |n| n.to_string_lossy().to_string()),
        ts
    );
    // Two backups in the same second must not clobber each other.
    let mut bak = path.with_file_name(&base);
    let mut i = 1u32;
    while bak.exists() {
        bak = path.with_file_name(format!("{base}-{i}"));
        i += 1;
    }
    std::fs::rename(path, &bak)?;
    Ok(bak)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_writes_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.json");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"two");
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "temp files must not linger: {names:?}");
    }

    #[test]
    fn unique_tmp_paths_differ() {
        let p = Path::new("/tmp/a.json");
        assert_ne!(unique_tmp_path(p), unique_tmp_path(p));
    }

    #[test]
    fn backup_moves_original_aside() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("c.json");
        std::fs::write(&p, b"bad").unwrap();
        let bak = backup_file(&p).unwrap();
        assert!(!p.exists());
        assert_eq!(std::fs::read(&bak).unwrap(), b"bad");
        // A second backup in the same second gets a distinct name.
        std::fs::write(&p, b"bad2").unwrap();
        let bak2 = backup_file(&p).unwrap();
        assert_ne!(bak, bak2);
        assert!(bak.exists() && bak2.exists());
    }
}
