use serde::{Deserialize, Serialize};
use snapforge_domain::{CaptureFormat, LastRegion};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use thiserror::Error;

/// Serializes every config save (and the self-heal rewrite in `load`) within
/// this process so two threads can't interleave writes to config.json.
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

const DEFAULT_FILENAME_PATTERN: &str = "screenshot-{date}-{time}";
/// Highest fps the Preferences UI offers (10/15/24/30/60).
const MAX_CONFIG_FPS: u32 = 60;

/// Per-field lenient deserializer: an unknown enum spelling or an
/// out-of-type value (e.g. `"jpg_quality": 300`) falls back to that field's
/// default with a warning instead of failing the whole config parse — which
/// would otherwise discard every other valid setting via the self-heal path.
fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    lenient_or(d, T::default)
}

fn lenient_or<'de, D, T>(d: D, fallback: impl FnOnce() -> T) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value::<T>(v.clone()).unwrap_or_else(|e| {
        tracing::warn!("[config] ignoring invalid value {v} ({e}); using default");
        fallback()
    }))
}

fn lenient_jpg_quality<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    lenient_or(d, || AppConfig::default().jpg_quality)
}

fn lenient_fps<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    lenient_or(d, || RecordingConfig::default().fps)
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),
}

// Serializes as the variant name ("Mp4"/"Gif", "Low"/"Medium"/"High") — the
// representation already on disk and in the Qt config bridge. The `alias`
// attributes also accept the lowercase spellings the recording request JSON
// sends, so the two wire paths share one serde definition and can't drift.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum RecordingFormat {
    #[default]
    #[serde(alias = "mp4")]
    Mp4,
    #[serde(alias = "gif")]
    Gif,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum RecordingQuality {
    #[serde(alias = "low")]
    Low,
    #[default]
    #[serde(alias = "medium")]
    Medium,
    #[serde(alias = "high")]
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordingConfig {
    #[serde(deserialize_with = "lenient")]
    pub format: RecordingFormat,
    #[serde(deserialize_with = "lenient_fps")]
    pub fps: u32,
    #[serde(deserialize_with = "lenient")]
    pub quality: RecordingQuality,
    // Persist the "show clicks while recording" toggle. Without this field serde
    // silently dropped the key on every save, so the checkbox never stuck.
    pub show_clicks: bool,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            format: RecordingFormat::default(),
            fps: 30,
            quality: RecordingQuality::default(),
            show_clicks: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyBindings {
    // Global shortcuts
    pub screenshot: String,
    pub capture_last_region: String,
    pub record_screen: String,
    pub open_history: String,
    pub open_preferences: String,
    // Annotation tool shortcuts
    pub tool_arrow: String,
    pub tool_rect: String,
    pub tool_circle: String,
    pub tool_line: String,
    pub tool_dottedline: String,
    pub tool_freehand: String,
    pub tool_text: String,
    pub tool_highlight: String,
    pub tool_blur: String,
    pub tool_steps: String,
    pub tool_callout: String,
    pub tool_colorpicker: String,
    pub tool_measure: String,
    // Size shortcuts
    pub size_small: String,
    pub size_medium: String,
    pub size_large: String,
    // Overlay action shortcuts
    pub action_save: String,
    pub action_copy: String,
    pub action_undo: String,
    pub action_redo: String,
    pub action_cancel: String,
    pub action_forcequit: String,
}

impl Default for HotkeyBindings {
    fn default() -> Self {
        Self {
            screenshot: "CmdOrCtrl+Shift+S".to_string(),
            capture_last_region: "CmdOrCtrl+Shift+L".to_string(),
            record_screen: "CmdOrCtrl+Shift+R".to_string(),
            open_history: "CmdOrCtrl+Shift+H".to_string(),
            open_preferences: "CmdOrCtrl+Comma".to_string(),
            tool_arrow: "A".to_string(),
            tool_rect: "R".to_string(),
            tool_circle: "C".to_string(),
            tool_line: "L".to_string(),
            tool_dottedline: "D".to_string(),
            tool_freehand: "F".to_string(),
            tool_text: "T".to_string(),
            tool_highlight: "H".to_string(),
            tool_blur: "B".to_string(),
            tool_steps: "N".to_string(),
            tool_callout: "K".to_string(),
            tool_colorpicker: "I".to_string(),
            tool_measure: "M".to_string(),
            size_small: "1".to_string(),
            size_medium: "2".to_string(),
            size_large: "3".to_string(),
            action_save: "CmdOrCtrl+S".to_string(),
            action_copy: "CmdOrCtrl+C".to_string(),
            action_undo: "CmdOrCtrl+Z".to_string(),
            action_redo: "CmdOrCtrl+Shift+Z".to_string(),
            action_cancel: "Escape".to_string(),
            action_forcequit: "Cmd+Shift+C".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub save_directory: PathBuf,
    pub auto_copy_clipboard: bool,
    pub show_notification: bool,
    pub launch_at_startup: bool,
    pub remember_last_region: bool,
    #[serde(deserialize_with = "lenient")]
    pub last_region: Option<LastRegion>,
    #[serde(deserialize_with = "lenient")]
    pub screenshot_format: CaptureFormat,
    #[serde(deserialize_with = "lenient_jpg_quality")]
    pub jpg_quality: u8,
    pub filename_pattern: String,
    pub hotkey_bindings: HotkeyBindings,
    pub recording: RecordingConfig,
    /// Catch-all for fields the Qt frontend persists that Rust doesn't model
    /// (e.g. `hotkeys`, `theme`). Without this, a save+load round-trip would
    /// silently strip every Qt-only field. BTreeMap so on-disk key order is
    /// stable across saves.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn default_save_directory() -> PathBuf {
    dirs::picture_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("Snapforge")
}

impl Default for AppConfig {
    fn default() -> Self {
        let save_directory = default_save_directory();

        Self {
            save_directory,
            auto_copy_clipboard: true,
            show_notification: true,
            launch_at_startup: false,
            remember_last_region: false,
            last_region: None,
            screenshot_format: CaptureFormat::default(),
            jpg_quality: 90,
            filename_pattern: DEFAULT_FILENAME_PATTERN.to_string(),
            hotkey_bindings: HotkeyBindings::default(),
            recording: RecordingConfig::default(),
            extra: BTreeMap::new(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("snapforge");
        config_dir.join("config.json")
    }

    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from(&Self::config_path())
    }

    /// Load (and normalize) the config at `path`. A missing file yields
    /// defaults. An oversized, non-UTF-8, or unparseable file is moved aside
    /// as `.bak-<ts>` and defaults are returned; defaults are only written
    /// back over `path` once the bad original has been safely preserved.
    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        // 4 MiB ceiling — well above any legitimate config; protects against a corrupt
        // or attacker-supplied file ballooning RAM.
        const MAX_CONFIG_BYTES: u64 = 4 * 1024 * 1024;
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() > MAX_CONFIG_BYTES {
                return Ok(Self::self_heal(
                    path,
                    &format!("implausibly large ({} bytes)", meta.len()),
                ));
            }
        }
        // Read raw bytes: `from_slice` reports invalid UTF-8 as a parse error,
        // so a non-UTF-8 file goes through the same backup-and-defaults path
        // instead of surfacing an IO error on every launch.
        let bytes = std::fs::read(path)?;
        match serde_json::from_slice::<Self>(&bytes) {
            Ok(mut config) => {
                config.normalize();
                Ok(config)
            }
            Err(e) => Ok(Self::self_heal(path, &format!("corrupt ({e})"))),
        }
    }

    /// Preserve the bad file, then return defaults. If the backup failed the
    /// original is left untouched (we never destroy the user's only copy).
    fn self_heal(path: &Path, why: &str) -> Self {
        let default = Self::default();
        let _guard = CONFIG_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match crate::backup_file(path) {
            Ok(bak) => {
                tracing::warn!(
                    "[config] {} is {}; preserved as {}; using defaults",
                    path.display(),
                    why,
                    bak.display()
                );
                match serde_json::to_vec_pretty(&default) {
                    Ok(bytes) => {
                        if let Err(e) = crate::write_atomic(path, &bytes) {
                            tracing::error!(
                                "[config] failed to write default config to {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                    Err(e) => tracing::error!("[config] failed to serialize defaults: {}", e),
                }
            }
            Err(e) => tracing::error!(
                "[config] {} is {}; backup failed: {}; using defaults without overwriting",
                path.display(),
                why,
                e
            ),
        }
        default
    }

    /// Clamp / repair values a hand-edited or stale config could carry:
    /// - `~`-prefixed `save_directory` is expanded; a relative or empty one
    ///   falls back to the default (relative paths would resolve against the
    ///   app's unpredictable CWD);
    /// - `jpg_quality` is clamped to 1..=100;
    /// - `recording.fps` is clamped to 1..=60 (the UI's range; 0 → default);
    /// - `filename_pattern` containing a path separator, `..`, or NUL — or
    ///   empty — falls back to the default (it must name a file, not a path).
    pub fn normalize(&mut self) {
        let dir = self.save_directory.to_string_lossy().into_owned();
        if dir == "~" || dir.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                self.save_directory =
                    home.join(dir.trim_start_matches('~').trim_start_matches('/'));
            }
        }
        if self.save_directory.as_os_str().is_empty() || self.save_directory.is_relative() {
            tracing::warn!(
                "[config] save_directory {:?} is not absolute; using default",
                self.save_directory
            );
            self.save_directory = default_save_directory();
        }

        self.jpg_quality = self.jpg_quality.clamp(1, 100);

        if self.recording.fps == 0 {
            self.recording.fps = RecordingConfig::default().fps;
        }
        self.recording.fps = self.recording.fps.clamp(1, MAX_CONFIG_FPS);

        let p = &self.filename_pattern;
        if p.trim().is_empty()
            || p.contains('/')
            || p.contains('\\')
            || p.contains("..")
            || p.contains('\0')
        {
            tracing::warn!("[config] invalid filename_pattern {:?}; using default", p);
            self.filename_pattern = DEFAULT_FILENAME_PATTERN.to_string();
        }
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to(&Self::config_path())
    }

    /// Serialize to `path` atomically (temp + fsync + rename) under the
    /// process-wide config lock.
    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        let contents = serde_json::to_string_pretty(self)?;
        let _guard = CONFIG_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }
        crate::write_atomic(path, contents.as_bytes()).map_err(ConfigError::Io)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn backups_in(dir: &Path) -> usize {
        fs::read_dir(dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("config.json.bak-")
            })
            .count()
    }

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(config.auto_copy_clipboard);
        assert!(config.show_notification);
        assert!(!config.launch_at_startup);
        assert!(!config.remember_last_region);
        assert!(config.last_region.is_none());
        assert_eq!(config.screenshot_format, CaptureFormat::Png);
        assert_eq!(config.jpg_quality, 90);
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = AppConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.auto_copy_clipboard, config.auto_copy_clipboard);
        assert_eq!(deserialized.screenshot_format, config.screenshot_format);
        assert_eq!(deserialized.jpg_quality, config.jpg_quality);
    }

    #[test]
    fn test_config_save_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("nested").join("config.json");

        let config = AppConfig {
            jpg_quality: 75,
            remember_last_region: true,
            save_directory: tmp.path().join("shots"),
            ..Default::default()
        };
        config.save_to(&config_path).unwrap();

        let loaded = AppConfig::load_from(&config_path).unwrap();
        assert_eq!(loaded.jpg_quality, 75);
        assert!(loaded.remember_last_region);
        assert_eq!(loaded.save_directory, tmp.path().join("shots"));
    }

    #[test]
    fn save_to_preserves_unknown_fields() {
        // Qt-only top-level keys (`hotkeys`, `theme`) must survive a
        // load -> save -> load round-trip via the `extra` flatten map.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        fs::write(
            &path,
            r#"{"theme":"Dark","hotkeys":{"global":{"screenshot":"Cmd+Alt+Shift+X"}}}"#,
        )
        .unwrap();
        let cfg = AppConfig::load_from(&path).unwrap();
        cfg.save_to(&path).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(v["theme"], serde_json::json!("Dark"));
        assert_eq!(
            v["hotkeys"]["global"]["screenshot"],
            serde_json::json!("Cmd+Alt+Shift+X")
        );
    }

    #[test]
    fn missing_config_loads_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let loaded = AppConfig::load_from(&tmp.path().join("config.json")).unwrap();
        assert_eq!(loaded.jpg_quality, 90);
    }

    #[test]
    fn corrupt_config_is_backed_up_once_and_rewritten() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        fs::write(&path, "{ not json").unwrap();

        let loaded = AppConfig::load_from(&path).unwrap();
        assert_eq!(loaded.jpg_quality, 90);
        assert_eq!(backups_in(tmp.path()), 1);
        // The defaults were written back, so a second load is clean and does
        // NOT create another backup.
        let _ = AppConfig::load_from(&path).unwrap();
        assert_eq!(backups_in(tmp.path()), 1);
        let rewritten: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(rewritten.is_object());
    }

    #[test]
    fn non_utf8_config_self_heals() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        fs::write(&path, [0xff, 0xfe, b'{', 0x80]).unwrap();
        let loaded = AppConfig::load_from(&path).expect("non-UTF-8 must not error");
        assert_eq!(loaded.jpg_quality, 90);
        assert_eq!(backups_in(tmp.path()), 1);
    }

    #[test]
    fn unknown_enum_value_falls_back_per_field() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{"screenshot_format":"tiff","jpg_quality":300,"show_notification":false,
                "recording":{"format":"avi","fps":24}}"#,
        )
        .unwrap();
        assert_eq!(cfg.screenshot_format, CaptureFormat::Png);
        assert_eq!(
            cfg.jpg_quality, 90,
            "u8 overflow falls back to the field default"
        );
        assert!(!cfg.show_notification, "valid sibling fields survive");
        assert!(matches!(cfg.recording.format, RecordingFormat::Mp4));
        assert_eq!(cfg.recording.fps, 24);
    }

    #[test]
    fn normalize_repairs_bad_values() {
        let mut cfg = AppConfig {
            save_directory: PathBuf::from("relative/dir"),
            jpg_quality: 0,
            filename_pattern: "../evil/{date}".into(),
            ..Default::default()
        };
        cfg.recording.fps = 1000;
        cfg.normalize();
        assert!(cfg.save_directory.is_absolute());
        assert_eq!(cfg.jpg_quality, 1);
        assert_eq!(cfg.recording.fps, MAX_CONFIG_FPS);
        assert_eq!(cfg.filename_pattern, DEFAULT_FILENAME_PATTERN);

        let mut cfg = AppConfig {
            save_directory: PathBuf::from("~/Shots"),
            filename_pattern: "  ".into(),
            ..Default::default()
        };
        cfg.recording.fps = 0;
        cfg.normalize();
        if let Some(home) = dirs::home_dir() {
            assert_eq!(cfg.save_directory, home.join("Shots"));
        }
        assert_eq!(cfg.recording.fps, 30);
        assert_eq!(cfg.filename_pattern, DEFAULT_FILENAME_PATTERN);
    }

    #[test]
    fn load_normalizes() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        fs::write(&path, r#"{"jpg_quality":0,"filename_pattern":"a/b"}"#).unwrap();
        let cfg = AppConfig::load_from(&path).unwrap();
        assert_eq!(cfg.jpg_quality, 1);
        assert_eq!(cfg.filename_pattern, DEFAULT_FILENAME_PATTERN);
    }
}
