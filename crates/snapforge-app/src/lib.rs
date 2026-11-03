//! High-level use-case layer for snapforge.
//!
//! This crate sits above the domain/capture/encode/storage crates and
//! orchestrates them into single end-to-end operations (take a screenshot,
//! start a recording, stream click events). It knows nothing about C ABI;
//! the FFI lives one layer above in `snapforge-ffi`.
//!
//! Use cases accept a request DTO and return a result DTO (or a handle for
//! long-running operations). Errors are normalised through the [`AppError`]
//! enum so the FFI translator only has one error type to map.

use thiserror::Error;

pub mod clicks;
pub mod recording;
pub mod screenshot;

pub use snapforge_domain::{CaptureFormat, Rect};
pub use snapforge_storage::config::{RecordingFormat, RecordingQuality};

/// Single error type returned by every use case. Each variant wraps the
/// underlying crate-level error so callers can still recover the original
/// message via `Display`.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("capture failed: {0}")]
    Capture(#[from] snapforge_capture::capture::CaptureError),
    #[error("format failed: {0}")]
    Format(#[from] snapforge_encode::format::FormatError),
    #[error("clipboard failed: {0}")]
    Clipboard(#[from] snapforge_storage::clipboard::ClipboardError),
    #[error("config failed: {0}")]
    Config(#[from] snapforge_storage::config::ConfigError),
    #[error("recording failed: {0}")]
    Recording(#[from] snapforge_encode::record::RecordError),
    #[error("storage failed: {0}")]
    Storage(#[from] snapforge_storage::history::HistoryError),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// A required OS permission (e.g. Accessibility for the click tap) is
    /// missing.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    /// Filesystem failure outside a sub-crate (e.g. reserving an output path).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Unexpected internal failure (thread spawn, etc.).
    #[error("internal error: {0}")]
    Internal(String),
}

/// Atomically reserve a not-yet-existing output path derived from `path`.
///
/// If `path` is free it is used as-is; otherwise `-1`, `-2`, … is appended
/// to the stem (`shot.png` → `shot-1.png`) until a free name is found. The
/// winning name is claimed by creating an empty file with `create_new`, so
/// two saves racing in the same second (same timestamped name) can never
/// pick the same file and overwrite each other. The caller then overwrites
/// the placeholder, and should delete it if the write fails.
pub(crate) fn claim_unique_path(path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    use std::io::ErrorKind;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()));
    for i in 0u32..10_000 {
        let candidate = if i == 0 {
            path.to_path_buf()
        } else {
            path.with_file_name(format!("{stem}-{i}{}", ext.as_deref().unwrap_or("")))
        };
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(_) => return Ok(candidate),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        format!("no free file name for {}", path.display()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claim_unique_path_appends_suffix_on_collision() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("shot.png");
        let a = claim_unique_path(&p).unwrap();
        let b = claim_unique_path(&p).unwrap();
        let c = claim_unique_path(&p).unwrap();
        assert_eq!(a, p);
        assert_eq!(b, dir.path().join("shot-1.png"));
        assert_eq!(c, dir.path().join("shot-2.png"));
        assert!(a.exists() && b.exists() && c.exists());
    }

    #[test]
    fn claim_unique_path_without_extension() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("rec");
        claim_unique_path(&p).unwrap();
        assert_eq!(claim_unique_path(&p).unwrap(), dir.path().join("rec-1"));
    }

    #[test]
    fn claim_unique_path_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a").join("b").join("x.png");
        assert_eq!(claim_unique_path(&p).unwrap(), p);
    }
}
