//! Filesystem- and OS-clipboard-touching persistence layer for snapforge.
//! Owns the on-disk config, history index, and the clipboard image bridge.

pub mod clipboard;
pub mod config;
pub mod history;
