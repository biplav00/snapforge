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
}
