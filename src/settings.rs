use crate::i18n::LanguagePreference;
use crate::theme::ThemePreference;
use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub output_directory: PathBuf,
    pub workers: usize,
    pub experimental_parallel_downloads: bool,
    pub auto_decrypt: bool,
    pub keep_encrypted_original: bool,
    pub language: LanguagePreference,
    pub theme: ThemePreference,
    pub proxy: ProxySettings,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyMode {
    #[default]
    System,
    Manual,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyProtocol {
    #[default]
    Http,
    Socks5,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxySettings {
    pub mode: ProxyMode,
    pub protocol: ProxyProtocol,
    pub address: String,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            mode: ProxyMode::System,
            protocol: ProxyProtocol::Http,
            address: String::new(),
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        let output_directory = UserDirs::new()
            .and_then(|directories| directories.download_dir().map(PathBuf::from))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join("Endao Books");
        Self {
            output_directory,
            workers: 4,
            experimental_parallel_downloads: false,
            auto_decrypt: true,
            keep_encrypted_original: false,
            language: LanguagePreference::System,
            theme: ThemePreference::System,
            proxy: ProxySettings::default(),
        }
    }
}

impl AppSettings {
    pub fn download_workers(&self) -> usize {
        if self.experimental_parallel_downloads {
            self.workers.clamp(2, 16)
        } else {
            1
        }
    }
}

pub fn load() -> AppSettings {
    let Some(path) = settings_path() else {
        return AppSettings::default();
    };
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save(settings: &AppSettings) {
    let Some(path) = settings_path() else { return };
    let Some(parent) = path.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(settings) {
        let _ = fs::write(path, bytes);
    }
}

fn settings_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "Endao", "Endao Desktop")
        .map(|directories| directories.config_dir().join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_settings_files_default_to_system_language_and_proxy() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"output_directory":"books","workers":4,"auto_decrypt":true}"#)
                .unwrap();
        assert_eq!(settings.language, LanguagePreference::System);
        assert_eq!(settings.theme, ThemePreference::System);
        assert_eq!(settings.proxy.mode, ProxyMode::System);
        assert_eq!(settings.proxy.protocol, ProxyProtocol::Http);
        assert_eq!(settings.download_workers(), 1);
        assert!(!settings.experimental_parallel_downloads);
        assert!(!settings.keep_encrypted_original);
    }

    #[test]
    fn experimental_download_and_retention_preferences_survive_saved_settings() {
        let settings = AppSettings {
            workers: 8,
            experimental_parallel_downloads: true,
            keep_encrypted_original: true,
            ..AppSettings::default()
        };
        let restored: AppSettings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.download_workers(), 8);
        assert!(restored.keep_encrypted_original);
        let disabled = AppSettings {
            experimental_parallel_downloads: false,
            ..restored
        };
        assert_eq!(disabled.download_workers(), 1);
    }

    #[test]
    fn theme_preference_survives_saved_settings() {
        for theme in [
            ThemePreference::System,
            ThemePreference::Dark,
            ThemePreference::Light,
        ] {
            let settings = AppSettings {
                theme,
                ..AppSettings::default()
            };
            let json = serde_json::to_string(&settings).unwrap();
            let restored: AppSettings = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.theme, theme);
        }
    }
}
