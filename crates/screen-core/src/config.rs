use crate::types::{CaptureFormat, LastRegion};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    Read(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub save_directory: PathBuf,
    pub auto_copy_clipboard: bool,
    pub show_notification: bool,
    pub launch_at_startup: bool,
    pub remember_last_region: bool,
    pub last_region: Option<LastRegion>,
    pub screenshot_format: CaptureFormat,
    pub jpg_quality: u8,
    pub filename_pattern: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        let save_directory = dirs::picture_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
            .join("ScreenSnap");

        Self {
            save_directory,
            auto_copy_clipboard: true,
            show_notification: true,
            launch_at_startup: false,
            remember_last_region: false,
            last_region: None,
            screenshot_format: CaptureFormat::default(),
            jpg_quality: 90,
            filename_pattern: "screenshot-{date}-{time}".to_string(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("screensnap");
        config_dir.join("config.json")
    }

    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Self = serde_json::from_str(&contents)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ConfigError::Read)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, contents).map_err(ConfigError::Read)?;
        Ok(())
    }

    pub fn generate_filename(&self) -> String {
        let now = chrono::Local::now();
        self.filename_pattern
            .replace("{date}", &now.format("%Y-%m-%d").to_string())
            .replace("{time}", &now.format("%H-%M-%S").to_string())
    }

    pub fn save_file_path(&self) -> PathBuf {
        let filename = self.generate_filename();
        let ext = self.screenshot_format.extension();
        self.save_directory.join(format!("{}.{}", filename, ext))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
        let config_path = tmp.path().join("config.json");

        let mut config = AppConfig::default();
        config.jpg_quality = 75;
        config.remember_last_region = true;

        let contents = serde_json::to_string_pretty(&config).unwrap();
        fs::write(&config_path, &contents).unwrap();

        let loaded: AppConfig =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(loaded.jpg_quality, 75);
        assert!(loaded.remember_last_region);
    }

    #[test]
    fn test_generate_filename() {
        let config = AppConfig {
            filename_pattern: "screenshot-{date}-{time}".to_string(),
            ..Default::default()
        };
        let filename = config.generate_filename();
        assert!(filename.starts_with("screenshot-"));
        assert!(filename.len() > "screenshot-".len());
    }

    #[test]
    fn test_save_file_path() {
        let config = AppConfig {
            save_directory: PathBuf::from("/tmp/screenshots"),
            screenshot_format: CaptureFormat::Jpg,
            filename_pattern: "test-{date}".to_string(),
            ..Default::default()
        };
        let path = config.save_file_path();
        assert_eq!(path.extension().unwrap(), "jpg");
        assert!(path.starts_with("/tmp/screenshots"));
    }
}
