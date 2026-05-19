//! Image encoding (still frames) + screen-recording encode pipeline.
//! Pulls captured frames via `snapforge-capture` and configuration enums
//! via `snapforge-storage::config`.

pub mod format;
pub mod record;
