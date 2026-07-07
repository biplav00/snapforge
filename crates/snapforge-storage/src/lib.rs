//! Filesystem- and OS-clipboard-touching persistence layer for snapforge.
//! Owns the on-disk config, history index, and the clipboard image bridge.

pub mod clipboard;
pub mod config;
pub mod history;

/// Write `bytes` to `path` via a temp file in the same directory + rename, so
/// a crash mid-write can never leave a truncated config/history behind.
// ponytail: fixed `.tmp` sibling and no fsync — atomic against crashes within
// this single-process app, not against two processes saving the same file.
pub(crate) fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Preserve an unreadable (corrupt / implausibly large) file as a timestamped
/// `.bak-<ts>` sibling so the next save can't destroy the user's only copy.
pub(crate) fn backup_file(path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let bak = path.with_file_name(format!(
        "{}.bak-{}",
        path.file_name()
            .map_or_else(|| "file".into(), |n| n.to_string_lossy().to_string()),
        ts
    ));
    std::fs::copy(path, &bak)?;
    Ok(bak)
}
